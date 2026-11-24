//! Electron's `packages/runtime-adapters/test/a-claude-edit-says-what-it-changed.test.ts`, through the caller: the
//! recorded run (`test/fixtures/claude/edit-and-write.jsonl`, Claude Code on Haiku 4.5 changing notes.txt and writing
//! hello.md) is read line by line as the Claude runtime reads it (`records::claude_record`, then
//! `ClaudeNormalizer::accept`), and Electron's `claudeFilePatch` cases are a call's answering record.
use super::*;

fn recorded() -> Vec<String> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../packages/runtime-adapters/test/fixtures/claude/edit-and-write.jsonl");
    std::fs::read_to_string(path).unwrap().trim_end().lines().map(str::to_owned).collect()
}

fn normalize(lines: &[String]) -> Vec<Event> {
    let mut n = ClaudeNormalizer::default();
    lines.iter().flat_map(|raw| n.accept(&super::super::records::claude_record(raw))).collect()
}

fn completed_file_calls(events: &[Event]) -> Vec<&Value> {
    events
        .iter()
        .filter(|e| e.kind == "tool.completed")
        .map(|e| &e.payload)
        .filter(|p| p["name"] == "Edit" || p["name"] == "Write")
        .collect()
}

#[test]
fn an_edit_carries_the_hunk_claude_code_reported_line_numbers_and_all() {
    // case: an Edit carries the hunk Claude Code reported, line numbers and all
    let events = normalize(&recorded());
    let edit = completed_file_calls(&events).into_iter().find(|p| p["name"] == "Edit").unwrap();
    assert_eq!(
        edit["patch"],
        json!({
            "text": "--- C:\\work\\shop\\notes.txt\n+++ C:\\work\\shop\\notes.txt\n@@ -1,3 +1,3 @@\n line one\n-line two\n+line 2\n line three\n",
            "added": 1,
            "removed": 1,
            "truncated": false,
        })
    );
}

#[test]
fn a_write_of_a_new_file_carries_the_whole_file_as_added() {
    // case: a Write of a new file carries the whole file as added
    let events = normalize(&recorded());
    let write = completed_file_calls(&events).into_iter().find(|p| p["name"] == "Write").unwrap();
    let text = write["patch"]["text"].as_str().unwrap();
    assert!(text.starts_with("--- /dev/null\n+++ C:\\work\\shop\\hello.md\n@@ -0,0 +1,3 @@\n+# Hello\n"), "{text}");
    assert_eq!(write["patch"]["added"], 3);
    assert_eq!(write["patch"]["removed"], 0);
}

#[test]
fn a_read_carries_no_patch_a_result_without_a_change_is_not_one() {
    // case: a Read carries no patch: a result without a change is not one
    let events = normalize(&recorded());
    let others: Vec<&Value> = events
        .iter()
        .filter(|e| e.kind == "tool.completed")
        .map(|e| &e.payload)
        .filter(|p| p["name"] != "Edit" && p["name"] != "Write")
        .collect();
    assert!(!others.is_empty());
    assert!(others.iter().all(|p| p.get("patch").is_none()));
}

#[test]
fn a_failed_call_carries_none_whatever_its_record_holds() {
    // case: a failed call carries none, whatever its record holds
    let lines: Vec<String> = recorded()
        .into_iter()
        .map(|raw| {
            let mut record: Value = serde_json::from_str(&raw).unwrap();
            if record["type"] != "user" || record["tool_use_result"]["structuredPatch"].is_null() {
                return raw;
            }
            for block in record["message"]["content"].as_array_mut().into_iter().flatten() {
                if block["type"] == "tool_result" {
                    block["is_error"] = json!(true);
                }
            }
            record.to_string()
        })
        .collect();
    let events = normalize(&lines);
    let failed: Vec<&Value> = events.iter().filter(|e| e.kind == "tool.failed").map(|e| &e.payload).collect();
    assert_eq!(failed.len(), 2);
    assert!(failed.iter().all(|p| p.get("patch").is_none()));
}

/// The patch a call's answering record carries, as the runtime reads that record.
fn patch_of(tool_use_result: Value) -> Option<Value> {
    let mut n = ClaudeNormalizer::default();
    n.accept(&json!({"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu_1","name":"Edit"}}}));
    let mut answer = json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_1","content":"ok"}]}});
    if !tool_use_result.is_null() {
        answer["tool_use_result"] = tool_use_result;
    }
    n.accept(&answer).into_iter().find(|e| e.kind == "tool.completed").and_then(|e| e.payload.get("patch").cloned())
}

#[test]
fn compares_a_rewritten_file_against_what_it_was_when_no_hunks_came() {
    // case: compares a rewritten file against what it was when no hunks came
    let patch = patch_of(json!({"type":"update","filePath":"a.txt","content":"one\nTWO\n","originalFile":"one\ntwo\n","structuredPatch":[]})).unwrap();
    assert_eq!(patch["text"], "--- a/a.txt\n+++ b/a.txt\n@@ -1,2 +1,2 @@\n one\n-two\n+TWO\n");
}

#[test]
fn reads_nothing_from_a_result_that_is_not_a_files() {
    // case: reads nothing from a result that is not a file's
    assert_eq!(patch_of(Value::Null), None);
    assert_eq!(patch_of(json!({"type":"text","file":{"filePath":"a.txt","content":"x"}})), None);
    assert_eq!(patch_of(json!({"filePath":"a.txt","structuredPatch":[{"oldStart":"1","lines":[]}]})), None);
}
