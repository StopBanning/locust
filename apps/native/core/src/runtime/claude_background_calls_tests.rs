// Carries packages/runtime-adapters/test/a-backgrounded-call-says-so.test.ts and
// background-work-ends-with-the-run.test.ts (Electron 4ce2f37a), on the recording they use
// (fixtures/claude/background-stopped-with-run.jsonl, read from parity/events.json).
use super::{
    Event,
    claude_events::{ClaudeNormalizer, background_ending, claude_tool_backgrounded},
};
use serde_json::{Value, json};

const CALL: &str = "toolu_016MCbGpcf7wkMy2eHrDb6LC";

pub(super) fn recorded(fixture: &str) -> Vec<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../parity/events.json");
    let goldens: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let streams = goldens.as_array().cloned().unwrap_or_else(|| goldens["streams"].as_array().cloned().unwrap());
    let stream = streams.iter().find(|s| s["fixture"] == fixture).unwrap_or_else(|| panic!("{fixture} in events.json"));
    stream["lines"].as_array().unwrap().iter().map(|l| l.as_str().unwrap().to_owned()).collect()
}

fn replay(lines: &[String]) -> Vec<Event> {
    let mut claude = ClaudeNormalizer::default();
    lines.iter().filter(|l| !l.trim().is_empty()).flat_map(|l| claude.accept(&serde_json::from_str(l).unwrap())).collect()
}

fn steps<'a>(events: &'a [Event], kind: &str) -> Vec<&'a Value> {
    events.iter().filter(|e| e.kind == kind).map(|e| &e.payload).collect()
}

/// A Bash call in the three records Claude Code sends for one: opened, its input filled in, closed by its result.
fn bash_call(input: Value) -> Vec<Event> {
    let mut claude = ClaudeNormalizer::default();
    let mut events = claude.accept(&json!({ "type": "stream_event", "event": { "type": "content_block_start", "index": 0,
        "content_block": { "type": "tool_use", "id": "tool_1", "name": "Bash" } } }));
    events.extend(claude.accept(&json!({ "type": "assistant", "message": { "content": [{ "type": "tool_use", "id": "tool_1", "name": "Bash", "input": input }] } })));
    events.extend(claude.accept(&json!({ "type": "user", "message": { "content": [{ "type": "tool_result", "tool_use_id": "tool_1", "content": "ok" }] } })));
    events
}

fn finished(events: &[Event]) -> Value {
    events.iter().find(|e| e.kind == "tool.completed" || e.kind == "tool.failed").expect("a finished tool").payload.clone()
}

#[test]
fn a_backgrounded_bash_call_is_read_off_its_input() {
    assert!(claude_tool_backgrounded("Bash", &json!({ "command": "pnpm test", "run_in_background": true })));
    assert!(!claude_tool_backgrounded("Bash", &json!({ "command": "pnpm test" })));
    assert!(!claude_tool_backgrounded("Bash", &json!({ "command": "pnpm test", "run_in_background": false })));
}

#[test]
fn backgrounded_is_invented_on_no_other_tool() {
    assert!(!claude_tool_backgrounded("Read", &json!({ "file_path": "a.ts", "run_in_background": true })));
    assert!(!claude_tool_backgrounded("Task", &json!({ "run_in_background": true })));
}

#[test]
fn a_backgrounded_call_carries_it_onto_the_finished_tool_event() {
    let done = finished(&bash_call(json!({ "command": "pnpm test", "run_in_background": true })));
    assert_eq!(done["background"], true);
    assert!(done["command"].as_str().unwrap_or("").contains("pnpm test"), "{done}");
}

#[test]
fn an_ordinary_bash_call_says_nothing_about_the_background() {
    let done = finished(&bash_call(json!({ "command": "pnpm test" })));
    assert!(done.get("background").is_none(), "{done}");
}

fn recording() -> Vec<String> {
    recorded("claude/background-stopped-with-run.jsonl")
}

