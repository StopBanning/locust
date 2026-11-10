use super::{
    Runtime, Turn,
    claude::Claude,
    codex::{Codex, read_only_config},
};
use crate::{
    ledger::{self, Item},
    ledger_write::LedgerWriter,
    store::Store,
};
use serde_json::json;
use std::{
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "locust-m1-{}-{}",
            std::process::id(),
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
fn run(scenario: &str, keep: Option<PathBuf>) {
    let temp = Temp::new();
    let root = keep.unwrap_or_else(|| temp.0.join("native"));
    let store = Store::new(root, temp.0.join("electron")).unwrap();
    let mut writer = LedgerWriter::create(
        &store,
        "synthetic",
        "Synthetic prompt",
        "synthetic-model",
        "fake-codex",
    )
    .unwrap();
    // create_new prevents a second writer from acquiring this mission.
    assert!(
        LedgerWriter::create(
            &store,
            "synthetic",
            "Synthetic prompt",
            "synthetic-model",
            "fake-codex"
        )
        .is_err()
    );
    let mut command = Command::new("python3");
    command
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scripts/fixtures/m1-app-server.py"
        ))
        .arg(scenario);
    let mut runtime = Codex::spawn(&mut command).unwrap();
    runtime
        .start(&Turn {
            prompt: "Synthetic prompt".into(),
            folder: temp.0.clone(),
            model: Some("synthetic-model".into()),
            effort: None,
            resume: None,
            mode: crate::approval::Mode::Ask,
            approvals: None,
            mission_id: "synthetic".into(),
            run_id: "run_synthetic".into(),
        })
        .unwrap();
    if scenario == "interrupt" {
        runtime.stop().unwrap();
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut kinds = vec![];
    loop {
        assert!(Instant::now() < deadline, "fake timed out: {kinds:?}");
        if let Some(e) = runtime.next_event().unwrap() {
            writer
                .append(&e.kind, e.payload, e.thread.as_deref())
                .unwrap();
            let done = ["run.completed", "run.failed", "run.cancelled"].contains(&e.kind.as_str());
            kinds.push(e.kind);
            if done {
                break;
            }
        }
    }
    let contents = std::fs::read_to_string(&writer.path).unwrap();
    assert!(!contents.contains("RAW_REASONING_MUST_NOT_SURVIVE"));
    let transcript = ledger::read_transcript(&writer.path);
    assert!(transcript.error.is_none());
    assert!(!transcript.running);
    assert_eq!(
        kinds.iter().filter(|s| s.as_str() == "run.started").count(),
        1
    );
    match scenario {
        "success" => {
            assert_eq!(kinds.last().unwrap(), "run.completed");
            assert!(contents.contains("Checking the fixture"));
            assert!(kinds.iter().any(|s| s == "adapter.diagnostic"));
            assert!(
                transcript
                    .items
                    .iter()
                    .any(|i| matches!(i,Item::User(t) if t=="Synthetic prompt"))
            );
            assert_eq!(
                transcript
                    .items
                    .iter()
                    .filter(|i| matches!(i, Item::Assistant { .. }))
                    .count(),
                1
            );
            assert!(
                transcript
                    .items
                    .iter()
                    .any(|i| matches!(i,Item::Assistant{text,..} if text=="Hello world"))
            );
        }
        "interrupt" => assert_eq!(kinds.last().unwrap(), "run.cancelled"),
        _ => {
            assert_eq!(kinds.last().unwrap(), "run.failed");
            assert!(transcript.failed);
        }
    }
    if let Ok(reader) = std::env::var("LOCUST_TEST_TS_READER") {
        let phase = match scenario {
            "success" => "completed",
            "interrupt" => "cancelled",
            _ => "failed",
        };
        let status = Command::new("node")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/scripts/check-m1-ledger.mjs"
            ))
            .arg(reader)
            .arg(store.ledger_dir().unwrap())
            .arg("synthetic")
            .arg(phase)
            .status()
            .unwrap();
        assert!(status.success(), "TypeScript reader rejected Rust ledger");
    }
}
#[test]
fn fake_server_to_native_transcript() {
    run("success", None);
}
#[test]
fn fake_failure_records_limit_and_failure() {
    run("failure", None);
}
#[test]
fn fake_transport_loss_is_failure() {
    run("lost", None);
}
#[test]
fn interrupt_is_sent_and_recorded() {
    run("interrupt", None);
}
#[test]
fn ask_config_fails_closed() {
    assert!(read_only_config(&json!(null)).is_err());
    assert!(read_only_config(&json!({"mcp_servers":[]})).is_err());
    assert!(read_only_config(&json!({"mcp_servers":{"unsafe.name":{"command":"node"}}})).is_err());
}
#[test]
fn text_is_bounded_and_scrubbed() {
    let input = format!("hello\0 {}-abcdefghijklmnop password=hunter2", "sk");
    let clean = super::events::text(&input, 16384);
    assert!(
        !clean.contains("hunter2") && !clean.contains("abcdefghijklmnop") && !clean.contains('\0')
    );
    let long = format!("{}TAIL", "x".repeat(20000));
    let bounded = super::events::text(&long, 16384);
    assert!(bounded.encode_utf16().count() <= 16384 && bounded.ends_with("TAIL"));
}

