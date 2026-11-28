use super::*;

// case: makes the external origin visible in the thread and keeps the actual ask as a message
#[test]
fn sweep_thread_marks_its_actual_external_start_without_swallowing_the_ask() {
    let mut state = tests_live_state();
    let pane = state.ledger.as_mut().unwrap();
    pane.snapshot.running = false;
    pane.snapshot.live = None;
    pane.snapshot.turn_metas[0].started_by_kind = Some("mcp".into());
    pane.snapshot.turn_metas[0].mode = Some("ask".into());
    pane.snapshot.turn_metas[0].prompt = "Review this".into();
    pane.items.push(ledger::Item::User("Review this".into()));
    let mut rt = rune::Runtime::new(state, |s, _| {
        message_column(&s.ledger.as_ref().unwrap().items, "Codec", &Palette::new(true), s)
    }, (800.0, 680.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let elements = rt.elements();
    assert!(elements.iter().any(|e| e.key.as_deref() == Some("started-from-another-app-words")
        && e.text.as_deref() == Some("STARTED FROM ANOTHER APP · ASK MODE (READ ONLY)")));
    assert!(elements.iter().any(|e| e.text.as_deref() == Some("Review this")));
    // An external follow-up does not relabel a conversation begun in Locust.
    rt.update(|s, _| {
        let turns = &mut s.ledger.as_mut().unwrap().snapshot.turn_metas;
        turns.insert(0, ledger::Metadata { mission_id: "m_old".into(), ..Default::default() });
    });
    rt.frame(0.0);
    assert!(!rt.elements().iter().any(|e| !e.exiting && e.key.as_deref() == Some("started-from-another-app-words")));
}
