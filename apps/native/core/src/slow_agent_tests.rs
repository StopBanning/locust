//! Slow-agent deadline parity tests (Electron `main/a-slow-agent-holds-nothing-up.test.ts`).

use crate::agents::{
    Auth, FIRST_SCREEN_DEADLINE, Found, Integration, OutstandingChecks, Probe, SweepCheck,
    keep_what_was_answered, pending_found, row_status, settle_within, sweep_has_pending_agent,
};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

fn record(id: &str, status: Probe) -> Found {
    Found {
        id: id.into(),
        display_name: id.into(),
        installed: true,
        version: None,
        auth: match status {
            Probe::Ready => Auth::Authenticated,
            Probe::AuthRequired => Auth::Unauthenticated,
            _ => Auth::Unknown,
        },
        ready: status == Probe::Ready,
        status,
        unsupported_note: None,
        through_cli: false,
        checking: false,
    }
}

fn instant(id: &str, status: Probe) -> SweepCheck {
    let val = record(id, status);
    let name = id.to_owned();
    let id_owned = id.to_owned();
    SweepCheck {
        id: id.into(),
        run: Box::new(move |located| {
            located();
            Some(val)
        }),
        pending: Box::new(move || pending_found(&id_owned, &name)),
    }
}

struct Gate {
    pair: Arc<(Mutex<Option<Option<Found>>>, Condvar)>,
}

impl Gate {
    fn new() -> Self {
        Self {
            pair: Arc::new((Mutex::new(None), Condvar::new())),
        }
    }

    fn release(&self, value: Option<Found>) {
        let (lock, cv) = &*self.pair;
        let mut g = lock.lock().unwrap();
        *g = Some(value);
        cv.notify_all();
    }

    fn check(&self, id: &str) -> SweepCheck {
        let pair = Arc::clone(&self.pair);
        let id_owned = id.to_owned();
        let name = id.to_owned();
        SweepCheck {
            id: id.into(),
            run: Box::new(move |located| {
                located();
                let (lock, cv) = &*pair;
                let mut g = lock.lock().unwrap();
                while g.is_none() {
                    g = cv.wait(g).unwrap();
                }
                g.as_ref().cloned().flatten()
            }),
            pending: Box::new(move || pending_found(&id_owned, &name)),
        }
    }
}

#[test]
fn first_screen_deadline_is_the_fast_agents_time() {
    assert!(FIRST_SCREEN_DEADLINE >= Duration::from_millis(1_500));
    assert!(FIRST_SCREEN_DEADLINE <= Duration::from_millis(4_000));
}

#[test]
fn released_the_moment_every_agent_has_answered_not_at_deadline() {
    let started = Instant::now();
    let settled = settle_within(
        vec![instant("claude", Probe::Ready), instant("codex", Probe::Ready)],
        Duration::from_secs(5),
        |_, _| {},
    );
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(settled.answers.len(), 2);
    assert_eq!(settled.answers[0].id, "claude");
    assert_eq!(settled.answers[1].id, "codex");
    assert!(settled.left.is_empty());
}

#[test]
fn goes_on_without_a_slow_agent_at_the_deadline_and_shows_checking() {
    let opencode = Gate::new();
    let started = Instant::now();
    let settled = settle_within(
        vec![
            instant("claude", Probe::Ready),
            opencode.check("opencode"),
            instant("codex", Probe::AuthRequired),
        ],
        Duration::from_millis(40),
        |_, _| {},
    );
    let waited = started.elapsed();
    assert!(waited >= Duration::from_millis(30));
    assert!(waited < Duration::from_secs(1));
    assert_eq!(settled.answers[0], record("claude", Probe::Ready));
    assert_eq!(settled.answers[1], pending_found("opencode", "opencode"));
    assert_eq!(settled.answers[2], record("codex", Probe::AuthRequired));
    assert_eq!(settled.left.keys().map(String::as_str).collect::<Vec<_>>(), vec!["opencode"]);
    opencode.release(Some(record("opencode", Probe::Ready)));
}

