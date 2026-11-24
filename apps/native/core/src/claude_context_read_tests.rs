//! The context menu's read through the engine (index.ts's `resolve` for `createClaudeContextReader`, codex-mission.ts
//! `beforeStart`): `native::read_claude_context` on stored turns, `/context` answered by a stand-in that notes each run.
use crate::claude_context::{ContextCommand, Stop};
use crate::native::{self, Origin, Send, hooks, nobody, owns, send_from, set_open_folder};
use crate::store::Store;
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const FIXTURE: &str = include_str!("../../../desktop/src/main/fixtures/claude-context.json");

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn temp_store(name: &str) -> (Temp, Store) {
    let root = std::env::temp_dir().join(format!("locust-context-read-{name}-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    fs::create_dir_all(root.join("electron/mission-ledger")).unwrap();
    fs::create_dir_all(root.join("work")).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    fs::create_dir_all(store.ledger_dir().unwrap()).unwrap();
    (Temp(root), store)
}

/// A turn's ledger: its header (with `extra` merged into the metadata) and its events, oldest first.
fn write_turn(store: &Store, id: &str, runtime: &str, extra: Value, events: &[(&str, Value)]) {
    let mut header = json!({"schemaVersion": 27, "recordType": "mission.created", "metadata": {"missionId": id, "prompt": "Synthetic prompt",
        "runtime": runtime, "model": "sample", "createdAt": "2026-01-01T00:00:00Z"}});
    for (k, v) in extra.as_object().unwrap() {
        header["metadata"][k] = v.clone();
    }
    let mut lines = vec![header];
    for (n, (kind, payload)) in events.iter().enumerate() {
        lines.push(json!({"schemaVersion": 27, "recordType": "mission.event", "event": {"sequence": n + 1, "occurredAt": "2026-01-01T00:00:01Z",
            "type": kind, "payload": payload}}));
    }
    fs::write(store.ledger_dir().unwrap().join(format!("{id}.jsonl")), lines.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
}

fn finished(session: &str) -> Vec<(&'static str, Value)> {
    vec![("run.started", json!({"runtimeThreadId": session})), ("run.completed", json!({"runtimeThreadId": session}))]
}

/// The launch the engine keeps as a Claude turn starts: paths and flags only.
fn keep_launch(store: &Store, id: &str, cwd: &str) {
    let path = store.root().join("claude-context-launches.json");
    let mut kept: Value = fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| json!({}));
    kept[id] = json!({"cwd": cwd, "executablePath": "/usr/local/bin/claude", "sandbox": "read-only", "plugins": [],
        "args": ["--print", "--output-format", "stream-json", "--verbose", "--model", "opus"]});
    fs::create_dir_all(store.root()).unwrap();
    fs::write(path, kept.to_string()).unwrap();
}

/// `/context` answered with the fixture, each run's arguments noted.
fn stand_in() -> Arc<Mutex<Vec<Vec<String>>>> {
    let asked = Arc::new(Mutex::new(Vec::new()));
    let noted = asked.clone();
    *crate::invariants::lock(&hooks::CLAUDE_CONTEXT_RUN) = Some(Arc::new(move |command: &ContextCommand, _: &Stop| {
        noted.lock().unwrap().push(command.args.clone());
        Ok(FIXTURE.to_owned())
    }));
    asked
}

#[test]
fn a_finished_turn_on_another_agent_is_never_read() {
    let (t, store) = temp_store("codex");
    write_turn(&store, "m1", "codex", json!({}), &finished("sess-1"));
    keep_launch(&store, "m1", &t.0.to_string_lossy());
    let _hook = hooks::install(store, None);
    let asked = stand_in();
    assert_eq!(native::read_claude_context("m1"), None);
    assert!(asked.lock().unwrap().is_empty(), "/context is Claude Code's: never run for a Codex turn");
}

#[test]
fn a_turn_the_ledger_says_is_still_running_is_busy_and_not_read() {
    let (t, store) = temp_store("running");
    // Started, written to just now, and no terminal record: still running, though not by this process.
    write_turn(&store, "m1", "claude", json!({}), &[("run.started", json!({"runtimeThreadId": "sess-1"}))]);
    keep_launch(&store, "m1", &t.0.to_string_lossy());
    let _hook = hooks::install(store.clone(), None);
    let asked = stand_in();
    assert!(!owns("m1"), "not a turn this process runs");
    assert_eq!(native::read_claude_context("m1"), None);
    assert!(asked.lock().unwrap().is_empty(), "never run while a turn runs");

    // Once it has finished, the same conversation is read.
    write_turn(&store, "m1", "claude", json!({}), &finished("sess-1"));
    assert_eq!(native::read_claude_context("m1").map(|b| b.used), Some(45_200));
    assert_eq!(asked.lock().unwrap().len(), 1);
}

#[test]
fn a_claude_turn_starting_stops_a_read_in_progress_and_waits_for_it() {
    let (t, store) = temp_store("start");
    let work = t.0.join("work");
    write_turn(&store, "m1", "claude", json!({"command": {"cwd": work.to_string_lossy()}}), &finished("sess-1"));
    keep_launch(&store, "m1", &work.to_string_lossy());
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("scripts/fixtures/m3-claude.py");
    let _hook = hooks::install(store.clone(), Some((PathBuf::from("python3"), vec![fixture.to_string_lossy().into_owned(), "success".into()])));
    set_open_folder(&store, &work).unwrap();

    // The stand-in tells when it runs and when it is told to stop, then holds its answer until released.
    let (started_tx, started) = mpsc::channel::<()>();
    let (stopped_tx, stopped) = mpsc::channel::<()>();
    let (release, released) = mpsc::channel::<()>();
    let (started_tx, stopped_tx, released) = (Mutex::new(started_tx), Mutex::new(stopped_tx), Mutex::new(released));
    *crate::invariants::lock(&hooks::CLAUDE_CONTEXT_RUN) = Some(Arc::new(move |_: &ContextCommand, stop: &Stop| {
        let _ = started_tx.lock().unwrap().send(());
        let until = Instant::now() + Duration::from_secs(60);
        while !stop.is_stopped() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(2));
        }
        if stop.is_stopped() && !stop.timed_out() {
            let _ = stopped_tx.lock().unwrap().send(());
        }
        let _ = released.lock().unwrap().recv_timeout(Duration::from_secs(60));
        Ok(FIXTURE.to_owned())
    }));

    let reading = std::thread::spawn(|| native::read_claude_context("m1"));
    started.recv_timeout(Duration::from_secs(30)).expect("the read runs /context");

    let (sent_tx, sent) = mpsc::channel();
    let starting = std::thread::spawn(move || {
        let result = send_from(
            Send {
                teammate: nobody(Some("claude".into()), Some("opus".into())),
                conversation: Some("m1".into()),
                prompt: "Synthetic prompt".into(),
                model: Some("opus".into()),
                effort: None,
            },
            Origin { started_by: Some(json!({"kind": "app", "name": "test"})), ..Default::default() },
        );
        let _ = sent_tx.send(result.map(|s| s.conversation_id));
    });

    stopped.recv_timeout(Duration::from_secs(30)).expect("a Claude turn starting stops the /context read");
    // It waits for the read to finish: the turn has not started while the read holds its answer.
    assert!(sent.try_recv().is_err(), "the turn started before the read it stopped had finished");
    assert!(!owns("m1"));
    release.send(()).unwrap();

    assert_eq!(reading.join().unwrap(), None, "the menu shows nothing from a stopped read");
    let conversation = sent.recv_timeout(Duration::from_secs(60)).expect("the turn starts once the read is done").expect("started");
    starting.join().unwrap();
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    while owns(&conversation) {
        assert!(Instant::now() < deadline, "the turn never finished");
        std::thread::sleep(Duration::from_millis(10));
    }
}
