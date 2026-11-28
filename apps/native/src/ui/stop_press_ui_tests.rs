//! stop-press.test.ts (0.806) through the window: Stop pressed on a conversation whose run has not started yet (the
//! send is out, no run name has come back) stops the turn once it starts, instead of the press being dropped and the
//! turn running on. The runtime is the fake app-server held mid-turn until it is interrupted.
use super::*;
use rune::{Env, Key, Runtime};
use std::path::PathBuf;
use std::time::{Duration, Instant};

struct Dirs(PathBuf);
impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..6 {
        rt.frame(0.016);
    }
}

fn click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

fn label(rt: &Runtime<State>, key: &str) -> String {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.label.or(e.text)).unwrap_or_default()
}

/// Every terminal record any turn in the store wrote.
fn terminals(store: &locust_core::store::Store) -> Vec<String> {
    let dir = store.ledger_dir().unwrap();
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
        for line in std::fs::read_to_string(entry.path()).unwrap_or_default().lines() {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
            let kind = v.pointer("/event/type").and_then(|t| t.as_str()).unwrap_or_default();
            if matches!(kind, "run.completed" | "run.failed" | "run.cancelled") {
                out.push(kind.to_owned());
            }
        }
    }
    out
}

fn stop_window() -> (Dirs, locust_core::store::Store, native::hooks::Guard, Runtime<State>) {
    let root = std::env::temp_dir().join(format!("locust-stop-starting-{}-{}", std::process::id(), locust_core::invariants::unique_nanos()));
    let dirs = Dirs(root.clone());
    std::fs::create_dir_all(root.join("electron/mission-ledger")).unwrap();
    std::fs::create_dir_all(root.join("work")).unwrap();
    let store = locust_core::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    store
        .save_json("teammates.json", &serde_json::json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_codec", "name": "Codec"}], "missionOwners": {}, "settings": {}}))
        .unwrap();
    let script = format!("{}/scripts/fixtures/m1-app-server.py", env!("CARGO_MANIFEST_DIR"));
    let hook = native::hooks::install(store.clone(), Some((PathBuf::from("python3"), vec![script, "interrupt".into()])));
    let mut s = State::default();
    s.engine = Some(Kind::Native);
    s.data = Some(store.clone());
    apply_team(&mut s, vec![Teammate {
        id: "tm_codec".into(),
        name: "Codec".into(),
        role: "Code & Migrations".into(),
        runtime: Some("codex".into()),
        model: Some("synthetic-model".into()),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: Some(root.join("work").to_string_lossy().into()),
    }]);
    let mut rt = Runtime::new(s, crate::view, (1400.0, 800.0), Env::new(true, 1.0));
    rt.frame(0.0);
    (dirs, store, hook, rt)
}

fn send_from_composer(rt: &mut Runtime<State>) {
    click(rt, "composer");
    rt.paste("Synthetic prompt");
    rt.frame(0.016);
    rt.key(Key::Enter.into());
    settle(rt);

}

// case: remembers the press when the run is still starting, instead of dropping it
#[test]
fn stop_pressed_before_the_run_is_named_stops_it_once_it_starts() {
    let (dirs, store, _hook, mut rt) = stop_window();
    send_from_composer(&mut rt);

    // Sent, and nothing has named the run: the composer already offers Stop, and pressing it is remembered.
    assert!(rt.state.flight.is_some(), "the send is out: {:?}", rt.state.status);
    assert_eq!(label(&rt, "send"), "Stop", "Stop is offered while the run is starting");
    click(&mut rt, "send");
    assert!(rt.state.stop_when_named.is_some(), "the press is remembered, not dropped");
    assert_eq!(rt.state.status, "Stopping…");

    // The start lands with the run's name; the remembered press stops it, and the turn ends cancelled rather than
    // running on (the fake server holds the turn until it is interrupted).
    let until = Instant::now() + Duration::from_secs(30);
    let mut now = 0.0;
    while !terminals(&store).iter().any(|t| t == "run.cancelled") {
        assert!(Instant::now() < until, "the turn was never stopped: status {:?}, terminals {:?}", rt.state.status, terminals(&store));
        rt.update(move |s, tx| {
            poll(s, now, tx);
        });
        rt.frame(0.1);
        now += 0.1;
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(rt.state.stop_when_named.is_none(), "the press was answered once");
    assert_eq!(terminals(&store), vec!["run.cancelled".to_owned()]);
    on_quit(&mut rt.state, &mut Tx::default());
    drop(dirs);
}


// Runs in the isolated application test process. This exercises the actual Stop UI
// while the engine's existing boundary supplies a named, owned run without a child process.
struct NamedRun {
    calls: std::sync::Mutex<Vec<String>>,
    release: std::sync::Mutex<std::sync::mpsc::Receiver<()>>,
}

impl locust_core::engine_api::EngineApi for NamedRun {
    fn send(&self, _: native::Send, _: native::Origin) -> Result<engine::Started, String> { unreachable!() }
    fn stop(&self, id: &str) -> Result<String, String> {
        self.calls.lock().unwrap().push(id.to_owned());
        self.release.lock().unwrap().recv_timeout(Duration::from_secs(5))
            .map_err(|_| "fixture did not finish the Stop".to_owned())?;
        Ok("Stopped".into())
    }
    fn owns(&self, id: &str) -> bool { id == "m_1" }
    fn running(&self) -> Vec<String> { vec!["m_1".into()] }
    fn waiting(&self) -> Vec<locust_core::approval::Request> { Vec::new() }
    fn decide(&self, _: &str, _: locust_core::approval::Decision, _: Option<String>) -> Result<(), String> { unreachable!() }
    fn answer(&self, _: &str, _: Vec<(String, Vec<String>)>) -> Result<(), String> { unreachable!() }
    fn save_rule_and_decide(&self, _: &locust_core::approval::Request, _: locust_core::approval_rules::RuleEffect) -> Result<(), String> { unreachable!() }
    fn carry(&self, _: &str, _: &str, _: Option<&str>) -> Result<(String, Option<String>, String), String> { unreachable!() }
    fn shut_down(&self) {}
    fn wait_for_shutdown(&self) {}
    fn room_notices(&self) -> Vec<locust_core::room_tasks::RoomNotice> { Vec::new() }
}

// Electron also clears a transient error on this action. Native's Stop handler lives
// outside this lane; this test isolates the real cancelling phase of an already named run.
#[test]
fn messages_stop_on_a_named_live_turn_switches_the_live_line_immediately() {
    // `engine_api::install` chooses the engine for the whole process, and this fake says a run is going forever and
    // refuses every send: in `cargo test`'s one shared process every window test after it waited on that run while
    // holding the engine's lock, and the window suite never finished (28 Nov). So the test runs in a process of its own.
    const CHILD: &str = "LOCUST_STOP_NAMED_RUN_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let test = concat!(module_path!(), "::messages_stop_on_a_named_live_turn_switches_the_live_line_immediately");
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test.split_once("::").unwrap().1, "--nocapture"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("1 passed"),
            "{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    assert!(std::env::var_os(CHILD).is_some(), "the fake engine is installed only in this test's own process");
    let (release, wait) = std::sync::mpsc::channel();
    let fake = std::sync::Arc::new(NamedRun {
        calls: std::sync::Mutex::new(Vec::new()),
        release: std::sync::Mutex::new(wait),
    });
    locust_core::engine_api::install(fake.clone()).unwrap();
    let mut s = crate::messages::tests_live_state();
    s.engine = Some(Kind::Native);
    s.flight = Some(Flight {
        teammate_id: "tm_codec".into(), conversation_id: Some("m_1".into()),
        header: None, after_tip: None, message: None,
    });
    let pane = s.ledger.as_mut().unwrap();
    pane.snapshot.tip = "m_1".into();
    pane.items = vec![ledger::Item::User("Synthetic prompt".into())];
    s.teammates = vec![pane.header.clone()];
    let mut rt = Runtime::new(s, crate::view, (1400.0, 800.0), Env::new(true, 1.0));
    settle(&mut rt);
    assert_eq!(native_stop_key(&rt.state).as_deref(), Some("m_1"));
    assert_eq!(label(&rt, "send"), "Stop");
    click(&mut rt, "send");
    assert!(rt.state.stopping.is_pending(), "the actual Stop press must enter cancelling");
    assert!(rt.elements().into_iter().any(|e| !e.exiting && e.text.as_deref() == Some("Stopping…")));
    let until = Instant::now() + Duration::from_secs(5);
    while fake.calls.lock().unwrap().is_empty() {
        assert!(Instant::now() < until, "the window did not stop the named run");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(*fake.calls.lock().unwrap(), ["m_1"]);
    release.send(()).unwrap();
}