#[test]
fn background_work_is_keyed_by_the_call_that_started_it_not_filed_as_a_subagent() {
    let events = replay(&recording());
    let all: Vec<&Value> = ["step.started", "step.completed", "step.failed"].iter().flat_map(|k| steps(&events, k)).collect();
    assert!(!all.iter().any(|s| s["itemId"].as_str().is_some_and(|i| i.starts_with("subagent:"))));
    let began = steps(&events, "step.started").into_iter().find(|s| s["itemType"] == "background").expect("a background step");
    assert_eq!(began["stepKind"], "item");
    assert_eq!(began["itemId"], CALL);
    assert_eq!(began["status"], "running");
    assert_eq!(began["message"], "Background task: sleep 8 seconds then write to out.txt");
}

#[test]
fn background_work_says_it_was_stopped_with_the_run_after_the_runs_answer() {
    let events = replay(&recording());
    let ended = steps(&events, "step.completed").into_iter().find(|s| s["itemType"] == "background").expect("an ended step");
    assert_eq!(ended["stepKind"], "item");
    assert_eq!(ended["itemId"], CALL);
    assert_eq!(ended["status"], "stopped-with-run");
    let result_at = events.iter().position(|e| e.kind == "message.delta" && e.payload["final"] == true).expect("a final answer");
    let ended_at = events.iter().position(|e| e.kind == "step.completed" && e.payload["itemType"] == "background").unwrap();
    assert!(ended_at > result_at, "{ended_at} > {result_at}");
}

#[test]
fn a_background_call_carries_the_flag_its_command_and_what_it_was_for() {
    let events = replay(&recording());
    let call = events.iter().find(|e| e.kind == "tool.completed" && e.payload["itemId"] == CALL).expect("the call").payload.clone();
    assert_eq!(call["name"], "Bash");
    assert_eq!(call["command"], "sleep 8 && echo finished > out.txt");
    assert_eq!(call["title"], "Background task: sleep 8 seconds then write to out.txt");
    assert_eq!(call["background"], true);
}

#[test]
fn the_restated_background_task_list_opens_nothing() {
    let events = replay(&recording());
    assert!(!steps(&events, "step.started").iter().any(|s| s["status"] == "background_tasks_changed"));
}

#[test]
fn background_work_that_finishes_while_the_run_is_going_says_completed() {
    let lines: Vec<String> = recording().into_iter().filter(|l| !l.trim().is_empty()).collect();
    let notice = lines.iter().position(|l| l.contains("\"task_notification\"")).unwrap();
    let result = lines.iter().position(|l| l.contains("\"type\": \"result\"") || l.contains("\"type\":\"result\"")).unwrap();
    assert!(notice > result);
    let finished = lines[notice].replacen("\"status\": \"stopped\"", "\"status\": \"completed\"", 1);
    let mut reordered = lines[..result].to_vec();
    reordered.push(finished);
    reordered.extend_from_slice(&lines[result..notice]);
    let events = replay(&reordered);
    let ended = steps(&events, "step.completed").into_iter().find(|s| s["itemType"] == "background").expect("an ended step");
    assert_eq!(ended["status"], "completed");
}

#[test]
fn a_helper_that_was_not_backgrounded_is_still_a_helper() {
    let mut claude = ClaudeNormalizer::default();
    let started = claude.accept(&json!({ "type": "system", "subtype": "task_started", "task_id": "a1", "tool_use_id": "toolu_agent",
        "subagent_type": "Explore", "description": "Find the config" }));
    assert_eq!(started[0].kind, "step.started");
    assert_eq!(started[0].payload["itemId"], "subagent:a1");
    assert_eq!(started[0].payload["itemType"], "subagent");
}

#[test]
fn how_background_work_ended_tells_the_run_ending_apart_from_a_chosen_stop() {
    assert_eq!(background_ending(Some("stopped"), true), "stopped-with-run");
    assert_eq!(background_ending(Some("killed"), true), "stopped-with-run");
    assert_eq!(background_ending(Some("stopped"), false), "stopped");
    assert_eq!(background_ending(Some("completed"), true), "completed");
    assert_eq!(background_ending(Some("failed"), false), "failed");
    assert_eq!(background_ending(None, true), "ended");
}
