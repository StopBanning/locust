//! Electron `main/a-refusal-in-ask-says-the-mode.test.ts` (0.822): in Ask and Plan, OpenCode's server asks about
//! anything that acts and Locust answers no with nobody shown a card. The answer names the mode, never "declined".
use super::*;

#[test]
fn a_refusal_in_ask_or_plan_names_the_mode_and_says_nobody_was_asked() {
    assert_eq!(
        open_code_read_only_refusal(Mode::Ask),
        "Locust refused this without asking anyone: this teammate is in Ask mode, which reads and changes nothing, so nothing was run."
    );
    assert!(open_code_read_only_refusal(Mode::Plan).contains("Plan mode, which reads and plans"));
    for mode in [Mode::Ask, Mode::Plan] {
        assert!(!open_code_read_only_refusal(mode).to_lowercase().contains("declined"));
    }
}

#[test]
fn a_served_ask_or_plan_run_is_given_the_modes_refusal_and_only_when_nobody_answers() {
    // Electron: `handler === undefined && effectiveSandbox === 'read-only'`. Ask and Plan are the read-only modes.
    assert_eq!(rejection_message(Mode::Ask, false), open_code_read_only_refusal(Mode::Ask));
    assert_eq!(rejection_message(Mode::Plan, false), open_code_read_only_refusal(Mode::Plan));
    // A person was asked: their decline, never the mode's.
    assert_eq!(rejection_message(Mode::ApproveEach, true), "The person declined this in Locust.");
    // Not read-only and nobody asked (Approve each with no window, Edit files): the plain unasked refusal.
    assert_eq!(rejection_message(Mode::ApproveEach, false), OPENCODE_REFUSED_UNASKED);
    assert_eq!(rejection_message(Mode::AcceptEdits, false), OPENCODE_REFUSED_UNASKED);
    assert_eq!(
        OPENCODE_REFUSED_UNASKED,
        "Locust refused this without asking anyone: this teammate's mode does not allow it, so nothing was run."
    );
}
