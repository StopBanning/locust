use super::test_support::reader_check;
// Ports packages/runtime-adapters/test/agy-events.test.ts (fixture-driven) plus launch/dispatch.
use super::{
    Event, Runtime, Turn,
    agy_events::{AgyNormalizer, QUOTA_SENTENCE, quota_limit},
    antigravity::Antigravity,
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
            ".m4-agy-{}-{}-{}",
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
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/fixtures/agy")
}
fn fixture_lines(name: &str) -> Vec<String> {
    std::fs::read_to_string(fixture_dir().join(format!("{name}.jsonl")))
        .unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}
fn run_lines(lines: &[String], exit: Option<i32>, sandbox: &str) -> (AgyNormalizer, Vec<Event>) {
    let mut normal = AgyNormalizer::default().with_sandbox(sandbox);
    let mut events = Vec::new();
    for line in lines {
        events.extend(normal.accept_line(line));
    }
    events.extend(normal.finish(false, exit.or(Some(0))));
    (normal, events)
}
fn fake_script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/fixtures/m4-agy.py")
}

#[test]
fn agy_reads_a_file_fixture() {
    let (normal, events) = run_lines(&fixture_lines("reads-a-file"), Some(0), "read-only");
    assert_eq!(
        normal.thread.as_deref(),
        Some("c0da4624-b6cd-4e9e-9979-87b67b7abe60")
    );
    let tools: Vec<_> = events
        .iter()
        .filter(|e| e.kind.starts_with("tool."))
        .collect();
    assert_eq!(
        tools.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(),
        ["tool.started", "tool.completed"]
    );
    assert_eq!(tools[0].payload["name"], "view_file");
    assert!(
        tools[0].payload["command"]
            .as_str()
            .unwrap()
            .contains("note.txt")
    );
    let final_msg = events
        .iter()
        .rev()
        .find(|e| e.kind == "message.delta" && e.payload["final"] == true)
        .unwrap();
    assert!(
        final_msg.payload["text"]
            .as_str()
            .unwrap()
            .contains("hello from a file")
    );
    assert_eq!(events.last().unwrap().kind, "run.completed");
    assert_eq!(
        events.last().unwrap().payload["usage"]["inputTokens"],
        20836.0
    );
}

#[test]
fn agy_answered_then_unavailable() {
    let (_, events) = run_lines(
        &fixture_lines("answered-then-unavailable"),
        Some(1),
        "read-only",
    );
    assert!(events.iter().any(|e| {
        e.kind == "adapter.diagnostic"
            && e.payload["code"] == "antigravity.unavailable_after_answer"
    }));
    assert_eq!(events.last().unwrap().kind, "run.completed");
    assert!(!events.iter().any(|e| e.kind == "run.failed"));
}

#[test]
fn agy_write_denied() {
    let (_, events) = run_lines(&fixture_lines("write-denied"), Some(0), "read-only");
    let tools: Vec<_> = events
        .iter()
        .filter(|e| e.kind.starts_with("tool."))
        .map(|e| e.kind.as_str())
        .collect();
    assert_eq!(tools, ["tool.started", "tool.failed"]);
    assert!(events.iter().any(|e| {
        e.kind == "adapter.diagnostic"
            && e.payload["message"]
                .as_str()
                .unwrap_or("")
                .contains("not allowed to change files")
    }));
    assert_eq!(events.last().unwrap().kind, "run.completed");
}

#[test]
fn agy_edit_then_command_denied() {
    let (_, events) = run_lines(
        &fixture_lines("edit-then-command-denied"),
        Some(0),
        "workspace-write",
    );
    let tools: Vec<_> = events
        .iter()
        .filter(|e| e.kind.starts_with("tool."))
        .map(|e| format!("{}:{}", e.kind, e.payload["name"].as_str().unwrap_or("")))
        .collect();
    assert_eq!(
        tools,
        [
            "tool.started:write_to_file",
            "tool.completed:write_to_file",
            "tool.started:run_command",
            "tool.failed:run_command",
        ]
    );
    let failed = events
        .iter()
        .rev()
        .find(|e| e.kind == "tool.failed")
        .unwrap();
    assert_eq!(failed.payload["status"], "refused");
}

#[test]
fn agy_quota_used_up() {
    let mut normal = AgyNormalizer::default();
    let raw = r#"{"event":"result","result":{"conversation_id":"c1","status":"ERROR","response":"","error":"Individual quota reached. Please upgrade your subscription to increase your limits. Resets in 33h54m24s."}}"#;
    let mut events = normal.accept_line(raw);
    // Fixed clock: 2026-10-22T17:03:44Z
    let limit = quota_limit(
        "Individual quota reached. Please upgrade your subscription to increase your limits. Resets in 33h54m24s.",
        1_792_688_624_000,
    );
    assert_eq!(
        limit.as_deref(),
        Some("Antigravity's quota is used up · resets 2026-10-24T02:58:08.000Z")
    );
    events.extend(normal.finish(false, Some(1)));
    assert!(events.iter().any(|e| e.kind == "route.limit_detected"));
    assert_eq!(events.last().unwrap().kind, "run.failed");
    assert_eq!(events.last().unwrap().payload["message"], QUOTA_SENTENCE);
    assert_eq!(
        quota_limit("RESOURCE_EXHAUSTED", 0).as_deref(),
        Some("Antigravity's quota is used up")
    );
    assert!(quota_limit("API error UNAVAILABLE", 0).is_none());
}

