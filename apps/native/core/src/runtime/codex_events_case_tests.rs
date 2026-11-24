//! Electron's `packages/runtime-adapters/test/codex-events.test.ts`, case by case. That file tests Electron's normalizer
//! for `codex exec --json` lines; Rust has no exec transport (Codex runs through its app server only), so each case is
//! carried as the same behaviour on the app server's notifications: a Codex turn whose `codex app-server` is the
//! stand-in (codex_stand_in.rs), read through the host's calls (`start`, `next_event`, `stop`) and normalized by
//! events.rs. Electron's exec record → its app-server notification: `thread.started` → the thread `thread/start`
//! answered, `turn.started` → `turn/started`, `item.*` → `item/*` (agent_message → agentMessage, command_execution →
//! commandExecution, mcp_tool_call → mcpToolCall, file_change → fileChange, collab_tool_call → collabAgentToolCall),
//! `turn.completed`'s usage → `thread/tokenUsage/updated`, `error`/`turn.failed` → `error`, an exec item error that
//! is not the end → an `error` that will be retried. Fixed 24 Nov: a line that was not JSON ended the run; a tool's name
//! and status were not held to 512.
use super::codex_stand_in::*;
use super::*;

fn line(method: &str, params: Value) -> (&str, Value) {
    (method, params)
}

fn run(then: &[(&str, Value)]) -> Vec<Event> {
    let (mut codex, _told) = turn_with(then);
    to_the_end(&mut codex)
}

fn started() -> (&'static str, Value) {
    ("turn/started", json!({"threadId": "thread_9", "turn": {"id": "turn_1"}}))
}

fn completed() -> (&'static str, Value) {
    ("turn/completed", json!({"threadId": "thread_9", "turn": {"id": "turn_1", "status": "completed"}}))
}

fn strings(value: &Value, into: &mut Vec<String>) {
    match value {
        Value::String(s) => into.push(s.clone()),
        Value::Array(items) => items.iter().for_each(|v| strings(v, into)),
        Value::Object(map) => map.values().for_each(|v| strings(v, into)),
        _ => {}
    }
}

// case: normalizes the official success shape and waits for a clean host exit
#[test]
fn normalizes_the_success_shape() {
    let events = run(&[
        started(),
        line("item/completed", json!({"threadId": "thread_9", "item": {"id": "item-0", "type": "agentMessage", "text": "Mission complete."}})),
        line("thread/tokenUsage/updated", json!({"threadId": "thread_9", "tokenUsage": {"total": {"inputTokens": 120, "cachedInputTokens": 80, "outputTokens": 12, "cacheWriteInputTokens": 2}, "last": {"inputTokens": 120, "cachedInputTokens": 80, "outputTokens": 12, "cacheWriteInputTokens": 2}}})),
        completed(),
    ]);
    assert_eq!(kinds(&events), ["run.started", "message.delta", "run.completed"]);
    let message = &events[1].payload;
    assert_eq!((&message["itemId"], &message["operation"], &message["text"], &message["final"]), (&json!("item-0"), &json!("replace"), &json!("Mission complete."), &json!(true)));
    let done = &events[2].payload;
    assert_eq!(done["runtimeThreadId"], "thread_9");
    assert_eq!(done["usage"], json!({"inputTokens": 120, "cacheReadTokens": 80, "outputTokens": 12, "cacheWriteTokens": 2}));
    assert_eq!((&done["process"]["exitCode"], &done["process"]["signal"]), (&json!(0), &Value::Null));
}

// case: classifies and deduplicates the observed quota-exhaustion sequence
#[test]
fn classifies_and_deduplicates_quota_exhaustion() {
    let quota = "You've hit your usage limit. Try again later.";
    let events = run(&[
        line("error", json!({"threadId": "thread_9", "willRetry": true, "error": {"message": "nonterminal diagnostic"}})),
        started(),
        line("error", json!({"threadId": "thread_9", "error": {"message": quota, "codexErrorInfo": "usageLimitExceeded"}})),
        line("turn/completed", json!({"threadId": "thread_9", "turn": {"id": "turn_1", "status": "failed", "error": {"message": quota, "codexErrorInfo": "usageLimitExceeded"}}})),
    ]);
    let limits = of(&events, "route.limit_detected");
    assert_eq!(limits.len(), 1);
    assert_eq!((&limits[0].payload["kind"], &limits[0].payload["message"]), (&json!("quota-exhausted"), &json!(quota)));
    assert!(of(&events, "adapter.diagnostic").iter().any(|e| e.payload["terminal"] == false));
    let failed = of(&events, "run.failed");
    assert_eq!(failed.len(), 1);
    assert_eq!((&failed[0].payload["kind"], &failed[0].payload["message"], &failed[0].payload["runtimeTerminal"]), (&json!("quota-exhausted"), &json!(quota), &json!("failed")));
}

