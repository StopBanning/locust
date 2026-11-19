//! Electron's `routine-recovery.test.tsx`, the cases Rust builds, with Electron's values and words, on
//! `routines_panel.rs`'s `recovery_card` drawn headless and `routines::schedule::summary` (`routineScheduleSummary`).
//! Rust's card draws its decisions only once the review box is ticked, where Electron draws them disabled until then;
//! so a decision Electron shows disabled is checked here as not drawn before the tick, and drawn after it.
use super::*;
use rune::{Env, Runtime};
use serde_json::{Value, json};

fn routine() -> Value {
    let route = json!({"runtime": "codex", "model": "account-default", "mode": "ask"});
    json!({
        "routineId": "rt_one", "name": "Release", "teammateId": "tm_one", "route": route,
        "steps": ["Open PR", "Summarise", "Report"], "learnedFrom": [], "createdAt": "2026-09-01T00:00:00.000Z", "runs": 0,
        "schedule": {"kind": "every", "hours": 6},
        "execution": {"attemptId": "attempt_one", "status": "held", "step": 2, "of": 3, "workspaceId": "ws_test",
            "startedAt": "2026-09-04T00:00:00.000Z", "updatedAt": "2026-09-04T00:00:00.000Z",
            "steps": ["Open PR", "Summarise", "Report"], "route": route,
            "missionId": "mission_two", "runId": "run_two", "reason": "The dispatch outcome is uncertain.", "canContinue": false}
    })
}
fn with_execution(change: Value) -> Value {
    let mut r = routine();
    for (k, v) in change.as_object().unwrap() {
        r["execution"][k] = v.clone();
    }
    r
}

struct Harness {
    state: State,
    routine: Value,
}
fn view(h: &Harness, _: &mut Cx<Harness>) -> El {
    recovery_card(&h.state, &h.routine, col().w_full(), &Palette::new(true))
}
/// The card's words and its presses, before the review box is ticked or after.
fn draw(routine: Value, reviewed: bool) -> (String, Vec<String>) {
    let mut state = State::default();
    if reviewed {
        state.routines.reviewed = routine["execution"]["attemptId"].as_str().map(str::to_owned);
    }
    let mut rt = Runtime::new(Harness { state, routine }, view, (900.0, 900.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let all = rt.elements();
    let words = all.iter().filter_map(|n| n.text.clone()).collect::<Vec<_>>().join("\n");
    let presses = all.iter().filter(|n| n.role == "button").filter_map(|n| n.label.clone().or_else(|| n.text.clone())).collect();
    (words, presses)
}

/// In half: the box, the two sentences and the route; Rust does not list the routine's steps ("Open PR").
#[test]
fn review_acknowledgement_is_unchecked_and_both_decisions_wait_for_it_even_for_a_confirmed_step() {
    let confirmed = with_execution(json!({"canContinue": true}));
    let (said, _) = draw(confirmed.clone(), false);
    assert!(said.contains("□ I reviewed the saved conversation and external work"), "{said}");
    assert!(!said.contains("✓ I reviewed"));
    assert!(!said.contains("Continue remaining steps") && !said.contains("Abandon attempt and remove schedule"));
    assert!(said.contains("Neither stops a runtime or undoes work"));
    assert!(said.contains("starts from step 1"));
    assert!(said.contains("codex"));
    let (ticked, _) = draw(confirmed, true);
    assert!(ticked.contains("Continue remaining steps") && ticked.contains("Abandon attempt and remove schedule"));
}

/// In half: the card's words; Rust's card offers the saved conversation whenever there is one (Electron's case
/// passes no way to open it). `routineRunSummary`'s "last attempt abandoned" is not built.
#[test]
fn abandonment_stays_visible_after_acknowledgement_instead_of_reverting_to_never_ran() {
    let abandoned = with_execution(json!({"status": "abandoned", "reason": "Abandoned by you; schedule removed."}));
    let (said, presses) = draw(abandoned, false);
    assert!(said.contains("Attempt abandoned"), "{said}");
    assert!(said.contains("Step 2 of 3"));
    assert_eq!(presses, ["Open the saved conversation"]);
}

#[test]
fn says_the_reason_once_and_opens_the_saved_conversation_rather_than_printing_its_id() {
    let interrupted = with_execution(json!({"reason": "Review required: that run was interrupted. Nothing will be replayed."}));
    let (said, presses) = draw(interrupted, false);
    assert!(said.contains("That run was interrupted. Nothing will be replayed."), "{said}");
    assert!(!said.contains("Review required"));
    assert!(presses.iter().any(|p| p == "Open the saved conversation"));
    assert!(!said.contains("mission_two") && !said.contains("Saved mission:"));
}

#[test]
fn names_when_keep_runs_it_next_six_hours_from_the_decision_not_the_attempt() {
    let before = chrono::Local::now();
    let (said, _) = draw(routine(), false);
    let after = chrono::Local::now();
    let when = |t: chrono::DateTime<chrono::Local>| format!("its next run starts from step 1 at {}, not straight away", (t + chrono::Duration::hours(6)).format("%a %-I:%M %p"));
    assert!(said.contains(&when(before)) || said.contains(&when(after)), "{said}");
}

#[test]
fn says_why_continue_is_off_and_offers_to_keep_the_schedule() {
    let (said, _) = draw(routine(), false);
    assert!(said.contains("Continue waits for step 2 to be confirmed finished. It was not, so step 3 would build on work that may not be there."), "{said}");
    assert!(said.contains("its next run starts from step 1 at ") && said.contains(", not straight away"));
    assert!(!said.contains("Keep the schedule"), "a decision waits for the review box");
    assert!(draw(routine(), true).0.contains("Keep the schedule"));
    let last = with_execution(json!({"step": 3}));
    let (said, _) = draw(last.clone(), true);
    assert!(said.contains("This was the last step, so there is nothing left to continue."));
    assert!(!said.contains("Continue remaining steps"));
    assert!(said.contains("Keep the schedule"));
    let mut unscheduled = last;
    unscheduled.as_object_mut().unwrap().remove("schedule");
    assert!(draw(unscheduled, true).0.contains("Keep the routine"));
}

#[test]
fn says_a_refused_start_would_be_refused_again_not_that_the_step_before_it_never_finished() {
    let too_long = "Step 2 is too long to send: 439 characters too long. Shorten it in Edit, then run the routine again. Nothing was started.";
    let refused = with_execution(json!({"step": 1, "reason": too_long, "settledAtDispatch": true}));
    let (said, _) = draw(refused, false);
    assert!(said.contains(too_long), "{said}");
    assert!(said.contains("Continue would be refused the same way, so it is off until that changes."));
    assert!(!said.contains("to be confirmed finished"));
}

/// In half: the schedule's words. Electron's card also says "Waiting for your review" for a dispatch not yet
/// resolved; Rust's says "Running · step 2 of 3" (a difference, in the notes for Colin).
#[test]
fn an_unresolved_dispatch_promises_no_next_run() {
    let dispatching = with_execution(json!({"status": "dispatching"}));
    let now = chrono::DateTime::parse_from_rfc3339("2026-09-08T00:00:00Z").unwrap().with_timezone(&chrono::Local);
    assert!(locust_core::routines::schedule::summary(&dispatching, now).unwrap().contains("held for review; no automatic retry"));
}
