//! Electron's `a-kept-setup-is-not-proof-its-edit-helped.test.ts` (2 cases): the judge of a tune is told that the kept
//! setup is a preference, not proof, and that no lesson is a normal outcome.
use super::*;

fn prompt() -> String {
    let column = |slot, label: &str, instructions: &str, why: Option<&str>, answer: &str| TuneLessonColumnEntry {
        slot,
        label: label.into(),
        instructions: Some(instructions.into()),
        why: why.map(Into::into),
        skill: None,
        answer: answer.into(),
        changes: None,
    };
    tune_lesson_prompt(
        "Explain the cause.",
        CompareSlotId::B,
        &[
            column(CompareSlotId::A, "Current", "Be concise.", None, "A."),
            column(CompareSlotId::B, "How it works", "Check your work.", Some("It stopped before checking."), "B."),
            column(CompareSlotId::C, "What it knows", "Name your assumptions.", Some("It guessed."), "C."),
        ],
        &[],
    )
}

// case: judges each edit from the runs, never from which setup the person kept
#[test]
fn judges_each_edit_from_the_runs_never_from_which_setup_the_person_kept() {
    let p = prompt();
    assert!(p.contains("says which answer they preferred, not that its edit caused the difference"), "{p}");
    assert!(p.contains("point to the moment in a run"));
    assert!(!p.contains("use \"helped\" when its run followed"));
}

// case: writes a lesson only when it would change work on a different ask, and no lesson is a normal outcome
#[test]
fn writes_a_lesson_only_when_it_would_change_work_on_a_different_ask() {
    let p = prompt();
    assert!(p.contains("would change how this teammate should work on a different ask"), "{p}");
    assert!(p.contains("which is a normal outcome"));
    assert!(!p.contains("When any edit row is \"helped\" or \"hurt\", write one lesson"));
}
