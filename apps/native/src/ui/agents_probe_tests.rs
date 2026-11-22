//! The agents list as the window keeps it while checks run: a late check read back from the held answer
//! (`a-late-check-is-never-lost.test.ts`, the window's half), a CLI that never answers given a way past
//! (`a-hung-cli-gets-a-way-past.test.tsx`), and Check again asking for a new sweep rather than the short-lived answer
//! (`runtime-discovery.test.ts`'s cache, as the window uses it).
use super::*;
use locust_core::agents::{Auth, Probe};
use locust_core::discovery_service::{DiscoveryService, Seams};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

fn agent(id: &str, name: &str, status: Probe, checking: bool) -> Found {
    Found {
        id: id.into(),
        display_name: name.into(),
        installed: true,
        version: (status == Probe::Ready).then(|| "1.18.27".into()),
        auth: if id == "opencode" { Auth::NotApplicable } else { Auth::Unknown },
        ready: status == Probe::Ready,
        status,
        unsupported_note: None,
        through_cli: false,
        checking,
    }
}

/// A service on fakes: every sweep it starts is counted, and the held answer is whatever the test puts there.
struct Fake {
    sweeps: Arc<AtomicUsize>,
    reads: Arc<AtomicUsize>,
    held: Arc<Mutex<Option<Vec<Found>>>>,
    service: &'static DiscoveryService,
}

fn fake(sweep_answer: Vec<Found>) -> Fake {
    let (sweeps, reads, held) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)), Arc::new(Mutex::new(None)));
    let (s, r, h) = (sweeps.clone(), reads.clone(), held.clone());
    let service = use_discovery(DiscoveryService::new(Seams {
        probe: Box::new(move || {
            s.fetch_add(1, Ordering::SeqCst);
            sweep_answer.clone()
        }),
        now: Box::new(|| 0),
        ttl_ms: 10_000,
        held: Box::new(move || {
            r.fetch_add(1, Ordering::SeqCst);
            h.lock().unwrap().clone()
        }),
    }));
    Fake { sweeps, reads, held, service }
}

