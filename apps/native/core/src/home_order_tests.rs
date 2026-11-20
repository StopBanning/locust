use super::accounts_lead;

#[test]
fn nothing_installed_puts_the_accounts_first() {
    assert!(accounts_lead(Some(&[false, false, false, false])));
}

#[test]
fn anything_installed_keeps_the_usual_order() {
    assert!(!accounts_lead(Some(&[true, false, false, false])));
    assert!(!accounts_lead(Some(&[false, false, true])));
}

#[test]
fn before_discovery_answers_home_does_not_reorder() {
    assert!(!accounts_lead(None));
}

#[test]
fn an_answer_with_no_agents_puts_no_empty_panel_first() {
    assert!(!accounts_lead(Some(&[])));
}

// Electron `freeStartStillFree` (status.ts), whose three answers the first screen's line reads.
use super::{FreeStart, free_start_still_free};

#[test]
fn a_ready_opencode_listing_a_free_model_is_still_free() {
    assert_eq!(free_start_still_free(true, ["opencode/big-pickle", "opencode/deepseek-v4.1-flash-free"]), FreeStart::Yes);
}

#[test]
fn a_ready_opencode_whose_list_has_nothing_free_is_the_only_no() {
    assert_eq!(free_start_still_free(true, ["opencode/big-pickle"]), FreeStart::No);
}

#[test]
fn not_installed_or_not_yet_read_never_weakens_the_promise() {
    assert_eq!(free_start_still_free(false, ["opencode/big-pickle"]), FreeStart::Unknown);
    assert_eq!(free_start_still_free(true, std::iter::empty()), FreeStart::Unknown);
}

// FirstLaunch.tsx `folded`: when Home's accounts fold to one line.
use super::{AgentRow, agents_fold};

fn row(installed: bool, ready: bool, checking: bool) -> AgentRow {
    AgentRow { installed, ready, checking }
}

#[test]
fn with_a_team_the_accounts_fold_once_every_installed_agent_is_ready() {
    assert!(agents_fold(&[row(true, true, false), row(false, false, false)], 2, false));
    assert!(!agents_fold(&[row(true, true, false), row(true, false, false)], 2, false), "an installed one not ready holds it open");
}

#[test]
fn with_a_team_one_still_being_checked_does_not_hold_the_fold_open() {
    assert!(agents_fold(&[row(true, true, false), row(true, false, true)], 2, false));
}

#[test]
fn before_a_team_every_agent_must_be_ready_and_nothing_still_checking() {
    assert!(!agents_fold(&[row(true, true, false), row(false, false, false)], 0, false), "one missing keeps the list");
    assert!(!agents_fold(&[row(true, true, false), row(true, false, true)], 0, false));
    assert!(agents_fold(&[row(true, true, false), row(true, true, false)], 0, false));
}

#[test]
fn nothing_ready_never_folds_and_show_all_opens_it() {
    assert!(!agents_fold(&[row(true, false, false)], 2, false));
    // A team whose agents are all missing or still being checked: nothing is ready, so nothing folds.
    assert!(!agents_fold(&[row(false, false, false), row(true, false, true)], 2, false));
    assert!(!agents_fold(&[row(true, true, false)], 2, true));
}
