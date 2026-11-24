//! Electron's `packages/runtime-adapters/test/claude-events.test.ts`, case by case, through the caller: the Claude Code
//! runtime (runtime/claude.rs) parses each stream-json line with `records::claude_record`, hands it to
//! `ClaudeNormalizer::accept`, fills `evidence` with what the process came to and asks `finish` for the terminal event.
//! Electron's free helpers (`summarizeInit`, `claudeToolTarget`, `limitKindFor`, `limitSentence`, `usageWindowText`,
//! `runtimeCommandsFrom`) are read here off the events `accept` makes, beside the helper where Electron calls it alone.
use super::*;
use crate::{ledger_write::LedgerWriter, store::Store};

const SIGNED_OUT: &str = "Claude Code is not signed in. Open a terminal, run claude, and sign in: with your Claude account, or with an Anthropic Console account to pay with an API key. Then send again. Locust never hands an API key from your environment to an agent; to use a Claude API key directly, add it in Settings, Your own models.";
const COMPACTED: &str = "The conversation outgrew the model's context, so Claude Code summarized it and carried on from the summary.";
const COMPACTED_ON_REQUEST: &str = "Claude Code summarized the conversation so far, as asked, and carries on from the summary.";

fn normalizer() -> ClaudeNormalizer {
    ClaudeNormalizer::default()
}

/// A record as the runtime reads it off the wire (claude.rs: `records::claude_record`, then `accept`).
fn record(n: &mut ClaudeNormalizer, value: Value) -> Vec<Event> {
    n.accept(&super::super::records::claude_record(&value.to_string()))
}

/// Electron's `completion(overrides)`: what the process came to, then the terminal event.
fn finish(n: &mut ClaudeNormalizer, exit_code: Option<i32>, cancelled: bool) -> Event {
    n.evidence.exit_code = exit_code;
    n.finish(cancelled, "")
}

fn init() -> Value {
    json!({
        "type": "system",
        "subtype": "init",
        "cwd": "C:\\work",
        "session_id": "33b21513-a4fb-478d-ad84-21cb7ef30571",
        "tools": ["Glob", "Grep", "Read"],
        "mcp_servers": [
            { "name": "claude.ai Gmail", "status": "connected" },
            { "name": "claude.ai Robinhood", "status": "connected" },
        ],
        "model": "claude-opus-5",
        "permissionMode": "plan",
        "slash_commands": ["deep-research", "verify", "debug"],
        "claude_code_version": "2.1.252",
        "messaging_socket_path": "\\\\.\\pipe\\LOCAL\\cc-msg-474d737129a0",
        "powershell_path": "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
        "agents": ["claude", "Explore"],
        "skills": ["deep-research"],
    })
}

fn delta(index: i64, text: &str) -> Value {
    json!({"type":"stream_event","event":{"type":"content_block_delta","index":index,"delta":{"type":"text_delta","text":text}}})
}

fn tool_start(id: &str, name: &str) -> Value {
    json!({"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":id,"name":name}}})
}

fn tool_result(id: &str, is_error: bool, content: &str) -> Value {
    json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":id,"is_error":is_error,"content":content}]}})
}

fn limit_info(info: Value) -> Value {
    json!({"type":"rate_limit_event","rate_limit_info":info})
}

fn diagnostics(events: &[Event]) -> Vec<String> {
    events.iter().filter(|e| e.kind == "adapter.diagnostic").map(|e| e.payload["message"].as_str().unwrap().to_owned()).collect()
}

fn item_id(e: &Event) -> String {
    e.payload["itemId"].as_str().unwrap().to_owned()
}

// ---- trap 2: system/init is an environment dump

#[test]
fn keeps_only_allow_listed_fields_and_replaces_name_lists_with_counts() {
    // case: keeps only allow-listed fields and replaces name lists with counts
    let events = record(&mut normalizer(), init());
    let summary = &events[0].payload["evidence"]["raw"];
    for (key, value) in [
        ("session_id", json!("33b21513-a4fb-478d-ad84-21cb7ef30571")),
        ("model", json!("claude-opus-5")),
        ("permissionMode", json!("plan")),
        ("claude_code_version", json!("2.1.252")),
        ("toolCount", json!(3)),
        ("mcpServerCount", json!(2)),
        ("slashCommandCount", json!(3)),
    ] {
        assert_eq!(summary[key], value, "{key}");
    }
}

#[test]
fn lets_none_of_the_machine_description_reach_the_event() {
    // case: lets none of the machine description reach the event
    let events = record(&mut normalizer(), init());
    let serialized = events.iter().map(|e| e.payload.to_string()).collect::<String>();
    for word in ["Gmail", "Robinhood", "pipe", "powershell.exe", "deep-research", "Explore"] {
        assert!(!serialized.contains(word), "{word}");
    }
}

#[test]
fn still_reports_the_session_id_as_the_runtime_thread() {
    // case: still reports the session id as the runtime thread
    let mut claude = normalizer();
    let events = record(&mut claude, init());
    assert_eq!(events[0].kind, "run.started");
    assert_eq!(claude.thread.as_deref(), Some("33b21513-a4fb-478d-ad84-21cb7ef30571"));
}

// ---- trap 1: the answer arrives twice

#[test]
fn appends_deltas_and_replaces_on_the_complete_assistant_message() {
    // case: appends deltas and REPLACES on the complete assistant message
    let mut claude = normalizer();
    record(&mut claude, init());
    let deltas: Vec<Event> = ["P", "RO", "BE_OK"].iter().flat_map(|t| record(&mut claude, delta(0, t))).collect();
    assert_eq!(deltas.len(), 3);
    for e in &deltas {
        assert_eq!(e.payload["operation"], "append");
    }
    let complete = record(&mut claude, json!({"type":"assistant","message":{"content":[{"type":"text","text":"PROBE_OK"}]}}));
    assert_eq!(complete[0].payload["operation"], "replace");
    assert_eq!(complete[0].payload["final"], true);
    assert_eq!(complete[0].payload["text"], "PROBE_OK");
}

#[test]
fn replaces_the_block_the_text_actually_streamed_into_not_an_assumed_block_0() {
    // case: replaces the block the text actually streamed into, not an assumed block_0
    let mut claude = normalizer();
    record(&mut claude, init());
    let streamed = record(&mut claude, delta(1, "PROBE_OK"));
    assert!(item_id(&streamed[0]).ends_with("block_1"), "{}", item_id(&streamed[0]));
    let complete = record(&mut claude, json!({"type":"assistant","message":{"content":[{"type":"text","text":"PROBE_OK"}]}}));
    assert_eq!(item_id(&complete[0]), item_id(&streamed[0]));
    assert_eq!(complete[0].payload["operation"], "replace");
}