#[test]
fn does_not_let_one_slow_agent_delay_another() {
    let cursor = Gate::new();
    let opencode = Gate::new();
    let settled = settle_within(
        vec![cursor.check("cursor"), instant("claude", Probe::Ready), opencode.check("opencode")],
        Duration::from_millis(30),
        |_, _| {},
    );
    let ids: Vec<&str> = settled.answers.iter().map(|f| f.id.as_str()).collect();
    assert_eq!(ids, vec!["cursor", "claude", "opencode"]);
    assert_eq!(settled.answers[1], record("claude", Probe::Ready));
    assert!(settled.left.contains_key("cursor"));
    assert!(settled.left.contains_key("opencode"));
    cursor.release(None);
    opencode.release(None);
}

#[test]
fn hands_over_a_late_answer_once_when_it_lands() {
    let heard: Arc<Mutex<Vec<(String, Option<Found>)>>> = Arc::new(Mutex::new(Vec::new()));
    let heard_cb = Arc::clone(&heard);
    let opencode = Gate::new();
    let settled = settle_within(
        vec![instant("claude", Probe::Ready), opencode.check("opencode")],
        Duration::from_millis(20),
        move |id, answer| {
            heard_cb.lock().unwrap().push((id.to_owned(), answer.cloned()));
        },
    );
    assert!(heard.lock().unwrap().is_empty());
    let real = record("opencode", Probe::Ready);
    opencode.release(Some(real.clone()));
    assert_eq!(settled.left.get("opencode").unwrap().wait(), Some(real.clone()));
    // Waited for, not slept on: the hand-over runs on its own thread, and 20 ms lost the race under a full gate (20 Nov).
    crate::load_waits::wait_for("the late answer to be handed over", || !heard.lock().unwrap().is_empty());
    std::thread::sleep(Duration::from_millis(20));
    let logged = heard.lock().unwrap();
    assert_eq!(logged.len(), 1);
    assert_eq!(logged[0], ("opencode".into(), Some(real)));
}

#[test]
fn tells_nobody_later_about_an_agent_that_answered_in_time() {
    let heard: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let heard_cb = Arc::clone(&heard);
    let claude = Gate::new();
    claude.release(Some(record("claude", Probe::Ready)));
    let _settled = settle_within(vec![claude.check("claude")], Duration::from_millis(500), move |id, _| {
        heard_cb.lock().unwrap().push(id.to_owned());
    });
    std::thread::sleep(Duration::from_millis(20));
    assert!(heard.lock().unwrap().is_empty());
}

#[test]
fn reads_a_hung_agent_as_checking_and_a_failed_one_reports_none() {
    let muse = Gate::new();
    let cursor = Gate::new();
    let heard: Arc<Mutex<Vec<(String, Option<Found>)>>> = Arc::new(Mutex::new(Vec::new()));
    let heard_cb = Arc::clone(&heard);
    let settled = settle_within(
        vec![instant("codex", Probe::Ready), muse.check("muse"), cursor.check("cursor")],
        Duration::from_millis(20),
        move |id, ans| heard_cb.lock().unwrap().push((id.to_owned(), ans.cloned())),
    );
    let shown = &settled.answers[1];
    assert!(shown.installed && shown.checking && !shown.ready);
    assert_eq!(row_status(shown, Integration::Live, false, None).tag, "CHECKING");
    cursor.release(None);
    std::thread::sleep(Duration::from_millis(20));
    let logged = heard.lock().unwrap().clone();
    assert_eq!(logged, vec![("cursor".into(), None)]);
    assert!(!logged.iter().any(|(id, _)| id == "muse"));
    muse.release(None);
}

#[test]
fn leaves_an_agent_with_no_answer_out_of_the_sweep() {
    let none = SweepCheck {
        id: "antigravity".into(),
        run: Box::new(|located| {
            located();
            None
        }),
        pending: Box::new(|| pending_found("antigravity", "Antigravity")),
    };
    let settled = settle_within(vec![none, instant("claude", Probe::Ready)], Duration::from_millis(50), |_, _| {});
    assert_eq!(settled.answers, vec![record("claude", Probe::Ready)]);
}

