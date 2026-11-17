// Carries packages/runtime-adapters/test/a-helpers-calls-are-its-rows-children.test.ts (Electron 4ce2f37a), on its
// recording (fixtures/claude/helper-calls.jsonl, read from parity/events.json).
use super::{Event, claude_events::ClaudeNormalizer, claude_background_calls_tests::recorded};
use serde_json::{Value, json};

const AGENT: &str = "toolu_01MrF76nyTfdvLaECwMtcA7V";
const GLOB: &str = "toolu_0134dcHNk1GH8kL57hFJ7THL";

fn normalize(lines: &[String]) -> Vec<Event> {
    let mut claude = ClaudeNormalizer::default();
    lines.iter().flat_map(|l| claude.accept(&serde_json::from_str(l).unwrap())).collect()
}
fn parent_of(event: &Event) -> Option<&str> {
    event.payload["parentItemId"].as_str()
}
/// An event with what changes run to run taken out (its evidence).
fn shape(event: &Event) -> Value {
    let mut payload = event.payload.clone();
    if let Some(o) = payload.as_object_mut() {
        o.remove("evidence");
    }
    json!({ "type": event.kind, "payload": payload })
}

#[test]
fn a_helpers_own_calls_arrive_as_children_of_its_row_opened_and_closed() {
    let events = normalize(&recorded("claude/helper-calls.jsonl"));
    let children: Vec<&Event> = events.iter().filter(|e| parent_of(e).is_some()).collect();
    let seen: Vec<(String, String, String)> = children
        .iter()
        .map(|e| (e.kind.clone(), e.payload["itemId"].as_str().unwrap_or("").to_owned(), parent_of(e).unwrap().to_owned()))
        .collect();
    assert_eq!(seen, [
        ("tool.started".to_owned(), GLOB.to_owned(), AGENT.to_owned()),
        ("tool.completed".to_owned(), GLOB.to_owned(), AGENT.to_owned()),
    ]);
    assert_eq!(children[0].payload["name"], "Glob");
    assert_eq!(children[0].payload["command"], "**/*");
    assert_eq!(children[0].payload["phase"], "started");
    assert_eq!(children[1].payload["name"], "Glob");
    assert_eq!(children[1].payload["phase"], "completed");
    assert!(children[1].payload["output"].to_string().contains("notes.md"), "{}", children[1].payload);
}

#[test]
fn a_helpers_calls_leave_the_teammates_own_items_as_a_stream_without_them_draws_them() {
    let lines = recorded("claude/helper-calls.jsonl");
    let events = normalize(&lines);
    let own: Vec<&Event> = events.iter().filter(|e| parent_of(e).is_none()).collect();
    let without: Vec<String> = lines
        .iter()
        .filter(|l| serde_json::from_str::<Value>(l).unwrap()["parent_tool_use_id"].as_str().is_none_or(str::is_empty))
        .cloned()
        .collect();
    let without_helper = normalize(&without);
    assert_eq!(own.iter().map(|e| shape(e)).collect::<Vec<_>>(), without_helper.iter().map(shape).collect::<Vec<_>>());
    let tool_ids: Vec<&str> = own.iter().filter(|e| e.kind.starts_with("tool.")).map(|e| e.payload["itemId"].as_str().unwrap_or("")).collect();
    assert_eq!(tool_ids, [AGENT, AGENT, AGENT]);
    let typed = own.iter().find(|e| e.kind == "tool.started" && e.payload.get("command").is_some()).expect("a typed Agent row");
    assert_eq!(typed.payload["name"], "Agent");
    assert_eq!(typed.payload["status"], "Explore");
}

#[test]
fn a_stream_where_no_helper_ran_has_no_helper_children() {
    let events = normalize(&recorded("claude/background-stopped-with-run.jsonl"));
    assert!(!events.is_empty());
    assert!(events.iter().all(|e| parent_of(e).is_none()));
}

// Rust's own, for the path Electron calls "measured not to happen": a helper's call that did stream its start is
// still the helper's.
#[test]
fn a_helpers_call_that_streams_its_start_is_still_the_helpers() {
    let mut claude = ClaudeNormalizer::default();
    let events = claude.accept(&json!({ "type": "stream_event", "parent_tool_use_id": AGENT, "event": { "type": "content_block_start",
        "index": 0, "content_block": { "type": "tool_use", "id": GLOB, "name": "Glob" } } }));
    assert_eq!(events.len(), 1);
    assert_eq!((events[0].kind.as_str(), parent_of(&events[0])), ("tool.started", Some(AGENT)));
}