// case: keeps an item error nonterminal when the turn later succeeds
#[test]
fn an_error_that_will_be_retried_stays_nonterminal() {
    let events = run(&[
        line("error", json!({"threadId": "thread_9", "willRetry": true, "error": {"message": "You've hit your usage limit. Try again later."}})),
        started(),
        line("item/completed", json!({"threadId": "thread_9", "item": {"id": "answer", "type": "agentMessage", "text": "Recovered safely."}})),
        completed(),
    ]);
    assert_eq!(of(&events, "adapter.diagnostic")[0].payload["terminal"], false);
    assert!(of(&events, "route.limit_detected").is_empty());
    assert!(of(&events, "run.failed").is_empty());
    assert_eq!(events.last().unwrap().kind, "run.completed");
}

// case: preserves unknown/additive provider data after redaction and never stores reasoning text
#[test]
fn keeps_no_secret_and_no_reasoning_text() {
    // Built at run time: no key-shaped literal in the public source.
    let key = format!("{}-abcdefghijklmnopqrstuvwxyz", "sk");
    // The app server's normalizer is a whitelist: an unknown notification adds nothing, so neither its data nor its
    // secrets reach the ledger; a reasoning item keeps only its public summary.
    let events = run(&[
        started(),
        line("future/event", json!({"threadId": "thread_9", "feature": {"enabled": true, "access_token": "very-secret-token-value"}, "api_key": key})),
        line("item/completed", json!({"threadId": "thread_9", "item": {"id": "reasoning-1", "type": "reasoning", "content": ["private chain of thought must never persist"], "text": "also private", "summary": ["Reading the files"], "status": "completed"}})),
        completed(),
    ]);
    let ledger = serde_json::to_string(&events.iter().map(|e| &e.payload).collect::<Vec<_>>()).unwrap();
    for secret in [key.as_str(), "very-secret-token-value", "private chain of thought", "also private"] {
        assert!(!ledger.contains(secret), "{secret}");
    }
    let step = of(&events, "step.completed")[0];
    assert_eq!((&step.payload["itemType"], &step.payload["message"]), (&json!("reasoning"), &json!("Reading the files")));
}

// case: turns malformed JSON into a redacted, nonterminal diagnostic
#[test]
fn malformed_json_is_a_redacted_nonterminal_diagnostic() {
    let key = format!("{}-abcdefghijklmnopqrstuvwxyz", "sk");
    let (mut codex, _told) = stand_in(json!({"turn/start": [
        {"result": {"turn": {"id": "turn_1"}}},
        {"raw": format!("{{\"type\":\"error\",\"api_key={key}\"\n")},
        {"send": {"jsonrpc": "2.0", "method": "turn/completed", "params": {"threadId": "thread_9"}}},
    ]}));
    codex.start(&turn_in(Mode::AcceptEdits)).unwrap();
    let events = to_the_end(&mut codex);
    let said = &events[0];
    assert_eq!(said.kind, "adapter.diagnostic");
    assert_eq!((&said.payload["code"], &said.payload["terminal"]), (&json!("codex.malformed_json"), &json!(false)));
    let text = said.payload.to_string();
    assert!(!text.contains(&key) && text.contains("[redacted]"), "{text}");
    assert_eq!(events.last().unwrap().kind, "run.completed");
}

