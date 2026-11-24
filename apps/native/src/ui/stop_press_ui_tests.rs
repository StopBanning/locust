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

// case: remembers the press when the run is still starting, instead of dropping it
#[test]
fn stop_pressed_before_the_run_is_named_stops_it_once_it_starts() {
    let root = std::env::temp_dir().join(format!("locust-stop-starting-{}-{}", std::process::id(), locust_core::invariants::unique_nanos()));
    let dirs = Dirs(root.clone());
    std::fs::create_dir_all(root.join("electron/mission-ledger")).unwrap();
    std::fs::create_dir_all(root.join("work")).unwrap();
    let store = locust_core::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    store
        .save_json("teammates.json", &serde_json::json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_codec", "name": "Codec"}], "missionOwners": {}, "settings": {}}))
        .unwrap();
    let script = format!("{}/scripts/fixtures/m1-app-server.py", env!("CARGO_MANIFEST_DIR"));
    let _hook = native::hooks::install(store.clone(), Some((PathBuf::from("python3"), vec![script, "interrupt".into()])));
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
    click(&mut rt, "composer");
    rt.paste("Synthetic prompt");
    rt.frame(0.016);
    rt.key(Key::Enter.into());
    settle(&mut rt);

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
