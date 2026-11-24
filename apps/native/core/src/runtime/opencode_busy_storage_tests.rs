//! Electron's `packages/runtime-adapters/test/a-server-whose-storage-is-busy-starts-again.test.ts`, case by case,
//! through the caller: a native OpenCode turn (`OpenCode::start`, then `next_event` until the end) whose server is a
//! stand-in that dies with the next of its `deaths` on stderr, or becomes the scripted OpenCode server
//! (`scripts/fixtures/m4-opencode-serve.py success`) once they run out. Each start is written down, one line apiece.
use super::*;
use crate::approval::Mode;
use serde_json::json;
use std::{
    path::PathBuf,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

// Its own words, from the judge it lost.
const LOCKED: &str = "\u{1b}[91m\u{1b}[1mError: \u{1b}[0mUnexpected error\r\n\r\ndatabase is locked";

const STAND_IN: &str = r#"
import json, os, sys, time
deaths = json.loads(os.environ['LOCUST_DEATHS'])
started = os.environ['LOCUST_STARTED']
n = len(open(started).read().splitlines()) if os.path.exists(started) else 0
with open(started, 'a') as f:
    f.write(('listening' if n >= len(deaths) else 'died') + '\n')
if n < len(deaths):
    if os.environ.get('LOCUST_LATE_WORDS') == '1':
        # Words that reach the pipe just after the exit does: a child of its own holds stderr and says them late.
        if os.fork() == 0:
            time.sleep(0.2)
            sys.stderr.write(deaths[n]); sys.stderr.flush()
            os._exit(0)
        os._exit(1)
    sys.stderr.write(deaths[n]); sys.stderr.flush()
    sys.exit(1)
os.execv(sys.executable, [sys.executable, os.environ['LOCUST_SERVER'], 'serve', 'success'])
"#;

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Servers {
    temp: Temp,
    command: Command,
}

/// Each server dies with the next of `deaths` (its stderr), or listens once they run out.
fn servers(deaths: &[&str], late_words: bool) -> Servers {
    let root = std::env::temp_dir().join(format!("locust-busy-storage-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(&root).unwrap();
    let script = root.join("stand-in.py");
    std::fs::write(&script, STAND_IN).unwrap();
    let server = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/..")).join("scripts/fixtures/m4-opencode-serve.py");
    let mut command = Command::new("python3");
    command
        .arg(&script)
        .current_dir(&root)
        .env("LOCUST_NATIVE_OPENCODE_PROTOCOL", "serve")
        .env("LOCUST_DEATHS", json!(deaths).to_string())
        .env("LOCUST_STARTED", root.join("started"))
        .env("LOCUST_SERVER", server)
        .env("LOCUST_LATE_WORDS", if late_words { "1" } else { "0" });
    Servers { temp: Temp(root), command }
}

impl Servers {
    fn started(&self) -> Vec<String> {
        std::fs::read_to_string(self.temp.0.join("started")).unwrap_or_default().lines().map(str::to_owned).collect()
    }
}

fn turn(folder: &std::path::Path) -> Turn {
    Turn {
        prompt: "Judge these three answers.".into(),
        folder: folder.to_path_buf(),
        model: None,
        effort: None,
        resume: None,
        mode: Mode::Auto,
        approvals: None,
        mission_id: "busy_storage".into(),
        run_id: "run_busy_storage".into(),
    }
}

fn kinds(events: &[Event]) -> String {
    events.iter().map(|e| format!("{} {}", e.kind, e.payload)).collect::<Vec<_>>().join("\n")
}

/// Start the turn and read every event to its end.
fn run(servers: &mut Servers, cancel: Arc<AtomicBool>) -> Vec<Event> {
    let mut rt = OpenCode::spawn(&mut servers.command).unwrap().with_cancel(cancel);
    rt.start(&turn(&servers.temp.0)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut events = vec![];
    loop {
        assert!(Instant::now() < deadline, "the OpenCode turn never ended: {}", kinds(&events));
        if let Some(e) = rt.next_event().unwrap() {
            let end = matches!(e.kind.as_str(), "run.completed" | "run.failed" | "run.cancelled");
            events.push(e);
            if end {
                return events;
            }
        } else {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

#[test]
fn starts_again_when_another_opencode_held_its_storage_and_the_turn_goes_on_as_usual() {
    // case: starts again when another OpenCode held its storage, and the turn goes on as usual
    let mut server = servers(&[LOCKED], false);
    let events = run(&mut server, Arc::new(AtomicBool::new(false)));
    let end = events.last().unwrap();
    assert_eq!(end.kind, "run.completed", "{}", kinds(&events));
    assert_eq!(end.payload["process"]["exitCode"], 0);
    assert_eq!(end.payload["process"]["stderr"], "");
    assert_eq!(server.started(), ["died", "listening"]);
    // The prompt went once, to the server that listened: one turn, one end.
    assert_eq!(events.iter().filter(|e| e.kind.starts_with("run.") && e.kind != "run.started").count(), 1);
}

#[test]
fn reads_words_that_trail_the_exit_before_deciding() {
    // case: reads words that trail the exit before deciding
    let mut server = servers(&[LOCKED], true);
    let events = run(&mut server, Arc::new(AtomicBool::new(false)));
    let end = events.last().unwrap();
    assert_eq!(end.kind, "run.completed", "{}", kinds(&events));
    assert_eq!(end.payload["process"]["exitCode"], 0);
    assert_eq!(server.started().len(), 2);
}

#[test]
fn gives_up_after_three_restarts_saying_the_server_exited_and_why() {
    // case: gives up after three restarts, saying the server exited and why
    let mut server = servers(&[LOCKED, LOCKED, LOCKED, LOCKED], false);
    let events = run(&mut server, Arc::new(AtomicBool::new(false)));
    let end = events.last().unwrap();
    assert_eq!(end.kind, "run.failed", "{}", kinds(&events));
    assert_eq!(end.payload["process"]["exitCode"], 1);
    let stderr = end.payload["process"]["stderr"].as_str().unwrap();
    assert!(stderr.contains("exited before the turn finished"), "{stderr}");
    assert!(stderr.contains("database is locked"), "{stderr}");
    assert_eq!(server.started().len(), 4);
    // No server listened, so no prompt was sent: the failure is all the turn said.
    assert_eq!(events.len(), 1, "{}", kinds(&events));
}

#[test]
fn is_not_started_again_for_any_other_reason() {
    // case: is not started again for any other reason
    let mut server = servers(&["Error: Config file at opencode.json is not valid JSON"], false);
    let events = run(&mut server, Arc::new(AtomicBool::new(false)));
    let end = events.last().unwrap();
    assert_eq!(end.kind, "run.failed", "{}", kinds(&events));
    assert_eq!(end.payload["process"]["exitCode"], 1);
    assert!(end.payload["process"]["stderr"].as_str().unwrap().contains("not valid JSON"), "{}", end.payload);
    assert_eq!(server.started().len(), 1);
}

#[test]
fn starts_nothing_more_once_the_person_stops_it() {
    // case: starts nothing more once the person stops it
    let mut server = servers(&[LOCKED], false);
    let stopping = Arc::new(AtomicBool::new(false));
    let started = server.temp.0.join("started");
    let stop = stopping.clone();
    // The person stops it as soon as the first server has died, while the next one waits for its turn.
    let watcher = std::thread::spawn(move || {
        let until = Instant::now() + Duration::from_secs(10);
        while Instant::now() < until && !std::fs::read_to_string(&started).is_ok_and(|s| s.lines().count() >= 1) {
            std::thread::sleep(Duration::from_millis(2));
        }
        stop.store(true, Ordering::Relaxed);
    });
    let events = run(&mut server, stopping);
    watcher.join().unwrap();
    let end = events.last().unwrap();
    assert_eq!(end.kind, "run.cancelled", "{}", kinds(&events));
    assert_eq!(end.payload["process"]["exitCode"], serde_json::Value::Null);
    assert_eq!(server.started().len(), 1);
}
