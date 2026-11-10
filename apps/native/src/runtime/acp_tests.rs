use super::*;
use crate::{
    approval::{Answer, Channel, Decision, Mode},
    ledger_write::LedgerWriter,
    store::Store,
};
use std::path::PathBuf;
static TEMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            ".m4-acp-{}-{}-{}",
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
fn run(scenario: &str) {
    let temp = Temp::new();
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    std::fs::write(store.electron().join("sentinel"), "untouched").unwrap();
    let mut command = Command::new("python3");
    command
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scripts/fixtures/m4-acp.py"
        ))
        .arg(scenario)
        .current_dir(&temp.0);
    if scenario == "model" {
        command
            .env("LOCUST_NATIVE_ACP_MODEL_OPTION", "model")
            .env("LOCUST_NATIVE_ACP_MODEL_VALUE", "picked");
    }
    let mut rt = Acp::spawn(&mut command).unwrap();
    let channel = Channel::new();
    let ch = channel.clone();
    let interactive = matches!(scenario, "approve" | "always");
    let always = scenario == "always";
    let answer = interactive.then(|| {
        std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if let Some(card) = ch.waiting().first() {
                    ch.decide(Answer::Decision {
                        approval_id: card.approval_id.clone(),
                        decision: if always {
                            Decision::ApproveAlways
                        } else {
                            Decision::ApproveOnce
                        },
                        reason: None,
                    })
                    .unwrap();
                    break;
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(10));
            }
        })
    });
    let turn = Turn {
        prompt: "Synthetic ACP prompt".into(),
        folder: temp.0.clone(),
        model: None,
        effort: None,
        resume: (scenario == "resume" || scenario == "no-resume").then(|| "s1".into()),
        mode: Mode::ApproveEach,
        approvals: (interactive || scenario == "pending-exit" || scenario == "terminal-pending")
            .then(|| channel.clone()),
        mission_id: "acp_fake".into(),
        run_id: "run_acp_fake".into(),
    };
    let mut writer = LedgerWriter::create_runtime(
        &store,
        "acp_fake",
        &turn.prompt,
        "account-default",
        "fake-agent",
        "acp",
        &[],
        None,
    )
    .unwrap();
    let start = rt.start(&turn);
    start.unwrap();
    if scenario == "flood" {
        std::thread::sleep(Duration::from_millis(200));
    }
    if scenario == "steer" {
        assert!(rt.steer("One more sentence."));
    }
    if scenario == "stop" {
        rt.stop().unwrap();
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut events = vec![];
    loop {
        assert!(Instant::now() < deadline, "ACP fake timed out: {scenario}");
        if let Some(e) = rt.next_event().unwrap() {
            let terminal = matches!(
                e.kind.as_str(),
                "run.completed" | "run.failed" | "run.cancelled"
            );
            for a in channel.take_recorded_for("acp_fake") {
                writer.append_approval(&a.to_json()).unwrap();
            }
            writer
                .append(&e.kind, e.payload.clone(), e.thread.as_deref())
                .unwrap();
            events.push(e);
            if terminal {
                break;
            }
        }
    }
    if let Some(handle) = answer {
        handle.join().unwrap();
    }
    let phase = if scenario == "stop" {
        "cancelled"
    } else if matches!(
        scenario,
        "lost"
            | "quota"
            | "refusal"
            | "pending-exit"
            | "version"
            | "no-resume"
            | "stderr-quota"
            | "oversize"
            | "flood"
    ) {
        "failed"
    } else {
        "completed"
    };
    assert_eq!(events.last().unwrap().kind, format!("run.{phase}"));
    assert_eq!(
        std::fs::read_to_string(store.electron().join("sentinel")).unwrap(),
        "untouched"
    );
    let ledger = std::fs::read_to_string(&writer.path).unwrap();
    for secret in [
        "REPLAY_MUST_NOT_SURVIVE",
        "FOREIGN_MUST_NOT_SURVIVE",
        "RAW_REASONING_MUST_NOT_SURVIVE",
        "PRIVATE_COMMAND_MUST_NOT_SURVIVE",
    ] {
        assert!(!ledger.contains(secret));
    }
    if phase == "completed" && scenario != "terminal-pending" {
        let messages: Vec<_> = events
            .iter()
            .filter(|e| e.kind == "message.delta" && e.payload["final"] == true)
            .map(|e| e.payload["text"].as_str().unwrap())
            .collect();
        assert_eq!(
            messages,
            if scenario == "steer" {
                vec!["Before.", "After.", "Before.", "After."]
            } else {
                vec!["Before.", "After."]
            }
        );
        let shell = events
            .iter()
            .find(|e| {
                e.kind
                    == if interactive {
                        "tool.completed"
                    } else {
                        "tool.failed"
                    }
            })
            .unwrap();
        assert_eq!(shell.payload["command"], "echo hi");
        assert_eq!(
            shell.payload["status"],
            if interactive { "completed" } else { "declined" }
        );
        let edit = events
            .iter()
            .find(|e| e.payload["itemId"] == "edit" && e.kind == "tool.completed")
            .unwrap();
        assert_eq!(edit.payload["patch"]["added"], 1);
        assert_eq!(edit.payload["patch"]["removed"], 1);
        assert_eq!(
            events.last().unwrap().payload["usage"]["inputTokens"].as_f64(),
            Some(if scenario == "steer" { 20.0 } else { 10.0 })
        );
    }
    if scenario == "quota" || scenario == "stderr-quota" {
        assert!(events.iter().any(|e| e.kind == "route.limit_detected"));
    }
    if scenario == "limit" {
        assert!(
            events
                .iter()
                .any(|e| e.payload["code"] == "acp.stopped_at_limit")
        );
    }
    if let Ok(reader) = std::env::var("LOCUST_TEST_TS_READER") {
        assert!(
            Command::new("node")
                .arg(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/scripts/check-m1-ledger.mjs"
                ))
                .arg(reader)
                .arg(store.ledger_dir().unwrap())
                .arg("acp_fake")
                .arg(phase)
                .status()
                .unwrap()
                .success()
        );
    }
}
#[test]
fn acp_denial_and_tool_edits() {
    run("success");
}
#[test]
fn acp_replayed_load() {
    run("resume");
}
#[test]
fn acp_always_is_once_on_wire() {
    run("always");
}
#[test]
fn acp_approve_once() {
    run("approve");
}
#[test]
fn acp_decline_is_not_stop() {
    run("declined");
}
#[test]
fn acp_stop() {
    run("stop");
}
#[test]
fn acp_transport_loss() {
    run("lost");
}
#[test]
fn acp_quota() {
    run("quota");
}
#[test]
fn acp_output_limit() {
    run("limit");
}
#[test]
fn acp_refusal() {
    run("refusal");
}
#[test]
fn acp_malformed_line() {
    run("malformed");
}
#[test]
fn acp_protocol_version() {
    run("version");
}
#[test]
fn acp_resume_capability() {
    run("no-resume");
}
#[test]
fn acp_command_words() {
    assert_eq!(
        command_words("\"/Program Files/goose\" acp").unwrap(),
        ["/Program Files/goose", "acp"]
    );
    for s in ["", "bash -c hi", "x | y", "x $HOME", "\"oops"] {
        assert!(command_words(s).is_err(), "{s}");
    }
}
#[test]
fn acp_reads_saved_command_and_model() {
    let t = Temp::new();
    std::fs::write(t.0.join("acp-agents.json"),r#"{"agents":[{"id":"acp_test","name":"Test","commandLine":"/usr/bin/true acp","answer":{"modelOption":{"id":"model"}}}]}"#).unwrap();
    let c = command(&t.0, &t.0, Some("acp_test::picked")).unwrap();
    assert_eq!(c.get_program(), "/usr/bin/true");
    assert_eq!(c.get_args().collect::<Vec<_>>(), ["acp"]);
    assert!(command(&t.0, &t.0, Some("acp_missing")).is_err());
}
#[test]
#[ignore = "Runs a real configured ACP CLI; requires LOCUST_REAL_ACP_COMMAND. No window."]
fn real_acp_one_line() {
    assert_eq!(
        std::env::var("LOCUST_REAL_ACP").ok().as_deref(),
        Some("1"),
        "set LOCUST_REAL_ACP=1 to run this real test"
    );
    let t = Temp::new();
    let electron = t.0.join("electron");
    std::fs::create_dir_all(&electron).unwrap();
    let words = command_words(
        &std::env::var("LOCUST_REAL_ACP_COMMAND")
            .expect("Set LOCUST_REAL_ACP_COMMAND to an already signed-in ACP CLI"),
    )
    .unwrap();
    let mut c = Command::new(&words[0]);
    c.args(&words[1..]).current_dir(&t.0);
    let store = Store::new(t.0.join("native"), electron).unwrap();
    let mut writer = LedgerWriter::create_runtime(
        &store,
        "real_acp",
        "Reply with OK only.",
        "account-default",
        &words[0],
        "acp",
        &words[1..],
        None,
    )
    .unwrap();
    let mut rt = Acp::spawn(&mut c).unwrap();
    rt.start(&Turn {
        prompt: "Reply with OK only.".into(),
        folder: t.0.clone(),
        model: None,
        effort: None,
        resume: None,
        mode: Mode::Ask,
        approvals: None,
        mission_id: "real_acp".into(),
        run_id: "run_real_acp".into(),
    })
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        assert!(Instant::now() < deadline);
        if let Some(e) = rt.next_event().unwrap() {
            writer
                .append(&e.kind, e.payload, e.thread.as_deref())
                .unwrap();
            if e.kind.starts_with("run.") && e.kind != "run.started" {
                assert_eq!(e.kind, "run.completed");
                break;
            }
        }
    }
}

