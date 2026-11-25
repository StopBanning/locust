//! A start asks only its own runtime (Electron `main/a-start-asks-only-its-runtime.test.ts`).
//!
//! Past five minutes, starting a run swept EVERY runtime and waited for the slowest -- 3-6 s before the first message
//! after a break. The start needs one answer.

use crate::agents::{Auth, Found, HeldAnswer, Probe, READY_TO_START_TTL_MS, StartReadiness, StartReadinessOptions};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const TTL: u64 = 5 * 60_000;

fn runtime(id: &str, status: Probe) -> Found {
    Found {
        id: id.into(),
        display_name: id.into(),
        installed: true,
        version: None,
        auth: if status == Probe::Ready { Auth::Authenticated } else { Auth::Unauthenticated },
        ready: status == Probe::Ready,
        status,
        unsupported_note: None,
        through_cli: false,
        checking: false,
    }
}

/// What a runtime asked alone answers: ready, and marked so a test can tell it from the held one.
fn fresh(id: &str) -> Found {
    Found { version: Some("asked alone".into()), ..runtime(id, Probe::Ready) }
}

struct Harness {
    readiness: Arc<StartReadiness>,
    held: Arc<Mutex<Option<HeldAnswer>>>,
    sweeps: Arc<AtomicUsize>,
    asked: Arc<Mutex<Vec<String>>>,
    fail_next: Arc<AtomicUsize>,
}

fn harness(clock: u64) -> Harness {
    let held = Arc::new(Mutex::new(Some(HeldAnswer {
        at: 0,
        value: vec![runtime("claude", Probe::Ready), runtime("codex", Probe::Ready), runtime("opencode", Probe::Ready)],
    })));
    let sweeps = Arc::new(AtomicUsize::new(0));
    let asked = Arc::new(Mutex::new(Vec::new()));
    let fail_next = Arc::new(AtomicUsize::new(0));
    let now = Arc::new(AtomicU64::new(clock));
    let readiness = Arc::new(StartReadiness::new(StartReadinessOptions {
        ttl_ms: TTL,
        now: Box::new(move || now.load(Ordering::SeqCst)),
        held: Box::new({
            let held = held.clone();
            move || held.lock().unwrap().clone()
        }),
        store: Box::new({
            let held = held.clone();
            move |value| {
                let mut h = held.lock().unwrap();
                let at = h.as_ref().map_or(0, |h| h.at);
                *h = Some(HeldAnswer { at, value });
            }
        }),
        sweep: Box::new({
            let (held, sweeps) = (held.clone(), sweeps.clone());
            move || {
                sweeps.fetch_add(1, Ordering::SeqCst);
                held.lock().unwrap().as_ref().map(|h| h.value.clone()).unwrap_or_default()
            }
        }),
        ask_one: Box::new({
            let (asked, fail_next) = (asked.clone(), fail_next.clone());
            move |id| {
                asked.lock().unwrap().push(id.to_owned());
                if fail_next.swap(0, Ordering::SeqCst) == 1 {
                    return Err("probe blew up".into());
                }
                Ok(Some(fresh(id)))
            }
        }),
        sweep_only: ["antigravity".to_owned()].into_iter().collect(),
    }));
    let first = held.lock().unwrap().clone().unwrap();
    readiness.swept(&first.value, 0);
    Harness { readiness, held, sweeps, asked, fail_next }
}

impl Harness {
    fn set_held(&self, value: Vec<Found>) {
        *self.held.lock().unwrap() = Some(HeldAnswer { at: 0, value });
    }
    fn asked(&self) -> Vec<String> {
        self.asked.lock().unwrap().clone()
    }
    fn sweeps(&self) -> usize {
        self.sweeps.load(Ordering::SeqCst)
    }
}

#[test]
fn the_start_trusts_a_ready_answer_for_five_minutes() {
    assert_eq!(READY_TO_START_TTL_MS, TTL);
}

// case: asks nothing while the last answer is fresh
#[test]
fn asks_nothing_while_the_last_answer_is_fresh() {
    let h = harness(60_000);
    let answer = h.readiness.for_start(Some("claude"));
    assert_eq!(h.sweeps(), 0);
    assert!(h.asked().is_empty());
    assert_eq!(answer.len(), 3);
    // One millisecond short of the limit is still fresh.
    let h = harness(TTL - 1);
    h.readiness.for_start(Some("claude"));
    assert_eq!((h.sweeps(), h.asked().len()), (0, 0));
}

