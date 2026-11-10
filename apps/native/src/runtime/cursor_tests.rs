use super::test_support::reader_check;
// Ports packages/runtime-adapters/test/cursor-events.test.ts (fixture-driven) plus launch/dispatch.
use super::{
    Event, Runtime, Turn,
    cursor::Cursor,
    cursor_events::{CursorNormalizer, is_message_fragment, tool_kind, tool_outcome},
};
use crate::{approval::Mode, ledger_write::LedgerWriter, store::Store};
use serde_json::json;
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            ".m4-cursor-{}-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/fixtures/cursor")
}
fn fixture_lines(name: &str) -> Vec<String> {
    std::fs::read_to_string(fixture_dir().join(name))
        .unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}
fn run_lines(lines: &[String], exit: Option<i32>) -> (CursorNormalizer, Vec<Event>) {
    let mut normal = CursorNormalizer::default();
    let mut events = Vec::new();
    for line in lines {
        events.extend(normal.accept_line(line));
    }
    events.push(normal.finish(false, exit.or(Some(0))));
    (normal, events)
}
fn messages(events: &[Event]) -> Vec<String> {
    let mut buffers = std::collections::HashMap::<String, String>::new();
    let mut order = Vec::new();
    for e in events {
        if e.kind != "message.delta" {
            continue;
        }
        let id = e.payload["itemId"].as_str().unwrap_or("").to_owned();
        if !buffers.contains_key(&id) {
            order.push(id.clone());
        }
        let text = e.payload["text"].as_str().unwrap_or("");
        if e.payload["operation"] == "replace" {
            buffers.insert(id, text.to_owned());
        } else {
            buffers
                .entry(id)
                .and_modify(|t| t.push_str(text))
                .or_insert(text.to_owned());
        }
    }
    order
        .into_iter()
        .map(|id| buffers.remove(&id).unwrap_or_default())
        .collect()
}
fn fake_script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/fixtures/m4-cursor.py")
}

#[test]
fn cursor_usage_from_result() {
    let lines = vec![
        json!({"type":"result","subtype":"success","is_error":false,"usage":{"inputTokens":120,"outputTokens":45,"cacheReadTokens":3000,"cacheWriteTokens":240}}).to_string(),
    ];
    let (_, events) = run_lines(&lines, Some(0));
    assert_eq!(events.last().unwrap().kind, "run.completed");
    assert_eq!(
        events.last().unwrap().payload["usage"]["inputTokens"],
        120.0
    );
    assert_eq!(
        events.last().unwrap().payload["usage"]["cacheWriteTokens"],
        240.0
    );
}

#[test]
fn cursor_usage_omitted_and_zeros() {
    let (_, events) = run_lines(
        &[json!({"type":"result","subtype":"success","is_error":false}).to_string()],
        Some(0),
    );
    assert!(events.last().unwrap().payload.get("usage").is_none());
    let (_, events) = run_lines(
        &[json!({"type":"result","subtype":"success","usage":{"inputTokens":0,"outputTokens":0,"cacheReadTokens":0,"cacheWriteTokens":0,"extra":"x"}}).to_string()],
        Some(0),
    );
    let usage = &events.last().unwrap().payload["usage"];
    assert_eq!(usage["inputTokens"], 0.0);
    assert!(usage.get("extra").is_none());
}

