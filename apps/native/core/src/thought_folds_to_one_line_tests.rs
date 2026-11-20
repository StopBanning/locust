//! `thought-folds-to-one-line.test.ts`, in the ledger: a thought is one line -- "Thought for 12s" -- with the words
//! folded under it, and it carries how long it took from the step that opened it (Colin, 2026-09-17).
use crate::ledger::{Item, Transcript};
use serde_json::{Value, json};

fn ev(seq: u64, kind: &str, payload: Value, seconds: i64) -> Value {
    json!({"id": format!("e{seq}"), "runId": "run_1", "missionId": "mission_1", "sequence": seq,
        "occurredAt": chrono::DateTime::from_timestamp_millis(1_700_000_000_000 + seconds * 1000).unwrap().to_rfc3339(),
        "sourceAdapter": "cursor", "type": kind, "payload": payload})
}

fn transcript(events: &[Value]) -> Transcript {
    let mut t = Transcript::default();
    t.apply(events.iter().map(|e| json!({"recordType": "mission.event", "event": e})).collect());
    t
}

#[test]
fn carries_how_long_it_took_from_the_step_that_opened_it() {
    let events = vec![
        ev(1, "run.started", json!({"runtimeThreadId": "t"}), 0),
        ev(2, "step.started", json!({"stepKind": "reasoning", "itemId": "r1"}), 1),
        ev(
            3,
            "step.completed",
            json!({"stepKind": "reasoning", "itemId": "r1", "message": "Two reads, then answer."}),
            13,
        ),
        ev(4, "run.completed", json!({"runtimeThreadId": "t", "process": {}}), 14),
    ];
    let t = transcript(&events);
    let thought = t
        .items
        .iter()
        .find(|i| matches!(i, Item::Thought { .. }))
        .expect("a reasoning step makes a thought");
    let Item::Thought { text, duration_ms, .. } = thought else { unreachable!() };
    assert_eq!(*duration_ms, Some(12_000));
    assert_eq!(text, "Two reads, then answer.");
}