// case: asks the starting runtime ALONE once the answer is stale
#[test]
fn asks_the_starting_runtime_alone_once_the_answer_is_stale() {
    let h = harness(TTL + 1);
    let answer = h.readiness.for_start(Some("claude"));
    assert_eq!(h.asked(), vec!["claude".to_owned()]);
    assert_eq!(h.sweeps(), 0);
    // The fresh answer replaces that runtime's alone, and is what the start gets and what is kept.
    assert_eq!(answer.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["claude", "codex", "opencode"]);
    assert_eq!(answer[0], fresh("claude"));
    assert_eq!(answer[1], runtime("codex", Probe::Ready));
    assert_eq!(answer[2], runtime("opencode", Probe::Ready));
    let kept = h.held.lock().unwrap().clone().unwrap();
    assert_eq!(kept.value, answer);
    // The sweep's own clock is left alone: the other runtimes were not asked.
    assert_eq!(kept.at, 0);
    // And that answer is fresh now: the next start on it asks nothing.
    h.readiness.for_start(Some("claude"));
    assert_eq!(h.asked().len(), 1);
    // A DIFFERENT runtime is still stale -- asking one did not vouch for all.
    h.readiness.for_start(Some("codex"));
    assert_eq!(h.asked().last().map(String::as_str), Some("codex"));
    assert_eq!(h.asked().len(), 2);
    assert_eq!(h.sweeps(), 0);
}

// case: still sweeps for a runtime that was not ready, and for Antigravity
#[test]
fn still_sweeps_for_a_runtime_that_was_not_ready_and_for_antigravity() {
    // The controls. A runtime that was signed out needs the full path (the screens learn from it too), and
    // Antigravity is not a CLI to ask alone.
    let h = harness(TTL + 1);
    h.set_held(vec![runtime("claude", Probe::AuthRequired), runtime("antigravity", Probe::Ready)]);
    h.readiness.for_start(Some("claude"));
    h.readiness.for_start(Some("antigravity"));
    assert_eq!(h.sweeps(), 2);
    assert!(h.asked().is_empty());
}

// case: falls back to the full sweep when asking alone fails
#[test]
fn falls_back_to_the_full_sweep_when_asking_alone_fails() {
    let h = harness(TTL + 1);
    h.fail_next.store(1, Ordering::SeqCst);
    h.readiness.for_start(Some("opencode"));
    assert_eq!(h.asked(), vec!["opencode".to_owned()]);
    assert_eq!(h.sweeps(), 1);
}

#[test]
fn a_start_with_no_runtime_or_no_answer_yet_sweeps() {
    let h = harness(60_000);
    h.readiness.for_start(None);
    assert_eq!(h.sweeps(), 1);
    *h.held.lock().unwrap() = None;
    h.readiness.for_start(Some("claude"));
    assert_eq!(h.sweeps(), 2);
    assert!(h.asked().is_empty());
}

#[test]
fn a_runtime_a_sweep_answered_later_is_fresh_from_that_answer_not_from_the_held_clock() {
    // index.ts stamps one runtime's own late answer (`startReadiness.swept([record], Date.now())`) after the sweep's
    // clock: that runtime is fresh from then, though the held answer is older than the limit.
    let h = harness(TTL + 1);
    let value = h.held.lock().unwrap().clone().unwrap().value;
    h.readiness.swept(&value[..1], TTL);
    h.readiness.for_start(Some("claude"));
    assert_eq!((h.sweeps(), h.asked().len()), (0, 0));
    // Another runtime, not stamped, is still stale.
    h.readiness.for_start(Some("codex"));
    assert_eq!(h.asked(), vec!["codex".to_owned()]);
}

// ---- a real start goes through it (index.ts discoverForStart, codex-mission.ts start) ----

use crate::native::{Send, hooks, not_ready_refusal, send_from};

