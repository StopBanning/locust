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

// case: says the reason once, and opens the saved conversation rather than printing its id
#[test]
fn says_the_reason_once_and_opens_the_saved_conversation_rather_than_printing_its_id() {
    let interrupted = with_execution(json!({"reason": "Review required: that run was interrupted. Nothing will be replayed."}));
    let (said, presses) = draw(interrupted, false);
    assert!(said.contains("That run was interrupted. Nothing will be replayed."), "{said}");
    assert!(!said.contains("Review required"));
    assert!(presses.iter().any(|p| p == "Open the saved conversation"));
    assert!(!said.contains("mission_two") && !said.contains("Saved mission:"));
}

// case: names when Keep runs it next -- six hours from the decision, not the attempt (0.402 beta retest)
#[test]
fn names_when_keep_runs_it_next_six_hours_from_the_decision_not_the_attempt() {
    let before = chrono::Local::now();
    let (said, _) = draw(routine(), false);
    let after = chrono::Local::now();
    let when = |t: chrono::DateTime<chrono::Local>| format!("its next run starts from step 1 at {}, not straight away", (t + chrono::Duration::hours(6)).format("%a %-I:%M %p"));
    assert!(said.contains(&when(before)) || said.contains(&when(after)), "{said}");
}

// case: says why Continue is off, and offers to keep the schedule (0.390 beta pass)
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

// case: says a refused start would be refused again, not that the step before it never finished (0.818)
#[test]
fn says_a_refused_start_would_be_refused_again_not_that_the_step_before_it_never_finished() {
    let too_long = "Step 2 is too long to send: 439 characters too long. Shorten it in Edit, then run the routine again. Nothing was started.";
    let refused = with_execution(json!({"step": 1, "reason": too_long, "settledAtDispatch": true}));
    let (said, _) = draw(refused, false);
    assert!(said.contains(too_long), "{said}");
    assert!(said.contains("Continue would be refused the same way, so it is off until that changes."));
    assert!(!said.contains("to be confirmed finished"));
}

/// In half: this checks the native shelf's schedule and Run protection. Electron also draws the
/// recovery review itself for an unresolved dispatch; native still shows its Running heading.
#[test]
fn an_unresolved_dispatch_promises_no_next_run() {
    let dispatching = with_execution(json!({"status": "dispatching"}));
    let now = chrono::DateTime::parse_from_rfc3339("2026-09-08T00:00:00Z").unwrap().with_timezone(&chrono::Local);
    assert!(locust_core::routines::schedule::summary(&dispatching, now).unwrap().contains("held for review; no automatic retry"));
    let rt = recovery_shelf(dispatching);
    let all = rt.elements();
    let when = all.iter().find(|el| el.key.as_deref() == Some("routine-when-rt_one")).expect("the real shelf shows the schedule");
    let when = when.text.as_deref().unwrap();
    assert!(when.contains("held for review; no automatic retry"), "{when}");
    assert!(!when.contains("due now") && !when.contains("next run"), "{when}");
    let run = all.iter().find(|el| el.key.as_deref() == Some("routine-run-rt_one")).expect("the real shelf draws Run");
    assert!(run.opacity < 0.5, "Run is off for an unresolved dispatch");
    assert!(run.help.is_none(), "Run is held by the attempt, not by a missing teammate");
}

/// The full native Routines surface, rather than a fixture that calls recovery_card directly.
fn recovery_shelf(held: Value) -> Runtime<State> {
    let mut state = State::default();
    crate::scenes::team_scene(&mut state);
    state.teammates[0].id = "tm_one".to_owned();
    state.routines.open = true;
    state.routines.file = json!({"schemaVersion": 1, "routines": [held]});
    let mut rt = Runtime::new(state, crate::view, (1000.0, 1800.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn has_ancestor(elements: &[rune::ElementInfo], at: usize, ancestor: usize) -> bool {
    let mut parent = elements[at].parent;
    while let Some(index) = parent {
        if index == ancestor {
            return true;
        }
        parent = elements[index].parent;
    }
    false
}

// case: is a card of its own, never the row meta the Routines screen crushes into one clipped cell (2026-09-27)
#[test]
fn recovery_words_and_decisions_are_on_the_card_below_its_compact_metadata() {
    let mut rt = recovery_shelf(routine());
    let (x, y) = rt.find("routine-reviewed-rt_one").expect("the real shelf draws the review acknowledgement");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
    rt.settle_all();
    rt.frame(0.0);
    assert_eq!(rt.state.routines.reviewed.as_deref(), Some("attempt_one"));

    let all = rt.elements();
    let keyed = |key: &str| all.iter().position(|el| !el.exiting && el.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("missing {key}"));
    let card = keyed("routine-rt_one");
    let meta = all[keyed("routine-chain-rt_one")].parent.expect("the compact summary lives in its own row");
    let regular_actions = keyed("routine-actions-rt_one");
    let heading = all.iter().position(|el| !el.exiting && el.text.as_deref().is_some_and(|t| t.starts_with("Waiting for your review · Step 2 of 3")))
        .expect("recovery heading");
    let reason = all.iter().position(|el| !el.exiting && el.text.as_deref() == Some("The dispatch outcome is uncertain."))
        .expect("recovery reason");
    let review = keyed("routine-reviewed-rt_one");
    let keep = keyed("routine-keep-schedule-rt_one");
    let abandon = keyed("routine-abandon-rt_one");
    let bottom = |at: usize| all[at].rect[1] + all[at].rect[3];
    for at in [heading, reason, review, keep, abandon] {
        let el = &all[at];
        assert!(has_ancestor(&all, at, card), "recovery stays in its own routine's card: {el:?}");
        assert!(!has_ancestor(&all, at, meta), "recovery must not be placed in the compact metadata row: {el:?}");
        assert!(!has_ancestor(&all, at, regular_actions), "recovery decisions are not the row's normal actions: {el:?}");
        assert!(el.rect[1] >= bottom(regular_actions) - 0.5, "recovery belongs below the normal row actions: {el:?}");
        assert!(el.visible[2] >= el.rect[2] - 0.5 && el.visible[3] >= el.rect[3] - 0.5, "recovery words and controls must not be clipped: {el:?}");
    }
    assert_eq!(all[keep].parent, all[abandon].parent, "the recovery decisions share their own action row");
}