#[test]
fn cursor_read_only_partial_fixture() {
    let (normal, events) = run_lines(&fixture_lines("read-only-partial.jsonl"), Some(0));
    assert_eq!(events[0].kind, "run.started");
    assert_eq!(
        normal.thread.as_deref(),
        Some("5ea83b16-7669-44d8-aaf3-97a1b41d581b")
    );
    let raw = &events[0].payload["evidence"]["raw"];
    let mut keys: Vec<_> = raw.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "apiKeySource",
            "cwd",
            "model",
            "permissionMode",
            "session_id"
        ]
    );
    assert_eq!(
        messages(&events),
        [
            "I'll read the README and summarize the project in one sentence.",
            "Pebble is a tiny CLI timer whose single command, `pebble start`, counts down 25 minutes and rings a bell.",
        ]
    );
    let finals: Vec<_> = events
        .iter()
        .filter(|e| e.kind == "message.delta" && e.payload["final"] == true)
        .map(|e| e.payload["itemId"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(finals, ["msg_0", "msg_1"]);
    let tools: Vec<_> = events
        .iter()
        .filter(|e| e.kind.starts_with("tool."))
        .collect();
    assert_eq!(
        tools.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(),
        ["tool.started", "tool.completed"]
    );
    assert!(!tools[0].payload["itemId"].as_str().unwrap().contains('\n'));
    assert_eq!(tools[0].payload["toolKind"], "read");
    assert!(
        tools[0].payload["command"]
            .as_str()
            .unwrap()
            .contains("README.md")
    );
    let steps: Vec<_> = events
        .iter()
        .filter(|e| e.kind == "step.started" || e.kind == "step.completed")
        .collect();
    assert!(steps.iter().all(|e| e.payload["stepKind"] == "reasoning"));
    assert_eq!(steps.len(), 4);
    let serialized =
        serde_json::to_string(&events.iter().map(|e| &e.payload).collect::<Vec<_>>()).unwrap();
    assert!(serialized.contains("Reading README.md to"));
    let completed: Vec<_> = steps
        .iter()
        .filter(|e| e.kind == "step.completed")
        .collect();
    assert_eq!(
        completed[0].payload["message"],
        "Reading README.md to summarize the project in one sentence."
    );
    assert_eq!(
        completed[1].payload["message"],
        "I'll read the README and provide a one-sentence summary of the project."
    );
    assert!(
        steps
            .iter()
            .filter(|e| e.kind == "step.started")
            .all(|e| e.payload.get("message").is_none())
    );
    // The captured stream with a fake secret repeated in a thinking fragment.
    let token = format!("{}-test-FAKESECRET-LOCUST-7731-abcdefghijklmnop", "sk");
    let mut lines = fixture_lines("read-only-partial.jsonl");
    let index = lines
        .iter()
        .position(|line| line.contains("Reading README.md to"))
        .unwrap();
    let mut record: serde_json::Value = serde_json::from_str(&lines[index]).unwrap();
    record["text"] = json!(format!("Reading README.md to {token}"));
    lines[index] = record.to_string();
    let (_, scrubbed) = run_lines(&lines, Some(0));
    let message = &scrubbed
        .iter()
        .find(|e| e.kind == "step.completed")
        .unwrap()
        .payload["message"];
    assert_eq!(
        message,
        "Reading README.md to [redacted] summarize the project in one sentence."
    );
    assert!(
        !serde_json::to_string(&scrubbed.iter().map(|e| &e.payload).collect::<Vec<_>>())
            .unwrap()
            .contains(token.as_str())
    );
    assert_eq!(events.last().unwrap().kind, "run.completed");
}

#[test]
fn cursor_write_mode_shell_rejected_fixture() {
    let (_, events) = run_lines(&fixture_lines("write-mode-shell-rejected.jsonl"), Some(0));
    let shells: Vec<_> = events
        .iter()
        .filter(|e| e.kind.starts_with("tool.") && e.payload["toolKind"] == "shell")
        .collect();
    let failed: Vec<_> = shells.iter().filter(|e| e.kind == "tool.failed").collect();
    let completed: Vec<_> = shells
        .iter()
        .filter(|e| e.kind == "tool.completed")
        .collect();
    assert_eq!(failed.len(), 4);
    assert!(failed.iter().all(|e| e.payload["status"] == "rejected"));
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].payload["command"], "ls -la");
    let refused: Vec<_> = events
        .iter()
        .filter(|e| e.kind == "tool.failed")
        .map(|e| e.payload["command"].as_str().unwrap_or("").to_owned())
        .collect();
    assert_eq!(refused, ["dir", "dir", "cmd.exe /c dir", "dir"]);
    assert_eq!(events.last().unwrap().kind, "run.completed");
    assert_eq!(
        events.last().unwrap().payload["usage"]["inputTokens"],
        15012.0
    );
    let kinds: Vec<_> = events
        .iter()
        .filter(|e| e.kind == "tool.completed")
        .map(|e| e.payload["toolKind"].as_str().unwrap().to_owned())
        .collect();
    assert!(kinds.iter().any(|k| k == "edit"));
    assert!(kinds.iter().any(|k| k == "glob"));
    let glob = events
        .iter()
        .find(|e| e.kind == "tool.completed" && e.payload["toolKind"] == "glob")
        .unwrap();
    assert_eq!(glob.payload["command"], "* in C:\\work\\scratch");
    assert_eq!(
        events[0].payload["evidence"]["raw"]["model"],
        "Composer 2.5"
    );
}