#[test]
fn gives_every_message_its_own_item_so_a_later_one_never_replaces_an_earlier_one() {
    // case: gives every message its own item, so a later one never replaces an earlier one
    let mut claude = normalizer();
    let mut say = |id: &str, text: &str| {
        record(&mut claude, json!({"type":"stream_event","parent_tool_use_id":null,"event":{"type":"message_start","message":{"id":id,"usage":{"input_tokens":2}}}}));
        let streamed = record(&mut claude, json!({"type":"stream_event","parent_tool_use_id":null,"event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":text}}}));
        let complete = record(&mut claude, json!({"type":"assistant","parent_tool_use_id":null,"message":{"id":id,"content":[{"type":"text","text":text}]}}));
        assert_eq!(item_id(&streamed[0]), item_id(&complete[0]));
        item_id(&streamed[0])
    };
    let first = say("msg_a", "I'll run these one at a time.");
    let second = say("msg_b", "The subagent is still working.");
    assert_ne!(first, second);
}

#[test]
fn gives_each_message_its_own_item_when_the_run_does_not_stream() {
    // case: gives each message its own item when the run does not stream
    let mut claude = normalizer();
    let a = record(&mut claude, json!({"type":"assistant","message":{"id":"msg_a","content":[{"type":"text","text":"First."}]}}));
    let b = record(&mut claude, json!({"type":"assistant","message":{"id":"msg_b","content":[{"type":"text","text":"Second."}]}}));
    assert_ne!(item_id(&a[0]), item_id(&b[0]));
}

#[test]
fn never_speaks_a_subagents_words_as_the_teammates() {
    // case: never speaks a subagent's words as the teammate's
    let mut claude = normalizer();
    assert!(record(&mut claude, json!({"type":"assistant","parent_tool_use_id":"toolu_agent","message":{"id":"msg_sub","content":[{"type":"text","text":"The file contains 5 lines."}]}})).is_empty());
    assert!(record(&mut claude, json!({"type":"stream_event","parent_tool_use_id":"toolu_agent","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"5"}}})).is_empty());
}

#[test]
fn never_ends_a_subagents_own_tool_as_a_nameless_call_of_the_teammates_0_543_run_01() {
    // case: never ends a subagent's own tool as a nameless call of the teammate's (0.543, RUN-01)
    let mut claude = normalizer();
    record(&mut claude, json!({"type":"assistant","parent_tool_use_id":"toolu_agent","message":{"id":"msg_sub","content":[{"type":"tool_use","id":"toolu_sub_read","name":"Read","input":{"file_path":"one.txt"}}]}}));
    let ended = record(&mut claude, json!({"type":"user","parent_tool_use_id":"toolu_agent","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_sub_read","content":"a"}]}}));
    assert_eq!(ended[0].kind, "tool.completed");
    assert_eq!(ended[0].payload["itemId"], "toolu_sub_read");
    assert_eq!(ended[0].payload["name"], "Read");
    assert_eq!(ended[0].payload["command"], "one.txt");
    assert_eq!(ended[0].payload["parentItemId"], "toolu_agent");
    assert!(record(&mut claude, json!({"type":"user","parent_tool_use_id":"toolu_agent","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_never_opened","content":"b"}]}})).is_empty());
}

// ---- an activity row can say what a tool touched

#[test]
fn carries_the_file_a_tool_acted_on_which_only_arrives_once_the_block_is_complete() {
    // case: carries the file a tool acted on, which only arrives once the block is complete
    let mut claude = normalizer();
    record(&mut claude, init());
    record(&mut claude, tool_start("toolu_1", "Read"));
    record(&mut claude, json!({"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_1","name":"Read","input":{"file_path":"C:/w/src/cli.js"}}]}}));
    let done = record(&mut claude, tool_result("toolu_1", false, "ok"));
    assert_eq!(done[0].kind, "tool.completed");
    assert_eq!(done[0].payload["name"], "Read");
    assert_eq!(done[0].payload["command"], "C:/w/src/cli.js");
}

#[test]
fn reads_each_tools_own_idea_of_a_target() {
    // case: reads each tool's own idea of a target
    let target = |name: &str, input: Value| {
        // The call opens with its name alone; its input arrives with the complete message, which names the target.
        let mut claude = normalizer();
        record(&mut claude, tool_start("t", name));
        let events = record(&mut claude, json!({"type":"assistant","message":{"id":"m","content":[{"type":"tool_use","id":"t","name":name,"input":input}]}}));
        events.iter().find(|e| e.kind == "tool.started").and_then(|e| e.payload["command"].as_str().map(str::to_owned))
    };
    assert_eq!(target("Bash", json!({"command":"npm test"})).as_deref(), Some("npm test"));
    assert_eq!(target("Glob", json!({"pattern":"src/**/*.js"})).as_deref(), Some("src/**/*.js"));
    assert_eq!(target("Write", json!({"file_path":"src/format.js"})).as_deref(), Some("src/format.js"));
    assert_eq!(target("Read", json!({})), None);
    assert_eq!(claude_tool_target("Read", &Value::Null), None);
}

// ---- trap 3: a rate-limit warning is not exhaustion

fn limit_kind_said(status: &str) -> Option<String> {
    let events = record(&mut normalizer(), limit_info(json!({"status":status,"rateLimitType":"seven_day","resetsAt":1788764400})));
    events.iter().find(|e| e.kind == "route.limit_detected").map(|e| e.payload["kind"].as_str().unwrap().to_owned())
}

#[test]
fn maps_allowed_warning_to_a_temporary_limit() {
    // case: maps allowed_warning to a temporary limit
    assert_eq!(limit_kind_said("allowed_warning").as_deref(), Some("temporary-rate-limit"));
    assert_eq!(limit_kind_said("allowed"), None);
    assert_eq!(limit_kind_said("rejected").as_deref(), Some("quota-exhausted"));
    assert_eq!(limit_kind_said("blocked").as_deref(), Some("quota-exhausted"));
}

