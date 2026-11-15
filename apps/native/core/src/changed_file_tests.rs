//! The five Electron changed-file-count cases, using the rows the native trace draws.
use crate::{activity_entries::Patch, activity_trace::activity_trace, ledger::{Item, Status, ToolDetail}};
fn edit(path: &str, patch_path: Option<&str>, failed: bool) -> Item {
    Item::Tool {
        name: "edit".into(), summary: path.into(),
        status: if failed { Status::Failed } else { Status::Done },
        detail: Box::new(ToolDetail {
            patch: patch_path.map(|path| Patch {
                text: format!("diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n@@ -1 +1 @@\n-draft\n+final\n"),
                added: 1, removed: 1, truncated: false,
            }), ..Default::default()
        }),
    }
}
fn files(items: &[Item]) -> Option<String> {
    activity_trace(items, &[], "completed").segments.into_iter()
        .find(|s| s.key == "files").map(|s| s.text)
}
#[test]
fn counts_one_copilot_multiline_patch_as_one_changed_file() {
    assert_eq!(files(&[edit("*** Begin Patch\n*** Update File: notes.ts\n@@\n-draft\n+final\n*** End Patch", Some("notes.ts"), false)]).as_deref(), Some("1 file"));
}
#[test]
fn counts_two_edits_to_the_same_path_once() {
    let detail = edit("notes.ts", Some("notes.ts"), false);
    assert_eq!(files(&[detail.clone(), detail]).as_deref(), Some("1 file"));
}
#[test]
fn still_counts_two_edits_to_two_paths_as_two() {
    assert_eq!(files(&[edit("notes.ts", Some("notes.ts"), false), edit("other.ts", Some("other.ts"), false)]).as_deref(), Some("2 files"));
}
#[test]
fn does_not_count_a_refused_write_as_a_changed_file() {
    assert_eq!(files(&[edit("blocked.txt", None, true)]), None);
}
#[test]
fn still_counts_the_writes_that_landed_beside_a_refused_one() {
    assert_eq!(files(&[edit("blocked.txt", None, true), edit("notes.ts", Some("notes.ts"), false)]).as_deref(), Some("1 file"));
}