#[test]
fn snapshot_limits_are_deduplicated_and_raw_content_deltas_are_ignored() {
    let mut n = super::events::Normalizer::default();
    let params = json!({"rateLimits":{"primary":{"usedPercent":100,"windowDurationMins":300,"resetsAt":1800000000}}});
    let first = n.accept("account/rateLimits/updated", &params);
    assert_eq!(first.len(), 2);
    assert_eq!(first[1].kind, "route.limit_detected");
    assert_eq!(first[1].payload["kind"], "quota-exhausted");
    assert!(n.accept("account/rateLimits/updated", &params).is_empty());
    assert!(
        n.accept(
            "item/reasoning/textDelta",
            &json!({"delta":"hidden raw content"})
        )
        .is_empty()
    );
    let events = n.accept("item/completed", &json!({"item":{"id":"refused","type":"commandExecution","status":"declined","exitCode":1}}));
    assert_eq!(events[0].kind, "tool.failed");
}

#[test]
fn writer_refuses_external_changes_and_invalid_ids() {
    use std::io::Write;
    let temp = Temp::new();
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    assert!(LedgerWriter::create(&store, "../escape", "prompt", "model", "fake").is_err());
    let mut w = LedgerWriter::create(&store, "safe", "prompt", "model", "fake").unwrap();
    let mut external = std::fs::OpenOptions::new()
        .append(true)
        .open(&w.path)
        .unwrap();
    external.write_all(b"\n").unwrap();
    assert!(w.append("run.started", json!({}), None).is_err());
    assert!(w.append("run.started", json!({}), None).is_err());
}