#[test]
fn cursor_tool_outcome_helpers() {
    let (failed, status, code) =
        tool_outcome(&json!({"success":{"command":"npm test","exitCode":1}}));
    assert!(failed);
    assert_eq!(status.as_deref(), Some("exit"));
    assert_eq!(code, Some(1));
    let (failed, _, code) = tool_outcome(&json!({"success":{"command":"ls","exitCode":0}}));
    assert!(!failed);
    assert_eq!(code, Some(0));
    for shape in [
        json!({"aborted":{}}),
        json!({"timedOut":{}}),
        json!({"denied":{}}),
        json!({}),
    ] {
        assert!(tool_outcome(&shape).0);
    }
    assert_eq!(tool_kind("readToolCall"), "read");
    assert_eq!(tool_kind("shellToolCall"), "shell");
    assert!(is_message_fragment(&json!({"timestamp_ms":1})));
    assert!(!is_message_fragment(
        &json!({"timestamp_ms":1,"model_call_id":"x"})
    ));
    assert!(!is_message_fragment(&json!({"model_call_id":"x"})));
}

#[test]
fn cursor_resumed_turn_fixture() {
    let (normal, events) = run_lines(&fixture_lines("resumed-turn.jsonl"), Some(0));
    assert_eq!(
        normal.thread.as_deref(),
        Some("5ea83b16-7669-44d8-aaf3-97a1b41d581b")
    );
    assert_eq!(messages(&events), ["Pebble"]);
    let deltas: Vec<_> = events
        .iter()
        .filter(|e| e.kind == "message.delta")
        .collect();
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0].payload["final"], true);
}

#[test]
fn cursor_how_a_run_ends() {
    let (_, events) = run_lines(
        &[json!({"type":"system","subtype":"init","session_id":"s"}).to_string()],
        Some(0),
    );
    assert_eq!(events.last().unwrap().kind, "run.failed");
    let (_, events) = run_lines(
        &[json!({"type":"result","subtype":"error","is_error":true,"result":"nope"}).to_string()],
        Some(0),
    );
    assert_eq!(events.last().unwrap().kind, "run.failed");
    let mut normal = CursorNormalizer::default();
    let cancelled = normal.finish(true, Some(0));
    assert_eq!(cancelled.kind, "run.cancelled");
    let mut normal = CursorNormalizer::default();
    let bad = normal.accept_line("{not json");
    assert_eq!(bad[0].kind, "adapter.diagnostic");
    assert_eq!(bad[0].payload["code"], "cursor.malformed_record");
}

#[test]
fn cursor_unknown_and_connection_once() {
    let lines = vec![
        json!({"type":"connection","subtype":"reconnecting","attempt":2}).to_string(),
        json!({"type":"retry","subtype":"starting","is_resume":true,"attempt":2}).to_string(),
        json!({"type":"interaction_query","toolCallId":"x"}).to_string(),
        json!({"type":"weird_one"}).to_string(),
        json!({"type":"weird_one"}).to_string(),
        json!({"type":"weird_two"}).to_string(),
        json!({"type":"result","subtype":"success","is_error":false}).to_string(),
    ];
    let (_, events) = run_lines(&lines, Some(0));
    assert!(events.iter().any(|e| {
        e.kind == "adapter.diagnostic"
            && e.payload["message"]
                .as_str()
                .unwrap()
                .contains("reconnecting")
    }));
    assert!(events.iter().any(|e| {
        e.kind == "adapter.diagnostic"
            && e.payload["message"]
                .as_str()
                .unwrap()
                .contains("picking the turn back up")
    }));
    assert!(!events.iter().any(|e| {
        e.payload["message"]
            .as_str()
            .unwrap_or("")
            .contains("interaction_query")
    }));
    let unknown: Vec<_> = events
        .iter()
        .filter(|e| e.payload["code"] == "cursor.unknown_event")
        .collect();
    assert_eq!(unknown.len(), 2);
}

#[test]
fn cursor_plan_fixture_does_not_shrink() {
    let (_, events) = run_lines(&fixture_lines("todo-plan.jsonl"), Some(0));
    let plans: Vec<_> = events.iter().filter(|e| e.kind == "plan.updated").collect();
    assert!(!plans.is_empty());
    let last = plans.last().unwrap();
    let len = last.payload["plan"].as_array().unwrap().len();
    assert!(len >= 3, "plan must not shrink to a partial merge: {len}");
    assert!(
        events
            .iter()
            .filter(|e| e.kind.starts_with("tool.") && e.payload["toolKind"] == "updateTodos")
            .count()
            == 0
    );
}

