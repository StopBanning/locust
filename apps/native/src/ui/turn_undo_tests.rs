//! The window's half of `an-undone-turn-says-so-on-its-card.test.tsx` and TurnUndo.tsx: a finished turn's foot offers
//! Undo under its files card, asks first, puts the files back off the window's thread, and then its heading says
//! "undone". A real checkpointed turn in a temporary folder, a temporary store, drawn by `message_column`.
use super::*;
use crate::store::Store;
use locust_core::checkpoints::{Turn, UndoState};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn items(paths: &[&str]) -> Vec<ledger::Item> {
    let mut rows: Vec<(&str, Value)> = vec![("run.started", json!({}))];
    for (i, path) in paths.iter().enumerate() {
        let id = format!("w{i}");
        let patch = json!({"text": format!("diff --git a/{path} b/{path}\nnew file mode 100644\n--- /dev/null\n+++ b/{path}\n@@ -0,0 +1 @@\n+x\n"), "truncated": false, "added": 1, "removed": 0});
        rows.push(("tool.started", json!({"itemId": id, "toolKind": "file_change", "name": "Write", "command": path, "phase": "started"})));
        rows.push(("tool.completed", json!({"itemId": id, "toolKind": "file_change", "name": "Write", "command": path, "phase": "completed", "patch": patch})));
    }
    rows.push(("run.completed", json!({"process": {"exitCode": 0}})));
    let mut t = locust_core::ledger::Transcript::default();
    t.apply(
        rows.iter()
            .enumerate()
            .map(|(i, (kind, payload))| json!({"recordType": "mission.event", "event": {"type": kind, "sequence": i + 1, "occurredAt": "2026-10-28T05:00:00.000Z", "payload": payload}}))
            .collect(),
    );
    t.items
}

/// A store and a folder in which `mission` wrote `report.md` and `notes.md`, its checkpoint finished.
fn turn_that_wrote(mission: &str) -> (Temp, Store, PathBuf) {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!("locust-turn-undo-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    std::fs::create_dir_all(root.join("work")).unwrap();
    std::fs::create_dir_all(root.join("electron/mission-ledger")).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    let work = root.join("work").canonicalize().unwrap();
    let turn = Turn::begin(&store, &work);
    std::fs::write(work.join("report.md"), "x\n").unwrap();
    std::fs::write(work.join("notes.md"), "x\n").unwrap();
    turn.finish(&store, mission).unwrap();
    (Temp(root), store, work)
}

fn window(folder: &Path, mission: &str, items: Vec<ledger::Item>) -> rune::Runtime<State> {
    let mut s = State::default();
    let header = Teammate { id: "tm".into(), name: "Codec".into(), role: "Custom".into(), model: None, runtime: Some("claude".into()), folder: Some(folder.to_string_lossy().into_owned()), runs_in: "ask".into(), account: None, busy: false };
    s.teammates.push(header.clone());
    s.conversations.insert("tm".into(), Default::default());
    s.ledger = Some(LedgerPane {
        cold_tip: None,
        header,
        conversation: Conversation { conversation_id: Some(mission.into()), turns: Vec::new() },
        items,
        dir: PathBuf::new(),
        worker: None,
        snapshot: ledger::Snapshot { turn_metas: vec![ledger::Metadata { mission_id: mission.into(), ..Default::default() }], ..Default::default() },
        due: 0.0,
        loading: false,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.0,
    });
    rune::Runtime::new(s, |s, _| message_column(&s.ledger.as_ref().unwrap().items, "Codec", &Palette::new(true), s), (800.0, 2000.0), rune::Env::new(true, 1.0))
}

/// The turn-undo and Changes polls, a frame each time, until `done`.
fn pump(rt: &mut rune::Runtime<State>, what: &str, done: impl Fn(&State) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !done(&rt.state) {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        rt.update(|s, _| {
            crate::turn_undo::poll(s);
            crate::changes_panel::poll(s);
        });
        rt.frame(0.05);
        std::thread::sleep(Duration::from_millis(10));
    }
    for _ in 0..10 {
        rt.frame(0.05);
    }
}

fn press(rt: &mut rune::Runtime<State>, ending: &str) {
    for _ in 0..10 {
        rt.frame(0.05);
    }
    let key = rt.elements().into_iter().filter_map(|e| e.key).find(|k| k.ends_with(ending)).unwrap_or_else(|| panic!("no {ending}"));
    let (x, y) = rt.find(&key).unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
}

fn said(rt: &rune::Runtime<State>) -> String {
    rt.elements().iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n")
}

#[test]
fn is_told_by_the_undo_below_the_card_when_files_were_put_back_not_when_nothing_was() {
    let (_t, store, work) = turn_that_wrote("mission_undo");
    let _hook = locust_core::native::hooks::install(store.clone(), None);
    let mut rt = window(&work, "mission_undo", items(&["report.md", "notes.md"]));
    pump(&mut rt, "the turn's undo state", |s| matches!(s.turn_undo.states.get("mission_undo"), Some(UndoState::Ready { .. })));
    let before = said(&rt);
    assert!(before.contains("Edited 2 files"), "{before}");
    assert!(!before.contains("undone"), "{before}");
    assert!(before.contains("Undo these changes"), "{before}");
    // Asked first, in words: how many files, and that a file changed since is left as it is.
    press(&mut rt, "/undo-ask");
    let asking = said(&rt);
    assert!(asking.contains("Put back the 2 files this turn changed, as they were before it? A file changed since is left as it is."), "{asking}");
    assert!(work.join("report.md").exists(), "nothing changes until Undo is pressed");
    press(&mut rt, "/undo-go");
    pump(&mut rt, "Undo's answer", |s| s.turn_undo.states.get("mission_undo").is_some_and(UndoState::put_back));
    let after = said(&rt);
    assert!(after.contains("Edited 2 files \u{b7} undone"), "{after}");
    assert!(after.contains("Undone: 2 files put back as they were before this turn."), "{after}");
    assert!(!after.contains("Undo these changes"), "{after}");
    assert!(!work.join("report.md").exists() && !work.join("notes.md").exists(), "the files are put back as they were: absent");

    // Undo that put nothing back says so, and the card does not say undone.
    let mut rt = window(&work, "mission_none", items(&["report.md"]));
    rt.update(|s, _| {
        s.turn_undo.states.insert("mission_none".into(), UndoState::from_receipt(&json!({"restored": [], "leftAlone": [{"path": "report.md", "why": "it changed after this turn"}]})));
    });
    rt.frame(0.05);
    let none = said(&rt);
    assert!(none.contains("Nothing was put back."), "{none}");
    assert!(none.contains("left as it is: it changed after this turn"), "{none}");
    assert!(none.contains("Edited 1 file") && !none.contains("undone"), "{none}");
}

#[test]
fn draws_no_undo_for_a_turn_that_shared_its_folder_or_has_no_record() {
    for state in [UndoState::Shared, UndoState::None] {
        let mut rt = window(Path::new("/nowhere"), "mission_x", items(&["report.md"]));
        rt.update(|s, _| {
            s.turn_undo.states.insert("mission_x".into(), state.clone());
        });
        rt.frame(0.05);
        let shown = said(&rt);
        assert!(shown.contains("Edited 1 file"), "{shown}");
        assert!(!shown.contains("Undo these changes") && !shown.contains("Undone"), "{state:?}: {shown}");
    }
}
