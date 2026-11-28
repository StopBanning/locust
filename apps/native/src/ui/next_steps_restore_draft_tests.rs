use super::*;
use rune::{Env, Runtime};

#[test]
fn next_steps_do_it_here_restores_draft_when_send_refuses_without_folder_question() {
    let mut s = State::default();
    let wren = Teammate {
        id: "tm_wren".into(),
        name: "Wren".into(),
        role: "Engineer".into(),
        runtime: Some("claude".into()),
        model: None,
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: None,
    };
    s.teammates = vec![wren];
    s.conversations.insert(
        "tm_wren".into(),
        Conversation {
            conversation_id: Some("conv-1".into()),
            turns: vec![],
        },
    );
    s.selected = 0;
    s.demo = false;
    s.route_can_run = true;
    // With no ledger pane and a load_error, may_start(s) is true so the handler calls submit_composer,
    // which refuses via !can_send(s) without opening a folder confirmation question.
    s.load_error = Some("The engine is not ready.".into());
    s.draft.replace_text("My unfinished draft.");
    assert!(may_start(&s), "precondition: Do it here must reach submit_composer");

    let mut rt = Runtime::new(
        s,
        |s, _| {
            let steps = [NextStep {
                title: "Clean the Linux cache too".into(),
                brief: "Do the same for the AppImage cache.".into(),
            }];
            view(s, &steps, "scope-restore", true, &Palette::new(true)).unwrap()
        },
        (700., 300.),
        Env::new(true, 1.),
    );
    for _ in 0..4 {
        rt.frame(0.05);
    }

    let (x, y) = rt.find("next-step-here-scope-restore-0").expect("Do it here button");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);

    assert!(rt.state.flight.is_none());
    assert!(rt.state.folder_question.is_none());
    assert!(rt.state.next_steps_gone.is_empty(), "a refused task stays on screen");
    assert_eq!(
        rt.state.draft.text, "My unfinished draft.",
        "Do it here restores the person's draft when the send refuses without a folder question"
    );
}