#[test]
fn cursor_replace_never_takes_text_back() {
    let long = "x".repeat(20_000);
    let half = "x".repeat(10_000);
    let lines = vec![
        json!({"type":"system","subtype":"init","session_id":"s","model":"Auto","permissionMode":"default","cwd":"/","apiKeySource":"login"}).to_string(),
        json!({"type":"assistant","timestamp_ms":1,"message":{"content":[{"type":"text","text":half}]}}).to_string(),
        json!({"type":"assistant","timestamp_ms":2,"message":{"content":[{"type":"text","text":half}]}}).to_string(),
        // Complete message shorter than delivered after bound — empty append final.
        json!({"type":"assistant","model_call_id":"m","message":{"content":[{"type":"text","text":long}]}}).to_string(),
        json!({"type":"result","subtype":"success","is_error":false}).to_string(),
    ];
    let (_, events) = run_lines(&lines, Some(0));
    let finals: Vec<_> = events
        .iter()
        .filter(|e| e.kind == "message.delta" && e.payload["final"] == true)
        .collect();
    assert!(!finals.is_empty());
    assert!(
        finals
            .iter()
            .any(|e| e.payload["operation"] == "append" && e.payload["text"] == "")
    );
}

#[test]
fn cursor_fake_process_writes_ledger_accepted_by_ts_reader() {
    let temp = Temp::new();
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    std::fs::write(store.electron().join("sentinel"), "untouched").unwrap();
    let mut command = Command::new("python3");
    command
        .arg(fake_script())
        .arg("read-only-partial")
        .current_dir(&temp.0);
    let mut rt = Cursor::spawn(&mut command).unwrap();
    let turn = Turn {
        prompt: "Synthetic Cursor prompt".into(),
        folder: temp.0.clone(),
        model: None,
        effort: None,
        resume: None,
        mode: Mode::Ask,
        approvals: None,
        mission_id: "cursor_ro".into(),
        run_id: "run_cursor_ro".into(),
    };
    let mut writer = LedgerWriter::create_runtime(
        &store,
        "cursor_ro",
        &turn.prompt,
        "account-default",
        "fake-cursor",
        "cursor",
        &[],
        None,
    )
    .unwrap();
    rt.start(&turn).unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut events = vec![];
    loop {
        assert!(Instant::now() < deadline, "cursor fake timed out");
        if let Some(e) = rt.next_event().unwrap() {
            let terminal = matches!(
                e.kind.as_str(),
                "run.completed" | "run.failed" | "run.cancelled"
            );
            writer
                .append(&e.kind, e.payload.clone(), e.thread.as_deref())
                .unwrap();
            events.push(e);
            if terminal {
                break;
            }
        }
    }
    let last = events.last().unwrap();
    assert_eq!(
        last.kind,
        "run.completed",
        "kinds={:?} last={}",
        events.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(),
        last.payload
    );
    assert!(events.iter().any(|e| e.kind == "message.delta"));
    assert_eq!(
        std::fs::read_to_string(store.electron().join("sentinel")).unwrap(),
        "untouched"
    );
    reader_check(&store, "cursor_ro", "completed");
}

