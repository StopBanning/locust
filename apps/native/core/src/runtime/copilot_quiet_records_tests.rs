// Carries packages/runtime-adapters/test/copilots-search-index-record-is-not-news.test.ts and
// copilots-new-records-are-not-news.test.ts (Electron 4ce2f37a): Copilot's housekeeping records say nothing.
use super::{Event, copilot_events::CopilotNormalizer};
use serde_json::{Value, json};

fn fed(records: &[Value]) -> Vec<Event> {
    let mut copilot = CopilotNormalizer::default();
    records.iter().flat_map(|r| copilot.accept_line(&r.to_string())).collect()
}

#[test]
fn copilots_search_index_record_says_nothing_and_keeps_nothing() {
    let events = fed(&[json!({ "type": "session.indexed_search",
        "data": { "query": "what this project is", "results": ["README.md: A scratch project"] } })]);
    assert!(events.is_empty(), "{:?}", events.iter().map(|e| &e.payload).collect::<Vec<_>>());
}

#[test]
fn copilot_1_0_95s_new_records_say_nothing() {
    let events = fed(&[
        json!({ "type": "model.call_final_result", "data": { "model": "gpt-6-luna", "isByok": false, "result": "success" }, "ephemeral": true, "id": "a", "timestamp": "2026-10-10T12:44:33.447Z" }),
        json!({ "type": "session.background_tasks_changed", "data": {}, "ephemeral": true, "id": "b", "timestamp": "2026-10-10T12:44:34.306Z" }),
        json!({ "type": "tool.shell_output", "data": { "toolCallId": "call_1", "stream": "stdout", "text": "locust-hook-check", "sequence": 0 }, "ephemeral": true, "id": "c", "timestamp": "2026-10-10T12:44:34.519Z" }),
        json!({ "type": "tool.execution_partial_result", "data": { "toolCallId": "call_1", "partialOutput": "locust-hook-check\n" }, "ephemeral": true, "id": "d", "timestamp": "2026-10-10T12:44:34.534Z" }),
    ]);
    assert!(events.is_empty(), "{:?}", events.iter().map(|e| &e.payload).collect::<Vec<_>>());
}

#[test]
fn copilot_still_says_so_when_a_model_call_failed_or_a_background_notice_carries_something() {
    let events = fed(&[
        json!({ "type": "model.call_final_result", "data": { "model": "gpt-6-luna", "isByok": false, "result": "error" }, "ephemeral": true, "id": "a", "timestamp": "2026-10-10T12:44:33.447Z" }),
        json!({ "type": "session.background_tasks_changed", "data": { "tasks": [{ "id": "shell_0", "status": "running" }] }, "ephemeral": true, "id": "b", "timestamp": "2026-10-10T12:44:34.306Z" }),
    ]);
    let codes: Vec<_> = events.iter().map(|e| e.payload["code"].as_str().unwrap_or("")).collect();
    assert_eq!(codes, ["copilot.unknown_event", "copilot.background_tasks_changed"]);
}
