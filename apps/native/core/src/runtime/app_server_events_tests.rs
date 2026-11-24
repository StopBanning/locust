//! Electron's `packages/runtime-adapters/test/app-server-events.test.ts`, case by case, through the caller: the Codex
//! runtime hands every app-server notification to `Normalizer::accept` and a run that ended without the server saying so
//! to `Normalizer::finish` (runtime/codex.rs). Electron's `limitFromSnapshot`, `usageWindowFromSnapshot`, `toolNameOf`
//! and `toolCommandOf` have no free-standing Rust twin: what they decide is read here off the events `accept` makes.
use super::*;
use crate::{ledger_write::LedgerWriter, store::Store};

fn normalizer() -> Normalizer {
    Normalizer::default()
}

fn kinds(events: &[Event]) -> Vec<&str> {
    events.iter().map(|e| e.kind.as_str()).collect()
}

fn limits(events: &[Event]) -> Vec<&Event> {
    events.iter().filter(|e| e.kind == "route.limit_detected").collect()
}

/// The limit a single `account/rateLimits/updated` push says, if any (Electron's `limitFromSnapshot`).
fn limit_from_snapshot(snapshot: Value) -> Option<(String, String)> {
    let events = normalizer().accept("account/rateLimits/updated", &json!({ "rateLimits": snapshot }));
    limits(&events).first().map(|e| {
        (e.payload["kind"].as_str().unwrap().to_owned(), e.payload["message"].as_str().unwrap().to_owned())
    })
}

/// The reading a single push keeps (Electron's `usageWindowFromSnapshot`).
fn usage_window_from_snapshot(snapshot: Value) -> Option<String> {
    readings(&normalizer().accept("account/rateLimits/updated", &json!({ "rateLimits": snapshot }))).into_iter().next()
}

fn readings(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter(|e| e.kind == "adapter.diagnostic" && e.payload["code"] == "codex.usage_window")
        .map(|e| e.payload["message"].as_str().unwrap().to_owned())
        .collect()
}

fn failure(error: Value) -> Vec<Event> {
    normalizer().accept("error", &json!({"threadId":"t","turnId":"u","willRetry":false,"error":error}))
}

fn start_item(item: Value) -> (&'static str, Value) {
    ("item/started", json!({"threadId":"t","turnId":"u","startedAtMs":1,"item":item}))
}

fn complete_item(item: Value) -> (&'static str, Value) {
    ("item/completed", json!({"threadId":"t","turnId":"u","completedAtMs":2,"item":item}))
}

fn feed(app: &mut Normalizer, (method, params): (&str, Value)) -> Vec<Event> {
    app.accept(method, &params)
}

// ---- run lifecycle

#[test]
fn takes_the_runtime_thread_id_from_thread_started_without_emitting_an_event() {
    // case: takes the runtime thread id from thread/started without emitting an event
    let mut app = normalizer();
    assert!(app.accept("thread/started", &json!({"thread":{"id":"th_1"}})).is_empty());
    assert_eq!(app.thread.as_deref(), Some("th_1"));
}

#[test]
fn starts_on_turn_started_and_completes_on_turn_completed() {
    // case: starts on turn/started and completes on turn/completed
    let mut app = normalizer();
    app.accept("thread/started", &json!({"thread":{"id":"th_1"}}));
    let started = app.accept("turn/started", &json!({"threadId":"th_1","turn":{}}));
    let completed = app.accept("turn/completed", &json!({"threadId":"th_1","turn":{}}));
    assert_eq!(kinds(&started), ["run.started"]);
    assert_eq!(kinds(&completed), ["run.completed"]);
    assert!(app.ended);
}

#[test]
fn goes_quiet_once_finalized() {
    // case: goes quiet once finalized
    let mut app = normalizer();
    app.accept("turn/completed", &json!({"threadId":"t","turn":{}}));
    assert!(app.accept("item/agentMessage/delta", &json!({"itemId":"a","delta":"x"})).is_empty());
    // Electron's `finish("cancelled")` after the end is nothing; in Rust the runtime never asks for one once the
    // normalizer has ended: codex.rs `next_event` hands back only what was queued (see the fake-server run below).
}

