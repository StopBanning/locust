//! Electron's `a-cancelled-run-spent-nothing.test.ts` (all 5 cases), with Electron's values and words.
use crate::benchmark::{cost_line, cost_total, mission_cost_tail};
use crate::relay::mission_cost_from_events;
use serde_json::{Value, json};

fn turn(input_tokens: u64, output_tokens: u64) -> Vec<Value> {
    vec![json!({
        "id": "e",
        "runId": "r",
        "missionId": "m",
        "sequence": 1,
        "type": "run.completed",
        "occurredAt": "2026-09-14T06:00:00.000Z",
        "sourceAdapter": "opencode",
        "payload": {
            "usage": { "inputTokens": input_tokens, "outputTokens": output_tokens },
            "evidence": { "redacted": true }
        }
    })]
}

#[test]
fn says_nothing_at_all_about_a_run_that_reported_nothing() {
    assert_eq!(cost_line(mission_cost_from_events(&[]).as_ref()), None);
}

#[test]
fn never_puts_a_conversation_scoped_fact_on_the_mission_strip() {
    let this_run = mission_cost_from_events(&turn(500, 20));
    let tail = cost_line(this_run.as_ref()).unwrap();
    assert!(!tail.contains("conversation"));
    assert!(tail.contains("500"));
    assert!(!tail.contains("2.3k"));
}

#[test]
fn shows_this_run_when_this_run_reported_one() {
    let this_run = mission_cost_from_events(&turn(500, 20));
    let tail = cost_line(this_run.as_ref()).unwrap();
    assert!(tail.contains("500"));
}

#[test]
fn still_offers_the_conversation_total_on_the_conversation_scoped_surface() {
    let earlier = mission_cost_from_events(&turn(1800, 189));
    let current = mission_cost_from_events(&turn(500, 20));
    let whole = cost_total(&[earlier, current]).expect("conversation total");
    assert!(whole.line.contains("2.3k"), "got {}", whole.line);
}

#[test]
fn marks_a_live_run_as_still_going() {
    let this_run = mission_cost_from_events(&turn(500, 20));
    assert!(mission_cost_tail(this_run.as_ref(), true).contains("so far"));
}
