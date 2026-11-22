//! the-plan-marks-the-step-underway.test.tsx (5 cases): while a turn is underway the plan's running step carries an orb
//! of its own in place of the pulsing pip; with no turn running the pip stays; a done step is never marked; and the
//! plan is told only whether a turn is underway, never which orb the live line draws.

use super::*;

const STEPS: [(&str, &str); 3] = [("Read the ledger", "done"), ("Fix the parser", "running"), ("Write it up", "pending")];

fn plan() -> ledger::Item {
    let steps = STEPS.iter().map(|(text, state)| ledger::PlanStep { text: (*text).into(), state: (*state).into() }).collect();
    ledger::Item::Plan { id: "plan".into(), steps, finished: false, stopped: false }
}

/// A conversation whose turn is still running (`running`), with the live line up or not (`live`, in the register
/// named): the thread drawn in a window, the plan card among its items.
fn state(running: bool, live: Option<&str>) -> State {
    let mut s = State::default();
    let mut line = locust_core::thread_view::live_line(&[], true, "2026-09-04T21:47:00.000Z", false);
    if let (Some(register), Some(l)) = (live, line.as_mut()) {
        l.register = register.into();
        l.label = if register == "tool" { "shell".into() } else { "Thinking".into() };
        l.orb = if register == "tool" { "working" } else { "thinking" };
    }
    s.ledger = Some(LedgerPane {
        cold_tip: None,
        header: Teammate { id: "tm_codec".into(), name: "Codec".into(), role: "Engineer".into(), runtime: None, model: None, account: None, busy: false, runs_in: "local".into(), folder: None },
        conversation: Conversation { conversation_id: Some("m_1".into()), turns: Vec::new() },
        items: Vec::new(), dir: std::path::PathBuf::new(), worker: None,
        snapshot: ledger::Snapshot {
            running,
            live: if live.is_some() { line } else { None },
            turn_metas: vec![ledger::Metadata { mission_id: "m_1".into(), ..Default::default() }],
            ..Default::default()
        },
        due: 0.0, loading: false, after_tip: None, after_tip_deadline: None, backoff: 0.0,
    });
    s
}

/// Every element of the thread, in draw order, with the plan card in it.
fn drawn(mut s: State) -> Vec<rune::ElementInfo> {
    // A turn that did something, so its plan is a list of work with states (not the answer of one that only planned).
    let call = ledger::Item::Tool { name: "shell".into(), status: ledger::Status::Done, summary: "cat ledger.rs".into(), detail: Box::default() };
    s.ledger.as_mut().unwrap().items = vec![ledger::Item::User("fix it".into()), plan(), call];
    let mut rt = rune::Runtime::new(s, |s, _| super::super::messages::message_column(&s.ledger.as_ref().unwrap().items, "Codec", &Palette::new(true), s), (800.0, 900.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(0.016);
    rt.elements()
}

fn underway() -> Vec<rune::ElementInfo> {
    drawn(state(true, Some("thinking")))
}

fn at(els: &[rune::ElementInfo], part: &str) -> Option<usize> {
    els.iter().position(|e| e.key.as_deref().is_some_and(|k| k.contains(part)))
}

fn keys(els: &[rune::ElementInfo]) -> Vec<String> {
    els.iter().filter_map(|e| e.key.clone()).filter(|k| k.contains("plan")).collect()
}

// case: draws the orb on the running step when a turn is still going
#[test]
fn draws_the_orb_on_the_running_step_when_a_turn_is_still_going() {
    let els = underway();
    assert!(at(&els, &format!("plan/plan-orb/{PLAN_ORB}")).is_some(), "{:?}", keys(&els));
    // The plan is drawn as the card (a list of work), not as an answer.
    assert!(els.iter().any(|e| e.text.as_deref() == Some("PLAN")));
}

// case: replaces the pulsing pip rather than joining it
#[test]
fn replaces_the_pulsing_pip_rather_than_joining_it() {
    let els = underway();
    assert!(at(&els, "plan-orb").is_some());
    assert!(at(&els, "is-pulsing").is_none(), "{:?}", keys(&els));
}

// case: keeps the pip when no turn is running
#[test]
fn keeps_the_pip_when_no_turn_is_running() {
    // The run has not ended (an approval or the turn's opening holds the live line back), but nothing is underway.
    let els = drawn(state(true, None));
    assert!(at(&els, "plan-orb").is_none(), "{:?}", keys(&els));
    assert!(at(&els, "plan-dot/1/is-pulsing").is_some(), "{:?}", keys(&els));
    // And once the run has ended, neither: nothing says a step is running after the fact.
    let ended = drawn(state(false, None));
    assert!(at(&ended, "plan-orb").is_none() && at(&ended, "is-pulsing").is_none(), "{:?}", keys(&ended));
}

// case: never marks a done step as underway
#[test]
fn never_marks_a_done_step_as_underway() {
    let els = underway();
    let orb = at(&els, "plan-orb").expect("the orb");
    let first = els.iter().position(|e| e.text.as_deref() == Some("Read the ledger")).unwrap();
    let second = els.iter().position(|e| e.text.as_deref() == Some("Fix the parser")).unwrap();
    assert!(orb > first && orb < second, "the orb sits on the second (running) row: {orb} vs {first}, {second}");
    assert_eq!(els.iter().filter(|e| e.key.as_deref().is_some_and(|k| k.ends_with(&format!("plan-orb/{PLAN_ORB}")))).count(), 1);
}

// case: is not told which orb to draw
#[test]
fn is_not_told_which_orb_to_draw() {
    // The same plan whatever the live line is doing: thinking or running a tool, the step's orb is the plan's own.
    let plan_part = |els: Vec<rune::ElementInfo>| -> Vec<(Option<String>, Option<String>)> {
        let from = els.iter().position(|e| e.text.as_deref() == Some("PLAN")).unwrap();
        let to = els.iter().position(|e| e.text.as_deref() == Some("Write it up")).unwrap();
        els[from..=to].iter().map(|e| (e.key.clone(), e.text.clone())).collect()
    };
    let thinking = plan_part(drawn(state(true, Some("thinking"))));
    let tool = plan_part(drawn(state(true, Some("tool"))));
    assert!(thinking.iter().any(|(k, _)| k.as_deref().is_some_and(|k| k.ends_with(&format!("plan-orb/{PLAN_ORB}")))));
    assert_eq!(thinking, tool);
    assert!(!thinking.iter().any(|(k, _)| k.as_deref().is_some_and(|k| k.contains("thinking") || k.contains("working"))));
}
