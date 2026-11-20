//! judges-see-the-check.test.tsx, the core of "your check, in a comparison that edits" (0.810, 0.811): a check's
//! outcome in a word, the same failure in every copy, and the line over the columns that says what the judges read.
use super::*;

fn check(outcome: JudgedCheckOutcome, lines: &[&str]) -> JudgedCheck {
    JudgedCheck { outcome, lines: lines.iter().map(|l| (*l).to_owned()).collect(), unchanged: false }
}

// case: reads each outcome in a word
#[test]
fn reads_each_outcome_in_a_word() {
    assert_eq!(check_word(Some(&check(JudgedCheckOutcome::Passed, &[]))), "passed");
    assert_eq!(check_word(Some(&check(JudgedCheckOutcome::Failed, &["x"]))), "failed");
    assert_eq!(check_word(Some(&check(JudgedCheckOutcome::TimedOut, &[]))), "timed out");
    assert_eq!(check_word(Some(&check(JudgedCheckOutcome::CouldNotRun, &[]))), "could not run");
    assert_eq!(check_word(Some(&check(JudgedCheckOutcome::Running, &[]))), "running");
    assert_eq!(check_word(None), "\u{2014}");
}

#[test]
fn failed_everywhere_only_when_every_copy_failed_with_the_same_new_lines() {
    let missing = check(JudgedCheckOutcome::Failed, &["sh: vitest: command not found"]);
    assert!(check_failed_everywhere(&[Some(&missing), Some(&missing)]));
    let could_not = check(JudgedCheckOutcome::CouldNotRun, &["sh: vitest: command not found"]);
    assert!(check_failed_everywhere(&[Some(&missing), Some(&could_not)]), "could not run is a failure too");
    let passed = check(JudgedCheckOutcome::Passed, &[]);
    let failed = check(JudgedCheckOutcome::Failed, &["FAIL totals.test.mjs", "TypeError: summarize is not a function"]);
    assert!(!check_failed_everywhere(&[Some(&passed), Some(&failed)]));
    assert!(!check_failed_everywhere(&[Some(&missing), Some(&failed)]), "different words tell them apart");
    assert!(!check_failed_everywhere(&[Some(&missing)]), "one copy is no comparison");
    assert!(!check_failed_everywhere(&[Some(&missing), None]));
    let silent = check(JudgedCheckOutcome::Failed, &[]);
    assert!(!check_failed_everywhere(&[Some(&silent), Some(&silent)]), "nothing said is no same failure");
    // Still running is no failure, the same or otherwise.
    let running = check(JudgedCheckOutcome::Running, &[]);
    assert!(!check_failed_everywhere(&[Some(&running), Some(&running)]));
}

#[test]
fn the_line_over_the_columns_says_what_the_judges_read_or_where_to_set_a_check() {
    use JudgedCheckOutcome::*;
    let ran = [Some(("npm test", Passed)), Some(("npm test", Failed))];
    assert_eq!(check_note(true, &ran, Some("npm test")).as_deref(), Some("Each judge also reads what your check, npm test, said in each copy."));
    assert_eq!(
        check_note(true, &[None, None], None).as_deref(),
        Some("Set a check command in Settings > Project folder, and each judge also reads whether every copy passes it.")
    );
    assert_eq!(check_note(true, &[None, None], Some("  ")).as_deref(), check_note(true, &[None, None], None).as_deref(), "a blank command is none");
    // Set, but no copy has run it yet: nothing to promise and nothing to ask.
    assert_eq!(check_note(true, &[None, None], Some("npm test")), None);
    // Answers that change nothing, or work in the folder itself: no copy ran a check.
    assert_eq!(check_note(false, &ran, Some("npm test")), None);
    assert_eq!(check_note(false, &[None, None], None), None);
    // One still running: a judge asked now waits for it.
    assert_eq!(
        check_note(true, &[Some(("npm test", Running)), Some(("npm test", Passed))], Some("npm test")).as_deref(),
        Some("Your check, npm test, is still running in a copy. A judge asked now waits for it to finish, then reads what it said in each copy.")
    );
}
