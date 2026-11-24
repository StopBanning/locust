use super::*;

fn thread(running: bool) -> rune::Runtime<State> {
    let mut state = crate::messages::tests_live_state();
    let pane = state.ledger.as_mut().unwrap();
    if !running {
        pane.snapshot.running = false;
        pane.snapshot.live = None;
    }
    let step = |text: &str, state: &str| ledger::PlanStep {
        text: text.into(),
        state: state.into(),
    };
    pane.items = vec![
        ledger::Item::User("Fix the parser".into()),
        ledger::Item::Plan {
            id: "orb-plan".into(),
            steps: vec![
                step("Read the ledger", "done"),
                step("Fix the parser", "running"),
            ],
            finished: !running,
            stopped: false,
        },
        ledger::Item::Tool {
            name: "shell".into(),
            status: ledger::Status::Done,
            summary: "cat ledger.rs".into(),
            detail: Box::default(),
        },
    ];
    let mut rt = rune::Runtime::new(
        state,
        |s, _| {
            crate::messages::message_column(
                &s.ledger.as_ref().unwrap().items,
                "Codec",
                &Palette::new(true),
                s,
            )
        },
        (800.0, 900.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt.frame(0.016);
    rt
}

// case: gives the plan card one big orb and one small one
#[test]
fn a_running_plan_draws_both_orbs_at_their_own_sizes() {
    let rt = thread(true);
    let elements = rt.elements();
    let plan = elements
        .iter()
        .position(|e| e.text.as_deref() == Some("PLAN"))
        .unwrap();
    let head = elements
        .iter()
        .position(|e| e.key.as_deref() == Some("orb-plan/plan-head-orb"))
        .unwrap();
    assert!(head < plan);
    assert_eq!(elements[head].trace_name, Some("thinking-orb"));
    assert_eq!(&elements[head].rect[2..], &[24.0, 24.0]);
    let orb = elements
        .iter()
        .find(|e| e.key.as_deref() == Some("orb-plan/plan-orb/listening"))
        .unwrap();
    assert_eq!(orb.trace_name, Some("thinking-orb"));
    assert_eq!(&orb.rect[2..], &[20.0, 20.0]);
    assert_eq!(
        elements
            .iter()
            .filter(|e| e.trace_name == Some("thinking-orb")
                && e.key
                    .as_deref()
                    .is_some_and(|key| key.starts_with("orb-plan/")))
            .count(),
        2
    );
}

// case: keeps the sidebar row off the orbs: it wears the spark, a glyph, which no scale can smudge (0.392)
#[test]
fn a_running_sidebar_row_shimmers_with_an_empty_marker_space() {
    let mut s = State::default();
    s.recent.push(ConversationRow {
        id: "mission_running".into(),
        title: "Fix the parser".into(),
        teammate: String::new(),
        started_by: String::new(),
        state: "running".into(),
        updated: "2026-11-21T09:00:00.000Z".into(),
    });
    let mut rt = rune::Runtime::new(
        s,
        |s, _| crate::sidebar::sidebar(s, &Palette::new(true)),
        (280.0, 800.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    assert!(crate::sidebar::shimmering(&rt.state));
    let elements = rt.elements();
    let footprint = elements
        .iter()
        .find(|e| e.key.as_deref() == Some("conversation-working-mission_running"))
        .unwrap();
    assert_eq!(&footprint.rect[2..], &[6.0, 6.0]);
    assert_eq!(footprint.fill.a, 0.0);
    assert!(
        elements
            .iter()
            .all(|e| e.trace_name != Some("thinking-orb"))
    );
}

#[test]
fn a_finished_plan_has_no_orb_or_repaint_deadline() {
    let mut rt = thread(false);
    assert!(
        rt.elements()
            .iter()
            .any(|e| e.text.as_deref() == Some("PLAN"))
    );
    assert!(
        rt.elements()
            .iter()
            .all(|e| e.trace_name != Some("thinking-orb"))
    );
    assert!(!crate::messages::live_sweeping(&rt.state));
    assert!(!rt.frame(0.2));
    assert_eq!(rt.wake_deadline(), None);
}
