//! Electron's `packages/runtime-adapters/test/a-muse-plan-used-up-is-said-once.test.ts` (0.823), case by case, through
//! the caller: the Muse Code runtime (runtime/muse.rs) hands each stream line to `MuseNormalizer::accept_line` and
//! asks `finish` for the end. The records are the 2026-10-22 run's, as Muse Code 1.4.11 wrote them (ids shortened,
//! the session folder's path made neutral).
use super::*;

const QUOTA: &str = "API error 429 [request_id=2370cf39-f35e-496c-8d65-7ca6df260286]: Subscription quota exhausted. Your usage window resets at 2026-10-26T00:00:00Z. (rate_limit_error)";
const LIMIT: &str = "Muse Code's plan is used up · resets 2026-10-26T00:00:00Z";
const FETCH: &str = "01a1648b-58af";
const MODEL: &str = "01a1648d-03c0";
const SEARCH: &str = "01a1648d-1111";
const QUEUED: &str = "01a1648d-2222";

fn record_as(payload_type: &str, payload: Value, record_type: &str) -> String {
    json!({"schema_version": 1, "stream": {"kind": "session", "id": "01a16488-07c9-7270-b9db-a0c54559f14d"}, "record_type": record_type, "payload_type": payload_type, "payload": payload}).to_string()
}

fn record(payload_type: &str, payload: Value) -> String {
    record_as(payload_type, payload, "event")
}

fn task(task_id: &str, event: Value) -> String {
    let kind = event["kind"].as_str().unwrap().to_owned();
    let mut event = event;
    event["task_id"] = json!(task_id);
    record(&format!("task.lifecycle.{kind}"), json!({"kind": "task_lifecycle", "task_id": task_id, "event": event}))
}

fn terminal() -> String {
    record("run.terminal.failed", json!({"kind": "run_terminal", "terminal": "failed", "text": "", "reason": QUOTA}))
}

fn lines() -> Vec<String> {
    vec![
        record("run.lifecycle.started", json!({})),
        task(FETCH, json!({"kind": "proposed", "task_kind": "tool.web_fetch"})),
        task(FETCH, json!({"kind": "started"})),
        task(FETCH, json!({
            "kind": "tool_output_ref",
            "output_ref": {"id": "web_fetch-page-1", "kind": "web_fetch-page", "uri": "tool-output://local/1/page.txt", "media_type": "text/plain; charset=utf-8", "byte_len": 15460, "digest": null, "path": "C:\\Users\\a\\.local\\share\\muse\\sessions\\page.txt"},
            "availability": "available",
        })),
        task(FETCH, json!({"kind": "completed"})),
        task(MODEL, json!({"kind": "proposed", "task_kind": "model.meta.response"})),
        task(SEARCH, json!({"kind": "proposed", "task_kind": "tool.search"})),
        task(SEARCH, json!({"kind": "started"})),
        task(QUEUED, json!({"kind": "proposed", "task_kind": "tool.read_file"})),
        task(MODEL, json!({"kind": "failed", "reason": QUOTA})),
        task(SEARCH, json!({"kind": "cancelled", "reason": "main run failed"})),
        task(QUEUED, json!({"kind": "cancelled", "reason": "main run failed"})),
        task(MODEL, json!({"kind": "cancelled", "reason": "main run failed"})),
        record_as("run.usage", json!({
            "kind": "run_usage",
            "models": [{"model": "muse-wonder-1.0", "input_tokens": 4428573, "cached_tokens": 4266120, "output_tokens": 8818, "reasoning_tokens": 4444, "calls": 43}],
            "calls": 44, "completed_calls": 43, "failed_calls": 1, "cancelled_calls": 0, "children_included": false,
            "allowance": {"tier": null, "window": {"used_percent": 92, "resets_at_ms": 1792706021000u64}, "weekly": {"used_percent": 97, "resets_at_ms": 1792972800000u64}, "as_of_ms": 1792688325219u64, "source": "stream"},
        }), "status"),
        terminal(),
    ]
}

fn run(input: &[String]) -> Vec<Event> {
    let mut muse = MuseNormalizer::default();
    let mut events = Vec::new();
    for line in input {
        events.extend(muse.accept_line(line));
    }
    events.extend(muse.finish(false, Some(1)));
    events
}

fn said(e: &Event) -> String {
    e.payload["message"].as_str().unwrap_or_default().to_owned()
}

#[test]
fn is_one_limit_with_its_reset_instant_and_a_failure_that_says_what_to_do() {
    let events = run(&lines());
    let limits: Vec<&Event> = events.iter().filter(|e| e.kind == "route.limit_detected").collect();
    assert_eq!(limits.len(), 1);
    assert_eq!(limits[0].payload["kind"], "quota-exhausted");
    assert_eq!(limits[0].payload["message"], LIMIT);
    let failed: Vec<String> = events.iter().filter(|e| e.kind == "run.failed").map(said).collect();
    assert_eq!(failed, vec![MUSE_QUOTA_SENTENCE.to_owned()]);
    assert!(!MUSE_QUOTA_SENTENCE.contains("429") && !MUSE_QUOTA_SENTENCE.contains("request_id"));
}

#[test]
fn leaves_no_unhandled_line_a_saved_output_and_the_runs_usage_are_not_shown_a_stopped_tool_fails_on_its_row() {
    let events = run(&lines());
    let diagnostics: Vec<String> = events.iter().filter(|e| e.kind == "adapter.diagnostic").map(said).collect();
    assert_eq!(diagnostics, Vec::<String>::new());
    let completed: Vec<&str> = events.iter().filter(|e| e.kind == "tool.completed").map(|e| e.payload["itemId"].as_str().unwrap()).collect();
    assert_eq!(completed, vec![FETCH]);
    let stopped: Vec<(&str, &str)> = events
        .iter()
        .filter(|e| e.kind == "tool.failed")
        .map(|e| (e.payload["itemId"].as_str().unwrap(), e.payload["status"].as_str().unwrap()))
        .collect();
    assert_eq!(stopped, vec![(SEARCH, "Muse Code stopped it (main run failed)")]);
}

#[test]
fn is_still_said_once_when_only_the_runs_own_end_says_it() {
    let events = run(&[record("run.lifecycle.started", json!({})), terminal()]);
    let limits: Vec<String> = events.iter().filter(|e| e.kind == "route.limit_detected").map(said).collect();
    assert_eq!(limits, vec![LIMIT.to_owned()]);
    let last = events.last().unwrap();
    assert_eq!(last.kind, "run.failed");
    assert_eq!(said(last), MUSE_QUOTA_SENTENCE);
}

#[test]
fn names_no_reset_it_was_not_told() {
    assert_eq!(muse_quota_limit("API error 429: Subscription quota exhausted. (rate_limit_error)"), "Muse Code's plan is used up");
    let events = run(&[
        record("run.lifecycle.started", json!({})),
        record("run.terminal.failed", json!({"kind": "run_terminal", "terminal": "failed", "text": "", "reason": "API error 429: Subscription quota exhausted. (rate_limit_error)"})),
    ]);
    let limits: Vec<String> = events.iter().filter(|e| e.kind == "route.limit_detected").map(said).collect();
    assert_eq!(limits, vec!["Muse Code's plan is used up".to_owned()]);
}
