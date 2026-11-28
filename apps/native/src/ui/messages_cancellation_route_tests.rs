use super::*;

// The cancellation card must use the route that actually stopped, not the sidebar's teammate defaults.
// A stopped OpenCode command can be absent from its ledger because it had not reported an end.
#[test]
fn messages_stopped_receipt_uses_the_runtime_recorded_for_this_turn() {
    for (recorded, current, wants_warning) in
        [("opencode", "codex", true), ("codex", "opencode", false)]
    {
        let mut s = tests_live_state();
        let pane = s.ledger.as_mut().unwrap();
        pane.header.runtime = Some(current.into());
        pane.snapshot.running = false;
        pane.snapshot.live = None;
        pane.snapshot.turn_metas[0].runtime = recorded.into();
        pane.snapshot.turn_metas[0].cwd = Some("/work".into());
        pane.items = vec![
            ledger::Item::User("Run the tests".into()),
            ledger::Item::Note {
                level: String::new(),
                text: "Stopped".into(),
            },
        ];
        let mut rt = rune::Runtime::new(
            s,
            |s, _| {
                message_column(
                    &s.ledger.as_ref().unwrap().items,
                    "Codec",
                    &Palette::new(true),
                    s,
                )
            },
            (900.0, 900.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.05);
        let said: Vec<String> = rt.elements().into_iter().filter_map(|el| el.text).collect();
        assert!(
            said.iter().any(|line| line == "This run was stopped"),
            "{said:?}"
        );
        assert_eq!(
            said.iter().any(
                |line| line.starts_with("An unreported OpenCode command may have been running")
            ),
            wants_warning,
            "{said:?}"
        );
        assert_eq!(
            said.iter()
                .any(|line| line == "Stopped before it used any tools, so nothing is half-done."),
            !wants_warning,
            "{said:?}"
        );
    }
}
