//! `packages/runtime-adapters/test/the-steps-show-what-the-runtimes-show.test.ts` (0.489): a step shows what the
//! runtime itself shows. Claude Code's command output rides on the tool's completion (`claude_events.rs`, called by
//! `runtime/claude.rs` for every line Claude Code prints); Codex's app server through `events.rs` `Normalizer::accept`
//! (called by `runtime/codex.rs`). The inputs and expected words are the Electron test's.
use super::{Event, claude_events::ClaudeNormalizer, events::Normalizer};
use serde_json::{Value, json};

fn claude_call(name: &str, content: Value) -> Vec<Event> {
    let mut claude = ClaudeNormalizer::default();
    [
        json!({"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"t1","name":name}}}),
        json!({"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":name,"input":{"command":"npm test"}}]}}),
        json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":content}]}}),
    ]
    .iter()
    .flat_map(|line| claude.accept(line))
    .collect()
}

fn completed_output(events: &[Event]) -> Option<String> {
    let done = events.iter().find(|e| e.kind == "tool.completed").expect("the step completed");
    done.payload.get("output").and_then(Value::as_str).map(str::to_owned)
}

#[test]
fn carries_a_command_s_output_as_claude_code_shows_it_under_the_command() {
    // case: carries a command's output, as Claude Code shows it under the command
    assert_eq!(completed_output(&claude_call("Bash", json!("Tests  6 passed (6)"))).as_deref(), Some("Tests  6 passed (6)"));
}

#[test]
fn reads_output_given_as_text_blocks() {
    let events = claude_call("Grep", json!([{"type":"text","text":"src/a.ts"},{"type":"text","text":"src/b.ts"}]));
    assert_eq!(completed_output(&events).as_deref(), Some("src/a.ts\nsrc/b.ts"));
}

#[test]
fn does_not_carry_a_file_s_text_for_a_read_which_claude_code_never_shows() {
    // case: does not carry a file's text for a Read, which Claude Code never shows
    assert_eq!(completed_output(&claude_call("Read", json!("1\tsecret line"))), None);
    // claudeShowsOutput("Edit") is false and an MCP tool's is true: read off the step each one completes.
    assert_eq!(completed_output(&claude_call("Edit", json!("The file was updated"))), None);
    assert_eq!(
        completed_output(&claude_call("mcp__github__list_issues", json!("#12 open"))).as_deref(),
        Some("#12 open")
    );
}

#[test]
fn scrubs_a_key_in_the_output() {
    let token = "sk-abcdefghijklmnopqrstuv"; // public-guard: allow
    let output = completed_output(&claude_call("Bash", json!(format!("token={token}")))).unwrap_or_default();
    assert!(!output.contains(token), "{output}");
}

#[test]
fn opens_and_closes_a_reasoning_step_around_a_thinking_block_with_no_text() {
    // case: opens and closes a reasoning step around a thinking block, with no text
    // Since Electron 1697b996 (AGENTS.md, "Reasoning a runtime SHOWS its own user is not hidden") Claude Code's shown
    // thinking is kept on the step that closes it (claude_thinking_tests.rs). What still holds of this case: one
    // reasoning step opens and closes around the block, the opening carries no text, and none of it is written as the
    // teammate's reply.
    let mut claude = ClaudeNormalizer::default();
    let events: Vec<Event> = [
        json!({"type":"stream_event","event":{"type":"message_start","message":{"id":"msg_1","usage":{}}}}),
        json!({"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}}),
        json!({"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"private words"}}}),
        json!({"type":"stream_event","event":{"type":"content_block_stop","index":0}}),
    ]
    .iter()
    .flat_map(|line| claude.accept(line))
    .collect();
    let steps: Vec<(&str, &str)> = events
        .iter()
        .filter(|e| (e.kind == "step.started" || e.kind == "step.completed") && e.payload["stepKind"] == "reasoning")
        .map(|e| (e.kind.as_str(), e.payload["stepKind"].as_str().unwrap_or("")))
        .collect();
    assert_eq!(steps, [("step.started", "reasoning"), ("step.completed", "reasoning")]);
    let opened = events.iter().find(|e| e.kind == "step.started").expect("a reasoning step opened");
    assert!(!opened.payload.to_string().contains("private words"), "{}", opened.payload);
    assert!(!events.iter().any(|e| e.kind == "message.delta"), "thinking written as the reply");
}

fn app() -> Normalizer {
    Normalizer::default()
}

fn note(method: &str, item: Value) -> (String, Value) {
    (method.to_owned(), json!({"threadId":"t","turnId":"u","item":item}))
}

fn accept(codex: &mut Normalizer, (method, params): (String, Value)) -> Vec<Event> {
    codex.accept(&method, &params)
}

#[test]
fn carries_a_command_s_output() {
    // case: carries a command's output
    let mut codex = app();
    accept(&mut codex, note("item/started", json!({"type":"commandExecution","id":"c1","command":"npm test"})));
    let events = accept(
        &mut codex,
        note(
            "item/completed",
            json!({"type":"commandExecution","id":"c1","command":"npm test","status":"completed","exitCode":0,"aggregatedOutput":"6 passed"}),
        ),
    );
    assert_eq!(completed_output(&events).as_deref(), Some("6 passed"));
}

#[test]
fn closes_a_reasoning_item_so_thinking_leaves_the_live_line() {
    // case: closes a reasoning item, so 'Thinking' leaves the live line
    let mut codex = app();
    let opened = accept(&mut codex, note("item/started", json!({"type":"reasoning","id":"r1"})));
    let closed = accept(&mut codex, note("item/completed", json!({"type":"reasoning","id":"r1"})));
    assert_eq!(opened.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(), ["step.started"]);
    assert_eq!(closed.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(), ["step.completed"]);
}

#[test]
fn does_not_read_the_person_s_own_message_as_the_teammate_writing() {
    // case: does not read the person's own message as the teammate writing
    let mut codex = app();
    assert!(accept(&mut codex, note("item/started", json!({"type":"userMessage","id":"u1","content":[]}))).is_empty());
}
