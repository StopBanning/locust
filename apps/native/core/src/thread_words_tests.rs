//! `a-helpers-summary-reads-as-words.test.ts` and the headline case of `a-long-silent-stretch-reads-as-its-moves.test.ts`.
use crate::thread_words::{helper_reported_back, plain_summary, thought_headline};

// case: drops bold, underline-bold, code and heading marks and keeps the words
#[test]
fn drops_bold_underline_bold_code_and_heading_marks_and_keeps_the_words() {
    assert_eq!(plain_summary("**33** files total in the current working directory"), "33 files total in the current working directory");
    assert_eq!(plain_summary("Found __three__ callers of `total()` in src/cart.py"), "Found three callers of total() in src/cart.py");
    assert_eq!(plain_summary("## Summary: the build passes"), "Summary: the build passes");
}

// case: drops single-star emphasis without touching a star that is not emphasis
#[test]
fn drops_single_star_emphasis_without_touching_a_star_that_is_not_emphasis() {
    assert_eq!(plain_summary("It is *not* in the folder"), "It is not in the folder");
    assert_eq!(plain_summary("Matches src/**/*.ts and 2 * 3 = 6"), "Matches src/**/*.ts and 2 * 3 = 6");
}

// case: is what the helper row says when it reported back
#[test]
fn is_what_the_helper_row_says_when_it_reported_back() {
    assert_eq!(helper_reported_back(Some("**33** files total in the current working directory")), "reported back \u{b7} 33 files total in the current working directory");
    assert_eq!(helper_reported_back(None), "reported back");
}

// case: folds a summary onto one line
#[test]
fn folds_a_summary_onto_one_line() {
    assert_eq!(plain_summary("Read 4 files.\nNo test covers it."), "Read 4 files. No test covers it.");
}

// case: reads a headline only where the summary leads with one
#[test]
fn reads_a_headline_only_where_the_summary_leads_with_one() {
    assert_eq!(thought_headline("**Planning the fix**\n\nFirst the test.").as_deref(), Some("Planning the fix"));
    assert_eq!(thought_headline("Planning the fix"), None);
}
