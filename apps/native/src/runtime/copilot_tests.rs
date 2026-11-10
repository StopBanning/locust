use super::test_support::reader_check;
// Ports packages/runtime-adapters/test/copilot-events.test.ts (fixture-driven) plus launch/dispatch.
use super::{
    Event, Runtime, Turn,
    copilot::Copilot,
    copilot_events::{CopilotNormalizer, failure_from, tool_command},
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
            ".m4-copilot-{}-{}-{}",
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
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/fixtures/copilot")
}
fn fixture_lines(name: &str) -> Vec<String> {
    std::fs::read_to_string(fixture_dir().join(name))
        .unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}
fn run_lines(lines: &[String], exit: Option<i32>, stderr: &str) -> (CopilotNormalizer, Vec<Event>) {
    let mut normal = CopilotNormalizer::default().with_session(Some("host_sess".into()));
    let mut events = Vec::new();
    for line in lines {
        events.extend(normal.accept_line(line));
    }
    normal.set_stderr(stderr);
    events.push(normal.finish(false, exit.or(Some(0))));
    (normal, events)
}
fn fake_script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/fixtures/m4-copilot.py")
}

#[test]
fn copilot_plain_reply_fixture() {
    let (normal, events) = run_lines(&fixture_lines("plain-reply.jsonl"), Some(0), "");
    assert_eq!(events[0].kind, "run.started");
    assert!(
        events
            .iter()
            .any(|e| e.kind == "message.delta" && e.payload["final"] == true)
    );
    assert_eq!(events.last().unwrap().kind, "run.completed");
    // CLI session id wins over host.
    assert!(normal.thread.as_deref().is_some_and(|t| t != "host_sess"));
    let serialized =
        serde_json::to_string(&events.iter().map(|e| &e.payload).collect::<Vec<_>>()).unwrap();
    assert!(!serialized.contains("You are GitHub Copilot"));
    assert!(!serialized.contains("reasoningOpaque"));
}

#[test]
fn copilot_write_apply_patch_fixture() {
    let (_, events) = run_lines(&fixture_lines("write-apply-patch.jsonl"), Some(0), "");
    let tools: Vec<_> = events
        .iter()
        .filter(|e| e.kind.starts_with("tool."))
        .collect();
    assert!(tools.iter().any(|e| e.kind == "tool.started"));
    assert!(
        tools
            .iter()
            .any(|e| e.kind == "tool.completed" && e.payload.get("patch").is_some())
    );
    assert_eq!(events.last().unwrap().kind, "run.completed");
}

#[test]
fn copilot_read_only_tools_denied_fixture() {
    let (_, events) = run_lines(&fixture_lines("read-only-tools-denied.jsonl"), Some(0), "");
    let failed: Vec<_> = events.iter().filter(|e| e.kind == "tool.failed").collect();
    assert!(failed.len() >= 2);
    assert!(!events.iter().any(|e| e.kind == "tool.completed"));
}

#[test]
fn copilot_policy_denied_from_stderr() {
    let stderr = std::fs::read_to_string(fixture_dir().join("stderr-policy-denied.txt")).unwrap();
    let (kind, message) = failure_from(&stderr).unwrap();
    assert_eq!(kind, "authentication-failed");
    assert!(message.contains("Copilot plan required"));
    let (_, events) = run_lines(&fixture_lines("policy-denied.jsonl"), Some(1), &stderr);
    assert_eq!(events.last().unwrap().kind, "run.failed");
    assert_eq!(
        events.last().unwrap().payload["kind"],
        "authentication-failed"
    );
}

#[test]
fn copilot_tool_command_shapes() {
    assert_eq!(
        tool_command(&json!("patch text")).as_deref(),
        Some("patch text")
    );
    assert_eq!(
        tool_command(&json!({"command":"ls"})).as_deref(),
        Some("ls")
    );
    assert_eq!(tool_command(&json!({"path":"/a"})).as_deref(), Some("/a"));
    assert_eq!(
        tool_command(&json!({"pattern":"*.rs"})).as_deref(),
        Some("*.rs")
    );
}

