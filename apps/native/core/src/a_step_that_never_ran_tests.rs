//! a-step-that-never-ran-is-not-still-to-come.test.ts (5 cases): a finished turn keeps its plan, with the step it did
//! not reach not marked done; the card is told the run ended, and that it stopped short only when it failed or was
//! cancelled.
use crate::ledger::{Item, PlanStep, Transcript};
use serde_json::{Value, json};

fn event(sequence: u64, kind: &str, payload: Value) -> Value {
    json!({"id": format!("run_1:{sequence}"), "runId": "run_1", "missionId": "mission_1", "sequence": sequence,
        "occurredAt": "2026-09-22T06:00:00.000Z", "sourceAdapter": "codex", "type": kind, "payload": payload})
}

fn plan(statuses: [&str; 2], final_: bool, sequence: u64) -> Value {
    event(sequence, "plan.updated", json!({"itemId": "plan", "final": final_, "plan": [
        {"step": "Read the config", "status": statuses[0]}, {"step": "Rewrite the failing test", "status": statuses[1]}]}))
}

fn turn(ending: Option<&str>) -> Vec<Value> {
    let mut events = vec![
        event(1, "run.started", json!({})),
        plan(["completed", "pending"], false, 2),
        event(3, "message.delta", json!({"itemId": "msg", "operation": "replace", "text": "I read the config.", "final": true})),
    ];
    if let Some(kind) = ending {
        events.push(event(4, kind, json!({})));
    }
    events
}

/// The thread's plan item: its steps, whether the card is told the run ended, and whether it stopped short.
fn plan_item(events: Vec<Value>) -> Option<(Vec<PlanStep>, bool, bool)> {
    let mut t = Transcript::default();
    t.apply(events.into_iter().map(|e| json!({"recordType": "mission.event", "event": e})).collect());
    t.items.into_iter().find_map(|i| match i {
        Item::Plan { steps, finished, stopped, .. } => Some((steps, finished, stopped)),
        _ => None,
    })
}

fn done(steps: &[PlanStep]) -> usize {
    steps.iter().filter(|s| s.state == "done").count()
}

// case: still shows the plan, with the step it did not reach
#[test]
fn a_finished_turn_still_shows_its_plan_with_the_step_it_did_not_reach() {
    let (steps, _, _) = plan_item(turn(Some("run.completed"))).expect("the plan vanished from a finished turn");
    assert_eq!(steps.len(), 2);
    assert_eq!(done(&steps), 1);
    assert_ne!(steps[1].state, "done");
}

// case: tells the card the run has ended
#[test]
fn tells_the_card_the_run_has_ended() {
    assert!(plan_item(turn(Some("run.completed"))).unwrap().1);
}

// case: says nothing of the sort while the run is still going
#[test]
fn says_nothing_of_the_sort_while_the_run_is_going() {
    let (_, finished, stopped) = plan_item(turn(None)).expect("a live plan");
    assert!(!finished && !stopped);
}

// case: says the run stopped short only when it failed or was stopped (0.368)
#[test]
fn says_the_run_stopped_short_only_when_it_failed_or_was_cancelled() {
    assert!(!plan_item(turn(Some("run.completed"))).unwrap().2);
    assert!(plan_item(turn(Some("run.failed"))).unwrap().2);
    assert!(plan_item(turn(Some("run.cancelled"))).unwrap().2);
}

// case: has nothing to report when the runtime closed its own plan
#[test]
fn has_nothing_to_report_when_the_runtime_closed_its_own_plan() {
    let tidy = vec![event(1, "run.started", json!({})), plan(["completed", "completed"], true, 2), event(3, "run.completed", json!({}))];
    let (steps, _, _) = plan_item(tidy).unwrap();
    assert_eq!(done(&steps), steps.len());
}
