//! The prompt follower reads approval routing from older and newer app replies.
use super::*;

#[test]
fn omitted_answer_in_locust_still_routes_the_approval_to_the_app() {
    let client = LocustClient::for_test(Arc::new(|method, _| match method {
        "cli/prompt" => Ok(json!({})),
        "cli/poll" => Ok(json!({
            "turn": {"phase":"completed"}, "events": [], "cursor": 0, "approvals": [
                {"approvalId":"old-card","kind":"command","summary":"Read the folder"},
                {"approvalId":"terminal-card","kind":"command","summary":"Run the check","answerInLocust":false}
            ]
        })),
        other => panic!("unexpected RPC {other}"),
    }));
    let session = CliSession {
        session_id: "fixture session".into(), key: "fixture key".into(), workspace: "fixture folder".into(),
        teammate: CliTeammate {
            id: "tm_1".into(), name: "Repo".into(), runtime: Some("codex".into()),
            model: Some("account-default".into()), runs_in: "ask".into(),
        },
    };
    let mut routes = Vec::new();
    let ended = client.prompt(&session, "Look here", None, &mut |_| Ok(()), Some(&mut |cards, _| {
        routes.extend(cards.iter().map(|card| (card.approval_id.clone(), card.answer_in_locust)));
    }), Duration::ZERO);
    assert_eq!(ended, Ok(Ended::EndTurn));
    assert_eq!(routes, [("old-card".into(), true), ("terminal-card".into(), false)]);
}