// case: synthesizes cancellation only from host-owned completion correlation
#[test]
fn cancellation_comes_only_from_the_host() {
    let (mut codex, _told) = stand_in(json!({
        "turn/start": [{"result": {"turn": {"id": "turn_1"}}}, {"send": {"jsonrpc": "2.0", "method": "turn/started", "params": {"threadId": "thread_9", "turn": {"id": "turn_1"}}}}],
        "turn/interrupt": [{"result": {}}, {"exit": 0}],
    }));
    codex.start(&turn_in(Mode::AcceptEdits)).unwrap();
    let before = events_until_end(&mut codex, Duration::from_millis(500));
    assert!(of(&before, "run.cancelled").is_empty());
    codex.stop().unwrap();
    let end = to_the_end(&mut codex).pop().unwrap();
    assert_eq!(end.kind, "run.cancelled");
    assert_eq!((&end.payload["runtimeThreadId"], &end.payload["process"]["exitCode"]), (&json!("thread_9"), &Value::Null));
}

// case: says so when IT stopped the run for output volume
#[test]
fn says_so_when_locust_stopped_the_run_for_output_volume() {
    let (mut codex, _told) = stand_in(json!({"turn/start": [
        {"result": {"turn": {"id": "turn_1"}}},
        {"raw": "x".repeat(8 * 1024 * 1024 + 16)},
        {"sleep": 5},
    ]}));
    codex.start(&turn_in(Mode::AcceptEdits)).unwrap();
    let failed = to_the_end(&mut codex).pop().unwrap();
    assert_eq!(failed.kind, "run.failed");
    assert_eq!((&failed.payload["kind"], &failed.payload["runtimeTerminal"]), (&json!("process-failed"), &json!("missing")));
    assert!(!failed.payload["message"].as_str().unwrap().contains("did not complete successfully"));
    assert_eq!(failed.payload["process"]["outputLimitExceeded"], true);
}

// case: rejects both EOF/exit mismatches instead of claiming success
#[test]
fn an_exit_without_the_turns_end_is_never_success() {
    // On the app server the turn's end is the run's terminal; a process that exits before it never claims success. An
    // exit code after `turn/completed` is not read, so the other exec mismatch has no counterpart here.
    let (mut codex, _told) = stand_in(json!({"turn/start": [
        {"result": {"turn": {"id": "turn_1"}}},
        {"send": {"jsonrpc": "2.0", "method": "turn/started", "params": {"threadId": "thread_9", "turn": {"id": "turn_1"}}}},
        {"exit": 0},
    ]}));
    codex.start(&turn_in(Mode::AcceptEdits)).unwrap();
    let failed = to_the_end(&mut codex).pop().unwrap();
    assert_eq!(failed.kind, "run.failed");
    assert_eq!(failed.payload["runtimeTerminal"], "missing");
}

// case: normalizes message append deltas and accumulated-text replacement
#[test]
fn appends_deltas_and_replaces_with_the_finished_text() {
    let events = run(&[
        line("item/agentMessage/delta", json!({"threadId": "thread_9", "itemId": "message-1", "delta": "Hel"})),
        line("item/agentMessage/delta", json!({"threadId": "thread_9", "itemId": "message-1", "delta": "lo"})),
        line("item/completed", json!({"threadId": "thread_9", "item": {"id": "message-1", "type": "agentMessage", "text": "Corrected"}})),
        completed(),
    ]);
    let said: Vec<(Value, Value, Value)> = of(&events, "message.delta").iter().map(|e| (e.payload["operation"].clone(), e.payload["text"].clone(), e.payload["final"].clone())).collect();
    assert_eq!(said, [
        (json!("append"), json!("Hel"), json!(false)),
        (json!("append"), json!("lo"), json!(false)),
        (json!("replace"), json!("Corrected"), json!(true)),
    ]);
}

// case: emits a tool failure without forcing the whole run to fail
#[test]
fn a_tool_failure_does_not_fail_the_run() {
    let events = run(&[
        started(),
        line("item/started", json!({"threadId": "thread_9", "item": {"id": "command-1", "type": "commandExecution", "command": "rg needle .", "status": "inProgress"}})),
        line("item/completed", json!({"threadId": "thread_9", "item": {"id": "command-1", "type": "commandExecution", "command": "rg needle .", "status": "failed", "exitCode": 2, "aggregatedOutput": "authorization=Bearer abcdefghijklmnop"}})),
        completed(),
    ]);
    let begun = &of(&events, "tool.started")[0].payload;
    assert_eq!((&begun["itemId"], &begun["name"], &begun["command"]), (&json!("command-1"), &json!("shell"), &json!("rg needle .")));
    let failed = of(&events, "tool.failed")[0];
    assert_eq!((&failed.payload["itemId"], &failed.payload["status"], &failed.payload["exitCode"]), (&json!("command-1"), &json!("failed"), &json!(2)));
    assert!(!failed.payload.to_string().contains("abcdefghijklmnop"));
    assert_eq!(events.last().unwrap().kind, "run.completed");
}

