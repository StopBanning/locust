//! Electron's `renderer/src/a-saved-turn-is-said-after-its-work.test.ts` (0.439): the host commits a turn on the
//! teammate's branch once the run has ended and records "Saved this turn on locust/wren as ..."; that line is the
//! turn's last, after its work, its answer and its closing line, never under the person's ask above the work.
use super::*;
use serde_json::json;

fn event(sequence: u64, kind: &str, payload: Value) -> Value {
    let mut payload = payload;
    payload["evidence"] = json!({ "redacted": true });
    json!({ "recordType": "mission.event", "event": { "id": format!("e{sequence}"), "runId": "r", "missionId": "m",
        "sequence": sequence, "occurredAt": format!("2026-09-28T10:00:{:02}.000Z", 10 + sequence),
        "sourceAdapter": "opencode", "type": kind, "payload": payload } })
}

fn saved(sequence: u64, message: &str) -> Value {
    event(sequence, "adapter.diagnostic", json!({ "level": "info", "code": crate::worktrees::CHECKPOINT_CODE, "message": message, "terminal": false }))
}

fn fold(records: Vec<Value>) -> Transcript {
    let mut t = Transcript::default();
    t.apply(records);
    t
}

// case: is the last line of its turn, after the work and the answer
#[test]
fn is_the_last_line_of_its_turn_after_the_work_and_the_answer() {
    let said = "Saved this turn on locust/wren as 4b744293dfad: 1 file (notes.md).";
    let t = fold(vec![
        event(1, "run.started", json!({ "runtimeThreadId": "t" })),
        event(2, "tool.started", json!({ "itemId": "w1", "toolKind": "file_change", "name": "write", "phase": "started" })),
        event(3, "tool.completed", json!({ "itemId": "w1", "toolKind": "file_change", "name": "write", "phase": "completed", "path": "notes.md" })),
        event(4, "message.delta", json!({ "itemId": "a1", "operation": "replace", "text": "Created notes.md.", "final": true })),
        event(5, "run.completed", json!({})),
        saved(6, said),
    ]);
    assert!(matches!(t.items.last(), Some(Item::Note { text, .. }) if text == said), "{:?}", t.items);
    let answer = t.items.iter().position(|i| matches!(i, Item::Assistant { .. })).expect("the answer");
    let trace = t.items.iter().position(|i| matches!(i, Item::Trace(_))).expect("the closing line");
    assert!(answer < t.items.len() - 1 && trace < t.items.len() - 1);
}

// case: is still said when the turn drew no work of its own, never folded away
#[test]
fn is_still_said_when_the_turn_drew_no_work_of_its_own_never_folded_away() {
    let said = "Saved this turn on locust/wren as 0123456789ab: 1 file (a.py).";
    let t = fold(vec![
        event(1, "run.started", json!({ "runtimeThreadId": "t" })),
        event(2, "message.delta", json!({ "itemId": "a1", "operation": "replace", "text": "Done.", "final": true })),
        event(3, "run.completed", json!({})),
        saved(4, said),
    ]);
    assert!(matches!(t.items.last(), Some(Item::Note { text, .. }) if text == said), "{:?}", t.items);
}
