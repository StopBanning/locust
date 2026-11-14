//! The next missionView sections in file order, through the window's ledger projection.
use crate::ledger::{Item, Transcript, read_plan};
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
fn opencode_plan() -> Value {
    json!([
        {"id":"1","content":"create a.txt containing alpha","status":"completed"},
        {"id":"2","content":"create b.txt containing beta","status":"in_progress"},
        {"id":"3","content":"create c.txt listing the two file names","status":"pending"}
    ])
}
#[test]
fn reads_the_steps_their_words_and_their_states() {
    let steps = read_plan(&opencode_plan());
    assert_eq!(steps.iter().map(|s| s.text.as_str()).collect::<Vec<_>>(), [
        "create a.txt containing alpha", "create b.txt containing beta",
        "create c.txt listing the two file names"
    ]);
    assert_eq!(steps.iter().map(|s| s.state.as_str()).collect::<Vec<_>>(), ["done", "running", "pending"]);
}
#[test]
fn and_the_thread_counts_how_far_through_it_is_in_the_open() {
    let transcript = fold(vec![
        event("run.started", json!({})),
        event("tool.started", json!({"itemId":"t1","name":"write"})),
        event("plan.updated", json!({"plan":opencode_plan()})),
    ], false);
    let steps = transcript.items.iter().find_map(|i| match i {
        Item::Plan { steps, .. } => Some(steps), _ => None,
    }).expect("the thread keeps a separate plan");
    assert_eq!(steps.len(), 3);
    assert_eq!(steps.iter().filter(|s| s.state == "done").count(), 1);
}
