//! `thinking-is-not-using-a-tool.test.ts` (QA-2026-09-29 round 2, R31): Codex opens an ITEM for its reasoning, and
//! every item that was not a message read "Using a tool...", so a run only thinking said so from its first second to
//! its last.
use crate::thread_view::live_line;
use serde_json::{Value, json};

fn event(n: usize, kind: &str, payload: Value) -> Value {
    json!({"id": format!("e{n}"), "runId": "r", "missionId": "m", "sequence": n,
        "occurredAt": "2026-09-29T10:00:00.000Z", "sourceAdapter": "codex", "type": kind, "payload": payload})
}

fn register(events: &[Value]) -> String {
    live_line(events, true, "", false)
        .expect("a running turn has a live line")
        .register
}

#[test]
fn reads_thinking_for_a_reasoning_item() {
    assert_eq!(
        register(&[event(
            1,
            "step.started",
            json!({"stepKind": "item", "itemId": "rs_1", "itemType": "reasoning"}),
        )]),
        "thinking"
    );
}

#[test]
fn still_reads_writing_for_a_message_item_and_a_tool_for_other_work() {
    assert_eq!(
        register(&[event(
            1,
            "step.started",
            json!({"stepKind": "item", "itemId": "msg_1", "itemType": "agentMessage"}),
        )]),
        "writing"
    );
    assert_eq!(
        register(&[event(
            1,
            "step.started",
            json!({"stepKind": "item", "itemId": "ws_1", "itemType": "webSearch"}),
        )]),
        "tool"
    );
}