#[test]
fn acp_exit_with_pending_card() {
    run("pending-exit");
}
#[test]
fn acp_terminal_with_pending_card() {
    run("terminal-pending");
}

#[test]
fn acp_steering_queues_next_prompt_and_sums_usage() {
    run("steer");
}
#[test]
fn acp_selected_model_set_before_prompt() {
    run("model");
}
#[test]
fn acp_teammate_uses_native_driver() {
    let t = Temp::new();
    let store = Store::new(t.0.join("native"), t.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    let _guard = crate::native::hooks::install(
        store.clone(),
        Some((
            "python3".into(),
            vec![
                concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/fixtures/m4-acp.py").into(),
                "success".into(),
            ],
        )),
    );
    let team=crate::native::parse_roster(&json!({"schemaVersion":1,"teammates":[{"teammateId":"peer","name":"Peer","role":"Custom","roleTitle":"Test","hue":"lime","createdAt":"2026-11-09T12:00:00Z","folder":t.0,"route":{"runtime":"acp","model":"acp_test","mode":"ask"}}]}).to_string()).unwrap();
    assert!(crate::native::supported(&team[0]));
    let started = crate::native::send(crate::native::Send {
        teammate: team[0].clone(),
        conversation: None,
        prompt: "Synthetic ACP prompt".into(),
        model: None,
        effort: None,
    })
    .unwrap();
    // Generic ACP requests always use the approval channel, regardless of teammate mode.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(card) = crate::native::waiting_approvals_for(&started.conversation_id).first() {
            crate::native::decide_approval(&card.approval_id, Decision::Deny, None).unwrap();
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
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
    assert!(ledger.contains("mission.approval"));
    assert_eq!(
        crate::native::prior_of(&started.conversation_id)
            .unwrap()
            .thread
            .as_deref(),
        Some("s1")
    );
}

#[test]
fn acp_stderr_quota_classifies_transport_exit() {
    run("stderr-quota");
}
#[test]
fn acp_oversize_fails_closed() {
    run("oversize");
}
#[test]
fn acp_queue_overflow_fails_closed() {
    run("flood");
}
