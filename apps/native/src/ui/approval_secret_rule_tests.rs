use super::*;
use rune::{Env, Runtime};

fn card(command: &str) -> Runtime<State> {
    let mut s = State::default();
    s.pending_approval = Some(approval::Request {
        approval_id: "approval_rule_fixture".into(),
        mission_id: "approval_rule_fixture_mission".into(),
        runtime: "codex".into(),
        kind: approval::Kind::Command,
        summary: "Run a command".into(),
        detail: command.into(),
        cwd: Some("/work/app".into()),
        requested_at: "2026-11-08T00:00:00.000Z".into(),
        always_covers: None,
        always_key: None,
        data_sent_says: None,
        reversible_says: None,
        patch: None,
        skippable: false, answer_in: None, claude_input: None,
        questions: Vec::new(),
    });
    native::approvals_channel().register_teammate("approval_rule_fixture_mission", "tm_codec");
    let mut rt = Runtime::new(s, |s, _| approval_card(s, &Palette::new(true)), (1200., 1200.), Env::new(true, 1.));
    super::approval_active_thread_tests::settle(&mut rt);
    rt
}

// case: is offered no "always allow" rule on its card
#[test]
fn approval_a_command_with_a_key_cannot_save_a_rule() {
    let _engine = locust_core::native::hooks::no_saved_rules();
    let key = ["sk", "ant", "api03", "Zq9xR2mT7vLp4WcN8bYd3KfH6sJ1gQe5"].join("-");
    for command in [format!("curl -H \"x-api-key: {key}\" https://api.example.com"), format!("ANTHROPIC_API_KEY={key} npm test")] {
        let mut rt = card(&command);
        assert!(rt.find("approve-once").is_some(), "a one-time answer is still offered");
        assert!(rt.find("dont-ask-again").is_none(), "a command carrying a key must not offer a saved allow");
        let (x, y) = rt.find("deny").unwrap();
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        super::approval_active_thread_tests::settle(&mut rt);
        assert!(!rt.elements().iter().any(|e| !e.exiting && e.text.as_deref() == Some("Never allow this (saves a rule)")),
            "a command carrying a key must not offer a saved deny");
    }
}

// case: still offers one for an ordinary command (the control)
#[test]
fn approval_an_ordinary_command_still_offers_an_exact_rule() {
    let _engine = locust_core::native::hooks::no_saved_rules();
    let rt = card("npm test");
    assert!(rt.find("dont-ask-again").is_some(), "npm test still offers an exact saved rule");
}
