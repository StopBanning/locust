//! a-background-helper-says-it-is-still-working.test.ts (3 cases): Claude Code's records for an Explore helper sent to
//! the background, read by Rust's own Claude reader into the thread, say what the helper is doing until it reports,
//! then what it reported; a turn that ends with it still out says it did not report.

use super::*;
use locust_core::runtime::claude_events::ClaudeNormalizer;
use serde_json::{Value, json};

fn agent_call() -> Vec<Value> {
    vec![
        json!({"type": "stream_event", "event": {"type": "content_block_start", "index": 0, "content_block": {"type": "tool_use", "id": "toolu_agent", "name": "Agent"}}}),
        json!({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": "toolu_agent", "name": "Agent", "input": {"description": "Survey the test files", "prompt": "List every test file", "subagent_type": "Explore", "run_in_background": true}}]}}),
    ]
}
fn launched() -> Value {
    json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": "toolu_agent", "content": "Async agent launched successfully."}]}})
}
fn started() -> Value {
    json!({"type": "system", "subtype": "task_started", "task_id": "t1", "tool_use_id": "toolu_agent", "is_backgrounded": true, "subagent_type": "Explore", "description": "Survey the test files"})
}
fn progress() -> Value {
    json!({"type": "system", "subtype": "task_progress", "task_id": "t1", "tool_use_id": "toolu_agent", "is_backgrounded": true, "description": "Reading src/app.test.ts", "last_tool_name": "Read"})
}
fn done() -> Value {
    json!({"type": "system", "subtype": "task_notification", "task_id": "t1", "tool_use_id": "toolu_agent", "status": "completed", "summary": "14 test files, all under src/"})
}

/// The helper row's words after its subagent label, its type, and the steps line, for these records in a turn that is
/// still going (`running`) or has completed.
fn helper(records: Vec<Value>, running: bool) -> (String, String, String) {
    let mut claude = ClaudeNormalizer::default();
    let mut events: Vec<Value> = vec![json!({"type": "run.started", "payload": {}})];
    for record in records {
        events.extend(claude.accept(&record).into_iter().map(|e| json!({"type": e.kind, "payload": e.payload})));
    }
    if !running {
        events.push(json!({"type": "run.completed", "payload": {}}));
    }
    let records = events.into_iter().enumerate().map(|(i, mut e)| {
        e["sequence"] = json!(i + 1);
        json!({"recordType": "mission.event", "event": e})
    });
    let mut t = ledger::Transcript::default();
    t.apply(records.collect());
    let finished = !running;
    let (name, status, summary, detail) = t.items.iter().find_map(|i| match i {
        ledger::Item::Tool { name, status, summary, detail } if detail.helper => Some((name, *status, summary, detail)),
        _ => None,
    }).expect("a helper row");
    let label = tool_row_label(name, status, summary, detail, finished);
    let said = label.split_once("subagent · ").map(|(_, rest)| rest.to_owned()).unwrap_or(label.clone());
    let line = locust_core::steps_line::steps_line(&t.items, finished, None, false).text();
    (said, detail.helper_type.clone(), line)
}

fn works_then_reports(first: Vec<Value>) {
    let mut going = agent_call();
    going.extend(first.clone());
    going.push(progress());
    let (said, kind, line) = helper(going.clone(), true);
    assert_eq!(said, "working in the background · Reading src/app.test.ts · last tool Read");
    assert_eq!(kind, "Explore");
    assert!(line.contains("1 working in the background"), "{line}");
    going.push(done());
    let (said, _, line) = helper(going, true);
    assert_eq!(said, "reported back · 14 test files, all under src/");
    assert!(!line.contains("working in the background"), "{line}");
}

// case: works in the background, saying what it is doing, until it reports (${order})
#[test]
fn works_in_the_background_until_it_reports_launch_first() {
    works_then_reports(vec![launched(), started()]);
}

// case: works in the background, saying what it is doing, until it reports (${order})
#[test]
fn works_in_the_background_until_it_reports_notice_first() {
    works_then_reports(vec![started(), launched()]);
}

// case: says it did not report when the turn ended with it still out
#[test]
fn says_it_did_not_report_when_the_turn_ended_with_it_still_out() {
    let mut records = agent_call();
    records.extend([launched(), started()]);
    assert_eq!(helper(records, false).0, "did not report");
}
