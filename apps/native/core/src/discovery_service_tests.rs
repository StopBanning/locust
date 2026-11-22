//! `runtime-discovery.test.ts`: the answer the window is given -- sanitized, shared while it is being built, reused for
//! a short while, and a generic error when a sweep cannot run.
use super::*;
use crate::agents::{Answer, Auth, Probe, Seams as Machine, discover_one};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

fn ok(text: &str) -> Option<Answer> {
    Some(Answer { exit: Some(0), stdout: text.into(), stderr: String::new(), timed_out: false })
}

fn help(id: &str) -> String {
    std::fs::read_to_string(format!("{}/../parity/agents/{id}-help.txt", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn ready(id: &str) -> Found {
    Found {
        id: id.into(),
        display_name: id.into(),
        installed: true,
        version: None,
        auth: Auth::Authenticated,
        ready: true,
        status: Probe::Ready,
        unsupported_note: None,
        through_cli: false,
        checking: false,
    }
}

fn service(probe: impl Fn() -> Vec<Found> + Send + Sync + 'static, now: Arc<AtomicU64>, ttl_ms: u64) -> Arc<DiscoveryService> {
    Arc::new(DiscoveryService::new(Seams {
        probe: Box::new(probe),
        now: Box::new(move || now.load(Ordering::SeqCst)),
        ttl_ms,
        held: Box::new(|| None),
    }))
}

// case: publishes only sanitized status fields
#[test]
fn what_the_window_is_given_is_the_status_and_never_where_the_cli_lives() {
    let (codex_help, claude_help) = (help("codex"), help("claude"));
    let find = |c: &str| matches!(c, "codex" | "claude").then(|| std::path::PathBuf::from(format!("/Users/private/bin/{c}")));
    let run = |p: &std::path::Path, args: &[&str]| -> Option<Answer> {
        let codex = p.ends_with("codex");
        match args.join(" ").as_str() {
            "--version" if codex => ok("codex-cli 0.151.0-alpha.7.2"),
            "--version" => ok("no version printed here"),
            "exec --help" => ok(&codex_help),
            "--help" => ok(&claude_help),
            "login status" => Some(Answer { exit: Some(1), stdout: "Not logged in\n".into(), stderr: String::new(), timed_out: false }),
            _ => ok("Logged in"),
        }
    };
    let exists = |_: &std::path::Path| true;
    let seams = Machine { find: &find, run: &run, exists: &exists, home: Some("/Users/private".into()), xdg_config_home: None, recall: None };
    let got: Vec<Found> = ["codex", "claude", "opencode"].iter().map(|id| discover_one(id, &seams).unwrap()).collect();
    let shape: Vec<(&str, bool, Option<&str>, Auth, bool, Probe)> =
        got.iter().map(|f| (f.id.as_str(), f.installed, f.version.as_deref(), f.auth, f.ready, f.status)).collect();
    assert_eq!(
        shape,
        [
            ("codex", true, Some("0.151.0-alpha.7.2"), Auth::Unauthenticated, false, Probe::AuthRequired),
            ("claude", true, None, Auth::Authenticated, true, Probe::Ready),
            ("opencode", false, None, Auth::NotApplicable, false, Probe::Missing),
        ]
    );
    assert!(!format!("{got:?}").contains("private"), "{got:?}");
}

// case: deduplicates in-flight probes and uses its short cache
#[test]
fn two_asks_share_one_sweep_and_an_answer_is_reused_until_it_is_old() {
    let calls = Arc::new(AtomicUsize::new(0));
    let (release, gate) = std::sync::mpsc::channel::<()>();
    let gate = Mutex::new(gate);
    let counted = calls.clone();
    let now = Arc::new(AtomicU64::new(1_000));
    let svc = service(
        move || {
            counted.fetch_add(1, Ordering::SeqCst);
            let _ = crate::invariants::lock(&gate).recv_timeout(std::time::Duration::from_secs(10));
            vec![ready("claude")]
        },
        now.clone(),
        100,
    );
    let (a, b) = (svc.clone(), svc.clone());
    let first = std::thread::spawn(move || a.get());
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while calls.load(Ordering::SeqCst) == 0 && std::time::Instant::now() < until {
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let second = std::thread::spawn(move || b.get());
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert_eq!(calls.load(Ordering::SeqCst), 1, "the second ask joins the sweep already running");
    release.send(()).unwrap();
    let (first, second) = (first.join().unwrap(), second.join().unwrap());
    assert_eq!(first, second);
    assert_eq!(first, Ok(vec![ready("claude")]));
    assert_eq!(calls.load(Ordering::SeqCst), 1, "one sweep for both");
    // Inside the short cache: no new sweep.
    assert_eq!(svc.get(), first);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    // Past it: a new sweep.
    now.fetch_add(101, Ordering::SeqCst);
    release.send(()).unwrap();
    assert_eq!(svc.get(), first);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    // Something changed on the machine: the next ask is a new sweep, inside the short cache or not.
    svc.invalidate();
    release.send(()).unwrap();
    assert_eq!(svc.get(), first);
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

// case: returns a generic error and never leaks adapter failures
#[test]
fn a_sweep_that_cannot_run_answers_with_generic_words() {
    let svc = service(|| panic!("secret path and raw stderr"), Arc::new(AtomicU64::new(0)), 10_000);
    let said = svc.get();
    assert_eq!(said, Err(DiscoveryFailed { code: "DISCOVERY_FAILED", message: "Local runtime discovery could not complete." }));
    assert!(!format!("{said:?}").contains("secret"));
}

// An invalidation during a sweep is not lost (runtime-discovery.ts, 2026-10-05): that sweep is asked once more, and
// what it built is not kept; a late answer is told to the window once.
#[test]
fn an_invalidation_while_a_sweep_runs_asks_again_and_keeps_nothing() {
    let calls = Arc::new(AtomicUsize::new(0));
    let holder: Arc<Mutex<Option<Arc<DiscoveryService>>>> = Arc::new(Mutex::new(None));
    let (counted, inside) = (calls.clone(), holder.clone());
    let svc = service(
        move || {
            let n = counted.fetch_add(1, Ordering::SeqCst);
            if n == 0
                && let Some(svc) = crate::invariants::lock(&inside).clone()
            {
                svc.answered_late("opencode");
            }
            vec![ready(if n == 0 { "before" } else { "after" })]
        },
        Arc::new(AtomicU64::new(0)),
        10_000,
    );
    *crate::invariants::lock(&holder) = Some(svc.clone());
    assert_eq!(svc.get(), Ok(vec![ready("after")]));
    assert_eq!(calls.load(Ordering::SeqCst), 2, "asked once more");
    assert_eq!(svc.get(), Ok(vec![ready("after")]));
    assert_eq!(calls.load(Ordering::SeqCst), 3, "the invalidated build was not cached");
    assert_eq!(svc.take_finished(), ["opencode".to_owned()].into_iter().collect());
    assert!(svc.take_finished().is_empty(), "told once");
    *crate::invariants::lock(&holder) = None;
}
