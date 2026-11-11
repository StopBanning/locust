use super::test_support::reader_check;
// Ports packages/runtime-adapters/test/muse-events.test.ts + the-prompt-muse-cannot-be-told.
use super::{
    Event, Runtime, Turn,
    muse::Muse,
    muse_events::{
        MUSE_BILLING_SENTENCE, MUSE_SANDBOX_SENTENCE, MuseNormalizer, muse_session_id_of,
        muse_task_is_internal,
    },
};
use crate::{approval::Mode, ledger_write::LedgerWriter, store::Store};
use serde_json::{Value, json};
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
        let p = std::env::temp_dir().join(format!(
            ".m4-muse-{}-{}-{}",
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
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/fixtures/muse")
}
fn capture() -> Vec<String> {
    let raw = std::fs::read(fixture_dir().join("echo-provider-run.jsonl")).unwrap();
    let text = if raw.starts_with(&[0xEF, 0xBB, 0xBF]) {
        String::from_utf8_lossy(&raw[3..]).into_owned()
    } else {
        String::from_utf8_lossy(&raw).into_owned()
    };
    text.lines()
        .map(|l| l.trim_end_matches('\r').to_owned())
        .filter(|l| !l.is_empty())
        .collect()
}
fn run_lines(lines: &[String], exit: Option<i32>) -> (MuseNormalizer, Vec<Event>) {
    let mut normal = MuseNormalizer::default();
    let mut events = Vec::new();
    for line in lines {
        events.extend(normal.accept_line(line));
    }
    events.extend(normal.finish(false, exit.or(Some(0))));
    (normal, events)
}
fn record(payload_type: &str, payload: Value, sequence: u64) -> String {
    json!({
        "schema_version": 1,
        "id": format!("018f0000-0000-7000-8000-0000000000{sequence:02}"),
        "stream": {"kind":"session","id":"01a0c5fe-68c8-7903-b6db-0d3c9021bfa6"},
        "sequence": sequence,
        "recorded_at": 1780531400000000u64 + sequence,
        "record_type": "event",
        "durability": "durable",
        "causation_id": "2dcba620-64bd-4fb8-93b6-c0f0af3d116b",
        "payload_type": payload_type,
        "payload_schema_version": 1,
        "payload": payload
    })
    .to_string()
}
fn lifecycle(kind: &str, extra: Value, sequence: u64) -> String {
    let task = "01a0c5fe-9999-7000-8000-000000000001";
    let mut event = json!({"kind": kind, "task_id": task});
    if let Some(obj) = extra.as_object() {
        for (k, v) in obj {
            event[k] = v.clone();
        }
    }
    record(
        &format!("task.lifecycle.{kind}"),
        json!({"kind":"task_lifecycle","task_id":task,"event":event}),
        sequence,
    )
}
fn fake_script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/fixtures/m4-muse.py")
}
fn messages(events: &[Event]) -> std::collections::HashMap<String, String> {
    let mut buffers = std::collections::HashMap::new();
    for e in events {
        if e.kind != "message.delta" {
            continue;
        }
        let id = e.payload["itemId"].as_str().unwrap_or("").to_owned();
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
    buffers
}

#[test]
fn muse_echo_capture_fixture() {
    let (normal, events) = run_lines(&capture(), Some(0));
    assert_eq!(
        normal.thread.as_deref(),
        Some("01a0c5fe-68c8-7903-b6db-0d3c9021bfa6")
    );
    assert!(
        !events
            .iter()
            .any(|e| e.payload["code"] == "muse.malformed_record")
    );
    assert_eq!(
        messages(&events).get("msg_0").map(String::as_str),
        Some("echo: Reply with exactly PING")
    );
    let finals: Vec<_> = events
        .iter()
        .filter(|e| e.kind == "message.delta" && e.payload["final"] == true)
        .collect();
    assert_eq!(finals.len(), 1);
    assert_eq!(finals[0].payload["operation"], "replace");
    assert!(!events.iter().any(|e| e.kind.starts_with("tool.")));
    assert!(
        !events
            .iter()
            .any(|e| e.payload["code"] == "muse.unknown_event")
    );
    assert!(events.iter().any(|e| {
        e.kind == "adapter.diagnostic" && e.payload["code"] == "muse.internal_task_failed"
    }));
    assert_eq!(events.last().unwrap().kind, "run.completed");
}

#[test]
fn muse_person_tool_row() {
    let mut normal = MuseNormalizer::default();
    let mut events = Vec::new();
    events.extend(normal.accept_line(&lifecycle(
        "proposed",
        json!({"task_kind":"tool.shell.command"}),
        1,
    )));
    events.extend(normal.accept_line(&lifecycle("accepted", json!({}), 2)));
    events.extend(normal.accept_line(&lifecycle("scheduled", json!({}), 3)));
    events.extend(normal.accept_line(&lifecycle(
        "side_effect_intent",
        json!({"operation":"shell.execute"}),
        4,
    )));
    events.extend(normal.accept_line(&lifecycle("started", json!({}), 5)));
    events.extend(normal.accept_line(&lifecycle("completed", json!({}), 6)));
    assert_eq!(
        events.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(),
        ["tool.started", "tool.completed"]
    );
    assert_eq!(events[0].payload["name"], "shell.command");
    assert_eq!(events[0].payload["command"], "shell.execute");
}

#[test]
fn muse_tool_failed_leaves_run() {
    let mut normal = MuseNormalizer::default();
    let mut events = Vec::new();
    events.extend(normal.accept_line(&lifecycle(
        "proposed",
        json!({"task_kind":"tool.file.write"}),
        1,
    )));
    events.extend(normal.accept_line(&lifecycle("started", json!({}), 2)));
    events.extend(normal.accept_line(&lifecycle(
        "failed",
        json!({"reason":"permission denied"}),
        3,
    )));
    events.extend(normal.accept_line(&record(
        "run.terminal.completed",
        json!({"terminal":"completed","text":"done","reason":null}),
        4,
    )));
    events.extend(normal.finish(false, Some(0)));
    assert!(events.iter().any(|e| e.kind == "tool.failed"));
    assert_eq!(events.last().unwrap().kind, "run.completed");
}

#[test]
fn muse_sandbox_said_once() {
    let reason = "environment failure: sandbox enforcement unavailable: windows_elevated setup_required: sandbox users are not ready";
    let mut normal = MuseNormalizer::default();
    let mut events = Vec::new();
    events.extend(normal.accept_line(&lifecycle(
        "proposed",
        json!({"task_kind":"tool.powershell"}),
        1,
    )));
    events.extend(normal.accept_line(&lifecycle("started", json!({}), 2)));
    events.extend(normal.accept_line(&lifecycle("failed", json!({"reason":reason}), 3)));
    events.extend(normal.accept_line(&lifecycle("started", json!({}), 4)));
    events.extend(normal.accept_line(&lifecycle("failed", json!({"reason":reason}), 5)));
    events.extend(normal.accept_line(&record(
        "run.terminal.completed",
        json!({"terminal":"completed","text":"done","reason":null}),
        6,
    )));
    events.extend(normal.finish(false, Some(0)));
    assert_eq!(events.iter().filter(|e| e.kind == "tool.failed").count(), 2);
    let said: Vec<_> = events
        .iter()
        .filter(|e| e.payload["code"] == "muse.sandbox_not_set_up")
        .collect();
    assert_eq!(said.len(), 1);
    assert_eq!(said[0].payload["message"], MUSE_SANDBOX_SENTENCE);
}

#[test]
fn muse_run_failures() {
    let (_, events) = run_lines(
        &[record(
            "run.lifecycle.started",
            json!({"prompt":"hello"}),
            1,
        )],
        Some(0),
    );
    assert_eq!(events.last().unwrap().kind, "run.failed");
    assert_eq!(events.last().unwrap().payload["runtimeTerminal"], "missing");

    let reason = "API error 402 [request_id=x]: Billing verification failed. (billing_error)";
    let (_, events) = run_lines(
        &[
            record("run.lifecycle.started", json!({"prompt":"hello"}), 1),
            record(
                "run.terminal.failed",
                json!({"kind":"run_terminal","terminal":"failed","text":"","reason":reason}),
                2,
            ),
        ],
        Some(1),
    );
    assert_eq!(
        events.last().unwrap().payload["message"],
        MUSE_BILLING_SENTENCE
    );

    let mut normal = MuseNormalizer::default().with_output_limit_exceeded(true);
    let mut events = normal.accept_line(&record(
        "run.lifecycle.started",
        json!({"prompt":"hello"}),
        1,
    ));
    events.extend(normal.finish(false, Some(1)));
    assert!(
        events.last().unwrap().payload["message"]
            .as_str()
            .unwrap()
            .contains("faster than Locust could record it")
    );

    let mut normal = MuseNormalizer::default();
    let mut events = normal.accept_line(&record(
        "run.lifecycle.started",
        json!({"prompt":"hello"}),
        1,
    ));
    events.extend(normal.finish(true, None));
    assert_eq!(events.last().unwrap().kind, "run.cancelled");
}

#[test]
fn muse_session_and_internal_facts() {
    assert_eq!(
        muse_session_id_of(&json!({"stream":{"kind":"session","id":"s1"}})).as_deref(),
        Some("s1")
    );
    assert!(muse_session_id_of(&json!({"stream":{"kind":"run","id":"r1"}})).is_none());
    assert!(muse_task_is_internal(Some("model.unknown.response")));
    assert!(muse_task_is_internal(Some("reminder.agent.skill-reminder")));
    assert!(!muse_task_is_internal(Some("tool.shell.command")));
    assert!(!muse_task_is_internal(None));
}

#[test]
fn muse_fake_process_writes_ledger() {
    let temp = Temp::new();
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    let mut command = Command::new("python3");
    command
        .arg(fake_script())
        .arg("echo-provider-run")
        .current_dir(&temp.0);
    let mut rt = Muse::spawn(&mut command).unwrap();
    let turn = Turn {
        prompt: "Synthetic Muse prompt".into(),
        folder: temp.0.clone(),
        model: None,
        effort: None,
        resume: None,
        mode: Mode::Ask,
        approvals: None,
        mission_id: "muse_echo".into(),
        run_id: "run_muse_echo".into(),
    };
    let mut writer = LedgerWriter::create_runtime(
        &store,
        "muse_echo",
        &turn.prompt,
        "account-default",
        "fake-muse",
        "muse",
        &[],
        None,
    )
    .unwrap();
    rt.start(&turn).unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut events = vec![];
    loop {
        assert!(Instant::now() < deadline, "muse fake timed out");
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
    reader_check(&store, "muse_echo", "completed");
}

#[test]
fn muse_teammate_uses_native_driver() {
    let t = Temp::new();
    let store = Store::new(t.0.join("native"), t.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    let _guard = crate::native::hooks::install(
        store.clone(),
        Some((
            "python3".into(),
            vec![
                fake_script().to_string_lossy().into_owned(),
                "echo-provider-run".into(),
            ],
        )),
    );
    let team = crate::native::parse_roster(
        &json!({"schemaVersion":1,"teammates":[{"teammateId":"m1","name":"Muse","role":"Custom","roleTitle":"Test","hue":"lime","createdAt":"2026-11-09T12:00:00Z","folder":t.0,"route":{"runtime":"muse","model":"account-default","mode":"ask"}}]}).to_string(),
    )
    .unwrap();
    assert!(crate::native::supported(&team[0]));
    let started = crate::native::send(crate::native::Send {
        teammate: team[0].clone(),
        conversation: None,
        prompt: "Synthetic Muse prompt".into(),
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
fn real_muse_one_line() {
    assert_eq!(
        std::env::var("LOCUST_REAL_MUSE").ok().as_deref(),
        Some("1"),
        "set LOCUST_REAL_MUSE=1 to run this real test"
    );
    let temp = Temp::new();
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    let folder = temp.0.join("ws");
    std::fs::create_dir_all(&folder).unwrap();
    let mut command = crate::runtime::muse::command(&folder, Mode::Ask, None, None, None).unwrap();
    let prompt_file =
        crate::runtime::muse::prepare_prompt_file(&mut command, "Reply with exactly: ok", &store)
            .unwrap();
    let mut rt = Muse::spawn_with_prompt_file(&mut command, Some(prompt_file)).unwrap();
    let turn = Turn {
        prompt: "Reply with exactly: ok".into(),
        folder,
        model: None,
        effort: None,
        resume: None,
        mode: Mode::Ask,
        approvals: None,
        mission_id: "muse_real".into(),
        run_id: "run_muse_real".into(),
    };
    let mut writer = LedgerWriter::create_runtime(
        &store,
        "muse_real",
        &turn.prompt,
        "account-default",
        "muse",
        "muse",
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
fn muse_prompt_is_unique_native_owned_and_preserves_launch_environment() {
    let temp = Temp::new();
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    let mut c = Command::new("muse");
    c.args([
        "exec",
        "--prompt-file",
        crate::runtime::muse::PROMPT_FILE_PLACEHOLDER,
    ])
    .env("MUSE_TEST", "kept")
    .env_remove("REMOVED")
    .current_dir(&temp.0);
    let first =
        crate::runtime::muse::prepare_prompt_file(&mut c, "private prompt", &store).unwrap();
    let second =
        crate::runtime::muse::prepare_prompt_file(&mut Command::new("muse"), "second", &store)
            .unwrap();
    assert_ne!(first, second);
    assert!(first.starts_with(temp.0.join("native")));
    assert_eq!(std::fs::read_to_string(&first).unwrap(), "private prompt");
    assert!(c.get_args().any(|a| a == first.as_os_str()));
    assert!(
        c.get_envs()
            .any(|(k, v)| k == "MUSE_TEST" && v == Some(std::ffi::OsStr::new("kept")))
    );
    assert!(c.get_envs().any(|(k, v)| k == "REMOVED" && v.is_none()));
}