fn run_claude(scenario: &str) {
    let temp = Temp::new();
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    let args = crate::runtime::claude::ask_args(Some("synthetic-model"));
    let mut writer = LedgerWriter::create_runtime(
        &store,
        "synthetic_claude",
        "Synthetic prompt",
        "synthetic-model",
        "fake-claude",
        "claude",
        &args,
        None,
    )
    .unwrap();
    let mut command = Command::new("python3");
    command
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scripts/fixtures/m3-claude.py"
        ))
        .arg(scenario);
    let mut runtime = Claude::spawn(&mut command).unwrap();
    runtime
        .start(&Turn {
            prompt: "Synthetic prompt".into(),
            folder: temp.0.clone(),
            model: Some("synthetic-model".into()),
            effort: None,
            resume: None,
            mode: crate::approval::Mode::Ask,
            approvals: None,
            mission_id: "synthetic".into(),
            run_id: "run_synthetic".into(),
        })
        .unwrap();
    if scenario == "interrupt" {
        // Wait until init lands, then stop.
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            assert!(Instant::now() < deadline, "init never arrived");
            if let Some(e) = runtime.next_event().unwrap() {
                writer
                    .append(&e.kind, e.payload.clone(), e.thread.as_deref())
                    .unwrap();
                if e.kind == "run.started" {
                    runtime.stop().unwrap();
                    break;
                }
            }
        }
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut kinds = vec![];
    loop {
        assert!(
            Instant::now() < deadline,
            "fake claude timed out: {kinds:?}"
        );
        if let Some(e) = runtime.next_event().unwrap() {
            writer
                .append(&e.kind, e.payload.clone(), e.thread.as_deref())
                .unwrap();
            let done = ["run.completed", "run.failed", "run.cancelled"].contains(&e.kind.as_str());
            kinds.push(e.kind);
            if done {
                break;
            }
        }
    }
    let contents = std::fs::read_to_string(&writer.path).unwrap();
    assert!(!contents.contains("RAW_REASONING_MUST_NOT_SURVIVE"));
    assert!(!contents.contains("Gmail"));
    assert!(!contents.contains("powershell"));
    let transcript = ledger::read_transcript(&writer.path);
    assert!(transcript.error.is_none(), "{:?}", transcript.error);
    assert!(!transcript.running);
    match scenario {
        "success" | "thinking" | "tools" | "approval" | "limit" => {
            assert_eq!(kinds.last().unwrap(), "run.completed");
            if scenario != "approval" && scenario != "limit" {
                assert!(
                    transcript
                        .items
                        .iter()
                        .any(|i| matches!(i, Item::Assistant { text, .. } if text.contains("Hello world"))),
                    "{transcript:?}"
                );
            }
            if scenario == "thinking" {
                assert!(kinds.iter().any(|k| k == "step.started"));
                assert!(kinds.iter().any(|k| k == "step.completed"));
            }
            if scenario == "tools" {
                assert!(kinds.iter().any(|k| k == "tool.started"));
                assert!(kinds.iter().any(|k| k == "tool.completed"));
            }
            if scenario == "approval" {
                // Headless Ask auto-denies; no mission.approval record without a card answer.
                assert!(
                    contents.contains("without an interactive")
                        || contents.contains("declines approvals")
                        || !contents.contains("mission.approval"),
                    "{contents}"
                );
            }
            if scenario == "limit" {
                assert!(kinds.iter().any(|k| k == "route.limit_detected"));
            }
        }
        "interrupt" => assert_eq!(kinds.last().unwrap(), "run.cancelled"),
        "failure" | "lost" => {
            assert_eq!(kinds.last().unwrap(), "run.failed");
            assert!(transcript.failed);
        }
        other => panic!("unknown scenario {other}"),
    }
    if let Ok(reader) = std::env::var("LOCUST_TEST_TS_READER") {
        let phase = match scenario {
            "interrupt" => "cancelled",
            "failure" | "lost" => "failed",
            _ => "completed",
        };
        let status = Command::new("node")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/scripts/check-m1-ledger.mjs"
            ))
            .arg(reader)
            .arg(store.ledger_dir().unwrap())
            .arg("synthetic_claude")
            .arg(phase)
            .status()
            .unwrap();
        assert!(status.success(), "TypeScript reader rejected Claude ledger");
    }
}

#[test]
fn claude_fake_text_and_completion() {
    run_claude("success");
}
#[test]
fn claude_fake_tools() {
    run_claude("tools");
}
#[test]
fn claude_fake_thinking_is_scrubbed() {
    run_claude("thinking");
}
#[test]
fn claude_fake_limit() {
    run_claude("limit");
}
#[test]
fn claude_fake_failure() {
    run_claude("failure");
}
#[test]
fn claude_fake_lost() {
    run_claude("lost");
}
#[test]
fn claude_fake_interrupt() {
    run_claude("interrupt");
}
#[test]
fn claude_fake_approval_declined() {
    run_claude("approval");
}

