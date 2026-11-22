//! Electron's `renderer/src/one-clock-for-a-run.test.ts`: a run's duration is its first event to its last
//! (`activity_trace::run_span_ms`, missionView's `runSpanMs`), and the turn's closing line says that same number.
//! Yurt's beta report (2026-09-23, #14): the fold said 37s and the Missions row 43s for one mission, the row timing
//! from the mission's creation to its last write.
use super::*;
use serde_json::json;

fn at(seconds: u32, kind: &str) -> Value {
    json!({ "id": format!("e{seconds}"), "runId": "r", "sequence": seconds,
        "occurredAt": format!("2026-09-23T12:00:{seconds:02}.000Z"), "sourceAdapter": "claude", "type": kind, "payload": {} })
}

// case: is its first event to its last
#[test]
fn is_its_first_event_to_its_last() {
    assert_eq!(run_span_ms(&[at(3, "run.started"), at(20, "tool.started"), at(40, "run.completed")]), Some(37_000));
    // Read off the earliest and the latest, wherever they sit in the record.
    assert_eq!(run_span_ms(&[at(20, "tool.started"), at(3, "run.started"), at(40, "run.completed")]), Some(37_000));
}

// case: is unknown with fewer than two events
#[test]
fn is_unknown_with_fewer_than_two_events() {
    assert_eq!(run_span_ms(&[at(3, "run.started")]), None);
    assert_eq!(run_span_ms(&[]), None);
    // An event whose time cannot be read is not one of the two.
    assert_eq!(run_span_ms(&[at(3, "run.started"), json!({ "type": "run.completed", "occurredAt": "later" })]), None);
}

// case: is what the fold line says
#[test]
fn is_what_the_fold_line_says() {
    let events = [at(3, "run.started"), at(40, "run.completed")];
    let trace = activity_trace(&[], &events, outcome(&events, false));
    assert_eq!(trace.segments[0].text, "37s");
}
