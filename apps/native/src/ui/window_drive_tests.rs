//! THE WINDOW, DRIVEN WITH NO SCREEN. The real view (`view`) in a Rune runtime with no window: clicks land on elements
//! found by key, text is pasted and Return pressed as a person would, and the window's own `poll` runs between frames.
//! The agents are the fakes in `scripts/fixtures`, the data folder a temporary one. Nothing here touches the person's
//! screen, Electron, or a real agent. Each test is a check that once needed a live session.
use super::*;
use crate::store::Store;
use rune::{Env, Key, Runtime};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

static TEMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Dirs(PathBuf);
impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn teammate(root: &Path) -> Teammate {
    Teammate {
        id: "tm_codec".into(),
        name: "Codec".into(),
        role: "Code & Migrations".into(),
        runtime: Some("codex".into()),
        model: Some("synthetic-model".into()),
        busy: false,
        runs_in: "ask".into(),
        folder: Some(root.join("work").to_string_lossy().into()),
    }
}

/// A window on the native engine with one teammate, over a temporary data folder.
fn window() -> (Dirs, Store, Runtime<State>) {
    let root = std::env::temp_dir().join(format!(
        "locust-drive-{}-{}",
        std::process::id(),
        TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("electron/mission-ledger")).unwrap();
    std::fs::create_dir_all(root.join("work")).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    // The team as the native file keeps it, so owners have names, as on a real computer.
    store
        .save_json("teammates.json", &json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_codec", "name": "Codec"}], "missionOwners": {}, "settings": {}}))
        .unwrap();
    let mut s = State::default();
    s.engine = Some(Kind::Native);
    s.data = Some(store.clone());
    apply_team(&mut s, vec![teammate(&root)]);
    let mut rt = Runtime::new(s, view, (1100.0, 780.0), Env::new(true, 1.0));
    rt.frame(0.0);
    (Dirs(root), store, rt)
}

fn fake(script: &str, scenario: &str) -> Option<(PathBuf, Vec<String>)> {
    Some((PathBuf::from("python3"), vec![format!("{}/scripts/fixtures/{script}", env!("CARGO_MANIFEST_DIR")), scenario.into()]))
}

/// Frames until what moved has landed: a card that slides in is clicked where it rests, not mid-flight.
fn settle(rt: &mut Runtime<State>) {
    for _ in 0..40 {
        rt.frame(0.05);
    }
}

/// A press on the element with this key, at its centre, as a pointer would.
fn click(rt: &mut Runtime<State>, key: &str) {
    settle(rt);
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

/// Text into whatever holds the keyboard, then Return.
fn type_and_send(rt: &mut Runtime<State>, text: &str) {
    click(rt, "composer");
    rt.paste(text);
    rt.frame(0.016);
    rt.key(Key::Enter.into());
    rt.frame(0.016);
}

/// The window's own poll and a frame, until `done`, or fail.
fn pump(rt: &mut Runtime<State>, what: &str, mut done: impl FnMut(&State) -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    let mut now = 0.0;
    while !done(&rt.state) {
        assert!(std::time::Instant::now() < deadline, "timed out waiting for {what}: status {:?}", rt.state.status);
        rt.update(move |s, tx| {
            poll(s, now, tx);
        });
        rt.frame(0.3);
        now += 0.3;
        std::thread::sleep(Duration::from_millis(15));
    }
}

fn answered(s: &State) -> bool {
    s.flight.is_none()
        && s.ledger.as_ref().is_some_and(|p| {
            !p.loading && !p.snapshot.running && p.items.iter().any(|i| matches!(i, ledger::Item::Assistant { text, .. } if text == "Hello world"))
        })
}

fn first_records(store: &Store) -> Vec<Value> {
    let dir = store.ledger_dir().unwrap();
    std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|t| t.lines().next().and_then(|l| serde_json::from_str(l).ok()))
        .collect()
}

#[test]
fn a_message_typed_into_the_box_is_sent_and_its_answer_shown() {
    let (_dirs, store, mut rt) = window();
    let _g = native::hooks::install(store.clone(), fake("m1-app-server.py", "success"));
    type_and_send(&mut rt, "Synthetic prompt");
    assert!(rt.state.draft.text.is_empty(), "Return sent what was typed");
    pump(&mut rt, "the answer", answered);
    let meta = &first_records(&store)[0]["metadata"];
    assert_eq!(meta["runtime"], "codex");
    assert!(meta["prompt"].as_str().unwrap().ends_with("Synthetic prompt"));
}

#[test]
fn a_model_of_another_agent_picked_in_the_picker_moves_the_reply_there() {
    let (_dirs, store, mut rt) = window();
    let _g = native::hooks::install(store.clone(), fake("m1-app-server.py", "success"));
    type_and_send(&mut rt, "Synthetic prompt");
    pump(&mut rt, "the first answer", answered);

    // The picker, opened from its chip, lists the teammate's own agent and another one.
    click(&mut rt, "model-chip");
    let model = |id: &str, name: &str| engine::Model { id: id.into(), name: name.into(), efforts: vec![] };
    let lists = vec![
            engine::Models { teammate: Some("tm_codec".into()), runtime: "codex".into(), model: Some("synthetic-model".into()), effort: None, models: vec![model("synthetic-model", "Synthetic")] },
            engine::Models { teammate: None, runtime: "claude".into(), model: None, effort: None, models: vec![model("sonnet", "Sonnet 5.5")] },
        ];
    // As the model job lands (the real one reads this computer's agents, which a test cannot rely on).
    rt.update(move |s, _| {
        s.models.insert("tm_codec".into(), lists);
    });
    rt.frame(0.016);
    // The other agent's models sit below the fold, as in a real list: search for one, as a person would.
    click(&mut rt, "model-search");
    rt.paste("sonnet");
    rt.frame(0.016);
    click(&mut rt, "model-claude-sonnet");
    assert_eq!(rt.state.picks.get("tm_codec").and_then(|p| p.runtime.as_deref()), Some("claude"), "the pick carries its agent");

    type_and_send(&mut rt, "Synthetic follow-up");
    pump(&mut rt, "the switched turn's ledger", |_| first_records(&store).iter().any(|r| r["metadata"]["runtime"] == "claude"));
    let switched = first_records(&store).into_iter().find(|r| r["metadata"]["runtime"] == "claude").unwrap();
    let meta = &switched["metadata"];
    assert_eq!(meta["model"], "sonnet");
    assert_eq!(meta["continuesFrom"]["reason"], json!("route-switch"));
    assert!(meta["prompt"].as_str().unwrap().starts_with("You are taking over this conversation from another agent (Codex"), "{}", meta["prompt"]);
}

#[test]
fn a_conversation_opened_from_the_rail_card_is_its_teammates_not_a_stand_in() {
    let (_dirs, store, mut rt) = window();
    let _g = native::hooks::install(store.clone(), fake("m1-app-server.py", "success"));
    type_and_send(&mut rt, "Synthetic prompt");
    pump(&mut rt, "the answer", answered);
    // The recent list names the teammate (it once named the agent, and the card was always empty).
    pump(&mut rt, "the recent row", |s| s.recent.iter().any(|r| r.teammate == "Codec"));
    let mission = rt.state.recent.iter().find(|r| r.teammate == "Codec").unwrap().id.clone();
    // Away from the conversation, then back to it through the face's card, as at 1100 points wide.
    click(&mut rt, "rail-tm_codec");
    click(&mut rt, &format!("rail-row-{mission}"));
    pump(&mut rt, "the conversation open", |s| s.ledger.as_ref().is_some_and(|p| !p.loading));
    let header = &rt.state.ledger.as_ref().unwrap().header;
    assert_eq!((header.id.as_str(), header.name.as_str()), ("tm_codec", "Codec"), "the owner, not \"Codex · Conversation\"");
    assert_eq!(header.runs_in, "ask", "a reply runs in the teammate's own mode");
}

#[test]
fn choosing_a_teammate_leaves_the_board_for_their_conversation() {
    let (_dirs, _store, mut rt) = window();
    rt.update(|s, _| home_panel::open_board(s));
    settle(&mut rt);
    assert!(rt.state.home.board_open);
    click(&mut rt, "rail-tm_codec");
    assert!(!rt.state.home.board_open, "the Board closed");
    assert!(rt.find("composer").is_some(), "the conversation's message box is on screen");
}
