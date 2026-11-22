//! Electron's `renderer/src/ledgerFailure.test.ts`: the card about broken records is the last place to be loose with
//! a claim.
use super::*;

fn find<'a>(rows: &'a [Row], tone: &str) -> Option<&'a Row> {
    rows.iter().find(|row| row.tone == tone)
}

fn says(text: &str, pattern: &'static str) -> bool {
    crate::invariants::regex(pattern).is_match(text)
}

// case: never says a run was held, because it was stopped
#[test]
fn never_says_a_run_was_held_because_it_was_stopped() {
    let said = sentence(Some("The disk reports no space."));
    assert!(says(&said, r"(?i)stopped"));
    assert!(!says(&said, r"(?i)\bheld\b|\bpaused\b"));
    assert!(says(TITLE, r"(?i)^stopped") && !says(TITLE, r"(?i)\bheld\b|\bpaused\b"));
}

// case: quotes the host rather than guessing at a cause
#[test]
fn quotes_the_host_rather_than_guessing_at_a_cause() {
    assert!(sentence(Some("The disk reports no space.")).contains("The disk reports no space."));
    // And when the host said nothing, it does not invent one.
    for blank in [None, Some(""), Some("   ")] {
        let said = sentence(blank);
        assert!(says(&said, r"(?i)could not write"), "{said}");
        assert!(!says(&said, r"(?i)disk|permission|space"), "{said}");
    }
}

// case: states the safe half without inventing a checkpoint number
#[test]
fn states_the_safe_half_without_inventing_a_checkpoint_number() {
    let rows = rows(None, false);
    let safe = find(&rows, "safe").expect("a safe row");
    assert!(says(&safe.text, r"(?i)only ever appends"));
    assert!(!says(&safe.text, r"[0-9]"));
}

// case: uses the checkpoint count when it genuinely has one
#[test]
fn uses_the_checkpoint_count_when_it_genuinely_has_one() {
    assert!(find(&rows(Some(14), false), "safe").unwrap().text.contains("14 checkpoints"));
    // Singular, not '1 checkpoints'.
    let one = find(&rows(Some(1), false), "safe").unwrap().text.clone();
    assert!(one.contains("1 checkpoint "));
    assert!(!one.contains("1 checkpoints"));
}

// case: says plainly that stopping did not undo the files already changed
#[test]
fn says_plainly_that_stopping_did_not_undo_the_files_already_changed() {
    assert!(rows(Some(3), false).iter().any(|row| says(&row.text, r"(?i)still changed")));
}

// case: does not offer to reopen a mission that was never written
#[test]
fn does_not_offer_to_reopen_a_mission_that_was_never_written() {
    let rows = rows(None, true);
    let safe = find(&rows, "safe").unwrap();
    assert!(!says(&safe.text, r"(?i)reopened"));
    assert!(says(&safe.text, r"(?i)nothing ran"));
    assert!(says(&rows.iter().find(|row| row.label == "At risk").unwrap().text, r"(?i)never began"));
    // And it does not send them to check a folder for work that never happened.
    assert!(!rows.iter().any(|row| says(&row.text, r"(?i)check the folder yourself")));
}

// case: still offers the mid-run copy when something WAS written
#[test]
fn still_offers_the_mid_run_copy_when_something_was_written() {
    let rows = rows(Some(3), false);
    assert!(says(&find(&rows, "safe").unwrap().text, r"(?i)reopened"));
    assert!(rows.iter().any(|row| says(&row.text, r"(?i)still changed")));
}

// case: admits the app cannot say what was lost
#[test]
fn admits_the_app_cannot_say_what_was_lost() {
    let risk = rows(Some(3), false).into_iter().find(|row| row.label == "At risk").unwrap();
    assert!(says(&risk.text, r"(?i)cannot tell you what it was"));
    // The lines a place that draws text shows carry all of it, title first.
    let lines = lines(Some("The disk reports no space."), None, false);
    assert_eq!(lines[0], TITLE);
    assert!(lines[1].starts_with("The disk reports no space."));
    assert!(lines.iter().any(|line| line.starts_with("At risk: This turn.")));
}
