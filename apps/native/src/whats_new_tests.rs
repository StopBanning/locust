//! Electron's `whats-new-reads-like-claude-codes.test.tsx`, its date case: `changelogDate` (`changelog::changelog_date`),
//! with Electron's values. The rest draws What's new and its splash, which Rust draws in `src/ui/home_panel.rs`.
use crate::changelog::changelog_date;

#[test]
fn a_date_reads_as_words_from_the_digits_never_through_a_clock_that_shifts_it_a_day() {
    assert_eq!(changelog_date("2026-09-23").as_deref(), Some("September 23, 2026"));
    assert_eq!(changelog_date("2026-01-05").as_deref(), Some("January 5, 2026"));
    // `changelogDate(undefined)`: a build with no date is `None` on Rust's entry, and is never read.
    assert_eq!(changelog_date("someday").as_deref(), Some("someday"));
}
