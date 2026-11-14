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
fn is_not_reported_as_a_turn_that_said_nothing() {
    let transcript = fold(vec![event("run.completed", json!({}))], true);
    assert!(!transcript.items.iter().any(|i| matches!(i, Item::Note { .. })));
}
#[test]
fn still_says_so_when_the_runtime_really_wrote_nothing_at_all() {
    let transcript = fold(vec![event("run.completed", json!({}))], false);
    assert!(transcript.items.iter().any(|i| matches!(i, Item::Note { text, .. } if text.contains("ended without a reply"))));
}
