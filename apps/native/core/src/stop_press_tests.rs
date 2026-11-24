//! stop-press.test.ts (0.806), case by case. Expected answers are written out as Electron's test writes them.
use super::*;

const STARTING: StoppableRun<'static> = StoppableRun { phase: "starting", run_id: None };
const RUNNING: StoppableRun<'static> = StoppableRun { phase: "running", run_id: Some("run_7") };

fn cancel(run_id: &str) -> StopPress {
    StopPress::Cancel { run_id: run_id.into() }
}

fn when_named(key: &str) -> StopPress {
    StopPress::CancelWhenNamed { key: key.into() }
}

// case: cancels by name when the run has one
#[test]
fn cancels_by_name_when_the_run_has_one() {
    assert_eq!(stop_press(Some(RUNNING), Some("run_7")), cancel("run_7"));
}

// case: remembers the press when the run is still starting, instead of dropping it
#[test]
fn remembers_the_press_when_the_run_is_still_starting_instead_of_dropping_it() {
    // The whole defect in one line: this used to be `Nothing`.
    assert_eq!(stop_press(Some(STARTING), Some("pending:2")), when_named("pending:2"));
}

// case: is still remembered on a run that is starting a SECOND turn
#[test]
fn is_still_remembered_on_a_run_that_is_starting_a_second_turn() {
    assert_eq!(stop_press(Some(StoppableRun { phase: "starting", run_id: None }), Some("pending:9")), when_named("pending:9"));
}

// case: does nothing when nothing is running
#[test]
fn does_nothing_when_nothing_is_running() {
    assert_eq!(stop_press(None, Some("run_7")), StopPress::Nothing);
    for phase in ["completed", "failed", "cancelled"] {
        assert_eq!(stop_press(Some(StoppableRun { phase, run_id: None }), Some("run_7")), StopPress::Nothing, "{phase}");
    }
}

// case: does nothing when a starting run has no key to be remembered by
#[test]
fn does_nothing_when_a_starting_run_has_no_key_to_be_remembered_by() {
    assert_eq!(stop_press(Some(STARTING), None), StopPress::Nothing);
}

// case: takes a second press on a run already cancelling, because the first may not have landed
#[test]
fn takes_a_second_press_on_a_run_already_cancelling_because_the_first_may_not_have_landed() {
    assert_eq!(stop_press(Some(StoppableRun { phase: "cancelling", run_id: Some("run_7") }), Some("run_7")), cancel("run_7"));
}

// case: agrees with the phases the composer draws the button for
#[test]
fn agrees_with_the_phases_the_composer_draws_the_button_for() {
    for phase in ["starting", "running", "cancelling"] {
        assert!(is_stoppable(Some(phase)), "{phase}");
        assert_ne!(stop_press(Some(StoppableRun { phase, run_id: None }), Some("k")), StopPress::Nothing, "{phase}");
    }
    for phase in [Some("completed"), Some("failed"), Some("cancelled"), None] {
        assert!(!is_stoppable(phase), "{phase:?}");
    }
}