struct Temp(std::path::PathBuf);
impl Temp {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!(
            "locust-start-ready-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(p.join("electron/mission-ledger")).unwrap();
        std::fs::create_dir_all(p.join("work")).unwrap();
        Self(p)
    }
    fn store(&self) -> crate::store::Store {
        crate::store::Store::new(self.0.join("native"), self.0.join("electron")).unwrap()
    }
    /// The turns this store holds, as mission files.
    fn missions(&self) -> usize {
        std::fs::read_dir(self.0.join("native/mission-ledger")).map_or(0, |d| d.count())
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fake_codex() -> Option<(std::path::PathBuf, Vec<String>)> {
    Some(("python3".into(), vec![concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/fixtures/m1-app-server.py").into(), "success".into()]))
}

fn send_to_codex(t: &Temp) -> Result<crate::engine::Started, String> {
    send_from(
        Send {
            teammate: crate::engine::Teammate {
                id: "tm_codec".into(),
                name: "Codec".into(),
                role: "Code & Migrations".into(),
                runtime: Some("codex".into()),
                model: Some("synthetic-model".into()),
                account: None,
                busy: false,
                runs_in: "ask".into(),
                folder: Some(t.0.join("work").to_string_lossy().into()),
            },
            conversation: None,
            prompt: "Say hello.".into(),
            model: Some("synthetic-model".into()),
            effort: None,
        },
        crate::native::Origin::default(),
    )
}

/// The engine asks through `h`'s readiness, the clock where `h` left it.
fn install(h: Harness) -> Harness {
    *crate::invariants::lock(&hooks::READINESS) = Some(h.readiness.clone());
    h
}

// case: asks the starting runtime ALONE once the answer is stale
#[test]
fn a_real_start_on_a_stale_ready_runtime_asks_it_alone_and_goes_through() {
    let t = Temp::new("stale");
    let _g = hooks::install(t.store(), fake_codex());
    let h = install(harness(TTL + 1));
    let started = send_to_codex(&t);
    assert!(started.is_ok(), "{started:?}");
    assert_eq!(h.asked(), vec!["codex".to_owned()]);
    assert_eq!(h.sweeps(), 0);
    assert_eq!(t.missions(), 1);
}

// case: asks nothing while the last answer is fresh
#[test]
fn a_real_start_on_a_fresh_ready_runtime_asks_nothing() {
    let t = Temp::new("fresh");
    let _g = hooks::install(t.store(), fake_codex());
    let h = install(harness(60_000));
    assert!(send_to_codex(&t).is_ok());
    assert_eq!((h.sweeps(), h.asked().len()), (0, 0));
}

// case: still sweeps for a runtime that was not ready, and for Antigravity
// case: returns an actionable generic error when Codex is not ready
#[test]
fn a_real_start_on_a_signed_out_runtime_sweeps_and_is_refused_in_electrons_words_before_anything_is_recorded() {
    let t = Temp::new("signed-out");
    let _g = hooks::install(t.store(), fake_codex());
    let h = install(harness(60_000));
    h.set_held(vec![runtime("codex", Probe::AuthRequired), runtime("claude", Probe::Ready)]);
    let refused = send_to_codex(&t).unwrap_err();
    assert_eq!(refused, "Codex CLI is signed out. Sign in to it, then retry discovery.");
    assert_eq!(h.sweeps(), 1);
    assert!(h.asked().is_empty());
    assert_eq!(t.missions(), 0, "a refused start leaves no record");
}

#[test]
fn a_real_start_on_a_runtime_discovery_does_not_find_is_refused_as_not_installed() {
    let t = Temp::new("missing");
    let _g = hooks::install(t.store(), fake_codex());
    let h = install(harness(60_000));
    h.set_held(vec![runtime("claude", Probe::Ready)]);
    assert_eq!(send_to_codex(&t).unwrap_err(), "Codex CLI is not installed on this machine. Install it, then retry discovery.");
    assert_eq!(t.missions(), 0);
}

#[test]
fn your_own_agent_and_an_added_account_are_not_asked_the_default_sign_in() {
    let t = Temp::new("skip");
    let _g = hooks::install(t.store(), fake_codex());
    let h = install(harness(60_000));
    h.set_held(Vec::new());
    assert_eq!(crate::native::start_readiness_refusal("acp", None), None);
    assert_eq!(crate::native::start_readiness_refusal("codex", Some("work")), None);
    assert_eq!((h.sweeps(), h.asked().len()), (0, 0));
    assert!(crate::native::start_readiness_refusal("codex", None).is_some());
}

// case: says a runtime whose own service did not answer could not be reached, not that it is signed out (0.495)
#[test]
fn which_kind_of_not_ready_is_said_as_electron_says_it() {
    let found = |status: Probe, installed: bool| Found { installed, ..runtime("claude", status) };
    assert_eq!(not_ready_refusal("claude", None), "Claude Code is not installed on this machine. Install it, then retry discovery.");
    assert_eq!(
        not_ready_refusal("claude", Some(&found(Probe::Missing, false))),
        "Claude Code is not installed on this machine. Install it, then retry discovery."
    );
    assert_eq!(not_ready_refusal("claude", Some(&found(Probe::AuthRequired, true))), "Claude Code is signed out. Sign in to it, then retry discovery.");
    assert_eq!(
        not_ready_refusal("cursor", Some(&found(Probe::Offline, true))),
        "Cursor Agent could not be reached just now: it did not answer when Locust checked on it, which usually means its own service is down. Try again in a minute, or pick another model."
    );
    let codex_down = not_ready_refusal("codex", Some(&runtime("codex", Probe::Offline)));
    assert!(codex_down.contains("could not be reached just now") && !codex_down.contains("sign in"), "{codex_down}");
    assert_eq!(not_ready_refusal("opencode", Some(&found(Probe::ProbeFailed, true))), "OpenCode is not ready. Install or sign in to it, then retry discovery.");
}

#[test]
fn a_sweep_stamps_an_answered_agent_fresh_and_leaves_one_still_being_checked_stale() {
    let clock = Arc::new(AtomicU64::new(TTL + 1));
    let held: Arc<Mutex<Option<HeldAnswer>>> =
        Arc::new(Mutex::new(Some(HeldAnswer { at: 0, value: vec![runtime("claude", Probe::Ready), runtime("codex", Probe::Ready)] })));
    let asked = Arc::new(Mutex::new(Vec::<String>::new()));
    let readiness = StartReadiness::new(StartReadinessOptions {
        ttl_ms: TTL,
        now: Box::new({
            let c = clock.clone();
            move || c.load(Ordering::SeqCst)
        }),
        held: Box::new({
            let h = held.clone();
            move || h.lock().unwrap().clone()
        }),
        store: Box::new({
            let h = held.clone();
            move |value| {
                let mut h = h.lock().unwrap();
                let at = h.as_ref().map_or(0, |a| a.at);
                *h = Some(HeldAnswer { at, value });
            }
        }),
        sweep: Box::new(Vec::new),
        ask_one: Box::new({
            let a = asked.clone();
            move |id| {
                a.lock().unwrap().push(id.to_owned());
                Ok(Some(runtime(id, Probe::Ready)))
            }
        }),
        sweep_only: Default::default(),
    });
    // Both asked alone at TTL + 1.
    readiness.for_start(Some("claude"));
    readiness.for_start(Some("codex"));
    assert_eq!(asked.lock().unwrap().len(), 2);
    // A sweep at 2 TTL + 1 answers claude, and has codex still being checked.
    let still_checking = Found { checking: true, ..runtime("codex", Probe::Ready) };
    crate::agents::hold_sweep(&held, &readiness, &[runtime("claude", Probe::Ready), still_checking], 2 * TTL + 1);
    clock.store(2 * TTL + 2, Ordering::SeqCst);
    readiness.for_start(Some("claude"));
    assert_eq!(asked.lock().unwrap().len(), 2, "the sweep's answer is fresh: claude is not asked again");
    readiness.for_start(Some("codex"));
    assert_eq!(asked.lock().unwrap().len(), 3, "a check still running has not been asked: codex is asked again");
    assert_eq!(asked.lock().unwrap().last().map(String::as_str), Some("codex"));
}

#[test]
fn a_start_on_an_agent_still_being_checked_waits_for_that_agents_own_check_alone() {
    let checking = |id: &str| Found { checking: true, ready: false, ..runtime(id, Probe::Unknown) };
    let asked = Mutex::new(Vec::<String>::new());
    let ask = |id: &str| {
        asked.lock().unwrap().push(id.to_owned());
        Some(fresh(id))
    };
    let found = crate::agents::waits_for_its_own_check(Some("claude"), vec![checking("claude"), checking("codex")], ask);
    assert_eq!(found, vec![fresh("claude"), checking("codex")]);
    assert_eq!(*asked.lock().unwrap(), vec!["claude".to_owned()]);
    // One already answered, Antigravity, and no runtime at all are never asked.
    let ready = vec![runtime("claude", Probe::Ready), checking("antigravity")];
    assert_eq!(crate::agents::waits_for_its_own_check(Some("claude"), ready.clone(), ask), ready);
    assert_eq!(crate::agents::waits_for_its_own_check(Some("antigravity"), ready.clone(), ask), ready);
    assert_eq!(crate::agents::waits_for_its_own_check(None, ready.clone(), ask), ready);
    assert_eq!(asked.lock().unwrap().len(), 1);
}

#[test]
fn the_engines_own_start_reads_the_last_full_sweep_and_asks_nothing_while_it_is_fresh() {
    // The process's own readiness: what a sweep held is what a start on a ready agent gets, with no probe run.
    let answer = vec![runtime("codex", Probe::Ready), runtime("claude", Probe::AuthRequired)];
    crate::agents::held_a_sweep(&answer);
    assert_eq!(crate::agents::discover_for_start(Some("codex")), answer);
}

// ---- LIVE2 #19: a start right after the engine opens refused a ready agent ----------------------------------------
// Under load, the first discovery's probe did not answer in time and the start refused "Install or sign in" for an
// agent that answered in 1.7 s from a shell. A check that did not answer is asked once more before a start refuses.

#[test]
fn a_real_start_on_a_runtime_whose_check_did_not_answer_asks_it_once_more_and_goes_through() {
    let t = Temp::new("probe-failed");
    let _g = hooks::install(t.store(), fake_codex());
    let h = install(harness(60_000));
    h.set_held(vec![runtime("codex", Probe::ProbeFailed), runtime("claude", Probe::Ready)]);
    let started = send_to_codex(&t);
    assert!(started.is_ok(), "{started:?}");
    // The full path first (it was not ready last time), then the starting runtime alone, once.
    assert_eq!((h.sweeps(), h.asked()), (1, vec!["codex".to_owned()]));
    // What it answered is kept for the next start.
    assert_eq!(h.held.lock().unwrap().clone().unwrap().value[0], fresh("codex"));
}

#[test]
fn a_signed_out_or_missing_runtime_is_an_answer_and_is_not_asked_again() {
    let h = harness(60_000);
    h.set_held(vec![runtime("codex", Probe::AuthRequired), Found { installed: false, ..runtime("claude", Probe::ProbeFailed) }]);
    h.readiness.for_start(Some("codex"));
    h.readiness.for_start(Some("claude"));
    assert_eq!((h.sweeps(), h.asked().len()), (2, 0));
}

#[test]
fn antigravity_silent_once_then_answering_is_ready_for_the_start() {
    // The agents seams: `agy models` times out the first time (the busy Mac) and lists models the second.
    let models_asked = Arc::new(AtomicUsize::new(0));
    let sweep = {
        let models_asked = models_asked.clone();
        move || {
            let find = |c: &str| (c == "agy").then(|| std::path::PathBuf::from("/usr/local/bin/agy"));
            let run = |_: &std::path::Path, a: &[&str]| -> Option<crate::agents::Answer> {
                let answer = |exit: Option<i32>, stdout: &str, timed_out: bool| crate::agents::Answer {
                    exit,
                    stdout: stdout.into(),
                    stderr: String::new(),
                    timed_out,
                };
                if a == ["--version"] {
                    return Some(answer(Some(0), "1.2.14\n", false));
                }
                if models_asked.fetch_add(1, Ordering::SeqCst) == 0 {
                    return Some(answer(None, "", true));
                }
                Some(answer(Some(0), "Fetching available models...\ngemini-4-argon-high\tGemini 4 Argon (High)\n", false))
            };
            let exists = |_: &std::path::Path| true;
            let seams = crate::agents::Seams { find: &find, run: &run, exists: &exists, home: None, xdg_config_home: None, recall: None };
            vec![crate::agents::discover_antigravity(&seams)]
        }
    };
    let readiness = StartReadiness::new(StartReadinessOptions {
        ttl_ms: TTL,
        now: Box::new(|| 0),
        held: Box::new(|| None),
        store: Box::new(|_| {}),
        sweep: Box::new(sweep),
        ask_one: Box::new(|id| panic!("{id} is not a CLI to ask alone")),
        sweep_only: crate::agents::sweep_only_at_start(),
    });
    let found = readiness.for_start(Some("antigravity"));
    assert_eq!((found[0].status, found[0].ready), (Probe::Ready, true));
    assert_eq!(models_asked.load(Ordering::SeqCst), 2);
}

#[test]
fn a_check_that_still_does_not_answer_is_said_as_electron_says_it() {
    // codex-mission.ts notReadyRefusal, readiness 'unknown'.
    let silent = Found { auth: Auth::Unknown, ..runtime("antigravity", Probe::ProbeFailed) };
    assert_eq!(
        not_ready_refusal("antigravity", Some(&silent)),
        "Antigravity did not answer in time when Locust checked whether it was ready, so nothing was sent. Try again in a moment."
    );
    assert_eq!(
        not_ready_refusal("claude", Some(&runtime("claude", Probe::Unknown))),
        "Claude Code did not answer in time when Locust checked whether it was ready, so nothing was sent. Try again in a moment."
    );
}