#[test]
fn cursor_teammate_uses_native_driver() {
    let t = Temp::new();
    let store = Store::new(t.0.join("native"), t.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    let _guard = crate::native::hooks::install(
        store.clone(),
        Some((
            "python3".into(),
            vec![
                fake_script().to_string_lossy().into_owned(),
                "read-only-partial".into(),
            ],
        )),
    );
    let team = crate::native::parse_roster(
        &json!({"schemaVersion":1,"teammates":[{"teammateId":"c1","name":"Cursor","role":"Custom","roleTitle":"Test","hue":"lime","createdAt":"2026-11-09T12:00:00Z","folder":t.0,"route":{"runtime":"cursor","model":"account-default","mode":"ask"}}]}).to_string(),
    )
    .unwrap();
    assert!(crate::native::supported(&team[0]));
    assert_ne!(team[0].runs_in, crate::native::IN_LOCUST);
    let started = crate::native::send(crate::native::Send {
        teammate: team[0].clone(),
        conversation: None,
        prompt: "Synthetic Cursor prompt".into(),
        model: None,
        effort: None,
    })
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    while crate::native::owns(&started.conversation_id) {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    let ledger = std::fs::read_to_string(
        store
            .ledger_dir()
            .unwrap()
            .join(format!("{}.jsonl", started.conversation_id)),
    )
    .unwrap();
    assert!(ledger.contains("run.completed"));
    assert_eq!(
        crate::native::prior_of(&started.conversation_id)
            .unwrap()
            .thread
            .as_deref(),
        Some("5ea83b16-7669-44d8-aaf3-97a1b41d581b")
    );
}

#[test]
#[ignore]
fn real_cursor_one_line() {
    assert_eq!(
        std::env::var("LOCUST_REAL_CURSOR").ok().as_deref(),
        Some("1"),
        "set LOCUST_REAL_CURSOR=1 to run this real test"
    );
    let temp = Temp::new();
    let electron = temp.0.join("electron");
    let native = temp.0.join("native");
    std::fs::create_dir_all(&electron).unwrap();
    let store = Store::new(native, electron).unwrap();
    let folder = temp.0.join("ws");
    std::fs::create_dir_all(&folder).unwrap();
    let mut command = crate::runtime::cursor::command(&folder, Mode::Ask, None, None).unwrap();
    let mut rt = Cursor::spawn(&mut command).unwrap();
    let turn = Turn {
        prompt: "Reply with exactly: ok".into(),
        folder,
        model: None,
        effort: None,
        resume: None,
        mode: Mode::Ask,
        approvals: None,
        mission_id: "cursor_real".into(),
        run_id: "run_cursor_real".into(),
    };
    let mut writer = LedgerWriter::create_runtime(
        &store,
        "cursor_real",
        &turn.prompt,
        "account-default",
        "cursor-agent",
        "cursor",
        &[],
        None,
    )
    .unwrap();
    rt.start(&turn).unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        assert!(Instant::now() < deadline, "real cursor timed out");
        if let Some(e) = rt.next_event().unwrap() {
            let terminal = matches!(
                e.kind.as_str(),
                "run.completed" | "run.failed" | "run.cancelled"
            );
            writer
                .append(&e.kind, e.payload.clone(), e.thread.as_deref())
                .unwrap();
            if terminal {
                assert_eq!(e.kind, "run.completed");
                break;
            }
        }
    }
}

#[test]
fn print_only_runtimes_refuse_unsupported_approve_each_before_launch() {
    let folder = std::path::Path::new("/unused");
    for result in [
        crate::runtime::cursor::command(folder, Mode::ApproveEach, None, None),
        crate::runtime::antigravity::command(folder, Mode::ApproveEach, None, None, None),
        crate::runtime::muse::command(folder, Mode::ApproveEach, None, None, None),
    ] {
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("does not support Approve-each")
        );
    }
}

#[test]
fn cursor_thinking_matches_ts_fragment_scrubbing_and_completed_message() {
    let mut n = CursorNormalizer::default();
    assert!(
        n.accept(&json!({"type":"thinking","subtype":"completed","text":"ignored"}))
            .is_empty()
    );
    let start = n.accept(&json!({"type":"thinking","subtype":"delta","text":format!("  The key {}-test-FAKESECRET-LOCUST-7731-abcdefghijklmnop looks like ", "sk")}));
    assert_eq!(start.len(), 1);
    assert_eq!(start[0].kind, "step.started");
    assert!(start[0].payload.get("message").is_none());
    assert!(n.accept(&json!({"type":"thinking","subtype":"delta","text":"a Stripe test secret. Bearer abcdefghijklmnop"})).is_empty());
    let done = n.accept(
        &json!({"type":"thinking","subtype":"completed","text":"completion text is not a delta"}),
    );
    assert_eq!(done.len(), 1);
    assert_eq!(done[0].kind, "step.completed");
    assert_eq!(
        done[0].payload["message"],
        "The key [redacted] looks like a Stripe test secret. Bearer [redacted]"
    );
    assert!(
        n.accept(&json!({"type":"thinking","subtype":"delta","text":"  "}))[0]
            .payload
            .get("message")
            .is_none()
    );
    let empty = n.accept(&json!({"type":"thinking","subtype":"completed"}));
    assert!(empty[0].payload.get("message").is_none());
}

#[test]
fn cursor_thinking_bounds_only_on_close_and_keeps_both_ends() {
    let mut n = CursorNormalizer::default();
    // A fragment over the former 1M cap must not be truncated before joining.
    n.accept(&json!({"type":"thinking","subtype":"delta","text":format!("HEAD{}", "x".repeat(1_000_100))}));
    n.accept(&json!({"type":"thinking","subtype":"delta","text":"y".repeat(4092)}));
    n.accept(&json!({"type":"thinking","subtype":"delta","text":"TAIL"}));
    let done = n.accept(&json!({"type":"thinking","subtype":"completed"}));
    let message = done[0].payload["message"].as_str().unwrap();
    assert_eq!(
        message,
        format!(
            "HEAD{}\n…[truncated]…\n{}TAIL",
            "x".repeat(12269),
            "y".repeat(4092)
        )
    );
    assert_eq!(message.encode_utf16().count(), 16384);
}
