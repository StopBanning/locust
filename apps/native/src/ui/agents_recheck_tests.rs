//! The window asks again after a sweep that left an agent unanswered, every RECHECK, and gives up after
//! SWEEPS_BEFORE_GIVING_UP (App.tsx's give-up budget); only then does the chat box count the agents not answering.
//! Fixed 20 Nov: Rust swept once, so an agent past the first-screen deadline said "checking" until Settings opened.
use super::*;
use locust_core::agents::{Auth, Probe};

fn codex(status: Probe, checking: bool) -> Found {
    Found {
        id: "codex".into(),
        display_name: "Codex CLI".into(),
        installed: true,
        version: (status == Probe::Ready).then(|| "1.0.0".into()),
        auth: Auth::Unknown,
        ready: status == Probe::Ready,
        status,
        unsupported_note: None,
        through_cli: false,
        checking,
    }
}

fn window() -> rune::Runtime<State> {
    let mut s = State::default();
    crate::scenes::composer_base_scene(&mut s, false);
    s.demo = false;
    let mut rt = rune::Runtime::new(s, |s, _| section(s, &Palette::new(true)), (1100.0, 1400.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

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

#[test]
fn an_agent_that_does_not_answer_is_asked_again_and_the_box_waits() {
    let mut rt = window();
    lands(&mut rt, vec![codex(Probe::ProbeFailed, false)]);
    assert_eq!(rt.state.settings_panel.agent_sweeps, 1);
    assert!(!rt.state.settings_panel.agents_gave_up, "one sweep is not giving up");
    assert_eq!(crate::composer_wiring::composer_placeholder(&rt.state), "Checking the AI agents on this machine…");
    assert_eq!(RECHECK, std::time::Duration::from_secs(15));
}

#[test]
fn after_four_sweeps_the_window_gives_up_and_the_box_counts() {
    let mut rt = window();
    for _ in 0..SWEEPS_BEFORE_GIVING_UP - 1 {
        lands(&mut rt, vec![codex(Probe::ProbeFailed, false)]);
        assert!(!rt.state.settings_panel.agents_gave_up);
    }
    lands(&mut rt, vec![codex(Probe::ProbeFailed, false)]);
    assert!(rt.state.settings_panel.agents_gave_up);
    let said = crate::composer_wiring::composer_placeholder(&rt.state);
    assert!(said.contains("An AI agent is installed but not answering"), "{said}");
}

#[test]
fn an_agent_still_being_asked_counts_as_unanswered() {
    let mut rt = window();
    for _ in 0..SWEEPS_BEFORE_GIVING_UP {
        lands(&mut rt, vec![codex(Probe::Unknown, true)]);
    }
    assert!(rt.state.settings_panel.agents_gave_up);
}

#[test]
fn an_answer_stops_the_asking() {
    let mut rt = window();
    lands(&mut rt, vec![codex(Probe::ProbeFailed, false)]);
    for _ in 0..SWEEPS_BEFORE_GIVING_UP {
        lands(&mut rt, vec![codex(Probe::Ready, false)]);
    }
    assert!(!rt.state.settings_panel.agents_gave_up, "nothing is left to give up on");
    assert_eq!(crate::composer_wiring::composer_placeholder(&rt.state), "Message Codec…");
}

#[test]
fn check_again_gives_the_budget_back() {
    let mut rt = window();
    for _ in 0..SWEEPS_BEFORE_GIVING_UP {
        lands(&mut rt, vec![codex(Probe::ProbeFailed, false)]);
    }
    assert!(rt.state.settings_panel.agents_gave_up);
    let (x, y) = rt.find("agents-check-again").expect("Check again is drawn");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
    assert!(!rt.state.settings_panel.agents_gave_up);
    assert_eq!(rt.state.settings_panel.agent_sweeps, 0);
}