#[test]
fn reads_resetsat_as_epoch_seconds_not_milliseconds() {
    // case: reads resetsAt as epoch seconds, not milliseconds
    let said = |resets: Value| {
        let events = record(&mut normalizer(), limit_info(json!({"status":"rejected","rateLimitType":"seven_day","resetsAt":resets})));
        events.iter().find(|e| e.kind == "route.limit_detected").unwrap().payload["message"].as_str().unwrap().to_owned()
    };
    assert_eq!(said(json!(1788764400)), "7-day window · resets 2026-09-07T07:00:00.000Z");
    assert!(!said(json!(1788764400000_i64)).contains("resets 2026"));
    assert_eq!(said(json!("nope")), "7-day window");
}

#[test]
fn emits_a_limit_event_carrying_the_window_and_status_in_words() {
    // case: emits a limit event carrying the window and status, in words
    let events = record(
        &mut normalizer(),
        limit_info(json!({"status":"allowed_warning","rateLimitType":"seven_day","utilization":0.31,"resetsAt":1788764400})),
    );
    assert_eq!(events[0].kind, "route.limit_detected");
    assert_eq!(events[0].payload["kind"], "temporary-rate-limit");
    assert_eq!(events[0].payload["message"], "You've used 31% of your 7-day window · resets 2026-09-07T07:00:00.000Z");
}

#[test]
fn keeps_a_warnings_figures_as_a_usage_reading_too_beside_the_notice() {
    // case: keeps a warning's figures as a usage reading too, beside the notice
    let events = record(
        &mut normalizer(),
        limit_info(json!({"status":"allowed_warning","rateLimitType":"seven_day","utilization":0.79,"resetsAt":1788764400})),
    );
    // In counting order: the writer numbers them as they come, so the notice is N and the reading N+1.
    assert_eq!(events.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(), ["route.limit_detected", "adapter.diagnostic"]);
    assert_eq!(events[1].payload["code"], "claude.usage_window");
    assert_eq!(events[1].payload["message"], "7-day window 79% used · resets 2026-09-07T07:00:00.000Z");
}

#[test]
fn prefers_every_window_a_warning_carries_over_its_own_one() {
    // case: prefers every window a warning carries over its own one
    let events = record(
        &mut normalizer(),
        limit_info(json!({"status":"allowed_warning","rateLimitType":"seven_day","utilization":0.79,"resetsAt":1788764400,
            "unifiedWindows":{"seven_day":{"utilization":0.79,"resetsAt":1788764400},"five_hour":{"utilization":0.4,"resetsAt":1788764400}}})),
    );
    let reading = &diagnostics(&events)[0];
    assert!(crate::invariants::regex(r"7-day window 79% used.* · 5-hour window 40% used").is_match(reading), "{reading}");
}

#[test]
fn words_a_limit_from_the_windows_own_reading_when_the_record_has_no_top_level_figure_and_a_rejection_without_the_warning() {
    // case: words a limit from the window's own reading when the record has no top-level figure, and a rejection without the warning
    let said = |info: Value| {
        let events = record(&mut normalizer(), limit_info(info));
        events.iter().find(|e| e.kind == "route.limit_detected").unwrap().payload["message"].as_str().unwrap().to_owned()
    };
    assert_eq!(
        said(json!({"status":"allowed_warning","rateLimitType":"five_hour","resetsAt":1788764400,"unifiedWindows":{"five_hour":{"utilization":0.92,"resetsAt":1788764400}}})),
        "You've used 92% of your 5-hour window · resets 2026-09-07T07:00:00.000Z"
    );
    assert_eq!(said(json!({"status":"allowed_warning","rateLimitType":"seven_day"})), "Your 7-day window is running low");
    assert_eq!(said(json!({"status":"rejected","rateLimitType":"seven_day_opus"})), "7-day opus window");
    assert_eq!(said(json!({"status":"rejected"})), "usage window");
}

#[test]
fn says_nothing_about_a_snapshot_that_reports_the_request_was_allowed() {
    // case: says nothing about a snapshot that reports the request was allowed
    assert!(record(&mut normalizer(), limit_info(json!({"status":"allowed","rateLimitType":"five_hour","resetsAt":1788764400}))).is_empty());
    assert!(record(&mut normalizer(), limit_info(json!({"rateLimitType":"five_hour"}))).is_empty());
}

#[test]
fn still_reports_an_approaching_limit_and_a_rejection() {
    // case: still reports an approaching limit and a rejection
    assert_eq!(limit_kind_said("allowed_warning").as_deref(), Some("temporary-rate-limit"));
    assert_eq!(limit_kind_said("rejected").as_deref(), Some("quota-exhausted"));
}

// ---- trap 4: NUL and length at the origin

fn strings(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(s) => out.push(s.clone()),
        Value::Array(items) => items.iter().for_each(|v| strings(v, out)),
        Value::Object(map) => map.values().for_each(|v| strings(v, out)),
        _ => {}
    }
}

#[test]
fn strips_nul_from_provider_text_before_it_can_reach_the_ledger() {
    // case: strips NUL from provider text before it can reach the ledger
    let events = record(&mut normalizer(), json!({"type":"assistant","message":{"content":[{"type":"text","text":"clean\u{0}text"}]}}));
    let mut all = vec![];
    for e in &events {
        strings(&e.payload, &mut all);
    }
    assert!(all.iter().all(|s| !s.contains('\0')));
    assert!(all.iter().any(|s| s.contains("cleantext")));
}

#[test]
fn clamps_an_over_long_tool_identity() {
    // case: clamps an over-long tool identity
    let events = record(&mut normalizer(), tool_start(&"x".repeat(4000), &"y".repeat(4000)));
    assert_eq!(events[0].kind, "tool.started");
    assert!(events[0].payload["itemId"].as_str().unwrap().chars().count() <= 512);
    assert!(events[0].payload["name"].as_str().unwrap().chars().count() <= 512);
}

// ---- tool pairing and terminal state

fn with_open_tool() -> ClaudeNormalizer {
    let mut claude = normalizer();
    record(&mut claude, tool_start("toolu_1", "Read"));
    claude
}

