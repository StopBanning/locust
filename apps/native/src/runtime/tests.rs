use super::{
    Runtime, Turn,
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