#[test]
fn waits_for_located_past_the_deadline_before_calling_pending() {
    let loc_pair = Arc::new((Mutex::new(false), Condvar::new()));
    let loc_pair_run = Arc::clone(&loc_pair);
    let shown_at = Arc::new(Mutex::new(false));
    let shown_cb = Arc::clone(&shown_at);
    let check = SweepCheck {
        id: "opencode".into(),
        run: Box::new(move |located| {
            let (lock, cv) = &*loc_pair_run;
            let mut g = lock.lock().unwrap();
            while !*g {
                g = cv.wait(g).unwrap();
            }
            located();
            std::thread::sleep(Duration::from_millis(80));
            Some(record("opencode", Probe::Ready))
        }),
        pending: Box::new(move || {
            *shown_cb.lock().unwrap() = true;
            pending_found("opencode", "opencode")
        }),
    };
    let handle = std::thread::spawn(move || settle_within(vec![check], Duration::from_millis(10), |_, _| {}));
    std::thread::sleep(Duration::from_millis(40));
    assert!(!*shown_at.lock().unwrap());
    {
        let (lock, cv) = &*loc_pair;
        *lock.lock().unwrap() = true;
        cv.notify_all();
    }
    let settled = handle.join().unwrap();
    assert!(*shown_at.lock().unwrap());
    assert_eq!(settled.answers, vec![pending_found("opencode", "opencode")]);
}

#[test]
fn pending_agent_marks_sweep_incomplete_for_model_cache() {
    assert!(sweep_has_pending_agent(&[pending_found("opencode", "OpenCode")]));
    assert!(!sweep_has_pending_agent(&[record("copilot", Probe::Ready)]));
    let mut not_installed = pending_found("opencode", "OpenCode");
    not_installed.installed = false;
    assert!(!sweep_has_pending_agent(&[not_installed]));
}

#[test]
fn start_waits_for_that_agents_own_check_and_no_other() {
    let opencode = Gate::new();
    let settled = settle_within(
        vec![instant("claude", Probe::Ready), opencode.check("opencode")],
        Duration::from_millis(20),
        |_, _| {},
    );
    let mut held = OutstandingChecks::default();
    held.set(settled.left.get("opencode").unwrap().clone());

    let started = Instant::now();
    assert_eq!(held.for_start(Some("claude"), &settled.answers), settled.answers);
    assert_eq!(held.for_start(None, &settled.answers), settled.answers);
    assert!(started.elapsed() < Duration::from_millis(500));

    let real = record("opencode", Probe::Ready);
    opencode.release(Some(real.clone()));
    let updated = held.for_start(Some("opencode"), &settled.answers);
    assert_eq!(updated, vec![record("claude", Probe::Ready), real]);
}

#[test]
fn keeps_last_definite_answer_when_later_sweep_goes_on_without_it() {
    let cursor = Gate::new();
    let settled = settle_within(
        vec![instant("claude", Probe::Ready), cursor.check("cursor")],
        Duration::from_millis(20),
        |_, _| {},
    );
    let answered_ready = record("cursor", Probe::Ready);
    let shown = keep_what_was_answered(&settled, &[answered_ready.clone(), record("claude", Probe::Ready)]);
    assert_eq!(shown[0], settled.answers[0]);
    assert_eq!(shown[1], answered_ready);

    // Keeps signed-out too, but never an indefinite/offline answer.
    let signed_out = record("cursor", Probe::AuthRequired);
    let offline = record("gemini", Probe::Offline);
    let settled2 = settle_within(
        vec![Gate::new().check("cursor"), Gate::new().check("opencode"), Gate::new().check("gemini")],
        Duration::from_millis(15),
        |_, _| {},
    );
    let shown2 = keep_what_was_answered(&settled2, &[signed_out.clone(), pending_found("opencode", "opencode"), offline]);
    assert_eq!(shown2[0], signed_out);
    assert_eq!(shown2[1], pending_found("opencode", "opencode"));
    assert_eq!(shown2[2], pending_found("gemini", "gemini"));

    // A start does not wait for an agent shown from its last definite answer.
    let mut held = OutstandingChecks::default();
    held.set(settled.left.get("cursor").unwrap().clone());
    let started = Instant::now();
    assert_eq!(held.for_start(Some("cursor"), &[answered_ready.clone()]), vec![answered_ready]);
    assert!(started.elapsed() < Duration::from_millis(500));
    cursor.release(None);
}
