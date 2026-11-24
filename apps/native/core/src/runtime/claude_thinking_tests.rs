//! claude-thinking-is-kept-for-its-thinking-row.test.ts (Electron 1697b996): Claude Code's shown thinking is kept on
//! the step that closes it, scrubbed of secrets and bounded as Cursor's is; redacted thinking, signatures and a
//! helper's thinking never are.
use super::{Event, claude_events::ClaudeNormalizer, events::text};
use serde_json::{Value, json};

fn stream(event: Value) -> Value {
    json!({"type":"stream_event","event":event,"parent_tool_use_id":null})
}
fn helper(mut v: Value) -> Value {
    v["parent_tool_use_id"] = json!("helper_call");
    v
}
fn start(index: i64, kind: &str, thinking: &str) -> Value {
    stream(json!({"type":"content_block_start","index":index,"content_block":{"type":kind,"thinking":thinking,"data":"OPAQUE REDACTED DATA","signature":"OPAQUE SIGNATURE"}}))
}
fn delta(thinking: &str, index: i64) -> Value {
    stream(json!({"type":"content_block_delta","index":index,"delta":{"type":"thinking_delta","thinking":thinking}}))
}
fn stop(index: i64) -> Value {
    stream(json!({"type":"content_block_stop","index":index}))
}
fn whole(thinking: &str, id: &str) -> Value {
    json!({"type":"assistant","message":{"id":id,"content":[{"type":"thinking","thinking":thinking,"signature":"OPAQUE SIGNATURE"}]}})
}
fn run(values: &[Value]) -> Vec<Event> {
    let mut n = ClaudeNormalizer::default();
    values.iter().flat_map(|v| n.accept(v)).collect()
}
fn thoughts(events: &[Event]) -> Vec<&Event> {
    events.iter().filter(|e| e.kind == "step.completed" && e.payload["stepKind"] == "reasoning").collect()
}
fn messages(events: &[Event]) -> Vec<Option<String>> {
    thoughts(events).iter().map(|e| e.payload["message"].as_str().map(str::to_owned)).collect()
}
fn started(events: &[Event]) -> usize {
    events.iter().filter(|e| e.kind == "step.started" && e.payload["stepKind"] == "reasoning").count()
}
fn all(events: &[Event]) -> String {
    events.iter().map(|e| e.payload.to_string()).collect()
}

#[test]
fn keeps_shown_streamed_thinking_on_the_closing_step_including_an_initial_fragment() {
    // case: keeps shown streamed thinking on the closing step, including an initial fragment
    let events = run(&[start(0, "thinking", "First "), delta("inspect the ", 0), delta("configuration.", 0), stop(0)]);
    assert_eq!(messages(&events), [Some("First inspect the configuration.".to_owned())]);
    assert_eq!(started(&events), 1);
    assert!(!events.iter().any(|e| e.kind == "message.delta"));
    assert!(events[0].payload.get("message").is_none());
}

#[test]
fn keeps_a_whole_shown_thinking_block_when_partial_output_was_not_sent() {
    // case: keeps a whole shown thinking block when partial output was not sent
    assert_eq!(messages(&run(&[whole("Inspect the configuration.", "msg_1")])), [Some("Inspect the configuration.".to_owned())]);
}

#[test]
fn uses_the_whole_block_before_its_stop_without_repeating_a_streamed_thought_or_its_later_echo() {
    // case: uses the whole block before its stop, without repeating a streamed thought or its later echo
    let events = run(&[
        stream(json!({"type":"message_start","message":{"id":"msg_1"}})),
        start(2, "thinking", ""),
        delta("Partial.", 2),
        whole("The whole shown thought.", "msg_1"),
        stop(2),
        whole("The whole shown thought.", "msg_1"),
    ]);
    assert_eq!(messages(&events), [Some("The whole shown thought.".to_owned())]);
    assert_eq!(started(&events), 1);
}

#[test]
fn scrubs_secrets_from_streamed_and_whole_thinking_including_a_key_split_between_deltas() {
    // case: scrubs secrets from streamed and whole thinking, including a key split between deltas
    let token = "sk-abcdefghijklmnopqrstuv"; // public-guard: allow
    let streamed = vec![start(0, "thinking", ""), delta("Check sk-abc", 0), delta("defghijklmnopqrstuv safely.", 0), stop(0)];
    let whole_block = vec![whole(&format!("Check {token} safely."), "msg_1")];
    for values in [streamed, whole_block] {
        let events = run(&values);
        let said = messages(&events)[0].clone().unwrap();
        assert!(said.contains("[redacted]"), "{said}");
        assert!(said.contains("safely."), "{said}");
        assert!(!all(&events).contains(token));
        assert!(!all(&events).contains("OPAQUE SIGNATURE"));
    }
}

#[test]
fn bounds_the_thinking_message_exactly_as_cursor_does_while_keeping_both_ends() {
    // case: bounds the thinking message exactly as Cursor does while keeping both ends
    let said = format!("Start. {} The end.", "x".repeat(30_000));
    let bounded = Some(text(&said, 16_384));
    assert_eq!(messages(&run(&[start(0, "thinking", ""), delta(&said, 0), stop(0)])), [bounded.clone()]);
    assert_eq!(messages(&run(&[whole(&said, "msg_1")])), [bounded.clone()]);
    let kept = bounded.unwrap();
    assert_eq!(kept.encode_utf16().count(), 16_384);
    assert!(kept.starts_with("Start. ") && kept.ends_with(" The end."));
}

#[test]
fn times_redacted_thinking_without_retaining_its_text_opaque_data_or_signature() {
    // case: times redacted thinking without retaining its text, opaque data or signature
    let events = run(&[
        start(0, "redacted_thinking", ""),
        delta("WORDS NEVER SHOWN", 0),
        stop(0),
        json!({"type":"assistant","message":{"id":"redacted","content":[{"type":"redacted_thinking","data":"OPAQUE REDACTED DATA"}]}}),
    ]);
    assert_eq!(messages(&events), [None, None]);
    for hidden in ["WORDS NEVER SHOWN", "OPAQUE REDACTED DATA", "OPAQUE SIGNATURE"] {
        assert!(!all(&events).contains(hidden), "{hidden}");
    }
}

#[test]
fn keeps_helper_thinking_out_of_the_teammate_row_and_leaves_the_teammates_open_thought_intact() {
    // case: keeps helper thinking out of the teammate row and leaves the teammate’s open thought intact
    let events = run(&[
        start(0, "thinking", ""),
        delta("The teammate’s thought.", 0),
        helper(start(0, "thinking", "")),
        helper(delta("HELPER THOUGHT", 0)),
        helper(stop(0)),
        helper(whole("HELPER THOUGHT", "msg_1")),
        delta(" Still thinking.", 0),
        stop(0),
    ]);
    assert_eq!(messages(&events), [Some("The teammate’s thought. Still thinking.".to_owned())]);
    assert!(!all(&events).contains("HELPER THOUGHT"));
}

#[test]
fn replays_captured_empty_thinking_as_timed_steps_without_inventing_words() {
    // case: replays captured empty thinking as timed steps without inventing words
    let lines = super::claude_background_calls_tests::recorded("claude/edit-and-write.jsonl");
    let values: Vec<Value> = lines.iter().filter(|l| !l.trim().is_empty()).map(|l| serde_json::from_str(l).unwrap()).collect();
    let events = run(&values);
    assert_eq!(messages(&events), [None, None, None]);
    assert!(events.iter().any(|e| e.kind == "tool.completed"));
}
