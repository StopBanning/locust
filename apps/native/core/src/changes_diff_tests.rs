//! Electron `renderer/src/diff.test.ts`: reading a unified diff. The fold and the expand button are drawn by the
//! window, so those cases are tested there too (`src/ui/changes_panel_diff_tests.rs`).
use super::*;

// A patch as Cursor actually emits one (absolute path, no `diff --git`), followed by one as Codex's tooling would
// (a/ b/ prefixes, two hunks).
const CURSOR_ADD: &str = "--- /dev/null\n+++ b/C:\\work\\scratch\\NOTES.md\n@@ -1,0 +1 @@\n+hello\n";

const TWO_HUNKS: &str = "diff --git a/src/billing.ts b/src/billing.ts
--- a/src/billing.ts
+++ b/src/billing.ts
@@ -12,4 +12,6 @@ registerHandlers()
 export function registerHandlers(app: App) {
   app.post(\"/hooks/billing/v2\", handleV2);
-  app.post(\"/hooks/billing/v3\", handleV3);
+  if (flags.billingV3) {
+    app.post(\"/hooks/billing/v3\", handleV3);
+  }
 }
@@ -41,4 +43,6 @@ handleV3()
 async function handleV3(req: Request) {
-  return old(req);
+  const body = parse(req);
+  return next(body);
+  // logged
 }
";

fn kinds(rows: &[Row]) -> Vec<char> {
    rows.iter().map(|r| r.kind).collect()
}
fn hunk_counts(h: &Hunk) -> (usize, usize) {
    File { hunks: vec![h.clone()], ..File::default() }.counts()
}

// ---- reading a unified diff ----

// case: reads an added file the way Cursor writes one
#[test]
fn reads_an_added_file_the_way_cursor_writes_one() {
    let files = parse(CURSOR_ADD);
    assert_eq!(files.len(), 1);
    let f = &files[0];
    assert_eq!(f.status, "ADDED");
    assert_eq!(f.path, "C:\\work\\scratch\\NOTES.md");
    assert_eq!(f.hunks.len(), 1);
    assert_eq!(f.hunks[0].rows, vec![Row { kind: '+', old: None, new: Some(1), text: "hello".into() }]);
}

// case: reads a modified file with two hunks and their headings
#[test]
fn reads_a_modified_file_with_two_hunks_and_their_headings() {
    let f = parse(TWO_HUNKS).remove(0);
    assert_eq!(f.status, "MODIFIED");
    assert_eq!(f.path, "src/billing.ts");
    assert_eq!(f.hunks.iter().map(|h| h.heading.as_str()).collect::<Vec<_>>(), ["registerHandlers()", "handleV3()"]);
    assert_eq!(kinds(&f.hunks[0].rows), [' ', ' ', '-', '+', '+', '+', ' ']);
    // A heading is what follows the range, without the spaces around it.
    let spaced = parse("--- a/x\n+++ b/x\n@@ -1 +1 @@   main()  \n-a\n+b\n").remove(0);
    assert_eq!(spaced.hunks[0].heading, "main()");
}

// case: numbers lines from the hunk start, on each side
#[test]
fn numbers_lines_from_the_hunk_start_on_each_side() {
    let f = parse(TWO_HUNKS).remove(0);
    let rows = &f.hunks[0].rows;
    assert_eq!((rows[0].old, rows[0].new), (Some(12), Some(12)));
    assert_eq!((rows[2].kind, rows[2].old, rows[2].new), ('-', Some(14), None));
    assert_eq!((rows[3].kind, rows[3].old, rows[3].new), ('+', None, Some(14)));
    assert_eq!((rows[6].kind, rows[6].old, rows[6].new), (' ', Some(15), Some(17)));
}

// ---- counts are derived, never authored ----

// case: computes a file total from its rows alone
#[test]
fn computes_a_file_total_from_its_rows_alone() {
    assert_eq!(parse(TWO_HUNKS)[0].counts(), (6, 2));
}

// case: rebuilds each @@ range from its rows, and it agrees with what was written
#[test]
fn rebuilds_each_range_from_its_rows_and_it_agrees_with_what_was_written() {
    let f = parse(TWO_HUNKS).remove(0);
    assert_eq!(f.hunks[0].range(), "@@ -12,4 +12,6 @@");
    // The fixture's own header says 4 and 6; its rows say 3 and 5. The rows win.
    assert_eq!(f.hunks[1].range(), "@@ -41,3 +43,5 @@");
}

// case: does not trust a header that disagrees with its rows
#[test]
fn does_not_trust_a_header_that_disagrees_with_its_rows() {
    // The runtime claims 60 added lines; two are present. The range shown describes the two.
    let f = parse("--- a/x\n+++ b/x\n@@ -1,1 +1,61 @@\n-a\n+b\n+c\n").remove(0);
    assert_eq!(f.hunks[0].range(), "@@ -1 +1,2 @@");
    assert_eq!(hunk_counts(&f.hunks[0]), (2, 1));
}

// ---- no open file ends in silence ----

// case: says all hunks are shown when they are
#[test]
fn says_all_hunks_are_shown_when_they_are() {
    let f = parse(TWO_HUNKS).remove(0);
    assert_eq!(completeness(&f, 2, false), "All 2 sections shown");
}

// case: offers to expand, naming exactly what remains
#[test]
fn offers_to_expand_naming_exactly_what_remains() {
    let f = parse(TWO_HUNKS).remove(0);
    // Four lines: the second hunk's one removal and three additions, never its context.
    assert_eq!(completeness(&f, 1, false), "Expand full file · 1 hunk more, 4 lines");
}

// case: states a remainder it could not render, rather than nothing
#[test]
fn states_a_remainder_it_could_not_render_rather_than_nothing() {
    let f = parse(TWO_HUNKS).remove(0);
    let cut = completeness(&f, 2, true);
    assert!(cut.contains("the rest was not recorded inline"), "{cut}");
    assert_eq!(cut, "The runtime reported more than fits here · 2 hunks rendered · the rest was not recorded inline");
}

// case: cannot produce an empty statement for any input
#[test]
fn cannot_produce_an_empty_statement_for_any_input() {
    let f = parse(TWO_HUNKS).remove(0);
    // Electron's -1 has no `usize`; 0 is the least a count of shown hunks can be.
    for shown in [0, 1, 2, 99] {
        for truncated in [false, true] {
            assert!(!completeness(&f, shown, truncated).is_empty(), "{shown} {truncated}");
        }
    }
    assert!(!completeness(&File::default(), 0, false).is_empty());
}

// ---- marking only what changed on a line ----

// case: leaves the common prefix and suffix plain
#[test]
fn leaves_the_common_prefix_and_suffix_plain() {
    let (old, new) = word_diff("  app.post(\"/v2\", handleV2);", "  app.post(\"/v3\", handleV3);");
    let changed = |spans: &[Span]| spans.iter().filter(|s| s.changed).map(|s| s.text.clone()).collect::<Vec<_>>();
    assert_eq!(changed(&old), ["2\", handleV2"]);
    assert_eq!(changed(&new), ["3\", handleV3"]);
    assert_eq!(old.iter().map(|s| s.text.as_str()).collect::<String>(), "  app.post(\"/v2\", handleV2);");
    // Two equal lines mark nothing; two empty ones are one plain, empty piece.
    assert!(word_diff("same", "same").0.iter().all(|s| !s.changed));
    assert_eq!(word_diff("", "").0, [Span { text: String::new(), changed: false }]);
    // A mark never splits a character.
    assert_eq!(changed(&word_diff("café", "cafè").1), ["è"]);
}

// case: pairs a removed line with the added line that replaced it, positionally
#[test]
fn pairs_a_removed_line_with_the_added_line_that_replaced_it_positionally() {
    let f = parse(TWO_HUNKS).remove(0);
    let rows = &f.hunks[1].rows;
    let spans = paired_spans(rows);
    assert_eq!(spans.len(), rows.len());
    let del = rows.iter().position(|r| r.kind == '-').unwrap();
    let first_add = rows.iter().position(|r| r.kind == '+').unwrap();
    assert!(spans[del].is_some());
    assert!(spans[first_add].is_some());
    // The second and third additions had no removal to pair with.
    let unpaired: Vec<_> = rows.iter().enumerate().filter(|(_, r)| r.kind == '+').skip(1).map(|(i, _)| i).collect();
    assert_eq!(unpaired.len(), 2);
    assert!(unpaired.iter().all(|&i| spans[i].is_none()));
    assert!(rows.iter().enumerate().filter(|(_, r)| r.kind == ' ').all(|(i, _)| spans[i].is_none()));
}

// ---- a line inside a hunk that looks like a header ----

// case: keeps a removed SQL comment as a removed line
#[test]
fn keeps_a_removed_sql_comment_as_a_removed_line() {
    let files =
        parse("--- a/db/schema.sql\n+++ b/db/schema.sql\n@@ -1,3 +1,3 @@\n CREATE TABLE t (\n--- legacy column, remove me\n+  id INTEGER\n );");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "db/schema.sql");
    assert_eq!(files[0].status, "MODIFIED");
    assert_eq!(files[0].counts(), (1, 1));
    assert_eq!(kinds(&files[0].hunks[0].rows), [' ', '-', '+', ' ']);
    assert_eq!(files[0].hunks[0].rows[1].text, "-- legacy column, remove me");
}

// case: keeps an added line beginning ++ as an added line, not a rename
#[test]
fn keeps_an_added_line_beginning_as_an_added_line_not_a_rename() {
    let files = parse("--- a/notes.txt\n+++ b/notes.txt\n@@ -1 +1,2 @@\n hello\n+++ world");
    assert_eq!(files.len(), 1);
    assert_eq!((files[0].path.as_str(), files[0].status.as_str()), ("notes.txt", "MODIFIED"));
    assert_eq!(files[0].counts(), (1, 0));
}

// case: still starts the next file once a hunk is used up
#[test]
fn still_starts_the_next_file_once_a_hunk_is_used_up() {
    let files = parse(
        "--- a/one.sql\n+++ b/one.sql\n@@ -1 +1 @@\n--- old\n+-- new\n--- a/two.txt\n+++ b/two.txt\n@@ -1 +1 @@\n-a\n+b",
    );
    assert_eq!(
        files.iter().map(|f| (f.path.as_str(), f.counts())).collect::<Vec<_>>(),
        [("one.sql", (1, 1)), ("two.txt", (1, 1))]
    );
}

// ---- a file that had no lines before ----

// case: is ADDED, whatever its header says
#[test]
fn is_added_whatever_its_header_says() {
    assert_eq!(parse("--- hello.txt\n+++ hello.txt\n@@ -0,0 +1 @@\n+hi\n")[0].status, "ADDED");
}

// case: but a file edited from its first line is still MODIFIED
#[test]
fn but_a_file_edited_from_its_first_line_is_still_modified() {
    assert_eq!(parse("--- hello.txt\n+++ hello.txt\n@@ -1 +1 @@\n-hi\n+bye\n")[0].status, "MODIFIED");
}

// ---- counts shown on a file row (diff.ts `fileRowCounts`, Rust's `FileEntry::row_counts`) ----

fn entry(counts: (usize, usize), truncated: bool, reported: Option<(usize, usize)>, large: bool) -> crate::activity_entries::FileEntry {
    crate::activity_entries::FileEntry { key: "k".into(), file: File::default(), counts, reported, large, truncated }
}

// case: shows the true reported counts for a truncated large file rewrite
#[test]
fn shows_the_true_reported_counts_for_a_truncated_large_file_rewrite() {
    assert_eq!(entry((0, 747), true, Some((1700, 1700)), true).row_counts(), Some((1700, 1700)));
}

// case: withholds counts for a truncated diff when true counts cannot be known
#[test]
fn withholds_counts_for_a_truncated_diff_when_true_counts_cannot_be_known() {
    assert_eq!(entry((0, 747), true, None, true).row_counts(), None);
}

// case: keeps counts unchanged for a small diff
#[test]
fn keeps_counts_unchanged_for_a_small_diff() {
    assert_eq!(entry((2, 1), false, None, false).row_counts(), Some((2, 1)));
}
