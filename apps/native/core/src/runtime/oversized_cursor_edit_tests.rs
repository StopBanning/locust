// Carries packages/runtime-adapters/test/an-oversized-cursor-edit-keeps-its-counts.test.ts (Electron 4ce2f37a).
// Cursor's edit result carries the whole file twice, past the cap; the stand-in keeps the counts and the diff
// from the record's first bytes, and none of the file.
use super::{cursor::oversized_stand_in, cursor_events::CursorNormalizer};
use serde_json::{Value, json};

const SECRET: &str = "the-content-of-the-whole-file";
const DIFF: &str = "--- a/notes.txt\n+++ b/notes.txt\n@@ -2,1 +2,1 @@\n-line two\n+line 2";
const ID: &str = "call-big-0\nfc_big_0";

fn edit_record(completed: bool) -> String {
    let mut edit = json!({ "args": { "path": "notes.txt", "streamContent": "x" } });
    if completed {
        edit["result"] = json!({ "success": { "path": "notes.txt", "linesAdded": 1, "linesRemoved": 1, "diffString": DIFF,
            "beforeFullFileContent": format!("{SECRET} {}", "y".repeat(2_000)),
            "afterFullFileContent": format!("{SECRET} {}", "z".repeat(2_000)) } });
    }
    // Key order as JSON.stringify writes Electron's record: type, subtype, call_id, tool_call.
    format!(
        r#"{{"type":"tool_call","subtype":"{}","call_id":{},"tool_call":{{"editToolCall":{},"toolCallId":{}}}}}"#,
        if completed { "completed" } else { "started" },
        json!(ID),
        edit,
        json!(ID)
    )
}

fn stand_in(line: &str) -> String {
    serde_json::to_string(&oversized_stand_in(line, line.len()).expect("a stand-in")).unwrap()
}

fn completed(events: &[super::Event]) -> Value {
    events.iter().find(|e| e.kind == "tool.completed").expect("a tool.completed").payload.clone()
}

#[test]
fn an_oversized_cursor_edit_leaves_its_counts_and_diff_in_the_stand_in_and_none_of_the_file() {
    let raw = stand_in(&edit_record(true));
    let parsed: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(parsed["type"], "locust.oversized");
    assert_eq!(parsed["recordType"], "tool_call");
    assert_eq!(parsed["edit"], json!({ "added": 1, "removed": 1, "diff": DIFF }));
    assert!(!raw.contains(SECRET), "{raw}");
}

#[test]
fn an_oversized_cursor_edit_closes_as_a_change_the_files_card_can_count() {
    let raw = stand_in(&edit_record(true));
    let mut cursor = CursorNormalizer::default();
    let mut events = cursor.accept_line(&edit_record(false));
    events.extend(cursor.accept_line(&raw));
    let done = completed(&events);
    assert_eq!(done["name"], "edit");
    assert_eq!(done["patch"]["added"], 1);
    assert_eq!(done["patch"]["removed"], 1);
    assert_eq!(done["patch"]["truncated"], false);
    assert!(done.get("status").is_none(), "{done}");
}

#[test]
fn an_oversized_cursor_edit_without_a_diff_closes_with_its_counts_marked_cut_short() {
    let counts = json!({ "type": "locust.oversized", "bytes": 330_000, "recordType": "tool_call",
        "callIds": ["call-big-0|fc_big_0"], "edit": { "added": 4, "removed": 2 } });
    let mut cursor = CursorNormalizer::default();
    let mut events = cursor.accept_line(&edit_record(false));
    events.extend(cursor.accept_line(&counts.to_string()));
    let patch = &completed(&events)["patch"];
    assert_eq!(patch["text"], "");
    assert_eq!(patch["added"], 4);
    assert_eq!(patch["removed"], 2);
    assert_eq!(patch["truncated"], true);
}

// Not an Electron case: Rust cuts the head by bytes, and a cut inside a character used to panic the reader.
#[test]
fn an_oversized_record_cut_inside_a_character_still_stands_in() {
    let line = format!(r#"{{"type":"tool_call","call_id":"c1","x":"{}"}}"#, "é".repeat(40_000));
    let parsed: Value = serde_json::from_str(&stand_in(&line)).unwrap();
    assert_eq!(parsed["callIds"], json!(["c1"]));
}

// Not an Electron case: callIdsIn names a call once, though Cursor's record names it twice (call_id and toolCallId).
#[test]
fn an_oversized_edits_stand_in_names_its_call_once() {
    let parsed: Value = serde_json::from_str(&stand_in(&edit_record(true))).unwrap();
    assert_eq!(parsed["callIds"], json!(["call-big-0|fc_big_0"]));
}