#[test]
fn pairs_a_tool_result_to_its_start_by_block_id() {
    // case: pairs a tool result to its start by block id
    let done = record(&mut with_open_tool(), json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_1","is_error":false}]}}));
    assert_eq!(done[0].kind, "tool.completed");
    assert_eq!(done[0].payload["name"], "Read");
}

#[test]
fn reports_an_errored_tool_result_as_failed() {
    // case: reports an errored tool result as failed
    let failed = record(&mut with_open_tool(), json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_1","is_error":true}]}}));
    assert_eq!(failed[0].kind, "tool.failed");
}

#[test]
fn says_a_signed_out_claude_code_once_in_words_a_person_can_act_on_as_a_sign_in_failure() {
    // case: says a signed-out Claude Code once, in words a person can act on, as a sign-in failure
    let mut claude = normalizer();
    let stand_in = record(
        &mut claude,
        json!({"type":"assistant","error":"authentication_failed","message":{"id":"m1","model":"<synthetic>","role":"assistant","content":[{"type":"text","text":"Not logged in · Please run /login"}]}}),
    );
    assert!(stand_in.is_empty());
    record(&mut claude, json!({"type":"result","subtype":"success","is_error":true,"result":"Not logged in · Please run /login"}));
    let terminal = finish(&mut claude, Some(1), false);
    assert_eq!(terminal.kind, "run.failed");
    assert_eq!(terminal.payload["kind"], "authentication-failed");
    assert_eq!(terminal.payload["message"], SIGNED_OUT);
}

#[test]
fn completes_only_when_the_provider_actually_said_it_finished() {
    // case: completes only when the provider actually said it finished
    let mut claude = normalizer();
    record(&mut claude, json!({"type":"result","subtype":"success","is_error":false,"result":"ok"}));
    assert_eq!(finish(&mut claude, Some(0), false).kind, "run.completed");
}

#[test]
fn says_what_the_run_was_refused_permission_to_do_on_an_otherwise_successful_result() {
    // case: says what the run was refused permission to do, on an otherwise successful result
    let mut claude = normalizer();
    let events = record(
        &mut claude,
        json!({"type":"result","subtype":"success","is_error":false,"terminal_reason":"completed",
            "result":"It looks like running that command requires your approval",
            "permission_denials":[{"tool_name":"Bash","tool_use_id":"toolu_01","tool_input":{"command":"node t.mjs","description":"Run t.mjs"}}]}),
    );
    let said = events.iter().find(|e| e.kind == "adapter.diagnostic").unwrap();
    assert_eq!(said.payload["level"], "warning");
    let message = said.payload["message"].as_str().unwrap();
    assert!(message.contains("Bash") && message.contains("node t.mjs"), "{message}");
    assert_eq!(finish(&mut claude, Some(0), false).kind, "run.completed");
}

#[test]
fn stays_quiet_when_nothing_was_refused() {
    // case: stays quiet when nothing was refused
    let events = record(&mut normalizer(), json!({"type":"result","subtype":"success","is_error":false,"result":"ok","permission_denials":[]}));
    assert!(events.iter().all(|e| e.kind != "adapter.diagnostic"));
}

#[test]
fn refuses_to_call_a_clean_exit_a_success_when_no_result_record_arrived() {
    // case: refuses to call a clean exit a success when no result record arrived
    let terminal = finish(&mut normalizer(), Some(0), false);
    assert_eq!(terminal.kind, "run.failed");
    assert_eq!(terminal.payload["runtimeTerminal"], "missing");
}

#[test]
fn reports_a_provider_error_result_as_a_failure() {
    // case: reports a provider error result as a failure
    let mut claude = normalizer();
    record(&mut claude, json!({"type":"result","subtype":"error_max_turns","is_error":true,"result":"too many turns"}));
    let terminal = finish(&mut claude, Some(0), false);
    assert_eq!(terminal.kind, "run.failed");
    assert!(terminal.payload["message"].as_str().unwrap().contains("too many turns"));
}

#[test]
fn reports_cancellation_as_cancelled_not_failed() {
    // case: reports cancellation as cancelled, not failed
    assert_eq!(finish(&mut normalizer(), Some(0), true).kind, "run.cancelled");
}

#[test]
fn stamps_every_event_with_the_claude_adapter() {
    // case: stamps every event with the claude adapter
    // The normalizer carries no adapter of its own: the run's writer stamps every event it appends with the mission's.
    let root = std::env::temp_dir().join(format!("locust-claude-stamp-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("native")).unwrap();
    std::fs::create_dir_all(root.join("electron")).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    let mut writer = LedgerWriter::create_runtime(&store, "mission_1", "p", "m", "claude", "claude", &[], None).unwrap();
    let mut claude = normalizer();
    let mut events = record(&mut claude, init());
    events.push(finish(&mut claude, Some(0), true));
    let written: Vec<Value> = events.into_iter().map(|e| writer.append(&e.kind, e.payload, e.thread.as_deref()).unwrap()).collect();
    assert_eq!(written.len(), 2);
    assert!(written.iter().all(|e| e["sourceAdapter"] == "claude"));
    assert!(written.iter().all(|e| e["runId"] == written[0]["runId"] && e["runId"].as_str().is_some_and(|r| !r.is_empty())));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn survives_a_malformed_record_without_throwing() {
    // case: survives a malformed record without throwing
    let mut claude = normalizer();
    let events = claude.accept(&super::super::records::claude_record("{not json"));
    assert_eq!(events[0].kind, "adapter.diagnostic");
    assert_eq!(events[0].payload["code"], "claude.malformed_record");
    assert!(!claude.ended);
    // and the run goes on: the next record is read as ever.
    assert_eq!(record(&mut claude, init())[0].kind, "run.started");
}

// ---- what the run cost, as Claude Code priced it

fn completed_with(result: Value) -> Event {
    let mut claude = normalizer();
    record(&mut claude, result);
    finish(&mut claude, Some(0), false)
}

#[test]
fn names_the_model_an_alias_turned_out_to_mean() {
    // case: names the model an alias turned out to mean
    let done = completed_with(json!({"type":"result","subtype":"success","is_error":false,"usage":{"input_tokens":10,"output_tokens":47},
        "modelUsage":{"claude-sonnet-5":{"canonicalModel":"claude-sonnet-5","contextWindow":1000000}}}));
    assert_eq!(done.payload["resolvedModel"], "claude-sonnet-5");
}

#[test]
fn names_none_when_a_run_used_more_than_one_model() {
    // case: names none when a run used more than one model
    let done = completed_with(json!({"type":"result","subtype":"success","is_error":false,"usage":{"input_tokens":10,"output_tokens":47},
        "modelUsage":{"claude-sonnet-5":{"canonicalModel":"claude-sonnet-5"},"claude-haiku-4-5-20251001":{"canonicalModel":"claude-haiku-4-5-20251001"}}}));
    assert!(done.payload.get("resolvedModel").is_none());
}

#[test]
fn carries_the_models_own_context_window_so_a_reading_has_a_real_denominator() {
    // case: carries the model's own context window, so a reading has a real denominator
    let done = completed_with(json!({"type":"result","subtype":"success","is_error":false,
        "usage":{"input_tokens":2,"output_tokens":5,"cache_read_input_tokens":23997,"cache_creation_input_tokens":15080},
        "modelUsage":{"claude-sonnet-5":{"contextWindow":1000000,"maxOutputTokens":64000}}}));
    let usage = &done.payload["usage"];
    assert_eq!(usage["contextWindow"], 1000000);
    assert_eq!(usage["cacheReadTokens"], 23997);
    assert_eq!(usage["cacheWriteTokens"], 15080);
}

#[test]
fn reports_no_window_when_the_result_names_none() {
    // case: reports no window when the result names none
    let done = completed_with(json!({"type":"result","subtype":"success","is_error":false,"usage":{"input_tokens":10,"output_tokens":47},
        "modelUsage":{"some-model":{"costUSD":0.01}}}));
    assert!(done.payload["usage"].get("contextWindow").is_none());
}

#[test]
fn carries_the_dollar_figure_and_the_token_counts_from_the_result_record_onto_the_receipt() {
    // case: carries the dollar figure and the token counts from the result record onto the receipt
    let done = completed_with(json!({"type":"result","subtype":"success","is_error":false,"total_cost_usd":0.0297808,
        "usage":{"input_tokens":10,"output_tokens":47,"cache_read_input_tokens":17648},
        "modelUsage":{"claude-haiku-4-5-20251001":{"costUSD":0.0297808}}}));
    assert_eq!(done.kind, "run.completed");
    assert_eq!(done.payload["usage"], json!({"usd":0.0297808,"inputTokens":10,"outputTokens":47,"cacheReadTokens":17648}));
    assert!(!done.payload.to_string().contains("modelUsage"));
}

#[test]
fn says_nothing_about_cost_when_the_result_record_carried_none() {
    // case: says nothing about cost when the result record carried none
    let done = completed_with(json!({"type":"result","subtype":"success","is_error":false}));
    assert!(done.payload.get("usage").is_none());
}

// ---- how full the conversation is, and who paid for the run

fn call(id: &str, cache_write: u64, cache_read: u64, parent: Option<&str>) -> Value {
    json!({"type":"stream_event","parent_tool_use_id":parent,"event":{"type":"message_start","message":{"id":id,"model":"claude-opus-5-5",
        "usage":{"input_tokens":2,"cache_creation_input_tokens":cache_write,"cache_read_input_tokens":cache_read,"output_tokens":1}}}})
}
fn ended(output: u64) -> Value {
    json!({"type":"stream_event","parent_tool_use_id":null,"event":{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":output}}})
}
fn window(overage: bool) -> Value {
    limit_info(json!({"status":"allowed","resetsAt":1790141400,"rateLimitType":"five_hour","overageStatus":"rejected","isUsingOverage":overage,
        "unifiedWindows":{"five_hour":{"utilization":0.65,"resetsAt":1790141400},"seven_day":{"utilization":0.45,"resetsAt":1790578800}}}))
}
fn big_result() -> Value {
    json!({"type":"result","subtype":"success","is_error":false,"total_cost_usd":3.06,
        "usage":{"input_tokens":46,"output_tokens":31000,"cache_read_input_tokens":4796000,"cache_creation_input_tokens":151697},
        "modelUsage":{"claude-opus-5-5":{"contextWindow":1000000}}})
}
fn receipt(claude: &mut ClaudeNormalizer) -> Value {
    finish(claude, Some(0), false).payload["usage"].clone()
}

#[test]
fn reads_the_context_off_the_last_call_never_off_the_runs_sum() {
    // case: reads the context off the last call, never off the run's sum
    let mut claude = normalizer();
    for v in [call("msg_1", 191_411, 0, None), ended(59), call("msg_2", 3_333, 191_411, None), ended(54), call("msg_3", 2_347, 233_702, None), ended(706), big_result()] {
        record(&mut claude, v);
    }
    let usage = receipt(&mut claude);
    assert_eq!(usage["contextTokens"], 2 + 2_347 + 233_702 + 706);
    assert_eq!(usage["cacheReadTokens"], 4_796_000);
    assert_eq!(usage["contextWindow"], 1_000_000);
}

#[test]
fn leaves_a_subagents_calls_out_its_context_is_its_own() {
    // case: leaves a subagent's calls out: its context is its own
    let mut claude = normalizer();
    for v in [
        call("msg_1", 2_347, 233_702, None),
        ended(706),
        call("msg_sub", 40_000, 0, Some("toolu_agent")),
        json!({"type":"assistant","parent_tool_use_id":"toolu_agent","message":{"id":"msg_sub","content":[],"usage":{"input_tokens":5,"cache_read_input_tokens":90000,"output_tokens":300}}}),
        big_result(),
    ] {
        record(&mut claude, v);
    }
    assert_eq!(receipt(&mut claude)["contextTokens"], 2 + 2_347 + 233_702 + 706);
}

#[test]
fn reads_the_complete_messages_usage_when_the_run_does_not_stream() {
    // case: reads the complete message's usage when the run does not stream
    let mut claude = normalizer();
    record(&mut claude, json!({"type":"assistant","parent_tool_use_id":null,"message":{"id":"msg_1","content":[{"type":"text","text":"a"}],"usage":{"input_tokens":2,"cache_creation_input_tokens":916,"cache_read_input_tokens":204276,"output_tokens":59}}}));
    record(&mut claude, json!({"type":"assistant","parent_tool_use_id":null,"message":{"id":"msg_2","content":[{"type":"text","text":"b"}],"usage":{"input_tokens":2,"cache_creation_input_tokens":1783,"cache_read_input_tokens":212934,"output_tokens":54}}}));
    record(&mut claude, big_result());
    assert_eq!(receipt(&mut claude)["contextTokens"], 2 + 1_783 + 212_934 + 54);
}

#[test]
fn says_no_context_at_all_when_no_call_stated_one() {
    // case: says no context at all when no call stated one
    let mut claude = normalizer();
    record(&mut claude, big_result());
    assert!(receipt(&mut claude).get("contextTokens").is_none());
}

#[test]
fn marks_a_run_a_subscriptions_windows_covered_and_keeps_claude_codes_dollar_figure_as_it_wrote_it() {
    // case: marks a run a subscription's windows covered, and keeps Claude Code's dollar figure as it wrote it
    let mut claude = normalizer();
    record(&mut claude, call("msg_1", 2_347, 233_702, None));
    record(&mut claude, window(false));
    record(&mut claude, big_result());
    let usage = receipt(&mut claude);
    assert_eq!(usage["billing"], "subscription");
    assert_eq!(usage["usd"], 3.06);
}

#[test]
fn does_not_where_any_of_the_run_spent_paid_extra_usage_or_where_no_window_was_reported() {
    // case: does not, where any of the run spent paid extra usage, or where no window was reported
    let mut extra = normalizer();
    record(&mut extra, window(false));
    record(&mut extra, window(true));
    record(&mut extra, big_result());
    assert!(receipt(&mut extra).get("billing").is_none());
    let mut keyed = normalizer();
    let mut keyed_init = init();
    keyed_init["apiKeySource"] = json!("none");
    record(&mut keyed, keyed_init);
    record(&mut keyed, big_result());
    assert!(receipt(&mut keyed).get("billing").is_none());
}

// ---- the signals Claude Code gives a person about what is happening

#[test]
fn passes_a_notification_through_as_a_diagnostic_immediate_ones_as_warnings_without_the_ctrl_o_hint() {
    // case: passes a notification through as a diagnostic, immediate ones as warnings, without the ctrl+o hint
    let mut n = normalizer();
    let events = record(&mut n, json!({"type":"system","subtype":"notification","key":"stop-hook-error","text":"Stop hook error occurred · ctrl+o to see","priority":"immediate"}));
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, "adapter.diagnostic");
    assert_eq!(events[0].payload["code"], "claude.notification");
    assert_eq!(events[0].payload["level"], "warning");
    assert_eq!(events[0].payload["message"], "Stop hook error occurred");
    assert_eq!(events[0].payload["terminal"], false);
    let quiet = record(&mut n, json!({"type":"system","subtype":"notification","text":"Plugins updated","priority":"normal"}));
    assert_eq!(quiet[0].payload["level"], "info");
    assert_eq!(quiet[0].payload["message"], "Plugins updated");
}

#[test]
fn follows_a_subagents_life_as_steps_and_hands_its_type_and_summary_to_the_agent_call_that_started_it() {
    // case: follows a subagent's life as steps, and hands its type and summary to the Agent call that started it
    let mut n = normalizer();
    record(&mut n, json!({"type":"system","subtype":"init","session_id":"s1"}));
    record(&mut n, tool_start("toolu_1", "Agent"));
    let started = record(&mut n, json!({"type":"system","subtype":"task_started","task_id":"t1","tool_use_id":"toolu_1","description":"Count README.md lines","subagent_type":"Explore","task_type":"local_agent"}));
    assert_eq!(started[0].kind, "step.started");
    assert_eq!(started[0].payload["stepKind"], "item");
    assert_eq!(started[0].payload["itemId"], "subagent:t1");
    assert_eq!(started[0].payload["itemType"], "subagent");
    assert_eq!(started[0].payload["message"], "Explore · Count README.md lines");
    let progress = record(&mut n, json!({"type":"system","subtype":"task_progress","task_id":"t1","tool_use_id":"toolu_1","description":"Reading README.md","subagent_type":"Explore","last_tool_name":"Read","usage":{"total_tokens":4092,"tool_uses":1}}));
    assert_eq!(progress[0].kind, "step.started");
    assert_eq!(progress[0].payload["itemId"], "subagent:t1");
    assert_eq!(progress[0].payload["message"], "Explore · Reading README.md · last tool Read");
    assert!(record(&mut n, json!({"type":"system","subtype":"task_updated","task_id":"t1","patch":{"status":"completed"}})).is_empty());
    assert!(record(&mut n, json!({"type":"system","subtype":"thinking_tokens","estimated_tokens":50,"estimated_tokens_delta":50})).is_empty());
    let done = record(&mut n, json!({"type":"system","subtype":"task_notification","task_id":"t1","tool_use_id":"toolu_1","status":"completed","summary":"3","usage":{"total_tokens":6787,"tool_uses":1}}));
    assert_eq!(done[0].kind, "step.completed");
    assert_eq!(done[0].payload["itemId"], "subagent:t1");
    assert_eq!(done[0].payload["status"], "completed");
    let result = record(&mut n, tool_result("toolu_1", false, "3"));
    assert_eq!(result[0].kind, "tool.completed");
    assert_eq!(result[0].payload["itemId"], "toolu_1");
    assert_eq!(result[0].payload["name"], "Agent");
    assert_eq!(result[0].payload["status"], "Explore");
    assert_eq!(result[0].payload["output"], "3");
}

#[test]
fn marks_a_subagent_that_failed_as_a_failed_step_and_the_launchers_failure_stays_the_runtimes_word() {
    // case: marks a subagent that failed as a failed step, and the launcher's failure stays the runtime's word
    let mut n = normalizer();
    record(&mut n, tool_start("toolu_2", "Agent"));
    let failed = record(&mut n, json!({"type":"system","subtype":"task_notification","task_id":"t2","tool_use_id":"toolu_2","status":"failed","summary":"no such file"}));
    assert_eq!(failed[0].kind, "step.failed");
    assert_eq!(failed[0].payload["itemId"], "subagent:t2");
    let result = record(&mut n, tool_result("toolu_2", true, "boom"));
    assert_eq!(result[0].kind, "tool.failed");
    assert_eq!(result[0].payload["status"], "error");
    assert_eq!(result[0].payload["output"], "no such file");
}

// ---- a running tool's heartbeat

#[test]
fn is_not_a_thread_item_and_never_an_unhandled_record() {
    // case: is not a thread item, and never an unhandled record
    let mut n = normalizer();
    record(&mut n, tool_start("toolu_9", "Bash"));
    assert!(record(&mut n, json!({"type":"tool_progress","tool_use_id":"toolu_9","tool_name":"Bash","parent_tool_use_id":null,"elapsed_time_seconds":31,"heartbeat":true,"session_id":"s1","uuid":"u1"})).is_empty());
    assert!(record(&mut n, json!({"type":"tool_progress","tool_use_id":"toolu_9","tool_name":"Agent","parent_tool_use_id":null,"elapsed_time_seconds":0,"subagent_type":"Explore","subagent_retry":{"agent_id":"a1","attempt":2,"max_retries":10},"session_id":"s1","uuid":"u2"})).is_empty());
    let result = record(&mut n, tool_result("toolu_9", false, "ok"));
    assert_eq!(result[0].kind, "tool.completed");
    assert_eq!(result[0].payload["itemId"], "toolu_9");
    assert_eq!(result[0].payload["name"], "Bash");
}

// ---- a still-allowed rate limit is a usage window

#[test]
fn words_the_windows_fullest_first_with_when_they_reset() {
    // case: words the windows, fullest first, with when they reset
    let events = record(
        &mut normalizer(),
        limit_info(json!({"status":"allowed","rateLimitType":"five_hour","resetsAt":1788660600,
            "unifiedWindows":{"five_hour":{"utilization":0.35,"resetsAt":1788660600},"seven_day":{"utilization":0.5,"resetsAt":1788764400}}})),
    );
    let reading = &diagnostics(&events)[0];
    assert!(
        crate::invariants::regex(r"^7-day window 50% used · resets .+ · 5-hour window 35% used · resets .+$").is_match(reading),
        "{reading}"
    );
    assert_eq!(usage_window_text(&json!({})), None);
    assert_eq!(usage_window_text(&json!("no")), None);
}

#[test]
fn becomes_a_usage_window_diagnostic_not_a_limit_while_the_status_is_allowed() {
    // case: becomes a usage_window diagnostic, not a limit, while the status is allowed
    let events = record(
        &mut normalizer(),
        limit_info(json!({"status":"allowed","rateLimitType":"five_hour","resetsAt":1788660600,"unifiedWindows":{"five_hour":{"utilization":0.35,"resetsAt":1788660600}}})),
    );
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, "adapter.diagnostic");
    assert_eq!(events[0].payload["code"], "claude.usage_window");
    assert_eq!(events[0].payload["level"], "info");
    assert_eq!(events[0].payload["terminal"], false);
    assert!(events[0].payload["message"].as_str().unwrap().contains("5-hour window 35% used"));
}

// ---- a run the host stopped for output volume

#[test]
fn says_so_instead_of_reporting_a_missing_result_record() {
    // case: says so instead of reporting a missing result record
    let mut target = normalizer();
    target.evidence.output_limit_exceeded = true;
    let failed = finish(&mut target, None, false);
    let message = failed.payload["message"].as_str().unwrap();
    assert!(message.contains("faster than Locust could record it"), "{message}");
    assert!(!message.contains("without a terminal result record"), "{message}");
}

// ---- a long call is named while it runs

#[test]
fn announces_the_call_again_once_its_command_and_description_are_known() {
    // case: announces the call again once its command and description are known
    let mut n = normalizer();
    record(&mut n, json!({"type":"stream_event","event":{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_long","name":"Bash"}}}));
    let whole = json!({"type":"assistant","parent_tool_use_id":null,"message":{"id":"msg_1","content":[{"type":"tool_use","id":"toolu_long","name":"Bash","input":{"command":"cd C:/work && npm test","description":"Run the test suite"}}]}});
    let events = record(&mut n, whole.clone());
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, "tool.started");
    assert_eq!(events[0].payload["itemId"], "toolu_long");
    assert_eq!(events[0].payload["name"], "Bash");
    assert_eq!(events[0].payload["command"], "cd C:/work && npm test");
    assert_eq!(events[0].payload["title"], "Run the test suite");
    assert_eq!(events[0].payload["phase"], "started");
    assert!(record(&mut n, whole).is_empty());
}

// ---- a call Claude Code refused

fn denied_sentences(n: &mut ClaudeNormalizer, denials: Value) -> Vec<String> {
    diagnostics(&record(n, json!({"type":"result","subtype":"success","is_error":false,"result":"ok","permission_denials":denials})))
}

#[test]
fn is_reported_as_refused_with_the_reason_not_as_failed() {
    // case: is reported as refused, with the reason, not as failed
    let mut n = normalizer();
    record(&mut n, tool_start("toolu_loop", "Bash"));
    assert!(record(&mut n, json!({"type":"system","subtype":"permission_denied","tool_name":"Bash","tool_use_id":"toolu_loop","decision_reason_type":"other","decision_reason":"Contains simple_expansion","message":"Contains simple_expansion"})).is_empty());
    let done = record(&mut n, tool_result("toolu_loop", true, "Contains simple_expansion"));
    assert_eq!(done[0].kind, "tool.failed");
    assert_eq!(done[0].payload["itemId"], "toolu_loop");
    assert_eq!(done[0].payload["status"], "refused");
    assert_eq!(done[0].payload["output"], "Contains simple_expansion");
}

#[test]
fn leaves_an_ordinary_failure_a_failure() {
    // case: leaves an ordinary failure a failure
    let mut n = normalizer();
    record(&mut n, tool_start("toolu_x", "Bash"));
    let done = record(&mut n, tool_result("toolu_x", true, "exit code 1"));
    assert_eq!(done[0].kind, "tool.failed");
    assert_eq!(done[0].payload["status"], "error");
}

fn guarded(n: &mut ClaudeNormalizer, id: &str, command: &str, reason: &str) -> Vec<Event> {
    record(n, tool_start(id, "Bash"));
    record(n, json!({"type":"assistant","message":{"id":format!("msg_{id}"),"content":[{"type":"tool_use","id":id,"name":"Bash","input":{"command":command}}]}}));
    record(n, tool_result(id, true, &format!("PreToolUse:Bash hook error: {reason}")))
}

#[test]
fn is_refused_when_locusts_guard_stopped_it_and_said_as_the_guards_not_the_modes() {
    // case: is refused when Locust's guard stopped it, and said as the guard's, not the mode's
    let reason = "Locust stopped this command before it ran: it ends Vivaldi by name, which ends every Vivaldi on this computer, the person's own windows too. End only what you started yourself, by its process id: taskkill /PID <id> /F on Windows, or kill <id>.";
    let mut n = normalizer();
    let done = guarded(&mut n, "toolu_guard", "taskkill //F //IM vivaldi.exe", reason);
    assert_eq!(done[0].kind, "tool.failed");
    assert_eq!(done[0].payload["itemId"], "toolu_guard");
    assert_eq!(done[0].payload["status"], "refused");
    assert_eq!(done[0].payload["output"], reason);
    assert_eq!(
        denied_sentences(&mut n, json!([{"tool_name":"Bash","tool_use_id":"toolu_guard","tool_input":{"command":"taskkill //F //IM vivaldi.exe"}}])),
        ["Locust stopped Bash `taskkill //F //IM vivaldi.exe` before it ran: it would have ended every Vivaldi on this computer, your own windows too. A teammate may end only what it started itself."]
    );
}

#[test]
fn says_an_id_it_stopped_as_the_persons_program_which_the_teammate_did_not_start() {
    // case: says an id it stopped as the person's program, which the teammate did not start
    let reason = "Locust stopped this command before it ran: process 10544 is Chrome, which no teammate started, so it is the person's own. Leave it running. A test browser you start needs a profile of its own (--user-data-dir=<a new folder>), so it never meets theirs.";
    let mut n = normalizer();
    let done = guarded(&mut n, "toolu_pid", "taskkill //F //PID 10544", reason);
    assert_eq!(done[0].payload["status"], "refused");
    assert_eq!(done[0].payload["output"], reason);
    assert_eq!(
        denied_sentences(&mut n, json!([{"tool_name":"Bash","tool_use_id":"toolu_pid","tool_input":{"command":"taskkill //F //PID 10544"}}])),
        ["Locust stopped Bash `taskkill //F //PID 10544` before it ran: process 10544 is Chrome, which no teammate started, so it is yours. A teammate may end only what it started itself."]
    );
}

#[test]
fn says_another_teammates_agent_and_the_teammates_own_agent_as_theirs() {
    // case: says another teammate's agent, and the teammate's own agent, as theirs
    let sentence = |reason: &str| {
        let mut n = normalizer();
        guarded(&mut n, "toolu_a", "taskkill //F //PID 8100", reason);
        denied_sentences(&mut n, json!([{"tool_name":"Bash","tool_use_id":"toolu_a","tool_input":{"command":"taskkill //F //PID 8100"}}]))
            .into_iter()
            .next()
            .unwrap_or_default()
    };
    assert_eq!(
        sentence("Locust stopped this command before it ran: process 8100 is an AI agent on Node, which this run did not start, so it is another teammate's or the person's. Leave it running."),
        "Locust stopped Bash `taskkill //F //PID 8100` before it ran: process 8100 is an AI agent on Node, which this teammate did not start, so it is another teammate's or yours. A teammate may end only what it started itself."
    );
    assert_eq!(
        sentence("Locust stopped this command before it ran: process 8100 is the agent this run is, so it is not this run's to end. Leave it running."),
        "Locust stopped Bash `taskkill //F //PID 8100` before it ran: process 8100 is the agent this teammate runs on. A teammate may end only what it started itself."
    );
}

#[test]
fn says_the_guards_and_the_modes_apart_when_a_run_met_both() {
    // case: says the guard's and the mode's apart when a run met both
    let mut n = normalizer();
    guarded(&mut n, "toolu_g2", "pkill node", "Locust stopped this command before it ran: it ends Node by name, which ends every Node on this computer, the person's own programs and other teammates' work too. End only what you started yourself, by its process id: taskkill /PID <id> /F on Windows, or kill <id>.");
    let said = denied_sentences(
        &mut n,
        json!([{"tool_name":"Bash","tool_input":{"command":"pkill node"}},{"tool_name":"Bash","tool_use_id":"toolu_other","tool_input":{"command":"node t.mjs"}}]),
    );
    assert_eq!(said[0], "Locust stopped Bash `pkill node` before it ran: it would have ended every Node on this computer, your own programs and other teammates' work too. A teammate may end only what it started itself.");
    assert!(said[1].starts_with("Claude Code was not permitted to use Bash `node t.mjs`, so it did not."), "{}", said[1]);
}

// ---- Claude Code compacting its conversation

fn boundary(trigger: &str) -> Value {
    json!({"type":"system","subtype":"compact_boundary","session_id":"ses_1","uuid":"u1","compact_metadata":{"trigger":trigger,"pre_tokens":35420,"post_tokens":7635}})
}

#[test]
fn says_so_once_as_a_line_the_host_can_read_and_not_as_an_empty_step() {
    // case: says so once, as a line the host can read, and not as an empty step
    let events = record(&mut normalizer(), boundary("auto"));
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, "adapter.diagnostic");
    assert_eq!(events[0].payload["code"], "claude.context_compacted");
    assert_eq!(events[0].payload["level"], "info");
    assert_eq!(events[0].payload["message"], COMPACTED);
    assert_eq!(record(&mut normalizer(), boundary("manual"))[0].payload["message"], COMPACTED_ON_REQUEST);
}

#[test]
fn keeps_the_persons_command_list_out_of_the_runs_record() {
    // case: keeps the person's command list out of the run's record
    let listed = json!({"type":"system","subtype":"commands_changed","commands":[{"name":"deliver","description":"a person's own skill","argumentHint":""}]});
    assert!(record(&mut normalizer(), listed).is_empty());
}

#[test]
fn hands_the_command_list_to_the_host_for_the_menu_and_still_keeps_it_out_of_the_record_0_426() {
    // case: hands the command list to the host for the / menu, and still keeps it out of the record (0.426)
    let mut listening = normalizer();
    let listed = json!({"type":"system","subtype":"commands_changed","commands":[
        {"name":"compact","description":"Clear conversation history but keep a summary in context","argumentHint":"<optional custom summarization instructions>"},
        {"name":"/security-review","description":"Complete a security review\n  of the pending changes"},
        {"name":"plugin:skill","description":"","argumentHint":""},
        {"name":"rm -rf /","description":"x"},
        {"name":"","description":"x"},
        {"description":"no name"},
        "compact",
    ]});
    assert!(record(&mut listening, listed).is_empty());
    let heard: Vec<(String, String, String)> =
        listening.take_commands().unwrap().into_iter().map(|c| (c.name, c.description, c.argument_hint)).collect();
    let row = |a: &str, b: &str, c: &str| (a.to_owned(), b.to_owned(), c.to_owned());
    assert_eq!(
        heard,
        [
            row("compact", "Clear conversation history but keep a summary in context", "<optional custom summarization instructions>"),
            row("security-review", "Complete a security review of the pending changes", ""),
            row("plugin:skill", "", ""),
        ]
    );
}

#[test]
fn bounds_what_a_command_list_can_put_in_the_menu() {
    // case: bounds what a command list can put in the menu
    let heard = |commands: Value| {
        let mut n = normalizer();
        record(&mut n, json!({"type":"system","subtype":"commands_changed","commands":commands}));
        n.take_commands().unwrap_or_default()
    };
    let many: Vec<Value> = (0..900).map(|i| json!({"name":format!("c{i}"),"description":"d".repeat(5000),"argumentHint":"h".repeat(500)})).collect();
    let commands = heard(json!(many));
    assert_eq!(commands.len(), 400);
    assert_eq!(commands[0].description.chars().count(), 240);
    assert_eq!(commands[0].argument_hint.chars().count(), 80);
    assert!(heard(json!({"not":"a list"})).is_empty());
    assert!(heard(json!([{"name":"x".repeat(65)}])).is_empty());
}
