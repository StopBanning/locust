//! Electron's `an-earlier-turn-that-only-messaged-is-not-silent.test.tsx` (2 cases), with Electron's values and words.
use crate::ledger::{Item, Transcript};
use serde_json::{Value, json};

fn event(sequence: u64, kind: &str, payload: Value) -> Value {
    json!({
        "id": format!("run:{sequence}"),
        "runId": "run",
        "sequence": sequence,
        "occurredAt": format!("2026-09-24T16:0{sequence}:00.000Z"),
        "sourceAdapter": "claude",
        "type": kind,
        "payload": payload,
    })
}

fn fold_earlier_turn(with_peer_link: bool) -> Transcript {
    let mut records = vec![
        json!({ "recordType": "mission.event", "event": event(1, "run.started", json!({ "runtimeThreadId": "s1" })) }),
        json!({ "recordType": "mission.event", "event": event(2, "message.delta", json!({
            "itemId": "m1",
            "operation": "replace",
            "text": "<locust-share to=\"Booty\">The build is green.</locust-share>",
            "final": true
        })) }),
        json!({ "recordType": "mission.event", "event": event(3, "run.completed", json!({})) }),
    ];
    if with_peer_link {
        records.push(json!({
            "recordType": "mission.peer",
            "link": {
                "direction": "posted",
                "messageId": "wm_1",
                "peerTeammateId": "tm_booty",
                "occurredAt": "2026-09-24T16:02:00.000Z"
            }
        }));
    }
    let mut t = Transcript::default();
    t.apply(records);
    t
}

#[test]
fn is_not_said_to_have_ended_without_a_reply() {
    let earlier = fold_earlier_turn(true);
    assert!(!earlier.items.iter().any(|i| matches!(i, Item::Note { text, .. } if text.contains("ended without a reply"))));
}

#[test]
fn still_is_when_it_really_wrote_nothing_and_messaged_nobody() {
    let earlier = fold_earlier_turn(false);
    assert!(earlier.items.iter().any(|i| matches!(i, Item::Note { text, .. } if text.contains("ended without a reply"))));
}