#[test]
fn goes_quiet_once_finalized_through_the_codex_runtime() {
    // case: goes quiet once finalized
    use super::super::{Runtime, Turn, codex::Codex};
    let root = std::env::temp_dir().join(format!("locust-quiet-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(&root).unwrap();
    let mut command = std::process::Command::new("python3");
    command.arg(concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/fixtures/m1-app-server.py")).arg("success");
    let mut runtime = Codex::spawn(&mut command).unwrap();
    runtime
        .start(&Turn {
            prompt: "Synthetic prompt".into(),
            folder: root.clone(),
            model: Some("synthetic-model".into()),
            effort: None,
            resume: None,
            mode: crate::approval::Mode::Ask,
            approvals: None,
            mission_id: "synthetic".into(),
            run_id: "run_synthetic".into(),
        })
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        assert!(std::time::Instant::now() < deadline, "the fake never completed");
        if let Some(e) = runtime.next_event().unwrap()
            && e.kind == "run.completed"
        {
            break;
        }
    }
    // The server exits after its turn, and a stop is asked: neither ends the finished run a second time.
    let _ = runtime.stop();
    let after = std::time::Instant::now() + std::time::Duration::from_millis(600);
    while std::time::Instant::now() < after {
        if let Some(e) = runtime.next_event().unwrap() {
            assert!(!["run.failed", "run.cancelled", "run.completed"].contains(&e.kind.as_str()), "said again: {}", e.kind);
        }
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn reports_a_lost_connection_as_failed_rather_than_completed() {
    // case: reports a lost connection as failed rather than completed
    let terminal = normalizer().finish(false, "");
    assert_eq!(terminal.kind, "run.failed");
    assert_eq!(terminal.payload["runtimeTerminal"], "missing");
}

#[test]
fn reports_cancellation_as_cancelled() {
    // case: reports cancellation as cancelled
    assert_eq!(normalizer().finish(true, "").kind, "run.cancelled");
}

// ---- assistant text

#[test]
fn appends_deltas_and_replaces_on_the_completed_item() {
    // case: appends deltas and replaces on the completed item
    let mut app = normalizer();
    let deltas: Vec<Event> = ["Hel", "lo"]
        .iter()
        .flat_map(|d| app.accept("item/agentMessage/delta", &json!({"itemId":"msg_1","delta":d,"threadId":"t","turnId":"u"})))
        .collect();
    assert_eq!(deltas.len(), 2);
    for e in &deltas {
        assert_eq!(e.kind, "message.delta");
        assert_eq!(e.payload["operation"], "append");
    }
    let complete = app.accept(
        "item/completed",
        &json!({"threadId":"t","turnId":"u","completedAtMs":1,"item":{"id":"msg_1","type":"agentMessage","text":"Hello"}}),
    );
    assert_eq!(complete[0].payload["operation"], "replace");
    assert_eq!(complete[0].payload["final"], true);
    assert_eq!(complete[0].payload["text"], "Hello");
}

#[test]
fn ignores_a_delta_with_no_text_or_no_item() {
    // case: ignores a delta with no text or no item
    let mut app = normalizer();
    assert!(app.accept("item/agentMessage/delta", &json!({"itemId":"a"})).is_empty());
    assert!(app.accept("item/agentMessage/delta", &json!({"delta":"x"})).is_empty());
}

// ---- tools

#[test]
fn opens_and_settles_a_command_execution() {
    // case: opens and settles a command execution
    let mut app = normalizer();
    let started = feed(&mut app, start_item(json!({"id":"exec_1","type":"commandExecution","command":"pnpm test"})));
    assert_eq!(started[0].kind, "tool.started");
    assert_eq!(started[0].payload["name"], "shell");
    assert_eq!(started[0].payload["command"], "pnpm test");
    let done = feed(
        &mut app,
        complete_item(json!({"id":"exec_1","type":"commandExecution","command":"pnpm test","exitCode":0,"status":"completed"})),
    );
    assert_eq!(done[0].kind, "tool.completed");
}

#[test]
fn records_a_declined_command_or_patch_as_failed_not_completed() {
    // case: records a declined command or patch as failed, not completed
    for kind in ["commandExecution", "fileChange"] {
        let mut app = normalizer();
        feed(&mut app, start_item(json!({"id":"item_1","type":kind,"command":"rm -rf build"})));
        let done = feed(&mut app, complete_item(json!({"id":"item_1","type":kind,"status":"declined"})));
        assert_eq!(done[0].kind, "tool.failed", "{kind}");
        assert_eq!(done[0].payload["status"], "declined");
    }
}

#[test]
fn treats_a_non_zero_exit_as_a_failure_even_when_the_status_says_otherwise() {
    // case: treats a non-zero exit as a failure even when the status says otherwise
    let mut app = normalizer();
    feed(&mut app, start_item(json!({"id":"exec_1","type":"commandExecution","command":"false"})));
    let done = feed(&mut app, complete_item(json!({"id":"exec_1","type":"commandExecution","exitCode":1,"status":"completed"})));
    assert_eq!(done[0].kind, "tool.failed");
}

#[test]
fn names_an_mcp_tool_by_server_and_tool() {
    // case: names an MCP tool by server and tool
    let name = |item: Value| feed(&mut normalizer(), start_item(item))[0].payload["name"].as_str().unwrap().to_owned();
    assert_eq!(name(json!({"id":"m","type":"mcpToolCall","server":"gmail","tool":"send"})), "gmail.send");
    assert_eq!(name(json!({"id":"f","type":"fileChange"})), "apply_patch");
    assert_eq!(name(json!({"id":"w","type":"webSearch"})), "web_search");
}

#[test]
fn describes_a_file_change_by_its_count_rather_than_its_diff() {
    // case: describes a file change by its count rather than its diff
    let events = feed(&mut normalizer(), start_item(json!({"id":"f","type":"fileChange","changes":[1,2,3]})));
    assert_eq!(events[0].payload["command"], "3 files");
}

#[test]
fn treats_an_unknown_non_tool_item_as_a_step_not_a_tool() {
    // case: treats an unknown non-tool item as a step, not a tool
    let events = feed(&mut normalizer(), start_item(json!({"id":"r_1","type":"reasoning"})));
    assert_eq!(events[0].kind, "step.started");
}

// ---- rate limits

#[test]
fn says_nothing_while_usage_is_comfortable() {
    // case: says nothing while usage is comfortable
    assert_eq!(limit_from_snapshot(json!({"weekly":{"usedPercent":31}})), None);
    assert_eq!(limit_from_snapshot(json!({})), None);
    assert_eq!(limit_from_snapshot(Value::Null), None);
}

#[test]
fn warns_near_the_limit_and_reports_exhaustion_at_it() {
    // case: warns near the limit and reports exhaustion at it
    assert_eq!(limit_from_snapshot(json!({"weekly":{"usedPercent":92}})).unwrap().0, "temporary-rate-limit");
    assert_eq!(limit_from_snapshot(json!({"weekly":{"usedPercent":100}})).unwrap().0, "quota-exhausted");
}

#[test]
fn reports_the_worst_window_not_the_first() {
    // case: reports the worst window, not the first
    let (_, message) = limit_from_snapshot(json!({"fiveHour":{"usedPercent":10},"weekly":{"usedPercent":95}})).unwrap();
    assert!(message.contains("weekly"), "{message}");
}

#[test]
fn accepts_a_fractional_utilization_as_well_as_a_percent() {
    // case: accepts a fractional utilization as well as a percent
    assert_eq!(limit_from_snapshot(json!({"weekly":{"utilization":0.95}})).unwrap().0, "temporary-rate-limit");
}

#[test]
fn announces_a_limit_once_rather_than_on_every_push() {
    // case: announces a limit once rather than on every push
    let mut app = normalizer();
    let first = app.accept("account/rateLimits/updated", &json!({"rateLimits":{"weekly":{"usedPercent":95}}}));
    let second = app.accept("account/rateLimits/updated", &json!({"rateLimits":{"weekly":{"usedPercent":96}}}));
    assert_eq!(limits(&first).len(), 1);
    assert!(limits(&second).is_empty());
}

// ---- usage readings

fn rollout() -> Value {
    json!({
        "limit_id": "codex",
        "limit_name": null,
        "primary": { "used_percent": 3.0, "window_minutes": 300, "resets_at": 1790363478 },
        "secondary": { "used_percent": 0.0, "window_minutes": 10080, "resets_at": 1790950278 },
        "credits": { "has_credits": false, "unlimited": false, "balance": "0" },
        "individual_limit": null,
        "plan_type": "plus",
    })
}

#[test]
fn reads_every_window_fullest_first_in_the_words_claudes_reading_uses() {
    // case: reads every window, fullest first, in the words Claude's reading uses
    assert_eq!(
        usage_window_from_snapshot(rollout()).unwrap(),
        "5-hour window 3% used · resets 2026-09-25T19:11:18.000Z · weekly window 0% used · resets 2026-10-02T14:11:18.000Z"
    );
    assert_eq!(
        usage_window_from_snapshot(json!({"primary":{"usedPercent":34,"windowDurationMins":300},"secondary":{"usedPercent":71.6,"windowDurationMins":10080}}))
            .unwrap(),
        "weekly window 72% used · 5-hour window 34% used"
    );
}

#[test]
fn reads_nothing_where_there_are_no_windows() {
    // case: reads nothing where there are no windows
    assert_eq!(usage_window_from_snapshot(json!({"credits":{"balance":"0"},"plan_type":"plus"})), None);
    assert_eq!(usage_window_from_snapshot(Value::Null), None);
}

#[test]
fn keeps_a_reading_as_a_codex_usage_window_diagnostic_once_per_change() {
    // case: keeps a reading as a codex.usage_window diagnostic, once per change
    let mut app = normalizer();
    let push = |app: &mut Normalizer, used: u64| {
        app.accept("account/rateLimits/updated", &json!({"rateLimits":{"primary":{"usedPercent":used,"windowDurationMins":300}}}))
    };
    let first = push(&mut app, 12);
    let same = push(&mut app, 12);
    let moved = push(&mut app, 13);
    assert_eq!(readings(&first), ["5-hour window 12% used"]);
    assert!(same.is_empty());
    assert_eq!(readings(&moved), ["5-hour window 13% used"]);
    assert!(first.iter().chain(&moved).all(|e| e.kind != "route.limit_detected"));
}

// ---- limits and sign-in on the app-server transport

#[test]
fn says_an_exhausted_quota_after_a_warning_was_already_said() {
    // case: says an exhausted quota after a warning was already said
    let mut app = normalizer();
    let warned = app.accept("account/rateLimits/updated", &json!({"rateLimits":{"weekly":{"usedPercent":95}}}));
    let spent = app.accept("account/rateLimits/updated", &json!({"rateLimits":{"weekly":{"usedPercent":100}}}));
    let said = |events: &[Event]| limits(events).iter().map(|e| e.payload["kind"].as_str().unwrap().to_owned()).collect::<Vec<_>>();
    assert_eq!(said(&warned), ["temporary-rate-limit"]);
    assert_eq!(said(&spent), ["quota-exhausted"]);
}

#[test]
fn classifies_a_usage_limit_error_from_codexs_own_error_info_and_names_the_limit() {
    // case: classifies a usage-limit error from Codex's own error info, and names the limit
    for info in [json!("usageLimitExceeded"), json!({"usageLimitExceeded":{}})] {
        let events = failure(json!({"message":"You have hit your limit.","codexErrorInfo":info}));
        assert_eq!(kinds(&events), ["route.limit_detected", "run.failed"]);
        assert_eq!(events[0].payload["kind"], "quota-exhausted");
        assert_eq!(events[1].payload["kind"], "quota-exhausted");
    }
}

#[test]
fn classifies_a_signed_out_account_from_codexs_own_error_info() {
    // case: classifies a signed-out account from Codex's own error info
    let events = failure(json!({"message":"401","codexErrorInfo":"unauthorized"}));
    assert_eq!(kinds(&events), ["run.failed"]);
    assert_eq!(events[0].payload["kind"], "authentication-failed");
}

#[test]
fn classifies_the_schemas_other_named_errors() {
    // case: classifies the schema's other named errors
    let kind_of = |info: &str| failure(json!({"message":"x","codexErrorInfo":info})).last().unwrap().payload["kind"].clone();
    assert_eq!(kind_of("rateLimitExceeded"), "temporary-rate-limit");
    assert_eq!(kind_of("cyberPolicy"), "safety-blocked");
    assert_eq!(kind_of("misalignmentPolicyViolation"), "safety-blocked");
}

#[test]
fn names_a_window_by_the_v2_field_windowdurationmins_read_from_the_generated_schema() {
    // case: names a window by the v2 field, windowDurationMins (read from the generated schema)
    let (_, five) = limit_from_snapshot(json!({"primary":{"usedPercent":93,"windowDurationMins":300}})).unwrap();
    assert!(!five.contains("primary"), "{five}");
    let (_, week) = limit_from_snapshot(json!({"secondary":{"usedPercent":99,"windowDurationMins":10080}})).unwrap();
    assert!(!week.contains("secondary"), "{week}");
}

#[test]
fn falls_back_to_the_message_when_there_is_no_error_info() {
    // case: falls back to the message when there is no error info
    let events = failure(json!({"message":"You've hit your usage limit. Try again later."}));
    let kind = events.last().unwrap().payload["kind"].as_str().unwrap().to_owned();
    assert!(["quota-exhausted", "temporary-rate-limit"].contains(&kind.as_str()), "{kind}");
}

// ---- a run that ends without the server saying so

#[test]
fn says_the_reason_the_run_knew_and_classifies_it() {
    // case: says the reason the run knew, and classifies it
    let failed = normalizer().finish(false, "Authentication required: run codex login.");
    assert_eq!(failed.payload["message"], "Authentication required: run codex login.");
    assert_eq!(failed.payload["kind"], "authentication-failed");
}

#[test]
fn says_in_a_sentence_that_codex_has_the_conversation_open_elsewhere_0_835() {
    // case: says, in a sentence, that Codex has the conversation open elsewhere (0.835)
    let sentence = "Codex has this conversation open somewhere else right now, most likely in the Codex app or a Codex in a terminal, and lets only one place write to it at a time. Close it there, then run it again.";
    let log = "thread 01a10f66-f0ff-7622-8e68-875be5256541 already has an active writer 2026-10-25T03:32:46.432808Z ERROR codex_app_server: Codex is ignoring 1 unrecognized configuration setting. 2026-10-25T03:32:48.486820Z ERROR codex_core::session::session: failed to initialize thread persistence: thread-store conflict: thread 01a10f66-f0ff-7622-8e68-875be5256541 already has an active writer";
    let failed = normalizer().finish(false, log);
    assert_eq!(failed.payload["message"], sentence);
    assert!(failed.payload["process"]["stderr"].as_str().unwrap().contains("already has an active writer"));
    let events = normalizer().accept("error", &json!({"threadId":"t","turnId":"u","willRetry":false,"error":{"message":log}}));
    assert_eq!(events.last().unwrap().payload["message"], sentence);
}

#[test]
fn still_says_something_when_it_knew_nothing() {
    // case: still says something when it knew nothing
    let failed = normalizer().finish(false, "");
    assert!(failed.payload["message"].as_str().unwrap().contains("ended before the turn completed"));
}

// ---- errors and noise

#[test]
fn does_not_end_the_run_on_an_error_the_provider_intends_to_retry() {
    // case: does not end the run on an error the provider intends to retry
    let mut app = normalizer();
    let events = app.accept("error", &json!({"threadId":"t","turnId":"u","willRetry":true,"error":{"message":"rate limited"}}));
    assert_eq!(events[0].kind, "adapter.diagnostic");
    assert!(!app.ended);
}

#[test]
fn ends_the_run_on_an_error_the_provider_will_not_retry() {
    // case: ends the run on an error the provider will not retry
    let mut app = normalizer();
    let events = app.accept("error", &json!({"threadId":"t","turnId":"u","willRetry":false,"error":{"message":"no auth"}}));
    assert_eq!(events[0].kind, "run.failed");
    assert!(app.ended);
}

#[test]
fn keeps_quiet_about_the_trust_bypass_locusts_own_command_guard_asks_for_and_passes_every_other_warning_on_0_720() {
    // case: keeps quiet about the trust bypass Locust's own command guard asks for, and passes every other warning on (0.720)
    let bypass = "`--dangerously-bypass-hook-trust` is enabled. Enabled hooks may run without review for this invocation.";
    let mut app = normalizer();
    assert!(app.accept("configWarning", &json!({"summary":bypass,"details":null})).is_empty());
    assert!(app.accept("warning", &json!({"threadId":"t","message":bypass})).is_empty());
    let other = app.accept("configWarning", &json!({"summary":"Codex is ignoring 1 unrecognized configuration setting.","details":null}));
    assert_eq!(other[0].kind, "adapter.diagnostic");
}

#[test]
fn stays_silent_on_the_protocols_many_unrelated_notifications() {
    // case: stays silent on the protocol's many unrelated notifications
    let mut app = normalizer();
    for method in [
        "mcpServer/startupStatus/updated",
        "remoteControl/status/changed",
        "thread/tokenUsage/updated",
        "thread/status/changed",
        "fs/changed",
    ] {
        assert!(app.accept(method, &json!({})).is_empty(), "{method}");
    }
}

#[test]
fn stamps_every_event_with_the_mission_runtime() {
    // case: stamps every event with the mission runtime
    // Rust's normalizer carries no runtime of its own; the ledger writer the run appends through stamps the mission's.
    let root = std::env::temp_dir().join(format!("locust-stamp-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("native")).unwrap();
    std::fs::create_dir_all(root.join("electron")).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    let mut writer = LedgerWriter::create_runtime(&store, "mission_1", "p", "m", "claude", "claude", &[], None).unwrap();
    let events = normalizer().accept("turn/started", &json!({"threadId":"t","turn":{}}));
    let written = writer.append(&events[0].kind, events[0].payload.clone(), events[0].thread.as_deref()).unwrap();
    assert_eq!(written["sourceAdapter"], "claude");
    let _ = std::fs::remove_dir_all(&root);
}

// ---- saying which limit, and when it lifts

fn real() -> Value {
    json!({
        "primary": { "used_percent": 93.0, "window_minutes": 300, "resets_at": 1788892785 },
        "secondary": { "used_percent": 14.0, "window_minutes": 10080, "resets_at": 1789479585 },
    })
}

/// "resets" and a clock: Electron writes `HH:MM`; this app's clock is its own 12-hour one (time_marker::clock).
fn says_a_reset_clock(message: &str) -> bool {
    crate::invariants::regex(r"resets \d{1,2}:\d\d").is_match(message)
}

#[test]
fn names_the_window_a_person_recognises_not_the_apis_own_key() {
    // case: names the window a person recognises, not the API's own key
    let (_, message) = limit_from_snapshot(real()).unwrap();
    assert!(message.contains("5-hour"), "{message}");
    assert!(!message.contains("primary"), "{message}");
}

#[test]
fn still_reports_the_worst_window_which_is_the_one_that_stops_you() {
    // case: still reports the worst window, which is the one that stops you
    let (kind, message) = limit_from_snapshot(real()).unwrap();
    assert!(message.contains("93%"), "{message}");
    assert_eq!(kind, "temporary-rate-limit");
}

#[test]
fn says_when_it_lifts_because_that_is_the_decision() {
    // case: says when it lifts, because that is the decision
    let (_, message) = limit_from_snapshot(real()).unwrap();
    assert!(says_a_reset_clock(&message), "{message}");
}

#[test]
fn reads_the_camelcase_spelling_the_app_server_sends() {
    // case: reads the camelCase spelling the app-server sends
    let (_, message) = limit_from_snapshot(json!({"primary":{"usedPercent":93,"windowMinutes":300,"resetsAt":1788892785}})).unwrap();
    assert!(message.contains("5-hour"), "{message}");
    assert!(says_a_reset_clock(&message), "{message}");
}

#[test]
fn calls_the_seven_day_window_weekly() {
    // case: calls the seven-day window weekly
    let (_, message) = limit_from_snapshot(json!({"secondary":{"used_percent":99,"window_minutes":10080}})).unwrap();
    assert!(message.contains("weekly"), "{message}");
}

#[test]
fn falls_back_to_the_raw_key_when_no_window_is_given() {
    // case: falls back to the raw key when no window is given
    let (_, message) = limit_from_snapshot(json!({"primary":{"usedPercent":95}})).unwrap();
    assert!(message.contains("primary"), "{message}");
}

#[test]
fn says_nothing_about_a_reset_it_was_not_told() {
    // case: says nothing about a reset it was not told
    let (_, message) = limit_from_snapshot(json!({"primary":{"usedPercent":95,"windowMinutes":300}})).unwrap();
    assert!(!message.contains("resets"), "{message}");
}

#[test]
fn stays_quiet_well_below_the_ceiling() {
    // case: stays quiet well below the ceiling
    assert_eq!(limit_from_snapshot(json!({"primary":{"used_percent":14,"window_minutes":10080}})), None);
}

// ---- Codex compacting its conversation

const ON_ITS_OWN: &str = "The conversation outgrew the model's context, so Codex summarized it and carried on from the summary.";
const AS_ASKED: &str = "Codex summarized the conversation so far, as asked, and carries on from the summary.";

fn compaction_item() -> (&'static str, Value) {
    ("item/completed", json!({"item":{"type":"contextCompaction","id":"c1"}}))
}
fn compaction_notice() -> (&'static str, Value) {
    ("thread/compacted", json!({"threadId":"th_1"}))
}
fn asked() -> (&'static str, Value) {
    ("item/completed", json!({"item":{"type":"userMessage","id":"u1","content":[]}}))
}
fn compaction_lines(events: &[Event]) -> Vec<&Event> {
    events.iter().filter(|e| e.kind == "adapter.diagnostic" && e.payload["code"] == "codex.context_compacted").collect()
}

#[test]
fn is_one_line_for_one_compaction_however_it_is_reported() {
    // case: is one line for one compaction, however it is reported
    let orders: Vec<Vec<(&str, Value)>> = vec![
        vec![compaction_item(), compaction_notice()],
        vec![compaction_notice(), compaction_item()],
        vec![compaction_item()],
        vec![compaction_notice()],
    ];
    for order in orders {
        let mut app = normalizer();
        let events: Vec<Event> = std::iter::once(asked()).chain(order).flat_map(|n| feed(&mut app, n)).collect();
        let said = compaction_lines(&events);
        assert_eq!(said.len(), 1);
        assert_eq!(said[0].payload["level"], "info");
        assert_eq!(said[0].payload["message"], ON_ITS_OWN);
    }
}

#[test]
fn says_it_was_asked_for_when_the_turn_was_the_compaction_alone_compact_0_428() {
    // case: says it was asked for when the turn was the compaction alone: /compact (0.428)
    let mut app = normalizer();
    let events: Vec<Event> = [compaction_item(), compaction_notice()].into_iter().flat_map(|n| feed(&mut app, n)).collect();
    assert_eq!(compaction_lines(&events)[0].payload["message"], AS_ASKED);
}

#[test]
fn is_a_line_for_each_compaction_when_there_are_two() {
    // case: is a line for each compaction when there are two
    let mut app = normalizer();
    let events: Vec<Event> = [compaction_item(), compaction_notice(), compaction_item(), compaction_notice()]
        .into_iter()
        .flat_map(|n| feed(&mut app, n))
        .collect();
    assert_eq!(compaction_lines(&events).len(), 2);
}

#[test]
fn opens_no_step_for_the_compaction_item() {
    // case: opens no step for the compaction item
    assert!(normalizer().accept("item/started", &json!({"item":{"type":"contextCompaction","id":"c1"}})).is_empty());
}

// ---- a review Codex was asked for

#[test]
fn draws_only_the_findings_once_the_review_mode_bookends_are_not_steps() {
    // case: draws only the findings, once: the review-mode bookends are not steps
    let mut app = normalizer();
    app.accept("thread/started", &json!({"thread":{"id":"th_1"}}));
    app.accept("turn/started", &json!({"threadId":"th_1","turn":{}}));
    let mut bookends = vec![];
    for (method, kind, id, review) in [
        ("item/started", "enteredReviewMode", "r1", "current changes"),
        ("item/completed", "enteredReviewMode", "r1", "current changes"),
        ("item/started", "exitedReviewMode", "r2", "add subtracts"),
        ("item/completed", "exitedReviewMode", "r2", "add subtracts"),
    ] {
        bookends.extend(app.accept(method, &json!({"threadId":"th_1","item":{"type":kind,"id":id,"review":review}})));
    }
    assert!(bookends.is_empty());
}