#[test]
fn agy_conversation_not_found() {
    let (_, events) = run_lines(
        &fixture_lines("conversation-not-found"),
        Some(1),
        "read-only",
    );
    let failed = events.last().unwrap();
    assert_eq!(failed.kind, "run.failed");
    assert_eq!(failed.payload["sessionEnded"], true);
    assert!(
        failed.payload["message"]
            .as_str()
            .unwrap()
            .contains("began in the Antigravity app")
    );
}

#[test]
fn agy_unknown_model() {
    let (_, events) = run_lines(&fixture_lines("unknown-model"), Some(1), "read-only");
    assert_eq!(events.last().unwrap().kind, "run.failed");
    assert!(
        events.last().unwrap().payload["message"]
            .as_str()
            .unwrap()
            .contains("no-such-model")
    );
}

#[test]
fn agy_print_args_and_user_line() {
    let ask = crate::runtime::antigravity::print_args(Mode::Ask, Some("gemini"), None, None);
    assert!(!ask.iter().any(|a| a == "--dangerously-skip-permissions"));
    assert!(ask.windows(2).any(|w| w == ["--model", "gemini"]));
    assert!(ask.contains(&"-p=".into()));
    let auto = crate::runtime::antigravity::print_args(Mode::Auto, None, Some("low"), Some("c1"));
    assert!(auto.contains(&"--dangerously-skip-permissions".into()));
    let parsed: serde_json::Value =
        serde_json::from_slice(&crate::runtime::antigravity::user_line("Fix it")).unwrap();
    assert_eq!(parsed["event"], "user");
    assert_eq!(parsed["message"]["content"][0]["text"], "Fix it");
}

