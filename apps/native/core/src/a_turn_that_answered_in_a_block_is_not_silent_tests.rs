//! Electron's `a-turn-that-answered-in-a-block-is-not-silent.test.ts` (5 cases), with Electron's values and words.
//! Electron also runs some cases with `latestTurn: false`; Rust's `Transcript` has no latest-turn flag, so each is
//! tested once.
use crate::ledger::{Item, Transcript};
use serde_json::{Value, json};

fn event(sequence: u64, kind: &str, payload: Value) -> Value {
    json!({
        "id": format!("evt_{sequence}"),
        "runId": "run_1",
        "missionId": "mission_1",
        "sequence": sequence,
        "occurredAt": "2026-09-24T12:00:00.000Z",
        "sourceAdapter": "codex",
        "type": kind,
        "payload": payload,
    })
}

fn reply(sequence: u64, text: &str) -> Value {
    event(sequence, "message.delta", json!({ "itemId": "msg1", "operation": "append", "text": text, "final": true }))
}

fn thread(events: Vec<Value>) -> Transcript {
    let mut t = Transcript::default();
    t.apply(events.into_iter().map(|e| json!({ "recordType": "mission.event", "event": e })).collect());
    t
}

fn warned(t: &Transcript) -> bool {
    t.items.iter().any(|i| matches!(i, Item::Note { text, .. } if text.contains("ended without a reply")))
}

const FILE: &str = "<locust-file>\ndocs/report.md :: the rollup\n</locust-file>";
const ASK: &str = "<locust-ask>\nKeep v2, or migrate now?\n- Keep v2 :: Smaller\n- Migrate :: Bigger\n</locust-ask>";

#[test]
fn is_not_said_to_have_ended_without_a_reply_when_it_handed_over_a_file() {
    assert!(!warned(&thread(vec![reply(1, FILE), event(2, "run.completed", json!({}))])));
}

#[test]
fn is_not_said_to_have_ended_without_a_reply_when_it_asked_a_question() {
    assert!(!warned(&thread(vec![reply(1, ASK), event(2, "run.completed", json!({}))])));
}

#[test]
fn is_not_said_to_have_ended_without_a_reply_when_it_wrote_a_plan() {
    let plan = event(1, "plan.updated", json!({ "plan": ["Read the tests", "Write the fix"] }));
    assert!(!warned(&thread(vec![plan, event(2, "run.completed", json!({}))])));
}

#[test]
fn is_not_said_to_have_ended_without_a_reply_when_the_answer_was_a_summarized_conversation_0_426() {
    let compacted = event(1, "adapter.diagnostic", json!({
        "code": "claude.context_compacted",
        "level": "info",
        "terminal": false,
        "message": "Claude Code summarized the conversation so far, as asked, and carries on from the summary."
    }));
    let t = thread(vec![compacted, event(2, "run.completed", json!({}))]);
    assert!(!warned(&t));
    // What it did say is shown instead.
    assert!(t.items.iter().any(|i| matches!(i, Item::Note { text, .. } if text.contains("summarized the conversation so far, as asked"))), "{:?}", t.items);
}

#[test]
fn still_is_when_nothing_came_back_at_all() {
    assert!(warned(&thread(vec![event(1, "run.completed", json!({}))])));
    assert!(warned(&thread(vec![reply(1, "   "), event(2, "run.completed", json!({}))])));
}
