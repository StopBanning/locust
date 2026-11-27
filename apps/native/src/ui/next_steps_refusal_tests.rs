use super::*;
use rune::{Env, Runtime};

// Supplemental to the named Electron cases: the single-flight window must not lose cards instead of Start all.
#[test]
fn next_steps_refuse_without_losing_the_suggestion_or_the_persons_draft() {
    let mut s = State::default();
    crate::scenes::ledger_scene(&mut s, false, false);
    s.demo = false;
    s.ledger.as_mut().unwrap().items = vec![ledger::Item::Assistant {
        item_id: "reply-next".into(),
        r#final: true,
        text: "Done.\n\n<locust-next>\nClean the Linux cache too :: Do the same for the AppImage cache.\nTell people in What's new :: Add a line to the next changelog.\n</locust-next>".into(),
    }];
    s.draft.replace_text("My unfinished draft.");
    s.route_can_run = false;
    let mut rt = Runtime::new(s, crate::view, (1250., 1200.), Env::new(true, 1.));
    for _ in 0..15 { rt.frame(0.05); }
    let press = |rt: &mut Runtime<State>, prefix: &str| {
        let key = rt.elements().iter().find_map(|e| e.key.as_ref().filter(|k| k.starts_with(prefix) && k.ends_with("-0"))).expect(prefix).clone();
        let (x, y) = rt.find(&key).expect(&key);
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.05);
    };
    assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some("2 suggested tasks")));
    assert!(!rt.elements().iter().any(|e| e.text.as_deref() == Some("Start all")), "no batch action until every task can start");
    press(&mut rt, "next-step-here-");
    assert_eq!(rt.state.draft.text, "My unfinished draft.", "Do it here must keep the draft when refused");
    assert!(rt.state.next_steps_gone.is_empty() && rt.state.flight.is_none());

    rt.update(|s, _| {
        s.route_can_run = true;
        s.ledger.as_mut().unwrap().snapshot.running = true;
    });
    rt.frame(0.05);
    press(&mut rt, "next-step-start-");
    assert_eq!(rt.state.draft.text, "My unfinished draft.", "Start while busy leaves the draft alone");
    assert!(rt.state.ledger.is_some() && rt.state.next_steps_gone.is_empty() && rt.state.flight.is_none());

    press(&mut rt, "next-step-away-");
    assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some("1 suggested task")));
    assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some("Tell people in What's new")));
}
