use super::*;
use crate::{
    approval::{Channel, Mode},
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
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            ".m4-kilo-{}-{}-{}",
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
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/fixtures/m4-kilo-acp.py")
}
fn drive(rt: &mut Kilo, writer: &mut LedgerWriter) -> Vec<Event> {
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut events = vec![];
    loop {
        assert!(Instant::now() < deadline, "Kilo fake timed out");
        if let Some(e) = rt.next_event().unwrap() {
            let terminal = matches!(e.kind.as_str(), "run.completed" | "run.failed" | "run.cancelled");
            writer.append(&e.kind, e.payload.clone(), e.thread.as_deref()).unwrap();
            events.push(e);
            if terminal {
                break;
            }
        }
    }
    events
}
fn reader_check(store: &Store, id: &str, phase: &str) {
    if let Ok(reader) = std::env::var("LOCUST_TEST_TS_READER") {
        assert!(
            Command::new("node")
                .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/check-m1-ledger.mjs"))
                .arg(reader)
                .arg(store.ledger_dir().unwrap())
                .arg(id)
                .arg(phase)
                .status()
                .unwrap()
                .success()
        );
    }
}

#[test]
fn kilo_model_and_mode_defaults() {
    assert_eq!(model_for_run(None), AUTO_FREE_MODEL);
    assert_eq!(model_for_run(Some("account-default")), AUTO_FREE_MODEL);
    assert_eq!(model_for_run(Some("  ")), AUTO_FREE_MODEL);
    assert_eq!(model_for_run(Some("kilo/custom")), "kilo/custom");
    assert_eq!(session_mode(Mode::Ask), "ask");
    assert_eq!(session_mode(Mode::Plan), "ask");
    assert_eq!(session_mode(Mode::AcceptEdits), "code");
    assert_eq!(session_mode(Mode::ApproveEach), "code");
    assert_eq!(session_mode(Mode::Auto), "code");
    let env = isolation_env(Path::new("/tmp/kilo-home"));
    assert!(env.iter().any(|(k, v)| k == "KILO_CONFIG_DIR" && v == "/tmp/kilo-home"));
    assert!(env.iter().any(|(k, _)| k == "KILO_DISABLE_PROJECT_CONFIG"));
}

#[test]
fn kilo_acp_defaults_to_auto_free_and_rejects_without_host_cards() {
    let temp = Temp::new();
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    std::fs::write(store.electron().join("sentinel"), "untouched").unwrap();
    let mut command = Command::new("python3");
    command.arg(fixture()).arg("acp").arg("success").current_dir(&temp.0);
    let mut rt = Kilo::spawn(&mut command).unwrap();
    let turn = Turn {
        prompt: "Synthetic Kilo prompt".into(),
        folder: temp.0.clone(),
        model: None,
        effort: None,
        resume: None,
        mode: Mode::ApproveEach,
        approvals: Some(Channel::new()),
        mission_id: "kilo_acp".into(),
        run_id: "run_kilo_acp".into(),
    };
    let mut writer = LedgerWriter::create_runtime(
        &store,
        "kilo_acp",
        &turn.prompt,
        AUTO_FREE_MODEL,
        "fake-kilo",
        "kilo",
        &[],
        None,
    )
    .unwrap();
    rt.start(&turn).unwrap();
    let events = drive(&mut rt, &mut writer);
    assert_eq!(events.last().unwrap().kind, "run.completed");
    assert!(events.iter().any(|e| e.kind == "message.delta" && e.payload["text"] == "Kilo says hi."));
    assert!(events.iter().any(|e| e.kind == "tool.failed"));
    assert!(Channel::new().waiting().is_empty());
    assert_eq!(std::fs::read_to_string(store.electron().join("sentinel")).unwrap(), "untouched");
    let ledger = std::fs::read_to_string(&writer.path).unwrap();
    assert!(!ledger.contains("RAW_REASONING_MUST_NOT_SURVIVE"));
    assert!(!ledger.contains("mission.approval"));
    reader_check(&store, "kilo_acp", "completed");
}

#[test]
fn kilo_auto_allows_without_a_card() {
    let temp = Temp::new();
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    let mut command = Command::new("python3");
    command.arg(fixture()).arg("acp").arg("auto").current_dir(&temp.0);
    let mut rt = Kilo::spawn(&mut command).unwrap();
    let turn = Turn {
        prompt: "Auto Kilo".into(),
        folder: temp.0.clone(),
        model: Some("kilo/custom".into()),
        effort: None,
        resume: None,
        mode: Mode::Auto,
        approvals: None,
        mission_id: "kilo_auto".into(),
        run_id: "run_kilo_auto".into(),
    };
    let mut writer = LedgerWriter::create_runtime(&store, "kilo_auto", &turn.prompt, "kilo/custom", "fake-kilo", "kilo", &[], None).unwrap();
    rt.start(&turn).unwrap();
    let events = drive(&mut rt, &mut writer);
    assert_eq!(events.last().unwrap().kind, "run.completed");
    assert!(events.iter().any(|e| e.kind == "tool.completed"));
    reader_check(&store, "kilo_auto", "completed");
}

