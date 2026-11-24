//! Electron's `packages/runtime-adapters/test/line-diff.test.ts`, case by case, through the caller: an ACP edit reports
//! old and new text, and `patch_of` (what an ACP tool row's patch is made with) turns it into a real unified diff with
//! counts. Where Electron counts with toolPatchFrom, so does this (cursor_events' port of it), beside patch_of's own.
use super::*;
use crate::runtime::cursor_events::tool_patch_from;

/// "line 1\n" .. "line N\n", as Electron's `lines` helper writes them.
fn numbered(count: usize, from: usize) -> String {
    (from..from + count).map(|n| format!("line {n}\n")).collect()
}

/// The patch an ACP edit of `path` from `old` to `new` carries.
fn edit(path: &str, old: Option<&str>, new: &str) -> Option<Value> {
    let mut diff = json!({"type": "diff", "path": path, "newText": new});
    if let Some(old) = old {
        diff["oldText"] = json!(old);
    }
    patch_of(&json!([diff]))
}

fn text_of(patch: &Value) -> &str {
    patch["text"].as_str().unwrap()
}

/// added and removed, as patch_of counted them and as toolPatchFrom counts its text; the two must agree.
fn counts(patch: &Value) -> (u64, u64) {
    let read = tool_patch_from(text_of(patch)).unwrap();
    let own = (patch["added"].as_u64().unwrap(), patch["removed"].as_u64().unwrap());
    assert_eq!(own, (read["added"].as_u64().unwrap(), read["removed"].as_u64().unwrap()), "patch_of and its text disagree");
    own
}

// case: is nothing when nothing changed
#[test]
fn an_edit_that_changed_nothing_has_no_patch() {
    assert_eq!(edit("a.ts", Some("x\n"), "x\n"), None);
    assert_eq!(unified_diff_text("a.ts", Some("x\n"), "x\n"), None);
}

// case: writes one changed line with three lines of context either side
#[test]
fn one_changed_line_with_three_lines_of_context_either_side() {
    let before = numbered(10, 1);
    let after = before.replace("line 5\n", "line five\n");
    let patch = edit("src/a.ts", Some(&before), &after).unwrap();
    assert_eq!(
        text_of(&patch),
        ["--- a/src/a.ts", "+++ b/src/a.ts", "@@ -2,7 +2,7 @@", " line 2", " line 3", " line 4", "-line 5", "+line five", " line 6", " line 7", " line 8", ""].join("\n")
    );
}

// case: names an absolute path as it is, with no git prefix in front of a drive letter
#[test]
fn an_absolute_path_is_named_as_it_is() {
    let windows = edit("C:\\work\\pebble\\app.js", Some("a\n"), "b\n").unwrap();
    assert_eq!(text_of(&windows).split('\n').take(2).collect::<Vec<_>>(), ["--- C:\\work\\pebble\\app.js", "+++ C:\\work\\pebble\\app.js"]);
    let unix = edit("/home/p/app.js", Some("a\n"), "b\n").unwrap();
    assert_eq!(text_of(&unix).split('\n').nth(1), Some("+++ /home/p/app.js"));
}

// case: a new file is all added, from /dev/null
#[test]
fn a_new_file_is_all_added_from_dev_null() {
    let patch = edit("kept.txt", None, "kept\n").unwrap();
    assert_eq!(text_of(&patch), ["--- /dev/null", "+++ b/kept.txt", "@@ -0,0 +1,1 @@", "+kept", ""].join("\n"));
}

// case: keeps two distant changes as two hunks, and the counts are what the text says
#[test]
fn two_distant_changes_are_two_hunks_counted_as_written() {
    let before = numbered(40, 1);
    let after = before.replace("line 3\n", "line three\n").replace("line 35\n", "line thirty-five\n");
    let patch = edit("b.ts", Some(&before), &after).unwrap();
    assert_eq!(text_of(&patch).lines().filter(|l| l.starts_with("@@")).count(), 2);
    assert_eq!(counts(&patch), (2, 2));
    assert_eq!(tool_patch_from(text_of(&patch)).unwrap()["truncated"], false);
}

// case: finds the lines that stayed, not a rewrite of everything between
#[test]
fn finds_the_lines_that_stayed() {
    let patch = edit("c.ts", Some("a\nb\nc\nd\ne\n"), "a\nx\nc\ny\ne\n").unwrap();
    assert_eq!(counts(&patch), (2, 2));
    assert!(text_of(&patch).contains("\n c\n"));
}

// case: an insertion and a deletion count exactly
#[test]
fn an_insertion_and_a_deletion_count_exactly() {
    let five = numbered(5, 1);
    assert_eq!(counts(&edit("d.ts", Some(&five), &five.replace("line 3\n", "line 3\nextra\n")).unwrap()), (1, 0));
    assert_eq!(counts(&edit("d.ts", Some(&five), &five.replace("line 3\n", "")).unwrap()), (0, 1));
}
