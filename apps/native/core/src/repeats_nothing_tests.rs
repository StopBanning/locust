//! run-it-again-repeats-nothing.test.ts (8 cases): whether a failed run may simply be run again (`runtime_never_started`,
//! H4), and a long live run keeping its first event past the cap. Rust's cap is the ledger's event window, which keeps
//! the opening quarter and the tail, so the start is never what goes.
use super::runtime_never_started;
use crate::ledger::{Transcript, EVENT_WINDOW};
use serde_json::{json, Value};

fn event(kind: &str) -> Value {
    json!({"id": "e", "runId": "r", "missionId": "m", "sequence": 1, "occurredAt": "2026-09-14T06:42:00.000Z",
        "sourceAdapter": "cursor", "type": kind, "payload": {}})
}

fn never(kinds: &[&str]) -> bool {
    runtime_never_started(&kinds.iter().map(|k| event(k)).collect::<Vec<_>>())
}

// case: says yes when the runtime never started
#[test]
fn says_yes_when_the_runtime_never_started() {
    assert!(never(&["run.failed"]));
    assert!(never(&[]));
}

// case: says no the moment the runtime started, however early it then failed
#[test]
fn says_no_the_moment_the_runtime_started() {
    assert!(!never(&["run.started", "run.failed"]));
}

// case: says no for a run that reached a tool
#[test]
fn says_no_for_a_run_that_reached_a_tool() {
    assert!(!never(&["run.started", "tool.started", "run.failed"]));
}

// case: takes any event from the runtime, even one it does not know, as a start
#[test]
fn takes_an_unknown_event_as_a_start() {
    assert!(!never(&["runtime.unknown_event", "run.failed"]));
}

// case: H4: a run that reached a tool without ever saying run.started DID start (OpenCode, Muse, Antigravity)
#[test]
fn a_tool_without_run_started_did_start() {
    assert!(!never(&["step.started", "message.delta", "tool.started", "tool.completed", "run.failed"]));
    assert!(!never(&["message.delta", "run.failed"]));
}

// case: is still true for a run that never reached its runtime: its failure and the host’s notes
#[test]
fn the_failure_and_the_hosts_notes_alone_never_started() {
    assert!(never(&["adapter.diagnostic", "run.failed"]));
    assert!(never(&["run.failed"]));
}

// case: counts a runtime thread on any event as a start
#[test]
fn a_runtime_thread_on_any_event_is_a_start() {
    let mut failed = event("run.failed");
    failed["runtimeThreadId"] = json!("thread_1");
    assert!(!runtime_never_started(&[failed]));
}

fn record(sequence: usize, kind: &str, payload: Value) -> Value {
    json!({"recordType": "mission.event", "event": {"type": kind, "sequence": sequence,
        "occurredAt": "2026-09-14T06:42:00.000Z", "payload": payload}})
}

// case: H4: keeps its first event past the cap, so a failed turn still reads as started
#[test]
fn a_long_run_keeps_its_first_event_past_the_cap() {
    let mut rows = vec![record(1, "run.started", json!({}))];
    for i in 0..EVENT_WINDOW + 200 {
        rows.push(record(i + 2, "message.delta", json!({"itemId": format!("m{i}"), "operation": "append", "text": "x"})));
    }
    rows.push(record(EVENT_WINDOW + 202, "run.failed", json!({})));
    let mut t = Transcript::default();
    t.apply(rows);
    let kept: Vec<&Value> = t.events_in_view().collect();
    assert_eq!(kept.len(), EVENT_WINDOW);
    assert_eq!(kept[0]["type"], "run.started");
    assert_eq!(kept.last().unwrap()["type"], "run.failed");
    assert!(!runtime_never_started(t.events_in_view()), "the failed turn still reads as started");

    let mut short = Transcript::default();
    short.apply((1..=10).map(|i| record(i, "message.delta", json!({"itemId": format!("s{i}"), "operation": "append", "text": "y"}))).collect());
    assert_eq!(short.events_in_view().count(), 10);
}