#[test]
fn kilo_sets_model_before_mode() {
    let temp = Temp::new();
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    let mut command = Command::new("python3");
    command.arg(fixture()).arg("acp").arg("order").current_dir(&temp.0);
    let mut rt = Kilo::spawn(&mut command).unwrap();
    let turn = Turn {
        prompt: "Order".into(),
        folder: temp.0.clone(),
        model: None,
        effort: None,
        resume: None,
        mode: Mode::Ask,
        approvals: None,
        mission_id: "kilo_order".into(),
        run_id: "run_kilo_order".into(),
    };
    let mut writer = LedgerWriter::create_runtime(&store, "kilo_order", &turn.prompt, AUTO_FREE_MODEL, "fake-kilo", "kilo", &[], None).unwrap();
    rt.start(&turn).unwrap();
    let events = drive(&mut rt, &mut writer);
    assert_eq!(events.last().unwrap().kind, "run.completed");
}

#[test]
fn kilo_teammate_uses_native_driver() {
    let t = Temp::new();
    let store = Store::new(t.0.join("native"), t.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    let _guard = crate::native::hooks::install(
        store.clone(),
        Some((
            "python3".into(),
            vec![fixture().to_string_lossy().into_owned(), "acp".into(), "success".into()],
        )),
    );
    let team = crate::native::parse_roster(
        &json!({"schemaVersion":1,"teammates":[{"teammateId":"k1","name":"Kilo","role":"Custom","roleTitle":"Test","hue":"lime","createdAt":"2026-11-09T12:00:00Z","folder":t.0,"route":{"runtime":"kilo","model":"account-default","mode":"ask"}}]}).to_string(),
    )
    .unwrap();
    assert!(crate::native::supported(&team[0]));
    let started = crate::native::send(crate::native::Send {
        teammate: team[0].clone(),
        conversation: None,
        prompt: "Synthetic Kilo prompt".into(),
        model: None,
        effort: None,
    })
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    while crate::native::owns(&started.conversation_id) {
        assert!(Instant::now() < deadline);
        // Kilo must not raise a Locust approval card.
        assert!(crate::native::waiting_approvals_for(&started.conversation_id).is_empty());
        std::thread::sleep(Duration::from_millis(10));
    }
    let ledger = std::fs::read_to_string(store.ledger_dir().unwrap().join(format!("{}.jsonl", started.conversation_id))).unwrap();
    assert!(ledger.contains("run.completed"));
    assert!(!ledger.contains("mission.approval"));
    assert!(ledger.contains(AUTO_FREE_MODEL));
    assert_eq!(crate::native::prior_of(&started.conversation_id).unwrap().thread.as_deref(), Some("s1"));
}

#[test]
#[ignore = "real Kilo one-line turn; requires LOCUST_REAL_KILO=1 and kilo on PATH"]
fn real_kilo_one_line() {
    if std::env::var_os("LOCUST_REAL_KILO").is_none() {
        return;
    }
    let t = Temp::new();
    let store = Store::new(t.0.join("native"), t.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    let mut command = crate::runtime::kilo::command(&t.0, &store).unwrap();
    command.current_dir(&t.0);
    let model = model_for_run(None);
    let mut writer = LedgerWriter::create_runtime(
        &store,
        "real_kilo",
        "Reply with OK only.",
        &model,
        &command.get_program().to_string_lossy(),
        "kilo",
        &command.get_args().map(|a| a.to_string_lossy().into_owned()).collect::<Vec<_>>(),
        None,
    )
    .unwrap();
    let mut rt = Kilo::spawn(&mut command).unwrap();
    rt.start(&Turn {
        prompt: "Reply with OK only.".into(),
        folder: t.0.clone(),
        model: Some(model),
        effort: None,
        resume: None,
        mode: Mode::Ask,
        approvals: None,
        mission_id: "real_kilo".into(),
        run_id: "run_real_kilo".into(),
    })
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        assert!(Instant::now() < deadline);
        if let Some(e) = rt.next_event().unwrap() {
            writer.append(&e.kind, e.payload, e.thread.as_deref()).unwrap();
            if e.kind.starts_with("run.") && e.kind != "run.started" {
                assert_eq!(e.kind, "run.completed");
                break;
            }
        }
    }
}
