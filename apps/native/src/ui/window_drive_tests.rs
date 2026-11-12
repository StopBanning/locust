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

fn teammate(root: &Path, mode: &str) -> Teammate {
    Teammate {
        id: "tm_codec".into(),
        name: "Codec".into(),
        role: "Code & Migrations".into(),
        runtime: Some("codex".into()),
        model: Some("synthetic-model".into()),
        busy: false,
        runs_in: mode.into(),
        folder: Some(root.join("work").to_string_lossy().into()),
    }
}

/// A window on the native engine with one teammate (Codec, in Ask), over a temporary data folder.
fn window() -> (Dirs, Store, Runtime<State>) {
    window_in("ask")
}

fn window_in(mode: &str) -> (Dirs, Store, Runtime<State>) {
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
    apply_team(&mut s, vec![teammate(&root, mode)]);
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

fn kinds(store: &Store, mission: &str) -> Vec<String> {
    let path = store.ledger_dir().unwrap().join(format!("{mission}.jsonl"));
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter_map(|r| r["event"]["type"].as_str().map(str::to_owned))
        .collect()
}

#[test]
fn a_turn_started_outside_the_window_shows_its_card_on_the_board_and_allow_once_finishes_it() {
    let (_dirs, store, mut rt) = window_in("approve-each");
    let _g = native::hooks::install(store.clone(), fake("m1-app-server.py", "approve-allow"));
    // Started as the door starts one: by the engine, not by this window's send.
    let codec = rt.state.teammates[0].clone();
    let started = native::send_from(
        native::Send { teammate: codec, conversation: None, prompt: "Synthetic prompt".into(), model: None, effort: None },
        native::Origin { started_by: Some(json!({"kind": "mcp"})), ..Default::default() },
    )
    .unwrap();
    let mission = native::real_tip(&started.conversation_id).unwrap_or(started.conversation_id.clone());
    // The window learns of it by itself: the recent list and the Board's card (once, neither did).
    pump(&mut rt, "the card", |s| !s.home.asks.is_empty() && s.recent.iter().any(|r| r.id == mission));
    rt.update(|s, _| home_panel::open_board(s));
    // A card shown for under half a second cannot be approved (a stray click must not allow anything).
    std::thread::sleep(Duration::from_millis(600));
    click(&mut rt, &format!("board-allow-{mission}"));
    pump(&mut rt, "the turn's end", |_| kinds(&store, &mission).iter().any(|k| k == "run.completed"));
}

#[test]
fn stop_in_the_window_ends_a_long_turn_with_stopped_and_no_error() {
    let (_dirs, store, mut rt) = window();
    let _g = native::hooks::install(store.clone(), fake("m1-app-server.py", "interrupt"));
    type_and_send(&mut rt, "Synthetic prompt");
    pump(&mut rt, "the turn running", |s| can_stop(s));
    click(&mut rt, "send");
    pump(&mut rt, "the stopped turn", |s| s.ledger.as_ref().is_some_and(|p| !p.loading && !p.snapshot.running) && s.flight.is_none());
    let items = &rt.state.ledger.as_ref().unwrap().items;
    assert!(items.iter().any(|i| matches!(i, ledger::Item::Note { text, .. } if text == "Stopped")), "{items:?}");
    assert!(!items.iter().any(|i| matches!(i, ledger::Item::Note { level, .. } if level == "error")), "{items:?}");
}

#[test]
fn quitting_writes_each_running_turns_ending() {
    let (_dirs, store, mut rt) = window();
    let _g = native::hooks::install(store.clone(), fake("m1-app-server.py", "interrupt"));
    type_and_send(&mut rt, "Synthetic prompt");
    pump(&mut rt, "the turn running", |s| can_stop(s));
    let running = native::running_ids();
    assert!(!running.is_empty());
    on_quit(&mut rt.state, &mut Tx::default());
    let mission = running.iter().find(|id| store.ledger_dir().unwrap().join(format!("{id}.jsonl")).exists()).unwrap().clone();
    assert!(kinds(&store, &mission).iter().any(|k| k == "run.cancelled"), "{:?}", kinds(&store, &mission));
    assert!(native::running_ids().is_empty());
}

#[test]
fn a_mode_chosen_on_the_message_box_is_the_mode_the_turn_runs_in() {
    let (_dirs, store, mut rt) = window();
    let _g = native::hooks::install(store.clone(), fake("m1-app-server.py", "success"));
    click(&mut rt, "mode-chip");
    assert!(rt.find("mode-menu").is_some(), "the chip opens the menu");
    // Codex runs every mode; an agent that cannot stop and ask has Approve each greyed out (mode_facts tests).
    click(&mut rt, "mode-plan");
    assert_eq!(rt.state.composer_mode.as_deref(), Some("plan"));
    assert!(rt.find("mode-menu").is_none(), "choosing closes it");
    type_and_send(&mut rt, "Synthetic prompt");
    pump(&mut rt, "the answer", answered);
    assert_eq!(first_records(&store)[0]["metadata"]["mode"], "plan", "the ledger records the chosen mode, not the teammate's Ask");
    // Picking a teammate goes back to theirs.
    click(&mut rt, "rail-tm_codec");
    assert_eq!(rt.state.composer_mode, None);
}

#[test]
fn a_pressed_notification_opens_its_conversation() {
    let (_dirs, store, mut rt) = window();
    let _g = native::hooks::install(store.clone(), fake("m1-app-server.py", "success"));
    type_and_send(&mut rt, "Synthetic prompt");
    pump(&mut rt, "the answer", answered);
    let mission = rt.state.ledger.as_ref().unwrap().conversation.conversation_id.clone().unwrap();
    // Somewhere else in the window when the banner is pressed.
    rt.update(|s, _| home_panel::open_board(s));
    settle(&mut rt);
    crate::notify::clicked(&mission);
    pump(&mut rt, "the conversation open", |s| {
        !s.home.board_open && s.ledger.as_ref().is_some_and(|p| !p.loading && p.conversation.conversation_id.as_deref() == Some(mission.as_str()))
    });
    assert_eq!(rt.state.ledger.as_ref().unwrap().header.name, "Codec");
}

#[test]
fn all_of_a_teammates_conversations_opens_the_board_narrowed_to_them() {
    let (_dirs, store, mut rt) = window();
    let _g = native::hooks::install(store.clone(), fake("m1-app-server.py", "success"));
    type_and_send(&mut rt, "Synthetic prompt");
    pump(&mut rt, "the answer", answered);
    pump(&mut rt, "the recent row", |s| s.recent.iter().any(|r| r.teammate == "Codec"));
    click(&mut rt, "rail-tm_codec");
    click(&mut rt, "rail-all-theirs");
    assert!(rt.state.home.board_open);
    assert_eq!(rt.state.home.board_teammate.as_deref(), Some("Codec"));
    assert!(rt.find("board-narrowed").is_some(), "the Board says it shows only Codec's");
    click(&mut rt, "board-everyone");
    assert_eq!(rt.state.home.board_teammate, None);
    assert!(rt.find("board-narrowed").is_none());
}

#[test]
fn the_team_screen_edits_in_a_form_and_asks_before_removing() {
    let (_dirs, _store, mut rt) = window();
    rt.update(|s, _| crate::people_panel::open(s));
    settle(&mut rt);
    assert!(rt.find("team-card-tm_codec").is_some(), "a card per teammate");
    assert!(rt.find("team-form").is_none(), "no form until asked for");
    // Remove asks first; nothing is removed on the first press.
    click(&mut rt, "team-remove-tm_codec");
    assert_eq!(rt.state.people_panel.removing.as_deref(), Some("tm_codec"));
    assert!(rt.find("team-remove-yes-tm_codec").is_some());
    assert_eq!(rt.state.people_panel.team["teammates"].as_array().map(Vec::len), Some(1));
    rt.update(|s, _| s.people_panel.removing = None);
    // The pencil opens the form.
    click(&mut rt, "team-edit-tm_codec");
    assert!(rt.find("team-form").is_some());
}