#[test]
fn agy_fake_process_writes_ledger() {
    let temp = Temp::new();
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    let mut command = Command::new("python3");
    command
        .arg(fake_script())
        .arg("reads-a-file")
        .current_dir(&temp.0);
    let mut rt = Antigravity::spawn(&mut command).unwrap();
    let turn = Turn {
        prompt: "Synthetic Antigravity prompt".into(),
        folder: temp.0.clone(),
        model: None,
        effort: None,
        resume: None,
        mode: Mode::Ask,
        approvals: None,
        mission_id: "agy_ro".into(),
        run_id: "run_agy_ro".into(),
    };
    let mut writer = LedgerWriter::create_runtime(
        &store,
        "agy_ro",
        &turn.prompt,
        "account-default",
        "fake-agy",
        "antigravity",
        &[],
        None,
    )
    .unwrap();
    rt.start(&turn).unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut events = vec![];
    loop {
        assert!(Instant::now() < deadline, "agy fake timed out");
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
    assert_eq!(events.last().unwrap().kind, "run.completed");
    reader_check(&store, "agy_ro", "completed");
}

#[test]
fn agy_teammate_uses_native_driver() {
    let t = Temp::new();
    let store = Store::new(t.0.join("native"), t.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    let _guard = crate::native::hooks::install(
        store.clone(),
        Some((
            "python3".into(),
            vec![
                fake_script().to_string_lossy().into_owned(),
                "reads-a-file".into(),
            ],
        )),
    );
    let team = crate::native::parse_roster(
        &json!({"schemaVersion":1,"teammates":[{"teammateId":"a1","name":"Agy","role":"Custom","roleTitle":"Test","hue":"lime","createdAt":"2026-11-09T12:00:00Z","folder":t.0,"route":{"runtime":"antigravity","model":"account-default","mode":"ask"}}]}).to_string(),
    )
    .unwrap();
    assert!(crate::native::supported(&team[0]));
    let started = crate::native::send(crate::native::Send {
        teammate: team[0].clone(),
        conversation: None,
        prompt: "Synthetic Antigravity prompt".into(),
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
}

#[test]
#[ignore]
fn real_antigravity_one_line() {
    assert_eq!(
        std::env::var("LOCUST_REAL_ANTIGRAVITY").ok().as_deref(),
        Some("1"),
        "set LOCUST_REAL_ANTIGRAVITY=1 to run this real test"
    );
    let temp = Temp::new();
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    let folder = temp.0.join("ws");
    std::fs::create_dir_all(&folder).unwrap();
    let mut command =
        crate::runtime::antigravity::command(&folder, Mode::Ask, None, None, None).unwrap();
    let mut rt = Antigravity::spawn(&mut command).unwrap();
    let turn = Turn {
        prompt: "Reply with exactly: ok".into(),
        folder,
        model: None,
        effort: None,
        resume: None,
        mode: Mode::Ask,
        approvals: None,
        mission_id: "agy_real".into(),
        run_id: "run_agy_real".into(),
    };
    let mut writer = LedgerWriter::create_runtime(
        &store,
        "agy_real",
        &turn.prompt,
        "account-default",
        "agy",
        "antigravity",
        &[],
        None,
    )
    .unwrap();
    rt.start(&turn).unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        assert!(Instant::now() < deadline);
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
fn agy_quota_precedes_failure_and_finish_is_once() {
    for error in [
        "Individual quota reached. Resets in 33h54m24s.",
        "RESOURCE_EXHAUSTED",
    ] {
        let mut n = AgyNormalizer::default();
        n.accept(&json!({"event":"result","result":{"conversation_id":"c1","status":"ERROR","response":"","error":error}}));
        let events = n.finish(false, Some(1));
        assert_eq!(
            events.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(),
            ["route.limit_detected", "run.failed"]
        );
        assert_eq!(events[0].payload["kind"], "quota-exhausted");
        assert_eq!(events[0].payload["evidence"]["runtimeEventType"], "result");
        assert!(n.finish(false, Some(1)).is_empty());
    }
}

#[test]
fn agy_final_strips_earlier_messages_in_arrival_order() {
    for _ in 0..25 {
        let mut n = AgyNormalizer::default();
        for (i, words) in [(1, "First."), (2, "Second."), (3, "Answer.")] {
            n.accept(&json!({"event":"step_update","step_update":{"step_index":i,"step_type":"agent_response","state":"DONE","text_delta":words}}));
        }
        let e=n.accept(&json!({"event":"result","result":{"status":"SUCCESS","response":"First. Second. Answer."}}));
        assert_eq!(
            e.iter()
                .find(|e| e.kind == "message.delta")
                .unwrap()
                .payload["text"],
            "Answer."
        );
    }
}

#[test]
fn agy_waiting_helper_and_third_uncached_reread() {
    let mut n = AgyNormalizer::default();
    let list = "You have 1 active subagent(s):\n[{\"role\":\"Soak & Sweep Runner\",\"state\":\"waiting_for_dependents\",\"lastStepTime\":\"2026-10-24T07:18:51Z\"}]";
    n.accept(&json!({"event":"step_update","step_update":{"step_index":1,"step_type":"tool","state":"DONE","tool_name":"manage_subagents","tool_info":{"output":list}}}));
    let mut warnings = 0;
    for i in 2..8 {
        let e=n.accept(&json!({"event":"step_update","step_update":{"step_index":i,"step_type":"agent_response","state":"DONE","duration_seconds":309.3,"usage":{"thinking_tokens":98,"input_tokens":229594,"cache_read_tokens":0}}}));
        let thought = e.iter().find(|e| e.kind == "step.completed").unwrap();
        assert_eq!(thought.payload["durationMs"], 309300);
        assert!(
            thought.payload["message"]
                .as_str()
                .unwrap()
                .contains("Waiting on its helper Soak & Sweep Runner")
        );
        for event in &e {
            if event.payload["code"] == "antigravity.rereading" {
                warnings += 1;
                assert_eq!(i, 4);
                assert!(
                    event.payload["message"]
                        .as_str()
                        .unwrap()
                        .contains("689,000 tokens")
                );
            }
        }
    }
    assert_eq!(warnings, 1);
    n.accept(&json!({"event":"step_update","step_update":{"step_index":8,"step_type":"tool","state":"DONE","tool_name":"manage_subagents","tool_info":{"output":"You have no active subagents."}}}));
    let e=n.accept(&json!({"event":"step_update","step_update":{"step_index":9,"step_type":"agent_response","state":"DONE","duration_seconds":5,"usage":{"thinking_tokens":10,"input_tokens":229594,"cache_read_tokens":228000}}}));
    assert!(e.iter().all(|e| e.payload.get("message").is_none()));
}

#[test]
fn agy_real_quota_shape_reaches_the_ledger_before_failure() {
    let temp = Temp::new();
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    let mut c = Command::new("python3");
    c.arg(fake_script()).arg("quota-exhausted");
    let mut rt = Antigravity::spawn(&mut c).unwrap();
    let mut w = LedgerWriter::create_runtime(
        &store,
        "agy_quota_review",
        "Synthetic quota stop",
        "account-default",
        "agy",
        "antigravity",
        &[],
        None,
    )
    .unwrap();
    rt.start(&Turn {
        prompt: "Synthetic quota stop".into(),
        folder: temp.0.clone(),
        model: None,
        effort: None,
        resume: None,
        mode: Mode::Ask,
        approvals: None,
        mission_id: "agy_quota_review".into(),
        run_id: "run_agy_quota_review".into(),
    })
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut kinds = vec![];
    loop {
        assert!(Instant::now() < deadline);
        if let Some(e) = rt.next_event().unwrap() {
            w.append(&e.kind, e.payload, e.thread.as_deref()).unwrap();
            kinds.push(e.kind.clone());
            if e.kind == "run.failed" {
                break;
            }
        }
    }
    assert_eq!(
        &kinds[kinds.len() - 2..],
        ["route.limit_detected", "run.failed"]
    );
    reader_check(&store, "agy_quota_review", "failed");
}
