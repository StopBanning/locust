//! Electron's `a-finished-message-stops-typing.test.ts` (cases 1-3), with Electron's values and words.
use crate::ledger::{Item, Transcript};
use crate::thread_view::streaming;
use serde_json::{Value, json};

fn event(sequence: u64, kind: &str, payload: Value) -> Value {
    json!({
        "id": format!("e{sequence}"),
        "runId": "r",
        "missionId": "m",
        "sequence": sequence,
        "occurredAt": format!("2026-10-03T20:00:{sequence:02}.000Z"),
        "sourceAdapter": "antigravity",
        "type": kind,
        "payload": payload,
    })
}

fn said(sequence: u64, item_id: &str, text: &str) -> Value {
    event(
        sequence,
        "message.delta",
        json!({ "itemId": item_id, "operation": "append", "text": text, "final": false }),
    )
}

fn tool(sequence: u64, item_id: &str, name: &str, command: &str) -> Value {
    event(
        sequence,
        "tool.started",
        json!({ "itemId": item_id, "toolKind": name, "name": name, "command": command, "phase": "started" }),
    )
}

fn streaming_of(events: Vec<Value>) -> Vec<(String, bool)> {
    let records = events
        .into_iter()
        .map(|e| json!({ "recordType": "mission.event", "event": e }))
        .collect();
    let mut t = Transcript::default();
    t.apply(records);
    t.items
        .iter()
        .filter_map(|item| match item {
            Item::Assistant { text, r#final, .. } => {
                Some((text.trim().to_owned(), streaming(*r#final, true)))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn stops_typing_once_a_call_starts_after_it() {
    let events = vec![
        event(1, "run.started", json!({})),
        said(2, "msg_76", "The matrix run is in progress."),
        tool(3, "tool_77", "run_command", "node _tools/matrix.mjs"),
    ];
    assert_eq!(
        streaming_of(events),
        vec![("The matrix run is in progress.".into(), false)]
    );
}

#[test]
fn stops_typing_once_another_message_begins_and_the_newest_still_types() {
    let events = vec![
        event(1, "run.started", json!({})),
        said(2, "msg_76", "The matrix run is in progress."),
        said(3, "msg_84", "Meanwhile I will view the frames."),
    ];
    assert_eq!(
        streaming_of(events),
        vec![
            ("The matrix run is in progress.".into(), false),
            ("Meanwhile I will view the frames.".into(), true),
        ]
    );
}

#[test]
fn types_again_if_its_own_text_resumes_after_a_call() {
    let events = vec![
        event(1, "run.started", json!({})),
        said(2, "msg_1", "Reading."),
        tool(3, "tool_2", "view_file", "a.txt"),
        said(4, "msg_1", " Still reading."),
    ];
    assert_eq!(
        streaming_of(events),
        vec![("Reading. Still reading.".into(), true)]
    );
}
