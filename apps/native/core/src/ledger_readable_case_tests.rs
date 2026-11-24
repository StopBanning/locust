// Carries the writer half of two packages/mission-store files (Electron 4ce2f37a), through the writer a run appends
// with (`LedgerWriter::append`, as native.rs records every runtime event) and read back as Electron's `getMission`
// reads (`mission_export::recover_mission_from_path`):
// - a-helpers-calls-are-kept-as-its-own: a helper's calls kept under its row, and a parent that names nothing refused;
// - a-long-field-is-kept-shortened: an event that is not long but wrong is still refused, while a long one is kept.
use crate::{ledger_write::LedgerWriter, mission_export::recover_mission_from_path, store::Store};
use serde_json::{Value, json};
use std::path::PathBuf;

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn writer(tag: &str, runtime: &str, prompt: &str) -> (Temp, LedgerWriter) {
    let root = std::env::temp_dir().join(format!("locust-readable-{tag}-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    let writer = LedgerWriter::create_runtime(&store, "mission_1", prompt, "account-default", runtime, runtime, &[], None).unwrap();
    (Temp(root), writer)
}

fn tool_payload(kind: &str, item: &str, name: &str, extra: Value) -> Value {
    let mut payload = json!({ "itemId": item, "toolKind": "tool_use", "name": name,
        "phase": if kind == "tool.started" { "started" } else { "completed" }, "evidence": { "redacted": true } });
    for (k, v) in extra.as_object().unwrap() {
        payload[k] = v.clone();
    }
    payload
}

fn under_helpers(events: &Value) -> Vec<String> {
    events.as_array().unwrap().iter().filter_map(|e| {
        let parent = e["payload"]["parentItemId"].as_str()?;
        Some(format!("{} {} under {parent}", e["type"].as_str().unwrap(), e["payload"]["itemId"].as_str().unwrap()))
    }).collect()
}

// case: writes a helper's calls at v22 and reads them back under the helper's row
#[test]
fn a_helpers_calls_are_written_at_v22_or_later_and_come_back_under_its_row() {
    let (_t, mut writer) = writer("helper", "claude", "Use a helper to count the files here.");
    let path = writer.path.clone();
    for (kind, item, name, extra) in [
        ("tool.started", "toolu_agent", "Agent", json!({ "command": "Count the files" })),
        ("tool.started", "toolu_glob", "Glob", json!({ "command": "**/*", "parentItemId": "toolu_agent" })),
        ("tool.completed", "toolu_glob", "Glob", json!({ "command": "**/*", "parentItemId": "toolu_agent", "output": "a.txt\nb.txt" })),
        ("tool.completed", "toolu_agent", "Agent", json!({ "command": "Count the files", "status": "Explore", "output": "There are 7 files." })),
    ] {
        writer.append(kind, tool_payload(kind, item, name, extra), None).unwrap();
    }
    drop(writer);
    let mission = recover_mission_from_path(&path).unwrap();
    // Written at whatever is current; v22 is the first that keeps a helper's calls.
    assert_eq!(mission["schemaVersion"], 27);
    assert_eq!(mission["issues"], json!([]));
    assert_eq!(under_helpers(&mission["events"]), ["tool.started toolu_glob under toolu_agent", "tool.completed toolu_glob under toolu_agent"]);
}

// case: refuses a parent that names nothing, so a reader never has to guess at one
#[test]
fn a_helper_parent_that_names_nothing_is_refused_and_nothing_reaches_the_file() {
    let (_t, mut writer) = writer("parent", "claude", "Use a helper to count the files here.");
    let path = writer.path.clone();
    let before = std::fs::read(&path).unwrap();
    for parent in [json!(""), json!("   "), json!(7)] {
        let error = writer.append("tool.started", tool_payload("tool.started", "toolu_glob", "Glob", json!({ "parentItemId": parent })), None).unwrap_err();
        assert!(error.to_string().contains("not readable by the ledger reader"), "{error}");
    }
    assert_eq!(std::fs::read(&path).unwrap(), before, "a refused event leaves the file as it was");
    // The writer goes on: the next readable event is written, numbered as if nothing had been tried.
    let kept = writer.append("tool.started", tool_payload("tool.started", "toolu_glob", "Glob", json!({ "parentItemId": "toolu_agent" })), None).unwrap();
    assert_eq!(kept["sequence"], 1);
    drop(writer);
    let mission = recover_mission_from_path(&path).unwrap();
    assert_eq!(mission["issues"], json!([]));
    assert_eq!(mission["events"].as_array().unwrap().len(), 1);
}

// case: is refused as before when it is not long but wrong
#[test]
fn an_event_that_is_wrong_rather_than_long_is_still_refused() {
    let (_t, mut writer) = writer("wrong", "acp", "Say something.");
    let path = writer.path.clone();
    // Vocabulary is not fitted: an operation nobody defined is still refused.
    let wrong = json!({ "itemId": "m1", "operation": "prepend", "text": "hi", "final": false, "evidence": { "redacted": false } });
    let error = writer.append("message.delta", wrong, None).unwrap_err();
    assert!(error.to_string().contains("not readable"), "{error}");
    // A message that is merely long is still kept, shortened, beside it.
    let long = json!({ "itemId": "m1", "operation": "append", "text": "x".repeat(200_000), "final": true, "evidence": { "redacted": false } });
    writer.append("message.delta", long, None).unwrap();
    drop(writer);
    let mission = recover_mission_from_path(&path).unwrap();
    assert_eq!(mission["issues"], json!([]));
    let events = mission["events"].as_array().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["payload"]["operation"], "append");
}
