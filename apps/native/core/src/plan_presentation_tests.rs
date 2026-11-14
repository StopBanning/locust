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
fn steps() -> Value {
    json!({"plan":[{"step":"Read the file","status":"completed"},{"step":"Edit it","status":"in_progress"}]})
}
#[test]
fn stands_on_its_own_even_when_the_run_also_did_something() {
    let transcript = fold(vec![
        event("plan.updated", steps()),
        event("tool.started", json!({"itemId":"i1","name":"edit","command":"src/notes.ts"})),
        event("tool.completed", json!({"itemId":"i1","name":"edit","command":"src/notes.ts","status":"ok"})),
        event("run.completed", json!({})),
    ], false);
    let plan = transcript.items.iter().position(|i| matches!(i, Item::Plan { .. })).expect("the plan is its own item, not a passenger on the fold");
    let work = transcript.items.iter().position(|i| matches!(i, Item::Tool { .. })).expect("the fold still carries the work");
    assert!(plan < work);
    let Item::Plan { steps, .. } = &transcript.items[plan] else { panic!("plan") };
    assert_eq!(steps.len(), 2);
    assert_eq!(steps.iter().filter(|s| s.state == "done").count(), 1);
}
#[test]
fn keeps_its_own_item_when_the_run_did_nothing_else() {
    let transcript = fold(vec![event("plan.updated", steps()), event("run.completed", json!({}))], false);
    assert!(transcript.items.iter().any(|i| matches!(i, Item::Plan { .. })));
    assert!(!transcript.items.iter().any(|i| matches!(i, Item::Tool { .. } | Item::Thought { .. })));
}
