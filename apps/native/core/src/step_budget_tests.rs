// Carries apps/desktop/src/shared/step-budget.test.ts (Electron 4ce2f37a): the warning on a routine step too long to send.
use crate::hand_off::{STEP_BUDGET, step_too_long_notice};

#[test]
fn a_step_that_fits_gets_no_warning() {
    assert_eq!(step_too_long_notice("Summarise yesterday."), None);
    assert_eq!(step_too_long_notice(&"x".repeat(STEP_BUDGET)), None);
}

#[test]
fn a_step_is_warned_about_the_moment_the_budget_is_passed() {
    assert!(step_too_long_notice(&"x".repeat(STEP_BUDGET + 1)).is_some());
}

#[test]
fn a_too_long_step_is_told_how_much_to_cut() {
    let notice = step_too_long_notice(&"x".repeat(STEP_BUDGET + 1_400)).unwrap();
    assert!(notice.contains("1400"), "{notice}");
    assert!(notice.contains("too long"), "{notice}");
}

#[test]
fn a_step_one_over_says_one_character() {
    assert!(step_too_long_notice(&"x".repeat(STEP_BUDGET + 1)).unwrap().contains("1 character too long"));
}

#[test]
fn the_step_budget_leaves_room_inside_the_prompt_cap() {
    const { assert!(STEP_BUDGET < 12_000) };
    const { assert!(STEP_BUDGET < 20_000) };
}