// case: does not mistake incidental usage-limit wording for confirmed quota exhaustion
#[test]
fn incidental_usage_limit_words_are_not_quota_exhaustion() {
    let events = run(&[line("error", json!({"threadId": "thread_9", "error": {"message": "Usage limit monitoring is disabled in this build."}}))]);
    assert!(of(&events, "route.limit_detected").is_empty());
    let failed = events.last().unwrap();
    assert_eq!(failed.kind, "run.failed");
    assert_ne!(failed.payload["kind"], "quota-exhausted");
}

// case: clamps identity fields the mission ledger caps at 512 characters
#[test]
fn clamps_identity_fields_to_512() {
    let events = run(&[
        line("item/completed", json!({"threadId": "thread_9", "item": {"id": "i".repeat(900), "type": "mcpToolCall", "server": "s".repeat(400), "tool": "t".repeat(400), "status": "u".repeat(700)}})),
        completed(),
    ]);
    let tools: Vec<&Event> = events.iter().filter(|e| e.kind == "tool.completed" || e.kind == "tool.failed").collect();
    let tool = &tools[0].payload;
    for field in ["itemId", "name", "status"] {
        assert!(tool[field].as_str().unwrap().chars().count() <= 512, "{field}");
    }
    assert!(tool["itemId"].as_str().unwrap().starts_with("iii"));
    assert!(tool["name"].as_str().unwrap().starts_with("sss"));
}

// case: strips NUL from every persisted string
#[test]
fn strips_nul_from_every_persisted_string() {
    let events = run(&[
        line("item/completed", json!({"threadId": "thread_9", "item": {"id": "answer\0", "type": "agentMessage", "text": "Durable\0 result"}})),
        line("item/completed", json!({"threadId": "thread_9", "item": {"id": "tool\0", "type": "mcpToolCall", "server": "s\0", "tool": "t", "status": "completed\0"}})),
        completed(),
    ]);
    let mut all = vec![];
    for e in &events {
        strings(&e.payload, &mut all);
    }
    assert!(!all.is_empty());
    assert_eq!(all.iter().filter(|s| s.contains('\0')).collect::<Vec<_>>(), Vec::<&String>::new());
}

// case: carries every changed path on the tool event, one per line, since Codex sends no diff
#[test]
fn carries_every_changed_path_one_per_line() {
    let events = run(&[
        line("item/completed", json!({"threadId": "thread_9", "item": {"id": "item_1", "type": "fileChange", "status": "completed", "changes": [{"path": "/w/README.md", "kind": {"type": "update"}}, {"path": "/w/src/prices.ts", "kind": {"type": "update"}}]}})),
        completed(),
    ]);
    assert_eq!(of(&events, "tool.completed")[0].payload["command"], "/w/README.md\n/w/src/prices.ts");
}

// case: draws spawn_agent as a tool call named by its tool, carrying the prompt, settled on completion
#[test]
fn draws_a_sub_agent_as_a_tool_named_by_its_tool_settled_on_completion() {
    // The app server names it spawnAgent; Electron's app-server normalizer carries no prompt for it, and neither does Rust.
    let item = json!({"id": "item_1", "type": "collabAgentToolCall", "tool": "spawnAgent", "prompt": "Check the latest Alphabet stock price.", "status": "inProgress"});
    let mut done = item.clone();
    done["status"] = json!("completed");
    let events = run(&[
        line("item/started", json!({"threadId": "thread_9", "item": item})),
        line("item/completed", json!({"threadId": "thread_9", "item": done})),
        completed(),
    ]);
    let begun = &of(&events, "tool.started")[0].payload;
    assert_eq!((&begun["itemId"], &begun["toolKind"], &begun["name"]), (&json!("item_1"), &json!("collabAgentToolCall"), &json!("subagent:spawnAgent")));
    let settled = &of(&events, "tool.completed")[0].payload;
    assert_eq!((&settled["itemId"], &settled["name"], &settled["status"]), (&json!("item_1"), &json!("subagent:spawnAgent"), &json!("completed")));
}
