//! an-opencode-run-that-edited-is-not-run-again.test.ts (1 case): a recorded OpenCode run that wrote a file, its
//! step_finish lines dropped and its process ending 1, reads as failed, says no run.started, and is still not taken
//! for a run that never started (`thread_view::runtime_never_started`), so nothing offers to run it again.
use super::Normalizer;
use serde_json::{json, Value};

fn recorded(fixture: &str) -> Vec<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../parity/events.json");
    let goldens: Value = serde_json::from_str(&std::fs::read_to_string(root).unwrap()).unwrap();
    let streams = goldens.as_array().cloned().or_else(|| goldens["streams"].as_array().cloned()).unwrap_or_else(|| goldens.as_object().unwrap().values().cloned().collect());
    let stream = streams.iter().find(|s| s["fixture"] == fixture).unwrap_or_else(|| panic!("{fixture} in events.json"));
    stream["lines"].as_array().unwrap().iter().map(|l| l.as_str().unwrap().to_owned()).collect()
}

// case: is not taken for one that never started
#[test]
fn a_run_that_wrote_and_failed_is_not_taken_for_one_that_never_started() {
    let mut n = Normalizer::new(None);
    let mut events = Vec::new();
    for line in recorded("opencode/write-mode-read-and-write.jsonl").iter().filter(|l| !l.trim().is_empty() && !l.contains("\"step_finish\"")) {
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            events.extend(n.accept(&v));
        }
    }
    events.extend(n.finish(false, Some(1), "", false));
    let records: Vec<Value> = events
        .iter()
        .map(|e| {
            let mut r = json!({"type": e.kind, "payload": e.payload});
            if let Some(thread) = &e.thread {
                r["runtimeThreadId"] = json!(thread);
            }
            r
        })
        .collect();
    assert!(records.iter().any(|r| r["type"] == "tool.completed"), "it wrote a file");
    assert_eq!(records.last().unwrap()["type"], "run.failed");
    assert!(!records.iter().any(|r| r["type"] == "run.started"));
    assert!(!crate::thread_view::runtime_never_started(&records), "a run that wrote is never offered to simply run again");
    // Each event names OpenCode's session, which alone counts as a start; the tool calls must count on their own too.
    assert!(records.iter().all(|r| r.get("runtimeThreadId").is_some()));
    let unnamed: Vec<Value> = records.iter().cloned().map(|mut r| { r.as_object_mut().unwrap().remove("runtimeThreadId"); r }).collect();
    assert!(!crate::thread_view::runtime_never_started(&unnamed), "the file it wrote is a start, session or not");
}

