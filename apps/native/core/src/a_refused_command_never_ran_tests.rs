//! `a-refused-command-never-ran.test.ts` (a replay of a Bash-heavy Haiku run, 2026-09-23): Claude Code refused two
//! loops before they ran, and the fold line read "ran 7 commands · 2 exited non-zero · 1 refused" -- five ran, two
//! never did, and nothing exited anything. A refused call is not a failure: nothing ran.
use crate::activity_trace::{activity_trace, outcome};
use crate::command_outcomes::is_shell;
use crate::ledger::{Item, Transcript};
use serde_json::{Value, json};

fn ev(seq: u64, kind: &str, payload: Value) -> Value {
    json!({"id": format!("run:{seq}"), "runId": "run", "sequence": seq,
        "occurredAt": chrono::DateTime::from_timestamp_millis(1_790_164_800_000 + (seq as i64) * 1000).unwrap().to_rfc3339(),
        "sourceAdapter": "claude", "type": kind, "payload": payload})
}

fn events() -> Vec<Value> {
    vec![
        ev(1, "run.started", json!({})),
        ev(
            2,
            "tool.started",
            json!({"itemId": "a", "toolKind": "tool_use", "name": "Bash", "command": "for i in 1 2 3; do echo $i; done", "phase": "started"}),
        ),
        ev(
            3,
            "tool.failed",
            json!({"itemId": "a", "toolKind": "tool_use", "name": "Bash", "command": "for i in 1 2 3; do echo $i; done", "phase": "completed", "status": "refused", "output": "Contains simple_expansion"}),
        ),
        ev(
            4,
            "tool.started",
            json!({"itemId": "b", "toolKind": "tool_use", "name": "Bash", "command": "wc -l data/log.txt", "phase": "started"}),
        ),
        ev(
            5,
            "tool.completed",
            json!({"itemId": "b", "toolKind": "tool_use", "name": "Bash", "command": "wc -l data/log.txt", "phase": "completed", "exitCode": 0}),
        ),
        ev(6, "run.completed", json!({})),
    ]
}

fn transcript(events: &[Value]) -> Transcript {
    let mut t = Transcript::default();
    t.apply(events.iter().map(|e| json!({"recordType": "mission.event", "event": e})).collect());
    t
}

#[test]
fn is_counted_as_refused_not_as_run_or_as_exiting_non_zero() {
    let events = events();
    let t = transcript(&events);
    let line = activity_trace(&t.items, &events, outcome(&events, false)).text();
    assert!(line.contains("ran wc -l data/log.txt"), "{line}");
    assert!(line.contains("1 refused"), "{line}");
    assert!(!line.contains("non-zero"), "{line}");
    assert!(!line.contains("ran 2"), "{line}");
}

#[test]
fn keeps_its_row_with_the_reason_the_runtime_gave() {
    let t = transcript(&events());
    let shells: Vec<&Item> = t
        .items
        .iter()
        .filter(|i| matches!(i, Item::Tool { name, .. } if is_shell(name)))
        .collect();
    assert_eq!(shells.len(), 2, "both shell rows stay");
    let Item::Tool { detail, .. } = shells[0] else { unreachable!() };
    assert_eq!(detail.runtime_status, "refused");
    assert_eq!(detail.output, "Contains simple_expansion");
    let Item::Tool { detail, .. } = shells[1] else { unreachable!() };
    assert!(detail.runtime_status.is_empty(), "the second was not refused: {}", detail.runtime_status);
    assert!(detail.output.is_empty());
}
