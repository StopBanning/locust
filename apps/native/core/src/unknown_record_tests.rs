//! Electron's `an-unknown-record-stays-out-of-the-thread.test.ts` (3 cases), with Electron's values and words.
use crate::ledger::{Item, Transcript};
use serde_json::{Value, json};

fn event(sequence: u64, kind: &str, payload: Value) -> Value {
    json!({
        "id": format!("e-{sequence}"),
        "runId": "r",
        "missionId": "m",
        "sequence": sequence,
        "occurredAt": "2026-09-22T22:40:00.000Z",
        "sourceAdapter": "claude",
        "type": kind,
        "payload": payload,
    })
}

fn run(diagnostic: Value) -> Transcript {
    let records = vec![
        event(1, "run.started", json!({ "evidence": { "raw": {} } })),
        event(2, "tool.started", json!({ "itemId": "toolu_1", "name": "Bash" })),
        diagnostic,
        event(4, "tool.completed", json!({ "itemId": "toolu_1", "name": "Bash", "output": "ok" })),
    ]
    .into_iter()
    .map(|e| json!({ "recordType": "mission.event", "event": e }))
    .collect();
    let mut t = Transcript::default();
    t.apply(records);
    t
}

#[test]
fn is_never_a_row_of_its_own_even_after_the_work_has_begun() {
    let t = run(event(
        3,
        "adapter.diagnostic",
        json!({
            "code": "claude.unknown_event",
            "level": "info",
            "terminal": false,
            "message": "Unhandled Claude record: tool_progress"
        }),
    ));
    assert!(!t.items.iter().any(|i| matches!(i, Item::Note { .. })));
}

#[test]
fn is_still_said_in_the_fold_with_the_turns_other_remarks() {
    let t = run(event(
        3,
        "adapter.diagnostic",
        json!({
            "code": "codex.unknown_event",
            "level": "info",
            "terminal": false,
            "message": "Unhandled Codex record: something_new"
        }),
    ));
    assert!(
        t.signal_rail
            .iter()
            .any(|row| row.name.contains("Unhandled Codex record: something_new"))
    );
}

#[test]
fn leaves_real_trouble_where_it_was_a_runtime_error_after_work_began_is_still_a_row() {
    let t = run(event(
        3,
        "adapter.diagnostic",
        json!({
            "code": "claude.runtime_error",
            "level": "error",
            "terminal": false,
            "message": "API Error: overloaded"
        }),
    ));
    let notes: Vec<_> = t
        .items
        .iter()
        .filter(|i| matches!(i, Item::Note { .. }))
        .collect();
    assert_eq!(notes.len(), 1);
}