fn window(agents: Vec<Found>) -> rune::Runtime<State> {
    let mut s = State::default();
    s.settings_panel.agents = Some(agents);
    let mut rt = rune::Runtime::new(s, |s, _| section(s, &Palette::new(true)), (1100.0, 1400.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn polled(rt: &mut rune::Runtime<State>) -> bool {
    let mut changed = false;
    rt.update(|s, _| changed = poll(s));
    rt.frame(0.05);
    changed
}

/// A sweep's answer lands through the job, as discovery's does.
fn lands(rt: &mut rune::Runtime<State>, answer: Vec<Found>) {
    rt.update(move |s, _| s.settings_panel.agents_job.request((), move |()| answer));
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut landed = false;
    while !landed && std::time::Instant::now() < until {
        rt.update(|s, _| landed = poll(s));
        if !landed {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    assert!(landed, "the answer never landed");
    rt.frame(0.05);
}

fn texts(rt: &rune::Runtime<State>) -> String {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect::<Vec<_>>().join(" | ")
}

fn label_of(rt: &rune::Runtime<State>, key: &str) -> Option<String> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.label)
}

fn press(rt: &mut rune::Runtime<State>, key: &str) {
    rt.frame(0.05);
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("{key} is drawn"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
}

// ---- a-late-check-is-never-lost.test.ts ----

// case: never spends the give-up budget on a check that is still running
#[test]
fn a_check_still_running_never_spends_the_give_up_budget() {
    let mut rt = window(Vec::new());
    for _ in 0..SWEEPS_BEFORE_GIVING_UP + 1 {
        lands(&mut rt, vec![agent("opencode", "OpenCode", Probe::ProbeFailed, true)]);
    }
    assert!(!rt.state.settings_panel.agents_gave_up, "OpenCode was only still being checked");
    // The control: a check that came back without an answer does spend it.
    for _ in 0..SWEEPS_BEFORE_GIVING_UP {
        lands(&mut rt, vec![agent("opencode", "OpenCode", Probe::ProbeFailed, false)]);
    }
    assert!(rt.state.settings_panel.agents_gave_up);
}

// case: reads the held answer back on a finish: no new check, and past giving up
#[test]
fn a_finished_late_check_is_read_back_once_with_no_new_check_even_after_giving_up() {
    let f = fake(vec![agent("opencode", "OpenCode", Probe::ProbeFailed, true)]);
    let mut rt = window(vec![agent("opencode", "OpenCode", Probe::ProbeFailed, true), agent("codex", "Codex CLI", Probe::Ready, false)]);
    rt.update(|s, _| s.settings_panel.agents_gave_up = true);
    assert!(!polled(&mut rt), "nothing finished yet: nothing read");
    assert_eq!(f.reads.load(Ordering::SeqCst), 0);
    // OpenCode answers twenty seconds in: the host patches what it holds and says it finished.
    *f.held.lock().unwrap() = Some(vec![agent("opencode", "OpenCode", Probe::Ready, false), agent("codex", "Codex CLI", Probe::Ready, false)]);
    f.service.answered_late("opencode");
    assert!(polled(&mut rt));
    let opencode = rt.state.settings_panel.agents.as_ref().unwrap().iter().find(|e| e.id == "opencode").unwrap().clone();
    assert!(opencode.ready && !opencode.checking, "{opencode:?}");
    assert!(texts(&rt).contains("1.18.27"), "{}", texts(&rt));
    assert_eq!(f.sweeps.load(Ordering::SeqCst), 0, "read back, not asked again");
    assert!(!rt.state.settings_panel.agents_job.is_pending(), "no new check was started");
    assert!(rt.state.settings_panel.agents_gave_up, "and the give-up budget did not stop it");
    assert_eq!(f.reads.load(Ordering::SeqCst), 1);
    // Once per finish: a list that says "checking" again is not read back until a new finish says so.
    rt.update(|s, _| s.settings_panel.agents = Some(vec![agent("opencode", "OpenCode", Probe::ProbeFailed, true)]));
    polled(&mut rt);
    assert_eq!(f.reads.load(Ordering::SeqCst), 1, "{:?}", rt.state.settings_panel.agents);
    // A finish heard before the answer that still calls it checking: that answer is reconciled when it lands.
    f.service.answered_late("opencode");
    rt.update(|s, _| s.settings_panel.agents = None);
    polled(&mut rt);
    lands(&mut rt, vec![agent("opencode", "OpenCode", Probe::ProbeFailed, true)]);
    let opencode = rt.state.settings_panel.agents.as_ref().unwrap().iter().find(|e| e.id == "opencode").unwrap().clone();
    assert!(opencode.ready, "{opencode:?}");
}

// ---- a-hung-cli-gets-a-way-past.test.tsx ----

fn hung() -> Found {
    Found { auth: Auth::Unknown, ..agent("opencode", "OpenCode", Probe::ProbeFailed, false) }
}

// case: says checking once, in the head note, while discovery is still asking
#[test]
fn while_discovery_is_still_asking_the_head_says_checking_once() {
    let rt = window(vec![hung()]);
    let words = texts(&rt);
    assert!(words.contains("checking 1 on this machine"), "{words}");
    assert!(words.contains("CHECKING"), "the row's state is its tag: {words}");
    assert!(rt.find("agents-row-check-again-opencode").is_none());
    assert!(!words.contains("did not answer its version check"), "{words}");
    // Words are for results: the row says nothing of its own while it is asked.
    assert!(!words.contains("OpenCode did not answer"), "{words}");
}

// case: says what was tried once discovery has given up, and offers Check again -- not Install, which it already is
#[test]
fn once_discovery_gave_up_the_row_says_what_was_tried_and_offers_check_again() {
    let mut rt = window(vec![hung()]);
    rt.update(|s, _| s.settings_panel.agents_gave_up = true);
    rt.frame(0.05);
    let words = texts(&rt);
    assert!(words.contains("did not answer its version check"), "{words}");
    assert!(rt.find("agents-row-check-again-opencode").is_some());
    assert!(!words.contains("Install"), "{words}");
    assert!(!words.contains("checking 1 on this machine"), "{words}");
    assert!(words.contains("1 on this machine \u{b7} none answering"), "{words}");
    // Its Check again asks again, with the budget back in full.
    press(&mut rt, "agents-row-check-again-opencode");
    assert!(!rt.state.settings_panel.agents_gave_up);
    assert_eq!(rt.state.settings_panel.agent_sweeps, 0);
    assert!(texts(&rt).contains("checking 1 on this machine"));
}

// case: says why, on the control itself
#[test]
fn the_rows_check_again_says_why() {
    let mut rt = window(vec![hung()]);
    rt.update(|s, _| s.settings_panel.agents_gave_up = true);
    rt.frame(0.05);
    let said = label_of(&rt, "agents-row-check-again-opencode").unwrap_or_default();
    assert!(said.contains("did not answer its version check in 5 seconds, four times"), "{said}");
    assert!(said.starts_with("OpenCode is on this machine"), "{said}");
}

// ---- runtime-discovery.test.ts, as the window uses the service ----

/// Check again asks for a new sweep: inside the short cache an ordinary re-ask reuses the answer, a press does not.
#[test]
fn check_again_starts_a_new_sweep_where_a_re_ask_reuses_the_answer() {
    let f = fake(vec![agent("codex", "Codex CLI", Probe::Ready, false)]);
    let mut rt = window(vec![agent("codex", "Codex CLI", Probe::ProbeFailed, false)]);
    let settle = |rt: &mut rune::Runtime<State>| {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while rt.state.settings_panel.agents_job.is_pending() && std::time::Instant::now() < until {
            polled(rt);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        polled(rt);
    };
    rt.update(|s, _| request(s));
    settle(&mut rt);
    rt.update(|s, _| request(s));
    settle(&mut rt);
    assert_eq!(f.sweeps.load(Ordering::SeqCst), 1, "the second ask was the short-lived answer");
    assert!(rt.state.settings_panel.agents.as_ref().unwrap()[0].ready);
    press(&mut rt, "agents-check-again");
    settle(&mut rt);
    assert_eq!(f.sweeps.load(Ordering::SeqCst), 2, "Check again swept again");
}
