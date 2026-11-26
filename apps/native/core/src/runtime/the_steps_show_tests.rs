//! the-steps-show-what-the-runtimes-show.test.ts (8 cases): what a step printed is carried as Claude Code and Codex show
//! it, thinking is a timed step, and Codex's live line is not fooled by a reasoning item or the person's own message.
//! Claude's cases go through `ClaudeNormalizer::accept` as runtime/claude.rs feeds it (`records::claude_record`);
//! Codex's through the app-server `events::Normalizer::accept` as runtime/codex.rs does. Expected values are Electron's.
use super::*;
use crate::runtime::Event;
use serde_json::{Value, json};

fn claude_record(n: &mut ClaudeNormalizer, value: Value) -> Vec<Event> {
    n.accept(&crate::runtime::records::claude_record(&value.to_string()))
}

/// Electron's claudeCall: a tool streamed open, its call, then its result with `content`.
fn claude_call(name: &str, content: Value) -> Vec<Event> {
    let mut n = ClaudeNormalizer::default();
    let mut events = claude_record(&mut n, json!({"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"t1","name":name}}}));
    events.extend(claude_record(&mut n, json!({"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":name,"input":{"command":"npm test"}}]}})));
    events.extend(claude_record(&mut n, json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":content}]}})));
    events
}

fn completed(events: &[Event]) -> &Event {
    events.iter().find(|e| e.kind == "tool.completed").expect("a tool.completed")
}

// Claude: what a step printed

#[test]
fn carries_a_commands_output_as_claude_code_shows_it_under_the_command() {
    assert_eq!(completed(&claude_call("Bash", json!("Tests  6 passed (6)"))).payload["output"], "Tests  6 passed (6)");
}

#[test]
fn reads_output_given_as_text_blocks() {
    let done = claude_call("Grep", json!([{"type":"text","text":"src/a.ts"},{"type":"text","text":"src/b.ts"}]));
    assert_eq!(completed(&done).payload["output"], "src/a.ts\nsrc/b.ts");
}

#[test]
fn does_not_carry_a_files_text_for_a_read_which_claude_code_never_shows() {
    assert!(completed(&claude_call("Read", json!("1\tsecret line"))).payload.get("output").is_none());
    assert!(!claude_shows_output(Some("Edit")));
    assert!(claude_shows_output(Some("mcp__github__list_issues")));
}

#[test]
fn scrubs_a_key_in_the_output() {
    let output = completed(&claude_call("Bash", json!("token=sk-abcdefghijklmnopqrstuv"))).payload["output"].as_str().unwrap_or_default().to_owned();
    assert!(!output.contains("sk-abcdefghijklmnopqrstuv"), "{output}");
}

// Claude: thinking, timed

/// case: opens and closes a reasoning step around a thinking block, with no text
/// The step opens and closes around the block as Electron's does. "With no text" is 0.489's rule, which this checkout's
/// Electron still has; AGENTS.md (Colin's call) and the newer claude-thinking-is-kept-for-its-thinking-row rule the port
/// follows keep Claude Code's SHOWN thinking for its thinking row, scrubbed and bounded: it rides the closing step as
/// its message. The opening step and every event's evidence still carry no words.
#[test]
fn opens_and_closes_a_reasoning_step_around_a_thinking_block() {
    let mut n = ClaudeNormalizer::default();
    let mut events = claude_record(&mut n, json!({"type":"stream_event","event":{"type":"message_start","message":{"id":"msg_1","usage":{}}}}));
    events.extend(claude_record(&mut n, json!({"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}})));
    events.extend(claude_record(&mut n, json!({"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"private words"}}})));
    events.extend(claude_record(&mut n, json!({"type":"stream_event","event":{"type":"content_block_stop","index":0}})));
    let steps: Vec<&Event> = events.iter().filter(|e| (e.kind == "step.started" || e.kind == "step.completed") && e.payload["stepKind"] == "reasoning").collect();
    assert_eq!(steps.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(), ["step.started", "step.completed"]);
    assert!(!steps[0].payload.to_string().contains("private words"), "{}", steps[0].payload);
    assert_eq!(steps[1].payload["message"], "private words", "kept for the thinking row (AGENTS.md)");
    // The evidence a step keeps is cut to the block's type (withoutThinkingEvidence).
    for e in &events {
        assert!(!e.payload["evidence"].to_string().contains("private words"), "{}", e.payload);
    }
}

// Codex's app server

fn app() -> crate::runtime::events::Normalizer {
    crate::runtime::events::Normalizer::default()
}

/// case: carries a command's output
#[test]
fn carries_a_commands_output_from_codexs_app_server() {
    let mut codex = app();
    codex.accept("item/started", &json!({"threadId":"t","turnId":"u","item":{"type":"commandExecution","id":"c1","command":"npm test"}}));
    let events = codex.accept("item/completed", &json!({"threadId":"t","turnId":"u","item":{"type":"commandExecution","id":"c1","command":"npm test","status":"completed","exitCode":0,"aggregatedOutput":"6 passed"}}));
    assert_eq!(completed(&events).payload["output"], "6 passed");
}

#[test]
fn closes_a_reasoning_item_so_thinking_leaves_the_live_line() {
    let mut codex = app();
    let opened = codex.accept("item/started", &json!({"threadId":"t","turnId":"u","item":{"type":"reasoning","id":"r1"}}));
    let closed = codex.accept("item/completed", &json!({"threadId":"t","turnId":"u","item":{"type":"reasoning","id":"r1"}}));
    assert_eq!(opened.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(), ["step.started"]);
    assert_eq!(closed.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(), ["step.completed"]);
}

#[test]
fn does_not_read_the_persons_own_message_as_the_teammate_writing() {
    let mut codex = app();
    assert!(codex.accept("item/started", &json!({"threadId":"t","turnId":"u","item":{"type":"userMessage","id":"u1","content":[]}})).is_empty());
}

// Claude: which tools print what they printed (claude-events.ts claudeShowsOutput, as Electron's list has it)

#[test]
fn shows_the_output_of_the_eight_named_tools_and_every_mcp_tool_and_of_no_other() {
    for name in ["Bash", "BashOutput", "PowerShell", "Grep", "Glob", "WebFetch", "WebSearch", "LS", "mcp__github__list_issues"] {
        assert!(claude_shows_output(Some(name)), "{name}");
        assert_eq!(completed(&claude_call(name, json!("printed"))).payload["output"], "printed", "{name}");
    }
    for name in ["Read", "Edit", "Write", "MultiEdit", "NotebookEdit", "TodoWrite", "Task"] {
        assert!(!claude_shows_output(Some(name)), "{name}");
    }
    assert!(!claude_shows_output(None));
}

#[test]
fn a_helpers_own_summary_is_its_output_not_what_the_tool_result_printed() {
    // claude-events.ts: `subagentSummary !== undefined` -> the summary is the output and the printed text is not read.
    let mut n = ClaudeNormalizer::default();
    let mut events = claude_record(&mut n, json!({"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"t1","name":"Grep"}}}));
    events.extend(claude_record(&mut n, json!({"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":"Grep","input":{"pattern":"cart"}}]}})));
    events.extend(claude_record(&mut n, json!({"type":"system","subtype":"task_notification","task_id":"k1","tool_use_id":"t1","status":"completed","summary":"Found the cart code in three files."})));
    events.extend(claude_record(&mut n, json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":"src/a.ts\nsrc/b.ts"}]}})));
    assert_eq!(completed(&events).payload["output"], "Found the cart code in three files.");
}