#[test]
#[ignore = "real Claude Code turn; needs an installed, signed-in claude on PATH"]
fn real_claude_one_line_prompt() {
    assert_eq!(
        std::env::var("LOCUST_REAL_CLAUDE").ok().as_deref(),
        Some("1"),
        "set LOCUST_REAL_CLAUDE=1 to run this real test"
    );
    let temp = Temp::new();
    let folder = temp.0.join("workspace");
    std::fs::create_dir_all(&folder).unwrap();
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    let mut command = crate::runtime::claude::command(&folder, Some("haiku")).unwrap();
    let args: Vec<String> = command
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let mut writer = LedgerWriter::create_runtime(
        &store,
        "real_claude",
        "Reply with exactly: ok",
        "haiku",
        &command.get_program().to_string_lossy(),
        "claude",
        &args,
        None,
    )
    .unwrap();
    let mut runtime = Claude::spawn(&mut command).unwrap();
    runtime
        .start(&Turn {
            prompt: "Reply with exactly: ok".into(),
            folder,
            model: Some("haiku".into()),
            effort: None,
            resume: None,
            mode: crate::approval::Mode::Ask,
            approvals: None,
            mission_id: "synthetic".into(),
            run_id: "run_synthetic".into(),
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut kinds = vec![];
    loop {
        assert!(
            Instant::now() < deadline,
            "real claude timed out: {kinds:?}"
        );
        if let Some(e) = runtime.next_event().unwrap() {
            writer
                .append(&e.kind, e.payload.clone(), e.thread.as_deref())
                .unwrap();
            let done = ["run.completed", "run.failed", "run.cancelled"].contains(&e.kind.as_str());
            kinds.push(e.kind);
            if done {
                break;
            }
        }
    }
    assert_eq!(kinds.last().unwrap(), "run.completed");
}

fn turn_fields(
    folder: PathBuf,
    mode: crate::approval::Mode,
    approvals: Option<std::sync::Arc<crate::approval::Channel>>,
    mission_id: &str,
) -> Turn {
    Turn {
        prompt: "Synthetic prompt".into(),
        folder,
        model: Some("synthetic-model".into()),
        effort: None,
        resume: None,
        mode,
        approvals,
        mission_id: mission_id.into(),
        run_id: format!("run_{mission_id}"),
    }
}

fn answer_when_pending(
    channel: std::sync::Arc<crate::approval::Channel>,
    decision: crate::approval::Decision,
) {
    for _ in 0..200 {
        if let Some(req) = channel.waiting().into_iter().next() {
            channel
                .decide(crate::approval::Answer::Decision {
                    approval_id: req.approval_id,
                    decision,
                    reason: None,
                })
                .unwrap();
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("no approval became pending");
}

#[test]
fn claude_interactive_allow_and_deny_and_after_end() {
    // Allow
    {
        let temp = Temp::new();
        let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
        let channel = crate::approval::Channel::new();
        let mut writer = LedgerWriter::create_runtime(
            &store,
            "claude_allow",
            "Synthetic prompt",
            "synthetic-model",
            "fake-claude",
            "claude",
            &[],
            None,
        )
        .unwrap();
        let mut command = Command::new("python3");
        command
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/scripts/fixtures/m3-claude.py"
            ))
            .env("CLAUDE_SCENARIO", "approval-allow");
        let mut runtime = Claude::spawn(&mut command).unwrap();
        let ch = channel.clone();
        std::thread::spawn(move || answer_when_pending(ch, crate::approval::Decision::ApproveOnce));
        runtime
            .start(&turn_fields(
                temp.0.clone(),
                crate::approval::Mode::ApproveEach,
                Some(channel.clone()),
                "claude_allow",
            ))
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            assert!(Instant::now() < deadline, "allow timed out");
            for recorded in channel.take_recorded() {
                writer.append_approval(&recorded.to_json()).unwrap();
            }
            if let Some(e) = runtime.next_event().unwrap() {
                let done =
                    ["run.completed", "run.failed", "run.cancelled"].contains(&e.kind.as_str());
                writer
                    .append(&e.kind, e.payload, e.thread.as_deref())
                    .unwrap();
                if done {
                    break;
                }
            }
        }
        channel.release("claude_allow");
        let contents = std::fs::read_to_string(&writer.path).unwrap();
        assert!(contents.contains("mission.approval"), "{contents}");
        assert!(contents.contains("\"answer\":\"allowed\""), "{contents}");
        if let Ok(reader) = std::env::var("LOCUST_TEST_TS_READER") {
            let status = Command::new("node")
                .arg(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/scripts/check-m1-ledger.mjs"
                ))
                .arg(reader)
                .arg(store.ledger_dir().unwrap())
                .arg("claude_allow")
                .arg("completed")
                .status()
                .unwrap();
            assert!(status.success());
        }
    }
    // Deny
    {
        let temp = Temp::new();
        let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
        let channel = crate::approval::Channel::new();
        let mut writer = LedgerWriter::create_runtime(
            &store,
            "claude_deny",
            "Synthetic prompt",
            "synthetic-model",
            "fake-claude",
            "claude",
            &[],
            None,
        )
        .unwrap();
        let mut command = Command::new("python3");
        command
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/scripts/fixtures/m3-claude.py"
            ))
            .env("CLAUDE_SCENARIO", "approval-deny");
        let mut runtime = Claude::spawn(&mut command).unwrap();
        let ch = channel.clone();
        std::thread::spawn(move || answer_when_pending(ch, crate::approval::Decision::Deny));
        runtime
            .start(&turn_fields(
                temp.0.clone(),
                crate::approval::Mode::ApproveEach,
                Some(channel.clone()),
                "claude_deny",
            ))
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            assert!(Instant::now() < deadline, "deny timed out");
            for recorded in channel.take_recorded() {
                writer.append_approval(&recorded.to_json()).unwrap();
            }
            if let Some(e) = runtime.next_event().unwrap() {
                let done =
                    ["run.completed", "run.failed", "run.cancelled"].contains(&e.kind.as_str());
                writer
                    .append(&e.kind, e.payload, e.thread.as_deref())
                    .unwrap();
                if done {
                    break;
                }
            }
        }
        let contents = std::fs::read_to_string(&writer.path).unwrap();
        assert!(contents.contains("\"answer\":\"denied\""), "{contents}");
    }
    // Answer after turn end is refused
    {
        let channel = crate::approval::Channel::new();
        let ch = channel.clone();
        let handle = std::thread::spawn(move || {
            ch.ask_claude(
                "run_x",
                "mission_ended",
                "req_x",
                "Bash",
                &json!({"command":"true"}),
                "/tmp",
                &|| false,
            )
        });
        for _ in 0..100 {
            if channel.pending_count() == 1 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let id = channel.waiting()[0].approval_id.clone();
        channel.release("mission_ended");
        let err = channel
            .decide(crate::approval::Answer::Decision {
                approval_id: id,
                decision: crate::approval::Decision::ApproveOnce,
                reason: None,
            })
            .unwrap_err();
        assert!(
            err.contains("no longer waiting") || err.contains("turn ended"),
            "{err}"
        );
        let _ = handle.join().unwrap();
    }
}

