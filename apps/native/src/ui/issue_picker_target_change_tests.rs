use super::*;
use rune::{Env, Runtime};

#[test]
fn issue_picker_closes_when_switching_to_another_conversation() {
    let mut s = State::default();
    crate::scenes::ledger_scene(&mut s, false, false);
    s.demo = true;
    s.ledger.as_mut().unwrap().header.folder = Some("/tmp/hearth".into());
    s.ledger.as_mut().unwrap().conversation.conversation_id = Some("conv-a".into());
    let first_target = target(&s).expect("conversation A target");
    s.issues.open = true;
    s.issues.picker_target = Some(first_target);
    s.issues.issues = vec![ListedIssue {
        number: 5,
        title: "Crashes on start".into(),
        url: "https://github.com/acme/hearth/issues/5".into(),
        author: Some("dana".into()),
        created_at: "2026-10-12T09:00:00Z".into(),
        labels: vec!["bug".into()],
    }];

    let mut rt = Runtime::new(s, |s, _| view(s, &Palette::new(true)).unwrap_or_else(el), (600., 400.), Env::new(true, 1.));
    rt.frame(0.05);
    assert!(rt.find("issue-5").is_some(), "the issue list is open on conversation A");

    // Switching to another conversation must close the old conversation's issue picker on poll.
    rt.update(|s, _| {
        s.ledger.as_mut().unwrap().conversation.conversation_id = Some("conv-b".into());
        assert!(poll(s), "poll reports that the stale issue picker closed");
    });
    for _ in 0..4 {
        rt.frame(0.05);
    }
    assert!(!rt.state.issues.open, "the previous conversation's issue picker closed");
    assert!(rt.find("issue-5").is_none(), "the closed picker is no longer drawn");
}
