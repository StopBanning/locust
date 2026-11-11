use super::*;
use crate::runtime::test_support::reader_check;
use crate::{
    approval::{Answer, Channel, Decision, Mode},
    ledger_write::LedgerWriter,
    store::Store,
};
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
        let p = std::env::temp_dir().join(format!(
            ".m4-opencode-{}-{}-{}",
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

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/.."))
        .join("scripts/fixtures")
        .join(name)
}

fn drive(
    rt: &mut OpenCode,
    writer: &mut LedgerWriter,
    channel: &Channel,
    mission: &str,
) -> Vec<Event> {
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut events = vec![];
    loop {
        assert!(Instant::now() < deadline, "OpenCode fake timed out");
        if let Some(e) = rt.next_event().unwrap() {
            let terminal = matches!(
                e.kind.as_str(),
                "run.completed" | "run.failed" | "run.cancelled"
            );
            for a in channel.take_recorded_for(mission) {
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
    events
}

#[test]
fn opencode_version_split_matches_discovery() {
    assert!(!version_uses_acp("1.18.27"));
    assert!(version_uses_acp("2.0.0"));
    assert!(version_uses_acp("v2.1.3"));
    assert!(!version_uses_acp("3.0.0"));
    assert!(!version_uses_acp("not-a-version"));
    assert_eq!(version_major("opencode 2.0.0"), Some(2));
}

#[test]
fn opencode_mode_config_keeps_question_off_and_mode_rules() {
    let ask = crate::runtime::opencode_config::legacy_config(Mode::Ask, None);
    assert_eq!(ask["tools"]["question"], false);
    assert_eq!(ask["permission"]["external_directory"], "deny");
    assert_eq!(ask["permission"]["edit"], "deny");
    assert_eq!(ask["permission"]["bash"], "ask");
    assert_eq!(ask["experimental"]["continue_loop_on_deny"], true);
    let auto = crate::runtime::opencode_config::legacy_config(Mode::Auto, None);
    assert_eq!(auto["permission"]["external_directory"], "allow");
    let each = crate::runtime::opencode_config::legacy_config(Mode::ApproveEach, None);
    assert_eq!(each["permission"]["*"], "ask");
    assert_eq!(each["permission"]["edit"], "ask");
    let temp = Temp::new();
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    let plugin = crate::runtime::opencode_config::write_guard(&store, true).unwrap();
    let acp = crate::runtime::opencode_config::acp_config(Mode::Ask, &plugin);
    assert_eq!(acp["default_agent"], "locust");
    assert!(
        acp["agents"]["locust"]["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["action"] == "question" && p["effect"] == "deny")
    );
}

#[test]
fn opencode_error_sentences_match_ts() {
    use crate::runtime::opencode_events::ErrorFacts;
    let free = ErrorFacts::of(&json!({"error":{"name":"FreeUsageLimitError","data":{"type":"FreeUsageLimitError","statusCode":429}}})).unwrap();
    assert_eq!(free.limit(), Some("quota-exhausted"));
    assert!(free.sentence(true).contains("free model has no usage left"));
    let rate = ErrorFacts::of(&json!({"error":{"name":"AI_APICallError","data":{"statusCode":429,"message":"slow down"}}})).unwrap();
    assert!(rate.sentence(false).contains("rate limited"));
    let auth = ErrorFacts::of(
        &json!({"error":{"name":"Auth","data":{"statusCode":401,"message":"nope"}}}),
    )
    .unwrap();
    assert!(auth.sentence(false).contains("opencode auth login"));
    assert!(auth.sentence(true).contains("Free models need no sign-in"));
}

#[test]
fn opencode_events_stream_and_finish() {
    let mut n = crate::runtime::opencode_events::Normalizer::new(Some("opencode/beam-free"));
    n.base.started_at = crate::ledger_write::now();
    let mut events = n.accept(&json!({"type":"step_start","sessionID":"s1","part":{}}));
    events.extend(
        n.accept(&json!({"type":"text_partial","sessionID":"s1","part":{"id":"p1","text":"Hel"}})),
    );
    events.extend(
        n.accept(&json!({"type":"text","sessionID":"s1","part":{"id":"p1","text":"Hello"}})),
    );
    events.extend(n.accept(&json!({"type":"step_finish","sessionID":"s1","part":{"reason":"stop","tokens":{"input":1,"output":2,"reasoning":0,"cache":{"read":0,"write":0}},"cost":0}})));
    let done = n.finish(false, Some(0), "", false);
    assert!(events.iter().any(|e| e.kind == "message.delta"
        && e.payload["text"] == "Hello"
        && e.payload["final"] == true));
    assert_eq!(done.last().unwrap().kind, "run.completed");
    assert_eq!(done.last().unwrap().payload["runtimeThreadId"], "s1");
}

fn acp_command(scenario: &str, env: &[(&str, String)]) -> Command {
    let mut c = Command::new("python3");
    c.arg(fixture("m4-opencode-acp.py"))
        .arg("acp")
        .arg(scenario);
    for (k, v) in env {
        c.env(k, v);
    }
    c
}

#[test]
fn opencode_acp_success_sets_locust_mode_and_records() {
    let temp = Temp::new();
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    std::fs::write(store.electron().join("sentinel"), "untouched").unwrap();
    let mut command = acp_command("success", &[]);
    command.current_dir(&temp.0);
    let mut rt = OpenCode::spawn(&mut command).unwrap();
    let channel = Channel::new();
    let ch = channel.clone();
    let answer = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(card) = ch.waiting().first() {
                ch.decide(Answer::Decision {
                    approval_id: card.approval_id.clone(),
                    decision: Decision::ApproveOnce,
                    reason: None,
                })
                .unwrap();
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
    });
    let turn = Turn {
        prompt: "Synthetic OpenCode prompt".into(),
        folder: temp.0.clone(),
        model: Some("opencode/beam-free".into()),
        effort: Some("high".into()),
        resume: None,
        mode: Mode::ApproveEach,
        approvals: Some(channel.clone()),
        mission_id: "opencode_acp".into(),
        run_id: "run_opencode_acp".into(),
    };
    let mut writer = LedgerWriter::create_runtime(
        &store,
        "opencode_acp",
        &turn.prompt,
        "opencode/beam-free",
        "fake-opencode",
        "opencode",
        &[],
        None,
    )
    .unwrap();
    rt.start(&turn).unwrap();
    let events = drive(&mut rt, &mut writer, &channel, "opencode_acp");
    answer.join().unwrap();
    assert_eq!(events.last().unwrap().kind, "run.completed");
    assert!(
        events
            .iter()
            .any(|e| e.kind == "message.delta" && e.payload["text"] == "OpenCode says hi.")
    );
    assert!(
        events
            .iter()
            .any(|e| e.kind == "tool.completed" && e.payload["command"] == "echo hi")
    );
    assert_eq!(
        std::fs::read_to_string(store.electron().join("sentinel")).unwrap(),
        "untouched"
    );
    let ledger = std::fs::read_to_string(&writer.path).unwrap();
    for secret in [
        "REPLAY_MUST_NOT_SURVIVE",
        "FOREIGN_MUST_NOT_SURVIVE",
        "RAW_REASONING_MUST_NOT_SURVIVE",
    ] {
        assert!(!ledger.contains(secret));
    }
    reader_check(&store, "opencode_acp", "completed");
}

#[test]
fn opencode_acp_retries_transport_before_prompt() {
    let temp = Temp::new();
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    let flag = temp.0.join("crashed-once");
    let mut command = acp_command(
        "crash-once",
        &[(
            "LOCUST_OPENCODE_CRASH_FLAG",
            flag.to_string_lossy().into_owned(),
        )],
    );
    command.current_dir(&temp.0);
    let mut rt = OpenCode::spawn(&mut command).unwrap();
    let turn = Turn {
        prompt: "Retry me".into(),
        folder: temp.0.clone(),
        model: None,
        effort: None,
        resume: None,
        mode: Mode::Ask,
        approvals: None,
        mission_id: "opencode_retry".into(),
        run_id: "run_opencode_retry".into(),
    };
    let mut writer = LedgerWriter::create_runtime(
        &store,
        "opencode_retry",
        &turn.prompt,
        "account-default",
        "fake-opencode",
        "opencode",
        &[],
        None,
    )
    .unwrap();
    rt.start(&turn).unwrap();
    // Ask auto-rejects the permission card; the turn still completes.
    let events = drive(&mut rt, &mut writer, &Channel::new(), "opencode_retry");
    assert!(flag.exists());
    assert_eq!(events.last().unwrap().kind, "run.completed");
    reader_check(&store, "opencode_retry", "completed");
}

#[test]
fn opencode_serve_streams_authenticated_sse() {
    let temp = Temp::new();
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    let mut command = Command::new("python3");
    command
        .arg(fixture("m4-opencode-serve.py"))
        .arg("serve")
        .arg("success")
        .current_dir(&temp.0);
    command.env("LOCUST_NATIVE_OPENCODE_PROTOCOL", "serve");
    let mut rt = OpenCode::spawn(&mut command).unwrap();
    let turn = Turn {
        prompt: "Serve me".into(),
        folder: temp.0.clone(),
        model: Some("opencode/beam-free".into()),
        effort: None,
        resume: None,
        mode: Mode::Ask,
        approvals: None,
        mission_id: "opencode_serve".into(),
        run_id: "run_opencode_serve".into(),
    };
    let mut writer = LedgerWriter::create_runtime(
        &store,
        "opencode_serve",
        &turn.prompt,
        "opencode/beam-free",
        "fake-opencode",
        "opencode",
        &[],
        None,
    )
    .unwrap();
    rt.start(&turn).unwrap();
    let events = drive(&mut rt, &mut writer, &Channel::new(), "opencode_serve");
    assert_eq!(events.last().unwrap().kind, "run.completed");
    assert!(events.iter().any(|e| {
        e.kind == "message.delta"
            && e.payload["text"]
                .as_str()
                .unwrap_or("")
                .contains("Served hello")
    }));
    assert_eq!(events.last().unwrap().payload["runtimeThreadId"], "sess1");
    // One reply is one message: a paced partial flushed after the final text must not open a second one with the
    // same words (every live native OpenCode reply showed twice before, 11/11).
    let items: std::collections::HashSet<&str> = events
        .iter()
        .filter(|e| e.kind == "message.delta")
        .filter_map(|e| e.payload["itemId"].as_str())
        .collect();
    assert_eq!(items.len(), 1, "the reply was split into {items:?}");
    reader_check(&store, "opencode_serve", "completed");
}

#[test]
fn opencode_serve_busy_restarts_then_runs() {
    let temp = Temp::new();
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    let flag = temp.0.join("busy-once");
    let mut command = Command::new("python3");
    command
        .arg(fixture("m4-opencode-serve.py"))
        .arg("serve")
        .arg("busy")
        .current_dir(&temp.0)
        .env("LOCUST_NATIVE_OPENCODE_PROTOCOL", "serve")
        .env("LOCUST_OPENCODE_BUSY_FLAG", &flag);
    let mut rt = OpenCode::spawn(&mut command).unwrap();
    let turn = Turn {
        prompt: "Busy then ok".into(),
        folder: temp.0.clone(),
        model: None,
        effort: None,
        resume: None,
        mode: Mode::Auto,
        approvals: None,
        mission_id: "opencode_busy".into(),
        run_id: "run_opencode_busy".into(),
    };
    let mut writer = LedgerWriter::create_runtime(
        &store,
        "opencode_busy",
        &turn.prompt,
        "account-default",
        "fake-opencode",
        "opencode",
        &[],
        None,
    )
    .unwrap();
    rt.start(&turn).unwrap();
    let events = drive(&mut rt, &mut writer, &Channel::new(), "opencode_busy");
    assert!(flag.exists());
    assert_eq!(events.last().unwrap().kind, "run.completed");
    reader_check(&store, "opencode_busy", "completed");
}

#[test]
fn opencode_teammate_uses_native_driver() {
    let t = Temp::new();
    let store = Store::new(t.0.join("native"), t.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    let _guard = crate::native::hooks::install(
        store.clone(),
        Some((
            "python3".into(),
            vec![
                fixture("m4-opencode-acp.py").to_string_lossy().into_owned(),
                "acp".into(),
                "success".into(),
            ],
        )),
    );
    let team = crate::native::parse_roster(
        &json!({"schemaVersion":1,"teammates":[{"teammateId":"oc","name":"Open","role":"Custom","roleTitle":"Test","hue":"lime","createdAt":"2026-11-09T12:00:00Z","folder":t.0,"route":{"runtime":"opencode","model":"opencode/beam-free","mode":"approve-each"}}]}).to_string(),
    )
    .unwrap();
    assert!(crate::native::supported(&team[0]));
    let started = crate::native::send(crate::native::Send {
        teammate: team[0].clone(),
        conversation: None,
        prompt: "Synthetic OpenCode prompt".into(),
        model: None,
        effort: None,
    })
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if let Some(card) = crate::native::waiting_approvals_for(&started.conversation_id).first() {
            crate::native::decide_approval(&card.approval_id, Decision::ApproveOnce, None).unwrap();
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
#[ignore = "real OpenCode one-line turn; requires LOCUST_REAL_OPENCODE=1 and opencode on PATH"]
fn real_opencode_one_line() {
    assert_eq!(
        std::env::var("LOCUST_REAL_OPENCODE").ok().as_deref(),
        Some("1"),
        "set LOCUST_REAL_OPENCODE=1 to run this real test"
    );
    let t = Temp::new();
    let store = Store::new(t.0.join("native"), t.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    let mut command = crate::runtime::opencode::command(&t.0, Mode::Ask, &store).unwrap();
    command.current_dir(&t.0);
    let mut writer = LedgerWriter::create_runtime(
        &store,
        "real_opencode",
        "Reply with OK only.",
        "account-default",
        &command.get_program().to_string_lossy(),
        "opencode",
        &command
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
        None,
    )
    .unwrap();
    let mut rt = OpenCode::spawn(&mut command).unwrap();
    rt.start(&Turn {
        prompt: "Reply with OK only.".into(),
        folder: t.0.clone(),
        model: None,
        effort: None,
        resume: None,
        mode: Mode::Ask,
        approvals: None,
        mission_id: "real_opencode".into(),
        run_id: "run_real_opencode".into(),
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
fn opencode_config_and_writable_homes_stay_native_for_both_versions() {
    for v2 in [false, true] {
        let temp = Temp::new();
        let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
        let mut c = Command::new("opencode");
        let plugin =
            super::super::opencode_config::configure(&mut c, Mode::Auto, &store, v2).unwrap();
        assert!(plugin.starts_with(store.root()));
        for key in [
            "XDG_CONFIG_HOME",
            "XDG_DATA_HOME",
            "XDG_CACHE_HOME",
            "XDG_STATE_HOME",
        ] {
            let (_, value) = c.get_envs().find(|(k, _)| *k == key).unwrap();
            assert!(PathBuf::from(value.unwrap()).starts_with(store.root()));
        }
    }
}
