//! Electron spawning-is-not-working.test.ts: the word on a run that has not been answered yet. Rust keeps the one fact
//! (`spoken`) as the line's register, `starting` or `working`, so the word and the quiet sentence cannot disagree.
use super::live_line;
use serde_json::{json, Value};

const AT: &str = "2026-09-19T10:00:00.000Z";

fn event(kind: &str) -> Value {
    json!({"id": format!("e-{kind}"), "runId": "r", "missionId": "m", "sequence": 1, "occurredAt": AT,
        "sourceAdapter": "cursor", "type": kind, "payload": {}})
}

fn line(events: &[Value]) -> super::LiveLine {
    live_line(events, true, AT, false).expect("a running turn draws its live line")
}

// case: says Starting while the app has only recorded launching it
#[test]
fn says_starting_while_only_the_launch_is_recorded() {
    let l = line(&[event("run.started")]);
    assert_eq!(l.label, "Starting");
    assert_eq!(l.register, "starting");
}

// case: says Starting before even that
#[test]
fn says_starting_before_anything_is_recorded() {
    assert_eq!(line(&[]).label, "Starting");
}

// case: says Working once the runtime itself has said something
#[test]
fn says_working_once_the_runtime_has_spoken() {
    let l = line(&[event("run.started"), event("step.completed")]);
    assert_eq!(l.label, "Working");
    assert_eq!(l.register, "working");
}

#[test]
fn a_diagnostic_alone_is_not_the_runtime_speaking() {
    let l = line(&[event("run.started"), event("adapter.diagnostic")]);
    assert_eq!(l.label, "Starting");
}

// case: draws its word and its quiet sentence from one fact
#[test]
fn draws_its_word_and_register_from_one_fact() {
    for events in [vec![], vec![event("run.started")], vec![event("run.started"), event("step.completed")]] {
        let l = line(&events);
        assert_eq!(l.label == "Working", l.register == "working");
    }
}