#[test]
fn copilot_fake_process_writes_ledger() {
    let temp = Temp::new();
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    let mut command = Command::new("python3");
    command
        .arg(fake_script())
        .arg("plain-reply")
        .current_dir(&temp.0);
    let mut rt = Copilot::spawn(&mut command).unwrap();
    let turn = Turn {
        prompt: "Synthetic Copilot prompt".into(),
        folder: temp.0.clone(),
        model: None,
        effort: None,
        resume: None,
        mode: Mode::Ask,
        approvals: None,
        mission_id: "copilot_plain".into(),
        run_id: "run_copilot_plain".into(),
    };
    let mut writer = LedgerWriter::create_runtime(
        &store,
        "copilot_plain",
        &turn.prompt,
        "account-default",
        "fake-copilot",
        "copilot",
        &[],
        None,
    )
    .unwrap();
    rt.start(&turn).unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut events = vec![];
    loop {
        assert!(Instant::now() < deadline, "copilot fake timed out");
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
    reader_check(&store, "copilot_plain", "completed");
}

#[test]
fn copilot_teammate_uses_native_driver() {
    let t = Temp::new();
    let store = Store::new(t.0.join("native"), t.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    let _guard = crate::native::hooks::install(
        store.clone(),
        Some((
            "python3".into(),
            vec![
                fake_script().to_string_lossy().into_owned(),
                "plain-reply".into(),
            ],
        )),
    );
    let team = crate::native::parse_roster(
        &json!({"schemaVersion":1,"teammates":[{"teammateId":"p1","name":"Pilot","role":"Custom","roleTitle":"Test","hue":"lime","createdAt":"2026-11-09T12:00:00Z","folder":t.0,"route":{"runtime":"copilot","model":"account-default","mode":"ask"}}]}).to_string(),
    )
    .unwrap();
    assert!(crate::native::supported(&team[0]));
    let started = crate::native::send(crate::native::Send {
        teammate: team[0].clone(),
        conversation: None,
        prompt: "Synthetic Copilot prompt".into(),
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
fn real_copilot_one_line() {
    assert_eq!(
        std::env::var("LOCUST_REAL_COPILOT").ok().as_deref(),
        Some("1"),
        "set LOCUST_REAL_COPILOT=1 to run this real test"
    );
    let temp = Temp::new();
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    let folder = temp.0.join("ws");
    std::fs::create_dir_all(&folder).unwrap();
    let mut command = crate::runtime::copilot::command(
        &folder,
        "Reply with exactly: ok",
        Mode::Ask,
        None,
        None,
        None,
        Some(crate::native::uuid_v4().unwrap().as_str()),
    )
    .unwrap();
    let mut rt = Copilot::spawn(&mut command).unwrap();
    let turn = Turn {
        prompt: "Reply with exactly: ok".into(),
        folder,
        model: None,
        effort: None,
        resume: None,
        mode: Mode::Ask,
        approvals: None,
        mission_id: "copilot_real".into(),
        run_id: "run_copilot_real".into(),
    };
    let mut writer = LedgerWriter::create_runtime(
        &store,
        "copilot_real",
        &turn.prompt,
        "account-default",
        "copilot",
        "copilot",
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
fn copilot_approve_each_enforces_agent_and_allow_all_off_before_prompt() {
    use crate::approval::{Answer, Channel, Decision};
    for scenario in ["allow", "deny", "missing", "refuses"] {
        let temp = Temp::new();
        let channel = Channel::new();
        let mut c = Command::new("python3");
        c.arg(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("scripts/fixtures/m4-copilot-approval.py"),
        )
        .arg(scenario)
        .arg("--acp");
        let mut rt = Copilot::spawn(&mut c).unwrap();
        rt.start(&Turn {
            prompt: "echo hi".into(),
            folder: temp.0.clone(),
            model: None,
            effort: None,
            resume: Some("s1".into()),
            mode: Mode::ApproveEach,
            approvals: Some(channel.clone()),
            mission_id: "copilot_review".into(),
            run_id: "run_review".into(),
        })
        .unwrap();
        let answer_channel = channel.clone();
        let expected = scenario.to_string();
        let worker = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if let Some(card) = answer_channel.waiting().first() {
                    answer_channel
                        .decide(Answer::Decision {
                            approval_id: card.approval_id.clone(),
                            decision: if expected == "allow" {
                                Decision::ApproveOnce
                            } else {
                                Decision::Deny
                            },
                            reason: None,
                        })
                        .unwrap();
                    return true;
                }
                if expected == "refuses" || Instant::now() > deadline {
                    return false;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        });
        let deadline = Instant::now() + Duration::from_secs(8);
        let terminal = loop {
            assert!(Instant::now() < deadline);
            if let Some(e) = rt.next_event().unwrap() {
                if matches!(e.kind.as_str(), "run.completed" | "run.failed") {
                    break e;
                }
            }
        };
        assert_eq!(
            terminal.kind,
            if scenario == "refuses" {
                "run.failed"
            } else {
                "run.completed"
            }
        );
        assert_eq!(worker.join().unwrap(), scenario != "refuses");
    }
}