#[test]
fn codex_interactive_allow_writes_approval() {
    let temp = Temp::new();
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    let channel = crate::approval::Channel::new();
    let mut writer = LedgerWriter::create(
        &store,
        "codex_allow",
        "Synthetic prompt",
        "synthetic-model",
        "fake-codex",
    )
    .unwrap();
    let mut command = Command::new("python3");
    command
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scripts/fixtures/m1-app-server.py"
        ))
        .arg("approve-allow");
    let mut runtime = Codex::spawn(&mut command).unwrap();
    let ch = channel.clone();
    std::thread::spawn(move || answer_when_pending(ch, crate::approval::Decision::ApproveOnce));
    runtime
        .start(&turn_fields(
            temp.0.clone(),
            crate::approval::Mode::ApproveEach,
            Some(channel.clone()),
            "codex_allow",
        ))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(Instant::now() < deadline, "codex allow timed out");
        for recorded in channel.take_recorded() {
            writer.append_approval(&recorded.to_json()).unwrap();
        }
        if let Some(e) = runtime.next_event().unwrap() {
            let done = ["run.completed", "run.failed", "run.cancelled"].contains(&e.kind.as_str());
            writer
                .append(&e.kind, e.payload, e.thread.as_deref())
                .unwrap();
            if done {
                break;
            }
        }
    }
    let contents = std::fs::read_to_string(&writer.path).unwrap();
    assert!(contents.contains("mission.approval"), "{contents}");
    assert!(contents.contains("\"answer\":\"allowed\""), "{contents}");
    if let Ok(reader) = std::env::var("LOCUST_TEST_TS_READER") {
        let status = Command::new("node")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/scripts/check-m1-ledger.mjs"
            ))
            .arg(reader)
            .arg(store.ledger_dir().unwrap())
            .arg("codex_allow")
            .arg("completed")
            .status()
            .unwrap();
        assert!(status.success());
    }
}
