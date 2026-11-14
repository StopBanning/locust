//! The next missionView sections in file order, through the window's ledger projection.
use crate::ledger::{Item, Transcript};
use serde_json::{Value, json};

fn event(kind: &str, payload: Value) -> Value {
    json!({"type":kind,"payload":payload,"occurredAt":"2026-08-31T16:00:00.000Z"})
}
fn fold(events: Vec<Value>, spoke_to_peers: bool) -> Transcript {
    let mut records: Vec<_> = events.into_iter().enumerate().map(|(i, mut e)| {
        e["sequence"] = json!(i + 1);
        json!({"recordType":"mission.event","event":e})
    }).collect();
    if spoke_to_peers {
        records.push(json!({"recordType":"mission.peer","link":{
            "direction":"posted","messageId":"share","peerTeammateId":"peer"
        }}));
    }
    let mut transcript = Transcript::default();
    transcript.apply(records);
    transcript
}
#[test]
fn takes_the_target_from_the_completion_when_the_start_had_none() {
    let transcript = fold(vec![
        event("tool.started", json!({"itemId":"t1","toolKind":"tool_use","name":"Read","phase":"started"})),
        event("tool.completed", json!({"itemId":"t1","toolKind":"tool_use","name":"Read","command":"src/cli.js","phase":"completed"})),
    ], false);
    let Item::Tool { name, summary, .. } = &transcript.items[0] else { panic!("tool") };
    assert_eq!(summary, "src/cli.js");
    assert_eq!(name, "Read");
}
#[test]
fn leaves_a_row_the_start_already_named_alone() {
    let transcript = fold(vec![
        event("tool.started", json!({"itemId":"t1","toolKind":"command_execution","name":"shell","command":"npm test","phase":"started"})),
        event("tool.completed", json!({"itemId":"t1","toolKind":"command_execution","name":"shell","command":"something else","phase":"completed"})),
    ], false);
    let Item::Tool { summary, .. } = &transcript.items[0] else { panic!("tool") };
    assert_eq!(summary, "npm test");
}
