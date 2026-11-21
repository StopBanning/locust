//! Electron's `main/attention-reader.test.ts` ("what a run ending says to a person who stepped away"), one Rust test
//! for each of its four cases, with its events and words. `said` is its harness: what the reader asked attention to
//! say, the teammate named as the window names the turn's owner, `-` for nobody.
use super::*;
use serde_json::{Value, json};

fn reply(text: &str) -> Value {
    json!({"type": "message.delta", "sourceAdapter": "claude", "payload": {"itemId": "m1", "operation": "append", "text": text, "final": true}})
}

fn limit(message: &str) -> Value {
    json!({"type": "route.limit_detected", "sourceAdapter": "claude", "payload": {"kind": "quota-exhausted", "message": message}})
}

fn completed() -> Value {
    json!({"type": "run.completed", "sourceAdapter": "claude", "payload": {}})
}

fn said(phase: &str, events: &[Value], owner: Option<&str>) -> Vec<String> {
    match run_ended(events, phase == "completed") {
        None => Vec::new(),
        Some(RunEnded::Decision { question }) => {
            // And the notification it becomes says who asks, or "A teammate".
            assert!(decision_text(owner, &question, None).0.starts_with(owner.unwrap_or("A teammate")));
            vec![format!("decision:{}:{question}", owner.unwrap_or("-"))]
        }
        Some(RunEnded::Limit { runtime, message }) => {
            vec![format!("limit:{}:{runtime}:{}", owner.unwrap_or("-"), message.as_deref().unwrap_or("-"))]
        }
    }
}

const ASK: &str = "I can do this two ways.\n<locust-ask>\nWhich database?\n- Postgres :: what we run today\n- SQLite :: no server\n</locust-ask>";

// case: a decision card in a completed reply is a question from the teammate
#[test]
fn a_decision_card_in_a_completed_reply_is_a_question_from_the_teammate() {
    assert_eq!(said("completed", &[reply(ASK), completed()], Some("Wren")), ["decision:Wren:Which database?"]);
    // The reply is rebuilt from its appends.
    let (head, tail) = ASK.split_at(40);
    let split = [
        json!({"type": "message.delta", "payload": {"itemId": "m1", "operation": "append", "text": head, "final": false}}),
        json!({"type": "message.delta", "payload": {"itemId": "m1", "operation": "append", "text": tail, "final": true}}),
        completed(),
    ];
    assert_eq!(said("completed", &split, Some("Wren")), ["decision:Wren:Which database?"]);
    // What the window then shows, and the turn a click opens; the finish is still said beside it.
    assert_eq!(
        ended_notice("mission_1", &[reply(ASK), completed()], "claude", Some("Wren"), || "Storage".into()),
        Some(Notice {
            title: "Wren is asking you something · Storage".into(),
            body: "Which database? · the run waits for your answer.".into(),
            opens: Some("mission_1".into()),
            limit: false,
        })
    );
}

// case: a run stopped at its account limit says so, with the runtime and its own words
#[test]
fn a_run_stopped_at_its_account_limit_says_so_with_the_runtime_and_its_own_words() {
    assert_eq!(
        said("interrupted", &[limit("You have hit your weekly limit.")], Some("Wren")),
        ["limit:Wren:claude:You have hit your weekly limit."]
    );
    // Only a quota running out: a provider turning requests away for a while is not a limit to pick past.
    let brief = json!({"type": "route.limit_detected", "sourceAdapter": "claude", "payload": {"kind": "temporary-rate-limit", "message": "slow down"}});
    assert_eq!(said("interrupted", &[brief.clone()], Some("Wren")), Vec::<String>::new());
    // What the window then shows, said instead of the finish.
    assert_eq!(
        ended_notice("mission_1", &[limit("You have hit your weekly limit.")], "codex", Some("Wren"), || "Storage".into()),
        Some(Notice {
            title: "Wren hit the Claude Code limit".into(),
            body: "You have hit your weekly limit. · pick where it continues.".into(),
            opens: Some("mission_1".into()),
            limit: true,
        })
    );
    assert_eq!(ended_notice("mission_1", &[brief], "claude", Some("Wren"), || "Storage".into()), None);
    // A run that failed at its quota with no limit event of its own is a limit of the turn's own agent.
    let failed = json!({"type": "run.failed", "payload": {"kind": "quota-exhausted", "message": "Out of credits."}});
    let notice = ended_notice("mission_1", &[failed], "codex", None, || "Storage".into()).unwrap();
    assert_eq!((notice.title.as_str(), notice.body.as_str(), notice.limit), ("A teammate hit the Codex CLI limit", "Out of credits. · pick where it continues.", true));
    let other = json!({"type": "run.failed", "payload": {"kind": "auth", "message": "Signed out."}});
    assert_eq!(ended_notice("mission_1", &[other], "codex", None, || "Storage".into()), None);
}

// case: a limit the run went on to complete past is not a limit; a plain completion says nothing
#[test]
fn a_limit_the_run_went_on_to_complete_past_is_not_a_limit_a_plain_completion_says_nothing() {
    assert_eq!(said("completed", &[limit("rate"), completed(), reply("Done.")], Some("Wren")), Vec::<String>::new());
    assert_eq!(said("completed", &[reply("All done, nothing to ask."), completed()], Some("Wren")), Vec::<String>::new());
    assert_eq!(ended_notice("mission_1", &[limit("rate"), completed(), reply("Done.")], "claude", Some("Wren"), || "Storage".into()), None);
}

// case: a decision in a run that did not complete is not asked; nobody's conversation is unnamed
#[test]
fn a_decision_in_a_run_that_did_not_complete_is_not_asked_nobodys_conversation_is_unnamed() {
    assert_eq!(said("failed", &[reply(ASK)], Some("Wren")), Vec::<String>::new());
    assert_eq!(said("completed", &[reply(ASK), completed()], None), ["decision:-:Which database?"]);
    let failed = json!({"type": "run.failed", "payload": {"message": "Codex exited"}});
    assert_eq!(ended_notice("mission_1", &[reply(ASK), failed], "claude", Some("Wren"), || "Storage".into()), None);
    // Nor one cut off before it ended, or stopped.
    assert_eq!(ended_notice("mission_1", &[reply(ASK)], "claude", Some("Wren"), || "Storage".into()), None);
    let cancelled = json!({"type": "run.cancelled", "payload": {}});
    assert_eq!(ended_notice("mission_1", &[reply(ASK), cancelled], "claude", Some("Wren"), || "Storage".into()), None);
    assert_eq!(ended_notice("mission_1", &[reply(ASK), completed()], "claude", None, || "Storage".into()).unwrap().title, "A teammate is asking you something · Storage");
}
