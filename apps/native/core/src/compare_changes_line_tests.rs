//! Electron's `a-comparison-that-edits-says-what-each-changed.test.tsx`, its `changesLine` words
//! (`compare::changes_line`), with Electron's values. The bar's and Keep's sentences are drawn in
//! `src/ui/compare_view.rs`.
use crate::compare::changes_line;

/// The words at each column's foot; where they sit (before its time) is the view's.
#[test]
fn says_what_each_changed_at_its_foot() {
    assert_eq!(changes_line(2, Some(12), Some(3)), "+12 −3 in 2 files");
    assert_eq!(changes_line(0, Some(0), Some(0)), "no changes");
}

#[test]
fn counts_one_file_as_a_file() {
    assert_eq!(changes_line(1, Some(1), Some(0)), "+1 −0 in 1 file");
}
