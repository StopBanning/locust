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
