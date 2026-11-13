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

/// The texts drawn inside the element with this key, as a person reads it.
fn text_in(rt: &Runtime<State>, key: &str) -> Vec<String> {
    let ([x, y, w, h], _) = rt.presented(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.elements()
        .into_iter()
        .filter_map(|e| {
            let (cx, cy) = (e.rect[0] + e.rect[2] / 2.0, e.rect[1] + e.rect[3] / 2.0);
            (cx >= x && cx <= x + w && cy >= y && cy <= y + h).then_some(e.text?)
        })
        .collect()
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
    // The pick reaches the agent's own command line, as launched (the ledger keeps the argv).
    let args: Vec<&str> = meta["command"]["args"].as_array().unwrap().iter().filter_map(|a| a.as_str()).collect();
    assert!(args.windows(2).any(|w| w == ["--model", "sonnet"]), "{args:?}");
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
    let started = locust_core::engine_api::api().send(
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
    let running = locust_core::engine_api::api().running();
    assert!(!running.is_empty());
    on_quit(&mut rt.state, &mut Tx::default());
    let mission = running.iter().find(|id| store.ledger_dir().unwrap().join(format!("{id}.jsonl")).exists()).unwrap().clone();
    assert!(kinds(&store, &mission).iter().any(|k| k == "run.cancelled"), "{:?}", kinds(&store, &mission));
    assert!(locust_core::engine_api::api().running().is_empty());
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

#[test]
fn the_plus_menu_attaches_a_file_picked_in_the_systems_dialog() {
    let (dirs, _store, mut rt) = window();
    let file = dirs.0.join("work").join("notes.md");
    std::fs::write(&file, "notes").unwrap();
    click(&mut rt, "plus");
    assert!(rt.find("plus-menu").is_some(), "the + opens its menu");
    click(&mut rt, "plus-attach");
    let (kind, options) = rt.file_dialog().expect("the picker is asked for");
    assert_eq!(kind, rune::FileDialogKind::Open);
    assert_eq!(options.directory.as_deref(), Some(dirs.0.join("work").as_path()), "it opens in the conversation's folder");
    // Answered as the system's dialog would answer it (no window: nothing is shown on a screen).
    rt.resolve_file_dialog(Some(file.clone())).unwrap();
    settle(&mut rt);
    assert!(rt.state.attached.iter().any(|p| p.ends_with("notes.md")), "{:?}", rt.state.attached);
}

#[test]
fn how_memory_is_kept_is_chosen_on_the_memory_screen() {
    let (_dirs, store, mut rt) = window();
    rt.update(|s, _| {
        crate::people_panel::open(s);
        s.people_panel.tab = 1;
        s.people_panel.memory_tab = true;
    });
    settle(&mut rt);
    assert!(rt.find("memory-about").is_some() && rt.find("memory-mode").is_some());
    click(&mut rt, "memory-mode-ask");
    assert_eq!(rt.state.people_panel.memory_mode, "ask");
    assert_eq!(crate::workspace_settings::load(&store).unwrap().memory_mode, "ask", "saved in the settings");
}

#[test]
fn the_folder_chip_names_the_folder_the_turn_runs_in() {
    let (dirs, store, mut rt) = window();
    let _g = native::hooks::install(store.clone(), fake("m1-app-server.py", "success"));
    // A teammate with a folder of its own: the chip names it.
    settle(&mut rt);
    assert_eq!(text_in(&rt, "folder-chip"), ["work"]);

    // One without: the turn runs where Locust has a folder open, and the chip names that one, not "No folder".
    let project = dirs.0.join("now-coding");
    std::fs::create_dir_all(&project).unwrap();
    native::set_open_folder(&store, &project).unwrap();
    let root = dirs.0.clone();
    rt.update(move |s, _| {
        apply_team(s, vec![Teammate { folder: None, ..teammate(&root, "ask") }]);
        // As the window's setup reads it.
        s.home.folder_path = native::open_folder().map(|p| p.to_string_lossy().into_owned());
    });
    settle(&mut rt);
    assert_eq!(text_in(&rt, "folder-chip"), ["now-coding"]);
    type_and_send(&mut rt, "Synthetic prompt");
    pump(&mut rt, "the answer", answered);
    let cwd = first_records(&store)[0]["metadata"]["command"]["cwd"].as_str().unwrap().to_owned();
    assert_eq!(
        std::fs::canonicalize(&cwd).unwrap(),
        std::fs::canonicalize(&project).unwrap(),
        "the turn ran in the folder the chip names"
    );
}

fn write_synthetic_turn(
    ledger_dir: &Path,
    mission_id: &str,
    continues_from: Option<&str>,
    prompt: &str,
    reply: &str,
    tool_output: Option<&str>,
    created_at: &str,
) {
    let mut meta = json!({
        "missionId": mission_id,
        "runId": format!("run_{mission_id}"),
        "prompt": prompt,
        "runtime": "codex",
        "model": "gpt-test",
        "requestedRouteId": "codex",
        "resolvedRouteId": "codex:default",
        "cliVersion": "0.160.0",
        "workspaceId": "ws_test",
        "sandbox": "workspace-write",
        "mode": "approve-each",
        "executionPolicyVersion": 1,
        "createdAt": created_at,
    });
    if let Some(parent) = continues_from {
        meta["continuesFrom"] = json!({
            "missionId": parent,
            "reason": "continued",
        });
    }
    let mut seq = 1u64;
    let mut lines = vec![
        json!({
            "schemaVersion": 27,
            "recordType": "mission.created",
            "ledgerSequence": seq,
            "occurredAt": created_at,
            "metadata": meta,
        }),
    ];
    seq += 1;
    lines.push(json!({
        "schemaVersion": 27,
        "recordType": "mission.event",
        "ledgerSequence": seq,
        "occurredAt": created_at,
        "event": {
            "id": format!("run_{mission_id}:1"),
            "runId": format!("run_{mission_id}"),
            "missionId": mission_id,
            "sequence": 1,
            "occurredAt": created_at,
            "sourceAdapter": "codex",
            "type": "run.started",
            "payload": {"runtimeThreadId": "thread_1"},
        }
    }));
    let mut ev_seq = 1u64;
    if let Some(output) = tool_output {
        seq += 1;
        ev_seq += 1;
        lines.push(json!({
            "schemaVersion": 27,
            "recordType": "mission.event",
            "ledgerSequence": seq,
            "occurredAt": created_at,
            "event": {
                "id": format!("run_{mission_id}:{ev_seq}"),
                "runId": format!("run_{mission_id}"),
                "missionId": mission_id,
                "sequence": ev_seq,
                "occurredAt": created_at,
                "sourceAdapter": "codex",
                "type": "tool.completed",
                "payload": {
                    "itemId": "tool_1",
                    "toolKind": "command_execution",
                    "name": "shell",
                    "command": "env",
                    "status": "completed",
                    "exitCode": 0,
                    "output": output,
                }
            }
        }));
    }
    seq += 1;
    ev_seq += 1;
    lines.push(json!({
        "schemaVersion": 27,
        "recordType": "mission.event",
        "ledgerSequence": seq,
        "occurredAt": created_at,
        "event": {
            "id": format!("run_{mission_id}:{ev_seq}"),
            "runId": format!("run_{mission_id}"),
            "missionId": mission_id,
            "sequence": ev_seq,
            "occurredAt": created_at,
            "sourceAdapter": "codex",
            "type": "message.delta",
            "payload": {
                "itemId": "msg_1",
                "operation": "append",
                "text": reply,
                "final": true,
            }
        }
    }));
    seq += 1;
    ev_seq += 1;
    lines.push(json!({
        "schemaVersion": 27,
        "recordType": "mission.event",
        "ledgerSequence": seq,
        "occurredAt": created_at,
        "event": {
            "id": format!("run_{mission_id}:{ev_seq}"),
            "runId": format!("run_{mission_id}"),
            "missionId": mission_id,
            "sequence": ev_seq,
            "occurredAt": created_at,
            "sourceAdapter": "codex",
            "type": "run.completed",
            "payload": {"runtimeThreadId": "thread_1"},
        }
    }));
    let body = lines
        .iter()
        .map(|l| serde_json::to_string(l).unwrap())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    std::fs::write(ledger_dir.join(format!("{mission_id}.jsonl")), body).unwrap();
}

fn seed_two_turn_ledger_conversation(dirs: &Dirs, store: &Store, rt: &mut Runtime<State>) -> String {
    let work = dirs.0.join("work");
    let ledger_dir = store.ledger_dir().unwrap();
    std::fs::create_dir_all(&ledger_dir).unwrap();
    store
        .save_json(
            "teammates.json",
            &json!({
                "schemaVersion": 1,
                "teammates": [{
                    "teammateId": "tm_codec",
                    "name": "Codec",
                    "role": "Code & Migrations",
                    "hue": "teal",
                    "createdAt": "2026-10-02T09:00:00.000Z",
                    "folder": work.to_string_lossy(),
                }],
                "missionOwners": {
                    "m_save_1": "tm_codec",
                    "m_save_2": "tm_codec",
                },
                "settings": {}
            }),
        )
        .unwrap();

    write_synthetic_turn(
        &ledger_dir,
        "m_save_1",
        None,
        "First prompt from disk",
        "First reply from disk.",
        None,
        "2026-10-02T09:00:00.000Z",
    );
    let secret = format!("{}-ant-abcdefghijklmnopqrst", "sk");
    write_synthetic_turn(
        &ledger_dir,
        "m_save_2",
        Some("m_save_1"),
        "Second prompt from disk",
        "Second reply from disk.",
        Some(&format!("ANTHROPIC_API_KEY={secret}")),
        "2026-10-02T09:01:00.000Z",
    );

    rt.update(|s, _| {
        s.layout_pref = "wide".into();
        refresh_recent(s);
    });
    pump(rt, "synthetic turns listed in sidebar", |s| {
        s.recent.iter().any(|r| r.id == "m_save_2")
    });
    click(rt, "recent-m_save_2");
    pump(rt, "synthetic conversation loaded from disk", |s| {
        s.ledger
            .as_ref()
            .is_some_and(|p| p.snapshot.tip == "m_save_2" && !p.loading)
    });
    settle(rt);
    secret
}

#[test]
fn save_the_record_writes_markdown_matching_ledger_and_optional_raw_json() {
    let (dirs, store, mut rt) = window();
    let _g = native::hooks::install(store.clone(), None);
    let secret = seed_two_turn_ledger_conversation(&dirs, &store, &mut rt);

    // Open from the conversation's sidebar actions (`...`) menu.
    click(&mut rt, "org-menu-open-m_save_2");
    click(&mut rt, "org-save-record-m_save_2");
    assert!(rt.state.save_record.open);
    assert!(!text_in(&rt, "save-record-claim").is_empty());

    // Save Markdown only.
    click(&mut rt, "save-record-submit");
    let (kind, options) = rt.file_dialog().expect("system save dialog opens");
    assert_eq!(kind, rune::FileDialogKind::Save);
    assert_eq!(options.title, "Save the record");
    assert!(
        options
            .file_name
            .as_deref()
            .is_some_and(|n| n.starts_with("Locust record - Codec - ") && n.ends_with(".md"))
    );

    let out_dir = dirs.0.join("downloads");
    std::fs::create_dir_all(&out_dir).unwrap();
    let target = out_dir.join("saved-record.md");
    rt.resolve_file_dialog(Some(target.clone())).unwrap();
    settle(&mut rt);

    let written = std::fs::read_to_string(&target).expect("markdown saved on disk");
    let saved_at = written
        .lines()
        .find_map(|line| line.strip_prefix("- **Saved:** "))
        .expect("Saved timestamp in header")
        .to_owned();
    let chain = locust_core::mission_export::record_turns(
        locust_core::mission_export::recover_mission,
        "m_save_2",
    );
    let expected = locust_core::mission_export::mission_record_markdown(
        &locust_core::mission_export::RecordSource {
            missions: &chain.missions,
            teammate: Some("Codec"),
            folder: Some(&dirs.0.join("work").to_string_lossy()),
            locust_version: env!("CARGO_PKG_VERSION"),
            saved_at: &saved_at,
            missing_parent: chain.missing_parent.as_deref(),
        },
    );
    assert_eq!(written, expected);
    assert!(!written.contains(&secret));
    assert!(
        text_in(&rt, "save-record-status")
            .iter()
            .any(|s| s.starts_with("Saved to ") && s.ends_with('.'))
    );

    // Toggle raw JSON on and save beside a second path without overwriting Markdown.
    click(&mut rt, "save-record-include-raw");
    click(&mut rt, "save-record-submit");
    let _ = rt.file_dialog().expect("second save dialog opens");
    let target2 = out_dir.join("with-raw.md");
    rt.resolve_file_dialog(Some(target2.clone())).unwrap();
    settle(&mut rt);

    let raw_path = out_dir.join("with-raw.json");
    assert!(target2.exists());
    let raw = std::fs::read_to_string(&raw_path).expect("raw json saved beside markdown");
    assert!(raw.contains(&secret));
    assert!(
        text_in(&rt, "save-record-status")
            .iter()
            .any(|s| s.contains("raw record beside it"))
    );
}

#[test]
fn save_the_record_cancel_writes_nothing_and_command_palette_opens_it() {
    let (dirs, store, mut rt) = window();
    let _g = native::hooks::install(store.clone(), None);
    let _ = seed_two_turn_ledger_conversation(&dirs, &store, &mut rt);

    // Open via Command Palette (Cmd+K).
    rt.update(|s, tx| {
        assert!(on_key(s, &KeyEvent::from(Key::Command('k')), tx));
    });
    settle(&mut rt);
    click(&mut rt, "palette-save-record");
    assert!(rt.state.save_record.open);

    click(&mut rt, "save-record-submit");
    let (kind, _) = rt.file_dialog().expect("system save dialog opens");
    assert_eq!(kind, rune::FileDialogKind::Save);
    rt.resolve_file_dialog(None).unwrap();
    settle(&mut rt);

    // Cancelling leaves the dialog open, clears status (like Electron's setSaid(undefined)), and writes nothing.
    assert!(rt.state.save_record.open);
    assert!(!rt.state.save_record.busy);
    assert_eq!(rt.state.save_record.said, None);
    assert!(
        !text_in(&rt, "save-record-dialog")
            .iter()
            .any(|s| s.contains("Saved") || s.contains("could not be saved"))
    );
}

#[test]
fn long_conversation_shows_prompt_rail_time_markers_and_pressing_mark_jumps() {
    let (dirs, store, mut rt) = window();
    let _g = native::hooks::install(store.clone(), None);
    let ledger_dir = dirs.0.join("electron/mission-ledger");
    let work = dirs.0.join("work");
    let mut owners = serde_json::Map::new();
    for i in 1..=5 {
        owners.insert(format!("m_rail_{i}"), json!("tm_codec"));
    }
    store
        .save_json(
            "teammates.json",
            &json!({
                "schemaVersion": 1,
                "teammates": [{
                    "teammateId": "tm_codec",
                    "name": "Codec",
                    "role": "Code & Migrations",
                    "hue": "teal",
                    "createdAt": "2026-10-02T09:00:00.000Z",
                    "folder": work.to_string_lossy(),
                }],
                "missionOwners": Value::Object(owners),
                "settings": {}
            }),
        )
        .unwrap();

    let long_reply = (1..=16)
        .map(|n| format!("Line {n} of the synthetic reply so the thread overflows the viewport."))
        .collect::<Vec<_>>()
        .join("\n");
    let times = [
        "2026-10-02T09:00:00.000Z",
        "2026-10-02T09:05:00.000Z",
        "2026-10-03T09:30:00.000Z",
        "2026-10-03T09:31:00.000Z",
        "2026-10-03T09:32:00.000Z",
    ];
    for i in 1..=5 {
        let id = format!("m_rail_{i}");
        let parent = (i > 1).then(|| format!("m_rail_{}", i - 1));
        write_synthetic_turn(
            &ledger_dir,
            &id,
            parent.as_deref(),
            &format!("Prompt number {i} from the person"),
            &format!("Reply {i}:\n{long_reply}"),
            None,
            times[i - 1],
        );
    }

    rt.update(|s, _| {
        s.layout_pref = "wide".into();
        refresh_recent(s);
    });
    pump(&mut rt, "long synthetic turns listed", |s| {
        s.recent.iter().any(|r| r.id == "m_rail_5")
    });
    click(&mut rt, "recent-m_rail_5");
    pump(&mut rt, "long synthetic conversation loaded", |s| {
        s.ledger
            .as_ref()
            .is_some_and(|p| p.snapshot.tip == "m_rail_5" && !p.loading)
    });
    settle(&mut rt);

    // Narrow thread with sidebar open (1100px window, 888px column): no room for the rail.
    assert!(rt.find("prompt-rail").is_none(), "prompt rail should withhold when room < 18");

    // Collapsing the sidebar gives the thread room, so the prompt rail appears with 5 marks.
    rt.update(|s, _| s.layout_pref = "compact".into());
    settle(&mut rt);
    assert!(rt.find("prompt-rail").is_some(), "prompt rail should appear");
    for i in 0..5 {
        assert!(
            rt.find(&format!("prompt-rail-mark-{i}")).is_some(),
            "mark {i} should appear"
        );
    }

    // Time markers appear for the 5-minute gap and the next-day turn.
    let all_text: Vec<String> = rt
        .elements()
        .into_iter()
        .filter_map(|e| e.text)
        .collect();
    assert!(
        all_text.iter().any(|t| t.contains("waited 5 min")),
        "expected quiet gap time marker in {all_text:?}"
    );
    assert!(
        all_text.iter().any(|t| t.contains("Oct 3") && t.contains("1d")),
        "expected next-day time marker in {all_text:?}"
    );

    // Pressing the first mark brings the first user message into view.
    click(&mut rt, "prompt-rail-mark-0");
    let conv_id = rt
        .state
        .ledger
        .as_ref()
        .and_then(|p| p.conversation.conversation_id.as_deref())
        .unwrap()
        .to_owned();
    let ([_, vy, _, vh], _) = rt.presented("messages").expect("messages viewport");
    let ([_, by, _, _], _) = rt
        .presented(&format!("{conv_id}/ledger-item-0"))
        .expect("first user bubble");
    assert!(
        by >= vy - 1.0 && by < vy + vh,
        "first user bubble (y={by}) should be inside viewport ({vy}..{})",
        vy + vh
    );
}

#[test]
fn scrolling_up_shows_way_down_working_says_so_and_pressing_returns_to_bottom() {
    let (dirs, store, mut rt) = window();
    let _g = native::hooks::install(store.clone(), None);
    let ledger_dir = dirs.0.join("electron/mission-ledger");
    let work = dirs.0.join("work");
    let mut owners = serde_json::Map::new();
    for i in 1..=4 {
        owners.insert(format!("m_way_{i}"), json!("tm_codec"));
    }
    store
        .save_json(
            "teammates.json",
            &json!({
                "schemaVersion": 1,
                "teammates": [{
                    "teammateId": "tm_codec",
                    "name": "Codec",
                    "role": "Code & Migrations",
                    "hue": "teal",
                    "createdAt": "2026-10-02T09:00:00.000Z",
                    "folder": work.to_string_lossy(),
                }],
                "missionOwners": Value::Object(owners),
                "settings": {}
            }),
        )
        .unwrap();

    let long_reply = (1..=20)
        .map(|n| format!("Line {n} of the synthetic reply so the thread overflows the viewport."))
        .collect::<Vec<_>>()
        .join("\n");
    for i in 1..=4 {
        let id = format!("m_way_{i}");
        let parent = (i > 1).then(|| format!("m_way_{}", i - 1));
        write_synthetic_turn(
            &ledger_dir,
            &id,
            parent.as_deref(),
            &format!("Way down prompt {i}"),
            &format!("Way down reply {i}:\n{long_reply}"),
            None,
            &format!("2026-10-02T09:0{i}:00.000Z"),
        );
    }

    rt.update(|s, _| {
        s.layout_pref = "wide".into();
        refresh_recent(s);
    });
    pump(&mut rt, "way down turns listed", |s| {
        s.recent.iter().any(|r| r.id == "m_way_4")
    });
    click(&mut rt, "recent-m_way_4");
    pump(&mut rt, "way down conversation loaded", |s| {
        s.ledger
            .as_ref()
            .is_some_and(|p| p.snapshot.tip == "m_way_4" && !p.loading)
    });
    settle(&mut rt);

    // At the bottom, the way down is absent.
    assert!(rt.find("jump-to-bottom").is_none());

    // Scroll to top: the way down appears with the idle chevron.
    rt.update(|_, tx| tx.scroll_to_instant("messages", 0.0, 0.0));
    settle(&mut rt);
    assert!(rt.find("jump-to-bottom").is_some(), "way down should appear when scrolled up");
    assert!(rt.find("jump-to-bottom-chevron").is_some());
    assert!(rt.find("jump-to-bottom-dots").is_none());

    // While a run is working, the way down wears the thinking dots and says so.
    rt.update(|s, _| {
        if let Some(pane) = &mut s.ledger {
            pane.snapshot.running = true;
        }
    });
    settle(&mut rt);
    assert!(rt.find("jump-to-bottom-dots").is_some());
    assert!(rt.find("jump-to-bottom-chevron").is_none());
    let has_working_label = rt.elements().into_iter().any(|e| {
        e.key.as_deref() == Some("jump-to-bottom")
            && e.label.as_deref() == Some("Still working. Go to the newest message")
    });
    assert!(has_working_label, "jump-to-bottom should carry working label");

    // Pressing the way down returns to the bottom and hides the pill.
    click(&mut rt, "jump-to-bottom");
    assert!(rt.find("jump-to-bottom").is_none(), "way down should hide at bottom");
}

#[test]
fn a_decision_is_answered_by_number_and_without_a_teammate_names_nobody() {
    let (_dirs, store, mut rt) = window();
    let _g = native::hooks::install(store, fake("m1-app-server.py", "cards-decision"));
    type_and_send(&mut rt, "Which file?");
    pump(&mut rt, "decision card", |s| {
        s.flight.is_none()
            && s.ledger.as_ref().is_some_and(|p| !p.loading && !p.snapshot.running)
    });
    settle(&mut rt);

    assert!(rt.find("decision-card").is_some(), "decision card should render");
    let card_texts = text_in(&rt, "decision-card").join("\n");
    assert!(card_texts.contains("Codec needs a decision to continue"), "{card_texts}");
    assert!(card_texts.contains("stopped here · nothing was changed"), "{card_texts}");
    assert!(card_texts.contains("Which file is right?"), "{card_texts}");
    assert!(card_texts.contains("Keep notes.md"), "{card_texts}");
    assert!(card_texts.contains("the newer one"), "{card_texts}");
    assert!(card_texts.contains("Keep NOTES.txt"), "{card_texts}");
    assert!(card_texts.contains("Or reply below — you are not limited to these"), "{card_texts}");

    let msg_texts = text_in(&rt, "messages").join("\n");
    assert!(!msg_texts.contains("locust-ask"), "{msg_texts}");

    // When a text field holds focus, number keys must not answer the decision.
    rt.update(|s, tx| {
        s.draft.focused = true;
        assert!(!on_key(s, &KeyEvent::from(Key::Text("1".into())), tx));
        s.draft.focused = false;
        assert!(on_key(s, &KeyEvent::from(Key::Text("1".into())), tx));
    });

    pump(&mut rt, "decision follow-up reply", |s| {
        s.flight.is_none()
            && s.ledger.as_ref().is_some_and(|p| {
                !p.loading
                    && !p.snapshot.running
                    && p.items.iter().any(|i| {
                        matches!(i, ledger::Item::Assistant { text, .. } if text == "Got it, keeping notes.md.")
                    })
            })
    });
    settle(&mut rt);
    assert!(rt.find("decision-card").is_none(), "decision card should disappear after answering");

    // Without a teammate (e.g. a @ledger conversation), the card names nobody and never says "runtime".
    rt.update(|s, _| {
        if let Some(pane) = &mut s.ledger {
            pane.header.id = LEDGER_TARGET.into();
            pane.header.name = "Codex CLI".into();
            pane.items.push(ledger::Item::Assistant {
                item_id: "ask-anon".into(),
                text: "<locust-ask>\nWhat next?\n- Option A\n- Option B\n</locust-ask>".into(),
                r#final: true,
            });
        }
    });
    settle(&mut rt);
    let anon_texts = text_in(&rt, "decision-card").join("\n");
    assert!(anon_texts.contains("A decision is needed to continue"), "{anon_texts}");
    assert!(!anon_texts.contains("runtime"), "{anon_texts}");
}

#[test]
fn a_reply_suggests_tasks_as_cards_and_acts_on_do_it_here() {
    let (_dirs, store, mut rt) = window();
    let _g = native::hooks::install(store, fake("m1-app-server.py", "cards-next"));
    type_and_send(&mut rt, "Clean up");
    pump(&mut rt, "suggested tasks", |s| {
        s.flight.is_none()
            && s.ledger.as_ref().is_some_and(|p| !p.loading && !p.snapshot.running)
    });
    settle(&mut rt);

    let msg_texts = text_in(&rt, "messages").join("\n");
    assert!(!msg_texts.contains("locust-next"), "{msg_texts}");
    assert!(msg_texts.contains("2 suggested tasks"), "{msg_texts}");
    // No Start all: it would put tasks away that this window cannot start together (next_steps.rs).
    assert!(!msg_texts.contains("Start all"), "{msg_texts}");
    assert!(msg_texts.contains("Clean the Linux cache too"), "{msg_texts}");
    assert!(msg_texts.contains("Tell people in What's new"), "{msg_texts}");
    assert!(msg_texts.contains("Do it here"), "{msg_texts}");

    // Put away the second task: the count drops to 1.
    let away_key = rt
        .elements()
        .into_iter()
        .find_map(|e| e.key.filter(|k| k.starts_with("next-step-away-") && k.ends_with("-1")))
        .expect("put away button for second task");
    click(&mut rt, &away_key);
    let after_away = text_in(&rt, "messages").join("\n");
    assert!(after_away.contains("1 suggested task"), "{after_away}");

    // Press "Do it here" on the remaining task and verify the next turn runs with its prompt.
    let here_key = rt
        .elements()
        .into_iter()
        .find_map(|e| e.key.filter(|k| k.starts_with("next-step-here-") && k.ends_with("-0")))
        .expect("do it here button");
    click(&mut rt, &here_key);
    pump(&mut rt, "next step follow-up reply", |s| {
        s.flight.is_none()
            && s.ledger.as_ref().is_some_and(|p| {
                !p.loading
                    && !p.snapshot.running
                    && p.items.iter().any(|i| {
                        matches!(i, ledger::Item::Assistant { text, .. } if text == "Cleaned the Linux cache.")
                    })
            })
    });
}

#[test]
fn a_reply_hands_over_files_and_half_written_tags_never_show_while_streaming() {
    let (_dirs, store, mut rt) = window();
    let _g = native::hooks::install(store.clone(), fake("m1-app-server.py", "cards-file"));
    type_and_send(&mut rt, "Send the report");
    pump(&mut rt, "handed files", |s| {
        s.flight.is_none()
            && s.ledger.as_ref().is_some_and(|p| !p.loading && !p.snapshot.running)
    });
    settle(&mut rt);

    let msg_texts = text_in(&rt, "messages").join("\n");
    assert!(!msg_texts.contains("locust-file"), "{msg_texts}");
    assert!(msg_texts.contains("docs/report.md"), "{msg_texts}");
    assert!(msg_texts.contains("the rollup you asked for"), "{msg_texts}");
    assert!(
        msg_texts.contains("Not shown as a file: //server/share/bad.md -- a file is handed over by its path on this computer, and this is not one."),
        "{msg_texts}"
    );
    drop(_g);

    // Streaming: a half-written tag never flashes on screen and the decision card waits until finished.
    let (_dirs2, store2, mut rt2) = window();
    let _g2 = native::hooks::install(store2, fake("m1-app-server.py", "cards-streaming"));
    type_and_send(&mut rt2, "Stream ask");
    pump(&mut rt2, "streaming delta", |s| {
        s.ledger.as_ref().is_some_and(|p| {
            p.snapshot.running
                && p.items.iter().any(|i| matches!(i, ledger::Item::Assistant { text, .. } if text.contains("Thinking")))
        })
    });
    rt2.frame(0.016);
    let mid_texts = text_in(&rt2, "messages").join("\n");
    assert!(!mid_texts.contains("<locust"), "half-written tag flashed: {mid_texts}");
    assert!(!mid_texts.contains("<locu"), "torn tag flashed: {mid_texts}");
    assert!(rt2.find("decision-card").is_none(), "decision card must wait until run finishes");

    pump(&mut rt2, "streaming finished", |s| {
        s.flight.is_none()
            && s.ledger.as_ref().is_some_and(|p| !p.loading && !p.snapshot.running)
    });
    settle(&mut rt2);
    assert!(rt2.find("decision-card").is_some(), "decision card should appear once finished");
}

#[test]
fn import_dialog_lists_synthetic_session_imports_it_and_opens_the_conversation() {
    let (dirs, store, mut rt) = window();
    let _g = native::hooks::install(store.clone(), None);

    let shop = dirs.0.join("shop");
    std::fs::create_dir_all(&shop).unwrap();
    let shop_s = shop.to_string_lossy().into_owned();

    let claude_home = dirs.0.join("synthetic-claude");
    let codex_home = dirs.0.join("synthetic-codex");
    let session_id = "aaaaaaaa-0000-4000-8000-000000000001";
    let session_file = claude_home
        .join("projects")
        .join("C--work-shop")
        .join(format!("{session_id}.jsonl"));
    std::fs::create_dir_all(session_file.parent().unwrap()).unwrap();
    let now = chrono::Utc::now();
    let t1 = (now - chrono::Duration::minutes(10))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let t2 = (now - chrono::Duration::minutes(9))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let t3 = (now - chrono::Duration::minutes(8))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let t4 = (now - chrono::Duration::minutes(7))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let lines = [
        json!({ "type": "user", "entrypoint": "cli", "sessionId": session_id, "cwd": shop_s, "timestamp": t1, "message": { "role": "user", "content": "Plan the sales dashboard" } }),
        json!({ "type": "assistant", "sessionId": session_id, "cwd": shop_s, "timestamp": t2, "message": { "role": "assistant", "model": "claude-opus-5-5", "stop_reason": "end_turn", "content": [{ "type": "text", "text": "Here is the plan." }] } }),
        json!({ "type": "user", "entrypoint": "cli", "sessionId": session_id, "cwd": shop_s, "timestamp": t3, "message": { "role": "user", "content": "Now add a chart" } }),
        json!({ "type": "assistant", "sessionId": session_id, "cwd": shop_s, "timestamp": t4, "message": { "role": "assistant", "stop_reason": "end_turn", "content": [{ "type": "text", "text": "Chart added." }] } }),
        json!({ "type": "ai-title", "sessionId": session_id, "aiTitle": "Sales dashboard plan" }),
    ];
    let body = lines
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    std::fs::write(&session_file, body).unwrap();

    rt.update(|s, _| {
        s.layout_pref = "wide".into();
        s.session_places = Some(locust_core::session_import::SessionPlaces {
            claude_home,
            codex_home,
            cursor_home: None,
        });
    });
    settle(&mut rt);

    click(&mut rt, "sidebar-add-button");
    click(&mut rt, "add-menu-import-conversation");
    assert!(rt.state.import_dialog.open);

    pump(&mut rt, "import list loaded", |s| {
        s.import_dialog.sessions.is_some()
    });
    settle(&mut rt);

    let dialog_text = text_in(&rt, "import-dialog").join("\n");
    assert!(dialog_text.contains("Sales dashboard plan"), "{dialog_text}");
    assert!(dialog_text.contains("Claude Code · shop"), "{dialog_text}");

    click(&mut rt, &format!("import-session-{session_id}"));
    pump(&mut rt, "imported conversation opens", |s| {
        !s.import_dialog.open
            && s.ledger
                .as_ref()
                .is_some_and(|p| !p.loading && !p.items.is_empty())
    });
    settle(&mut rt);

    let pane = rt.state.ledger.as_ref().expect("imported conversation open");
    assert_eq!(pane.header.runtime.as_deref(), Some("claude"));
    let msg_text = text_in(&rt, "messages").join("\n");
    assert!(msg_text.contains("Plan the sales dashboard"), "{msg_text}");
    assert!(msg_text.contains("Now add a chart"), "{msg_text}");
    assert!(msg_text.contains("Chart added."), "{msg_text}");

    let tip = pane
        .conversation
        .conversation_id
        .as_deref()
        .expect("tip mission id");
    let prior = native::prior_of(tip).expect("resume point");
    assert_eq!(prior.thread.as_deref(), Some(session_id));

    assert_eq!(
        locust_core::folders::path_of(&store, &locust_core::people::workspace_id(&shop))
            .unwrap()
            .as_deref(),
        Some(shop_s.as_str())
    );
}

#[test]
fn the_ai_agents_page_lists_each_agent_usable_first_with_what_to_do() {
    use locust_core::agents::{Auth, Found, Probe};
    let (_dirs, _store, mut rt) = window();
    let agent = |id: &str, name: &str, installed: bool, auth: Auth, status: Probe| Found {
        id: id.into(),
        display_name: name.into(),
        installed,
        version: installed.then(|| "1.2.3".to_string()),
        auth,
        ready: status == Probe::Ready,
        status,
        unsupported_note: None,
        through_cli: false,
        checking: false,
    };
    // Made-up discovery results (the test never runs a real agent), in an order the page must not keep.
    let found = vec![
        agent("gemini", "Gemini CLI", true, Auth::Authenticated, Probe::Ready),
        agent("opencode", "OpenCode", false, Auth::NotApplicable, Probe::Missing),
        agent("claude", "Claude Code", true, Auth::Unauthenticated, Probe::AuthRequired),
        agent("codex", "Codex CLI", true, Auth::Authenticated, Probe::Ready),
    ];
    rt.update(move |s, _| {
        s.settings_panel.open = true;
        s.settings_panel.page = crate::settings_pages::SettingsPageId::from_str("runtimes");
        s.settings_panel.agents = Some(found);
    });
    settle(&mut rt);
    let words = text_in(&rt, "agents-section").join("\n");
    assert!(words.contains("AI agents & accounts") && words.contains("Each AI agent starts with this Mac's sign-in."), "{words}");
    // Usable first, then signed out, then not installed, then planned (Electron's RUNTIME_BAND).
    let at = |needle: &str| words.find(needle).unwrap_or_else(|| panic!("{needle} missing:\n{words}"));
    assert!(at("Codex CLI") < at("Claude Code") && at("Claude Code") < at("OpenCode") && at("OpenCode") < at("Gemini CLI"), "{words}");
    // A ready row says nothing more; the others say what to do, with the line to copy.
    assert!(!words.contains("Signed in on this machine"), "said once, in the lede");
    assert!(words.contains("Installed, but Claude Code is not signed in."), "{words}");
    assert!(words.contains("claude") && rt.find("agents-copy-sign-in-claude").is_some(), "the sign-in line, with Copy");
    assert!(words.contains("npm install -g opencode-ai") && rt.find("agents-copy-install-opencode").is_some(), "the install line, with Copy");
    // No Install button here yet, so the sentence says to run the line, not to press a button that is not there.
    assert!(words.contains("OpenCode was not found on this machine. Run the line below.") && !words.contains("Press Install"), "{words}");
    assert!(words.contains("Not built yet. Shown so the roadmap is visible, not because it works."), "{words}");
    for tag in ["READY", "SIGN IN", "NOT INSTALLED", "PLANNED"] {
        assert!(words.contains(tag), "{tag} missing:\n{words}");
    }
    // While nothing has been found yet, the page says it is checking.
    rt.update(|s, _| s.settings_panel.agents = None);
    settle(&mut rt);
    assert!(text_in(&rt, "agents-section").join(" ").contains("Checking each AI agent on this Mac"));
}

#[test]
fn a_failing_check_shows_its_card_and_sends_back_to_fix() {
    let (dirs, store, mut rt) = window_in("accept-edits");
    let work = dirs.0.join("work");
    let ws_id = locust_core::workspace_settings::workspace_id_for(&work);
    let mut settings = locust_core::workspace_settings::WorkspaceSettings::default();
    let check_cmd = "grep -q FIXED notes.txt || (echo FAIL notes.txt is broken && exit 1)";
    settings.check_commands.insert(ws_id.clone(), check_cmd.into());
    settings.check_fixes.insert(ws_id, 2);
    locust_core::workspace_settings::save(&store, &settings).unwrap();

    let _g = native::hooks::install(store.clone(), fake("m1-app-server.py", "cards-edit-check"));
    type_and_send(&mut rt, "Write notes.txt.");

    pump(&mut rt, "failing check and fix turn", |s| {
        s.ledger.as_ref().is_some_and(|p| {
            !p.loading
                && !p.snapshot.running
                && p.after_tip.is_none()
                && s.running_checks.is_empty()
                && p.items.iter().any(|i| {
                    matches!(
                        i,
                        ledger::Item::EditCheck {
                            outcome: locust_core::edit_check::EditCheckOutcome::Passed,
                            fix: Some(1),
                            ..
                        }
                    )
                })
        })
    });
    settle(&mut rt);

    let msg_text = text_in(&rt, "messages").join("\n");
    assert!(msg_text.contains("Wrote notes.txt."), "{msg_text}");
    assert!(msg_text.contains("failed after this turn:"), "{msg_text}");
    assert!(msg_text.contains("FAIL notes.txt is broken"), "{msg_text}");
    assert!(msg_text.contains("failed, so Locust sent it back to Codec"), "{msg_text}");
    assert!(msg_text.contains("fix 1 of 2"), "{msg_text}");
    assert!(msg_text.contains("Fixed notes.txt."), "{msg_text}");
    assert!(msg_text.contains("passed after fix 1."), "{msg_text}");
}

#[test]
fn a_branch_pull_request_shows_its_chip_watches_and_shows_the_news_banner() {
    use locust_core::folder_commit::{FolderCommits, Network};
    use locust_core::pull_requests::PullLinkReader;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    struct FakeGh(AtomicUsize);
    impl Network for FakeGh {
        fn run(&self, program: &str, args: &[String], _cwd: &Path) -> Result<String, String> {
            if program != "gh" {
                return Err("unexpected program".into());
            }
            if args.first().map(String::as_str) == Some("auth") {
                return Ok("Logged in".into());
            }
            if args.first().map(String::as_str) == Some("pr")
                && args.get(1).map(String::as_str) == Some("view")
            {
                let call = self.0.fetch_add(1, Ordering::SeqCst);
                let check = if call == 0 {
                    json!({ "__typename": "CheckRun", "conclusion": "", "status": "IN_PROGRESS", "name": "CI" })
                } else {
                    json!({ "__typename": "CheckRun", "conclusion": "FAILURE", "status": "COMPLETED", "name": "CI" })
                };
                return Ok(json!({
                    "isDraft": false,
                    "number": 17,
                    "reviewDecision": "",
                    "state": "OPEN",
                    "statusCheckRollup": [check],
                    "title": "Fix the cart total",
                    "url": "https://github.com/someone/pebble/pull/17"
                })
                .to_string());
            }
            Err("unexpected gh call".into())
        }
    }

    let (dirs, _store, mut rt) = window();
    let work = dirs.0.join("work");
    let run_git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(&work)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {:?} failed", args);
    };
    run_git(&["init", "-q", "-b", "main"]);
    run_git(&["config", "user.email", "you@example.test"]);
    run_git(&["config", "user.name", "You"]);
    std::fs::write(work.join("cart.py"), "def total(): return 0\n").unwrap();
    run_git(&["add", "."]);
    run_git(&["commit", "-q", "-m", "first"]);
    run_git(&["remote", "add", "origin", "https://github.com/someone/pebble.git"]);
    run_git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    run_git(&["symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/main"]);
    run_git(&["checkout", "-q", "-b", "locust/fix-the-cart-total"]);

    let fake_gh = Arc::new(FakeGh(AtomicUsize::new(0)));
    rt.update(|s, _| {
        s.pr.commits = Arc::new(FolderCommits::new(Box::new(FakeGh(AtomicUsize::new(0)))));
        s.pr.link_reader = Arc::new(PullLinkReader::new(fake_gh.clone()));
        s.pr.folder = None;
        s.pr.record_opened_urls = true;
    });

    pump(&mut rt, "pull request chip in header", |s| {
        s.pr.current.as_ref().is_some_and(|(_, pr)| pr.is_some())
    });
    settle(&mut rt);

    assert!(rt.find("pr-chip").is_some(), "expected pr-chip in header");
    let chip_words = text_in(&rt, "pr-chip").join(" ");
    assert!(chip_words.contains("#17") && chip_words.contains("Open"), "{chip_words}");

    click(&mut rt, "pr-watch-toggle");
    assert!(text_in(&rt, "pr-watch-toggle").join(" ").contains("Watching"));

    rt.update(|s, _| crate::pull_request_chip::watch_look_now(s));
    pump(&mut rt, "pull request news banner", |s| s.pr.news.is_some());
    settle(&mut rt);

    assert!(rt.find("pr-news-banner").is_some(), "expected pr-news-banner");
    let banner_words = text_in(&rt, "pr-news-banner").join(" ");
    assert!(
        banner_words.contains("Pull request #17: 1 of 1 checks did not pass."),
        "{banner_words}"
    );
    assert!(banner_words.contains("Ask Codec to fix it"), "{banner_words}");

    click(&mut rt, "pr-news-ask");
    assert_eq!(
        rt.state.draft.text,
        "Pull request #17 (https://github.com/someone/pebble/pull/17): 1 of 1 checks did not pass. Look at what is wrong, fix it on this branch, and push the fix."
    );

    click(&mut rt, "pr-news-open");
    assert_eq!(
        rt.state.pr.opened_urls,
        vec!["https://github.com/someone/pebble/pull/17"]
    );

    click(&mut rt, "pr-news-dismiss");
    assert!(rt.find("pr-news-banner").is_none());
}

#[test]
fn own_copy_is_turned_on_in_the_card_and_a_copy_is_removed_only_after_its_changes_are_named() {
    let (dirs, store, mut rt) = window();
    let work = dirs.0.join("work");
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git").args(args).current_dir(&work).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    };
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "person@locust.test"]); // made-up person; public-guard: allow
    git(&["config", "user.name", "The person"]);
    std::fs::write(work.join("README.md"), "the project\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "first"]);

    // A whole teammate record, as the app writes one (the shared window() seeds only an id and a name).
    store
        .save_json("teammates.json", &serde_json::json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_codec", "name": "Codec",
            "hue": "lime", "role": "Code & Migrations", "createdAt": "2026-11-01T00:00:00.000Z", "route": {"runtime": "codex", "mode": "ask"},
            "folder": work.to_string_lossy()}],
            "missionOwners": {}, "settings": {}}))
        .unwrap();
    // The card's switch says the branch, and saving writes worktree: true.
    rt.update(|s, _| crate::people_panel::open(s));
    settle(&mut rt);
    click(&mut rt, "team-edit-tm_codec");
    let form = text_in(&rt, "team-form").join(" ");
    assert!(form.contains("Works in its own copy of the folder, on branch locust/codec."), "{form}");
    click(&mut rt, "team-own-copy");
    // The form runs past the window's height: scroll to Save, as a person would.
    rt.update(|_, tx| tx.scroll_key_to("team-save", 0.0));
    click(&mut rt, "team-save");
    assert!(rt.state.people_panel.error.is_empty() && !rt.state.people_panel.form_open, "{}", rt.state.people_panel.error);
    let team = locust_core::people::team(&store).unwrap();
    assert_eq!(team["teammates"][0]["worktree"], true, "{team}");

    // A copy with uncommitted work: Remove names the changes; only then can it go, branch kept.
    let tree = locust_core::worktrees::Manager::new(&work).ensure("tm_codec", "Codec", None).unwrap();
    std::fs::write(tree.join("draft.md"), "not committed\n").unwrap();
    rt.update(|s, _| {
        crate::settings_panel::open(s);
        s.settings_panel.page = crate::settings_pages::SettingsPageId::from_str("workspace");
    });
    settle(&mut rt);
    let row = text_in(&rt, "own-copies").join(" ");
    assert!(row.contains("OWN COPIES") && row.contains("Codec") && row.contains("locust/codec") && row.contains("Removing one keeps its branch"), "{row}");
    click(&mut rt, "own-copy-remove-tm_codec");
    let named = text_in(&rt, "own-copies").join(" ");
    assert!(named.contains("Codec's copy has 1 uncommitted change that is not on locust/codec: draft.md. Removing it deletes it for good."), "{named}");
    assert!(tree.join("draft.md").exists(), "nothing deleted on the first press");
    click(&mut rt, "own-copy-delete-tm_codec");
    let after = text_in(&rt, "own-copies").join(" ");
    assert!(after.contains("None yet. Turn Own copy on in a teammate's card and its next run makes one."), "{after}");
    assert!(!tree.exists(), "the copy is gone");
    let branches = std::process::Command::new("git").args(["branch", "--list", "locust/codec"]).current_dir(&work).output().unwrap();
    assert!(String::from_utf8_lossy(&branches.stdout).contains("locust/codec"), "its branch stays");
}

#[test]
fn selecting_text_in_a_reply_asks_on_the_side_and_keeps_main_ledger_unchanged() {
    let (_dirs, store, mut rt) = window();
    let _hook = native::hooks::install(store.clone(), fake("m1-app-server.py", "success"));
    type_and_send(&mut rt, "Synthetic prompt");
    pump(&mut rt, "main reply", answered);
    settle(&mut rt);

    let main_id = rt.state.ledger.as_ref().unwrap().snapshot.tip.clone();
    let main_ledger_path = store.write_path(&main_id).unwrap();
    let main_bytes_before = std::fs::read(&main_ledger_path).unwrap();

    let reply_elem = rt
        .elements()
        .into_iter()
        .find(|e| e.text.as_deref() == Some("Hello world"))
        .expect("Hello world reply element");
    let [rx, ry, rw, rh] = reply_elem.rect;
    let y = ry + rh / 2.0;
    rt.pointer_down(rx + 2.0, y);
    rt.pointer_move(rx + rw - 2.0, y);
    rt.pointer_up(rx + rw - 2.0, y);
    settle(&mut rt);

    assert!(rt.find("selection-ask").is_some(), "expected Ask about this chip");
    click(&mut rt, "selection-ask");
    // As in Electron, the quote lands in the conversation's own message box, and nothing opens.
    assert!(rt.state.draft.text.starts_with("> Hello world"), "{:?}", rt.state.draft.text);
    assert!(rt.find("side-chat-panel").is_none());
    assert!(rt.find("selection-ask").is_none(), "the chip goes once used");

    click(&mut rt, "ask-on-side-button");
    assert!(rt.find("side-chat-panel").is_some(), "expected side chat panel");

    click(&mut rt, "side-chat-input");
    rt.paste("Why?");
    rt.frame(0.016);
    rt.key(Key::Enter.into());
    rt.frame(0.016);

    pump(&mut rt, "side answer", |s| {
        s.side_chat.as_ref().is_some_and(|c| {
            c.turns.iter().any(|t| {
                t.phase == "completed"
                    && t.items.iter().any(|i| {
                        matches!(i, ledger::Item::Assistant { text, .. } if text == "Side answer from copy.")
                    })
            })
        })
    });
    settle(&mut rt);

    let side_words = text_in(&rt, "side-chat-panel").join(" ");
    assert!(side_words.contains("Side answer from copy."), "{side_words}");

    let main_bytes_after = std::fs::read(&main_ledger_path).unwrap();
    assert_eq!(
        main_bytes_before, main_bytes_after,
        "main conversation ledger must be unchanged"
    );

    let side_id = rt.state.side_chat.as_ref().unwrap().turns[0].mission_id.clone();
    let recent = native::list_recent(20, true).unwrap();
    assert!(recent.summaries.iter().all(|m| m.mission_id != side_id));
}

/// A project with Codec on its own branch and two turns saved there (cart.py changed, notes.md added).
fn own_branch_with_two_turns(dirs: &Dirs, store: &Store) -> (PathBuf, PathBuf) {
    use locust_core::worktrees::{CheckpointMessage, Manager};
    let work = dirs.0.join("work");
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git").args(args).current_dir(&work).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    };
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "person@locust.test"]); // made-up person; public-guard: allow
    git(&["config", "user.name", "The person"]);
    std::fs::write(work.join("cart.py"), "def total(items):\n    return 0\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "first"]);
    store
        .save_json("teammates.json", &serde_json::json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_codec", "name": "Codec",
            "hue": "lime", "role": "Code & Migrations", "createdAt": "2026-11-01T00:00:00.000Z", "route": {"runtime": "codex", "mode": "accept-edits"},
            "folder": work.to_string_lossy(), "worktree": true}],
            "missionOwners": {}, "settings": {}}))
        .unwrap();
    let manager = Manager::new(&work);
    let tree = manager.ensure("tm_codec", "Codec", None).unwrap();
    let turn = |subject: &str| CheckpointMessage {
        subject: subject.into(),
        body: None,
        trailers: vec![("Locust-Route".into(), "codex / synthetic-model".into())],
        author_name: "Codec".into(),
        author_email: "tm_codec@teammates.locust".into(), // made-up teammate author; public-guard: allow
    };
    std::fs::write(tree.join("cart.py"), "def total(items):\n    return sum(items)\n").unwrap();
    manager.checkpoint("tm_codec", &turn("Fix the cart total")).unwrap();
    std::fs::write(tree.join("notes.md"), "summed items\n").unwrap();
    manager.checkpoint("tm_codec", &turn("Write down why")).unwrap();
    (work, tree)
}

#[test]
fn review_changes_shows_a_teammates_branch_turn_by_turn_and_lands_it_as_one_commit() {
    let (dirs, store, mut rt) = window();
    let _hook = native::hooks::install(store.clone(), fake("m1-app-server.py", "success"));
    let (work, _tree) = own_branch_with_two_turns(&dirs, &store);
    // A conversation on screen, so the thread has its header (a turn that changes nothing saves nothing).
    type_and_send(&mut rt, "Synthetic prompt");
    pump(&mut rt, "the reply", answered);
    settle(&mut rt);
    click(&mut rt, "review-changes-open");
    pump(&mut rt, "the branch read", |s| s.review.as_ref().is_some_and(|r| r.review.is_some() && r.preview.is_some()));
    settle(&mut rt);
    let panel = text_in(&rt, "review-changes").join(" | ");
    assert!(panel.contains("Review Codec's changes"), "{panel}");
    assert!(panel.contains("locust/codec against main"), "{panel}");
    assert!(panel.contains("2 turns saved on locust/codec since it left main. Nothing here is in your branch until you land it."), "{panel}");
    assert!(panel.contains("The whole change") && panel.contains("Fix the cart total") && panel.contains("Write down why"), "{panel}");
    assert!(panel.contains("Land on main") && panel.contains("2 files as one commit"), "{panel}");

    // One turn, on its own.
    click(&mut rt, "review-turn-1");
    pump(&mut rt, "the first turn's diff", |s| s.review.as_ref().is_some_and(|r| r.turn_text.is_some()));
    let turn = rt.state.review.as_ref().unwrap().turn_text.clone().unwrap().1;
    assert!(turn.contains("cart.py") && !turn.contains("notes.md"), "{turn}");

    // Land it: the draft, then one commit of the person's, and the branch starts from it.
    click(&mut rt, "review-land-start");
    let draft = rt.state.review.as_ref().unwrap().message.text.clone();
    assert_eq!(draft, "Fix the cart total\n\n- Fix the cart total\n- Write down why\n\nLocust-Teammate: Codec\nLocust-Route: codex / synthetic-model");
    let card = text_in(&rt, "review-land").join(" | ");
    assert!(card.contains("The commit on main") && card.contains("2 files, one commit made as you. Your commit hooks run; if one refuses, your checkout is left as it was."), "{card}");
    click(&mut rt, "review-land-it");
    pump(&mut rt, "the landing, read again", |s| s.review.as_ref().is_some_and(|r| r.note.is_some() && r.preview.is_some()));
    let note = rt.state.review.as_ref().unwrap().note.clone().unwrap();
    assert!(note.starts_with("Landed on main as ") && note.ends_with(": 2 files, one commit of yours. Codec's branch now starts from there."), "{note}");
    let log = std::process::Command::new("git").args(["log", "--format=%an|%s", "main"]).current_dir(&work).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&log.stdout), "The person|Fix the cart total\nThe person|first\n");
    settle(&mut rt);
    let after = text_in(&rt, "review-changes").join(" | ");
    assert!(after.contains("Nothing new on locust/codec. Each turn Codec finishes is saved there as it ends."), "{after}");
    assert!(rt.find("review-land").is_none(), "nothing left to land");
    click(&mut rt, "review-close");
    assert!(rt.state.review.is_none());
}

#[test]
fn review_changes_names_a_conflict_and_asks_the_teammate_to_resolve_it_in_its_conversation() {
    let (dirs, store, mut rt) = window();
    let _hook = native::hooks::install(store.clone(), fake("m1-app-server.py", "success"));
    let (work, tree) = own_branch_with_two_turns(&dirs, &store);
    std::fs::write(work.join("cart.py"), "def total(items):\n    return len(items)\n").unwrap();
    let out = std::process::Command::new("git").args(["commit", "-q", "-am", "mine"]).current_dir(&work).output().unwrap();
    assert!(out.status.success());
    type_and_send(&mut rt, "Synthetic prompt");
    pump(&mut rt, "the reply", answered);
    settle(&mut rt);
    click(&mut rt, "review-changes-open");
    pump(&mut rt, "the branch read", |s| s.review.as_ref().is_some_and(|r| r.preview.is_some()));
    settle(&mut rt);
    // Codec runs in Ask, which cannot edit: the card names the conflict and says so instead of offering the ask.
    let read_only = text_in(&rt, "review-land").join(" | ");
    assert!(read_only.contains("cart.py was changed on main too, in the same places."), "{read_only}");
    assert!(read_only.contains("Codec is in a mode that cannot edit files. Switch the mode to Edit, then ask Codec to resolve it."), "{read_only}");
    assert!(rt.find("review-land-start").is_none(), "no Land button while it conflicts");
    // With an editing mode on the message box, the ask is offered.
    rt.update(|s, _| s.composer_mode = Some("accept-edits".into()));
    settle(&mut rt);
    let card = text_in(&rt, "review-land").join(" | ");
    assert!(card.contains("Ask Codec to resolve"), "{card}");

    click(&mut rt, "review-resolve");
    pump(&mut rt, "the ask", |s| s.review.as_ref().is_some_and(|r| r.note.is_some()));
    let note = rt.state.review.as_ref().unwrap().note.clone().unwrap();
    assert_eq!(note, "Asked Codec to resolve cart.py. Land it when that turn ends.");
    let sent = rt.state.flight.as_ref().and_then(|f| f.message.clone()).unwrap_or_default();
    assert_eq!(sent, "Merging main into your branch has begun, and cart.py conflict. Resolve them: keep what both sides meant, remove every conflict marker, and change nothing else.");
    assert!(std::fs::read_to_string(tree.join("cart.py")).unwrap().contains("<<<<<<<"), "the merge is begun on the branch");
    assert!(!std::fs::read_to_string(work.join("cart.py")).unwrap().contains("<<<<<<<"), "the person's checkout is untouched");
}

#[test]
fn privacy_page_backs_up_previews_and_applies_restore_at_next_start() {
    let (dirs, store, mut rt) = window();
    // The engine's running list is shared by every test in this process: holding the hooks keeps another test's live
    // turn from making this restore refuse ("A run is going").
    let _hook = native::hooks::install(store.clone(), None);
    let backup_parent = dirs.0.join("backups");
    std::fs::create_dir_all(&backup_parent).unwrap();

    // Seed native profile data.
    std::fs::write(
        store.root().join("teammates.json"),
        serde_json::json!({"schemaVersion": 15, "teammates": [{"name": "Wren"}, {"name": "Atlas"}]}).to_string(),
    )
    .unwrap();
    let ledger = store.ledger_dir().unwrap();
    std::fs::create_dir_all(&ledger).unwrap();
    std::fs::write(
        ledger.join("mission_a.jsonl"),
        "{\"schemaVersion\":27,\"recordType\":\"mission.created\",\"ledgerSequence\":1,\"occurredAt\":\"2026-08-01T12:00:00.000Z\",\"metadata\":{\"missionId\":\"mission_a\"}}\n",
    )
    .unwrap();

    // Open Settings -> Privacy & data.
    rt.update(|s, _| {
        s.settings_panel.open = true;
        s.settings_panel.page = Some(crate::settings_pages::SettingsPageId::Privacy);
        crate::backup_section::reload(s);
    });
    settle(&mut rt);

    // 1. Back up via the folder dialog.
    click(&mut rt, "backup-choose");
    let (kind, _opts) = rt.file_dialog().expect("backup folder picker");
    assert_eq!(kind, rune::FileDialogKind::Folder);
    rt.resolve_file_dialog(Some(backup_parent.clone())).unwrap();
    settle(&mut rt);
    let done_text = text_in(&rt, "backup-done").join(" ");
    assert!(done_text.contains("Backed up 2 teammates and 1 conversation"), "{done_text}");

    let backup_folder = std::fs::read_dir(&backup_parent)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|p| p.is_dir())
        .expect("backup folder created");

    // Mutate current profile after backup.
    std::fs::write(
        store.root().join("teammates.json"),
        serde_json::json!({"schemaVersion": 15, "teammates": [{"name": "Juno"}]}).to_string(),
    )
    .unwrap();

    // 2. Preview restore via the folder dialog.
    click(&mut rt, "backup-restore-choose");
    let (kind, _opts) = rt.file_dialog().expect("restore folder picker");
    assert_eq!(kind, rune::FileDialogKind::Folder);
    rt.resolve_file_dialog(Some(backup_folder)).unwrap();
    settle(&mut rt);
    let preview_text = text_in(&rt, "backup-preview").join(" ");
    assert!(preview_text.contains("2 teammates and 1 conversation"), "{preview_text}");
    assert!(preview_text.contains("1 teammate and 1 conversation, here now"), "{preview_text}");

    // 3. Ask for restore; check pending-restore.json is written (and test does not exit/relaunch).
    click(&mut rt, "backup-restore-confirm");
    settle(&mut rt);
    assert!(store.root().join(locust_core::profile_backup::PENDING_RESTORE_FILE).exists());
    assert!(rt.find("backup-restarting").is_some());

    // 4. Apply pending restore as the next start would.
    let outcome = locust_core::profile_backup::apply_pending_restore(
        store.root(),
        env!("CARGO_PKG_VERSION"),
        chrono::Local::now(),
    )
    .expect("pending restore outcome");
    assert!(outcome.ok, "{outcome:?}");

    let restored_tms: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(store.root().join("teammates.json")).unwrap())
            .unwrap();
    assert_eq!(restored_tms["teammates"], serde_json::json!([{"name": "Wren"}, {"name": "Atlas"}]));
    let aside = std::path::PathBuf::from(outcome.aside.expect("aside path"));
    let old_tms: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(aside.join("teammates.json")).unwrap())
            .unwrap();
    assert_eq!(old_tms["teammates"], serde_json::json!([{"name": "Juno"}]));
}

#[test]
fn privacy_page_previews_and_confirms_retention_keeping_running_and_electron() {
    let (dirs, store, mut rt) = window();
    let _g = native::hooks::install(store.clone(), fake("m1-app-server.py", "interrupt"));

    // Start a live running turn.
    type_and_send(&mut rt, "Synthetic prompt");
    pump(&mut rt, "the turn running", |s| can_stop(s));
    let running_ids = locust_core::engine_api::api().running();
    assert_eq!(running_ids.len(), 1);
    let running_id = running_ids[0].clone();

    let native_ledger = store.ledger_dir().unwrap();
    std::fs::create_dir_all(&native_ledger).unwrap();
    let electron_ledger = dirs.0.join("electron/mission-ledger");
    std::fs::create_dir_all(&electron_ledger).unwrap();

    let old_ts = (chrono::Utc::now() - chrono::Duration::days(45))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let old_line = serde_json::json!({
        "schemaVersion": 27,
        "recordType": "mission.created",
        "ledgerSequence": 1,
        "occurredAt": old_ts,
        "metadata": {"missionId": "mission_old_native", "createdAt": old_ts}
    });
    std::fs::write(native_ledger.join("mission_old_native.jsonl"), format!("{old_line}\n")).unwrap();

    let electron_line = serde_json::json!({
        "schemaVersion": 27,
        "recordType": "mission.created",
        "ledgerSequence": 1,
        "occurredAt": old_ts,
        "metadata": {"missionId": "mission_old_electron", "createdAt": old_ts}
    });
    std::fs::write(electron_ledger.join("mission_old_electron.jsonl"), format!("{electron_line}\n")).unwrap();

    // Backdate every record of the running turn (age is its latest record, as in Electron), so only the engine's
    // running list keeps it.
    let running_path = native_ledger.join(format!("{running_id}.jsonl"));
    // The live turn writes as it starts: wait until its ledger has held still for half a second, then backdate it.
    let size = || std::fs::metadata(&running_path).map(|m| m.len()).unwrap_or(0);
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut last = (size(), std::time::Instant::now());
    while last.1.elapsed() < Duration::from_millis(500) {
        assert!(std::time::Instant::now() < deadline, "the running turn kept writing");
        std::thread::sleep(Duration::from_millis(50));
        let now = size();
        if now != last.0 {
            last = (now, std::time::Instant::now());
        }
    }
    let raw = std::fs::read_to_string(&running_path).expect("the running turn's ledger");
    let lines: Vec<String> = raw
        .lines()
        .map(|line| match serde_json::from_str::<serde_json::Value>(line) {
            Ok(mut v) => {
                v["occurredAt"] = serde_json::json!(old_ts);
                if v["metadata"].is_object() {
                    v["metadata"]["createdAt"] = serde_json::json!(old_ts);
                }
                v.to_string()
            }
            Err(_) => line.to_owned(),
        })
        .collect();
    std::fs::write(&running_path, format!("{}\n", lines.join("\n"))).unwrap();

    rt.update(|s, _| {
        s.settings_panel.open = true;
        s.settings_panel.page = Some(crate::settings_pages::SettingsPageId::Privacy);
        crate::backup_section::reload(s);
    });
    settle(&mut rt);

    click(&mut rt, "retention-days-30");
    click(&mut rt, "retention-review");
    settle(&mut rt);

    let plan_text = text_in(&rt, "retention-plan").join(" ");
    assert!(plan_text.contains("Delete 1 turn for good"), "{plan_text}");
    assert!(plan_text.contains("1 turn kept because they are running"), "{plan_text}");

    click(&mut rt, "retention-confirm");
    settle(&mut rt);

    assert!(!native_ledger.join("mission_old_native.jsonl").exists(), "old native conversation deleted");
    assert!(native_ledger.join(format!("{running_id}.jsonl")).exists(), "running conversation stayed");
    assert!(electron_ledger.join("mission_old_electron.jsonl").exists(), "Electron conversation stayed");

    on_quit(&mut rt.state, &mut Tx::default());
}

#[test]
fn privacy_page_trash_lists_restores_and_keeps_owner() {
    let (_dirs, store, mut rt) = window();
    let _hook = native::hooks::install(store.clone(), None);
    let ledger = store.ledger_dir().unwrap();
    std::fs::create_dir_all(&ledger).unwrap();

    let created = serde_json::json!({
        "schemaVersion": 1,
        "recordType": "mission.created",
        "occurredAt": "2026-11-12T10:00:00.000Z",
        "metadata": {
            "missionId": "mission_del_1",
            "runId": "run_del_1",
            "prompt": "Check the trash restore",
            "runtime": "codex",
            "model": "gpt-6-astra",
            "requestedRouteId": "codex:account-default",
            "resolvedRouteId": "codex:gpt-6-astra",
            "workspaceId": "ws_test",
            "sandbox": "read-only",
            "executionPolicyVersion": 1,
            "createdAt": "2026-11-12T10:00:00.000Z"
        }
    });
    std::fs::write(ledger.join("mission_del_1.jsonl"), format!("{created}\n")).unwrap();
    locust_core::people::record_owner(&store, "mission_del_1", "tm_wren").unwrap();

    rt.update(|s, _| {
        s.layout_pref = "wide".into();
        refresh_recent(s);
    });
    pump(&mut rt, "mission_del_1 listed in sidebar", |s| {
        s.recent.iter().any(|r| r.id == "mission_del_1")
    });

    rt.update(|s, _| {
        s.org_menu_id = Some("mission_del_1".into());
    });
    settle(&mut rt);
    click(&mut rt, "org-delete-mission_del_1");
    settle(&mut rt);

    assert!(!ledger.join("mission_del_1.jsonl").exists());
    assert!(ledger.join(".trash/mission_del_1.jsonl").exists());
    assert_eq!(
        locust_core::people::owner_of(&store, "mission_del_1").as_deref(),
        Some("tm_wren")
    );

    rt.update(|s, _| {
        s.settings_panel.open = true;
        s.settings_panel.page = Some(crate::settings_pages::SettingsPageId::Privacy);
        crate::backup_section::reload(s);
    });
    settle(&mut rt);

    let trash_text = text_in(&rt, "trash-section").join(" ");
    assert!(trash_text.contains("1 deleted conversation, kept until you empty this."), "{trash_text}");
    assert!(trash_text.contains("Check the trash restore"), "{trash_text}");

    click(&mut rt, "trash-restore-mission_del_1");
    settle(&mut rt);
    rt.update(|s, _| {
        s.layout_pref = "wide".into();
        refresh_recent(s);
    });
    pump(&mut rt, "mission_del_1 listed again after restore", |s| {
        s.recent.iter().any(|r| r.id == "mission_del_1")
    });

    assert!(ledger.join("mission_del_1.jsonl").exists());
    assert!(!ledger.join(".trash/mission_del_1.jsonl").exists());
    assert_eq!(text_in(&rt, "trash-message").join(" "), "Put back.");
    assert_eq!(
        locust_core::people::owner_of(&store, "mission_del_1").as_deref(),
        Some("tm_wren")
    );

    on_quit(&mut rt.state, &mut Tx::default());
}

#[test]
fn privacy_page_empties_trash_and_drops_owner_from_teammates_json() {
    let (_dirs, store, mut rt) = window();
    let _hook = native::hooks::install(store.clone(), None);
    let ledger = store.ledger_dir().unwrap();
    std::fs::create_dir_all(&ledger).unwrap();

    let created = serde_json::json!({
        "schemaVersion": 1,
        "recordType": "mission.created",
        "occurredAt": "2026-11-12T10:00:00.000Z",
        "metadata": {
            "missionId": "mission_empty_1",
            "runId": "run_empty_1",
            "prompt": "Empty me for good",
            "runtime": "codex",
            "model": "gpt-6-astra",
            "requestedRouteId": "codex:account-default",
            "resolvedRouteId": "codex:gpt-6-astra",
            "workspaceId": "ws_test",
            "sandbox": "read-only",
            "executionPolicyVersion": 1,
            "createdAt": "2026-11-12T10:00:00.000Z"
        }
    });
    std::fs::write(ledger.join("mission_empty_1.jsonl"), format!("{created}\n")).unwrap();
    locust_core::people::record_owner(&store, "mission_empty_1", "tm_wren").unwrap();
    assert!(locust_core::trash::delete_mission(&ledger, "mission_empty_1", &[]).unwrap());

    rt.update(|s, _| {
        s.settings_panel.open = true;
        s.settings_panel.page = Some(crate::settings_pages::SettingsPageId::Privacy);
        crate::backup_section::reload(s);
    });
    settle(&mut rt);

    click(&mut rt, "trash-empty");
    settle(&mut rt);
    assert_eq!(text_in(&rt, "trash-empty-confirm").join(" "), "Delete 1 for good?");

    click(&mut rt, "trash-empty-confirm");
    settle(&mut rt);

    assert_eq!(text_in(&rt, "trash-message").join(" "), "Deleted 1 for good.");
    assert!(!ledger.join(".trash/mission_empty_1.jsonl").exists());
    assert_eq!(locust_core::people::owner_of(&store, "mission_empty_1"), None);

    on_quit(&mut rt.state, &mut Tx::default());
}

#[test]
fn after_restore_notice_shows_once_and_dismisses() {
    let (_dirs, store, mut rt) = window();
    let _hook = native::hooks::install(store.clone(), None);
    let outcome = locust_core::profile_backup::RestoreOutcome {
        ok: true,
        folder: "/tmp/Locust backup 2026-11-12 1000".into(),
        at: "2026-11-12T10:00:00.000Z".into(),
        aside: Some("/tmp/before-restore-20261112-100000".into()),
        counts: Some(locust_core::profile_backup::BackupCounts {
            teammates: 2,
            ..Default::default()
        }),
        reason: None,
    };
    std::fs::write(
        store.root().join("last-restore.json"),
        serde_json::to_string(&outcome.to_json()).unwrap(),
    )
    .unwrap();

    rt.update(|s, _| {
        s.last_restore = locust_core::profile_backup::take_last_restore(store.root());
    });
    settle(&mut rt);

    let banner = text_in(&rt, "restore-notice").join(" ");
    assert!(banner.contains("Restored 2 teammates"), "{banner}");
    assert!(locust_core::profile_backup::take_last_restore(store.root()).is_none());

    click(&mut rt, "restore-notice-ok");
    settle(&mut rt);
    assert!(rt.presented("restore-notice").is_none());

    on_quit(&mut rt.state, &mut Tx::default());
}

#[test]
fn inspector_opens_on_two_turn_conversation_and_shows_route_provenance_and_recovery() {
    let (_dirs, store, mut rt) = window();
    let _hook = native::hooks::install(store.clone(), None);
    let ledger = store.ledger_dir().unwrap();
    std::fs::create_dir_all(&ledger).unwrap();

    let t1_lines = [
        serde_json::json!({
            "schemaVersion": 24,
            "recordType": "mission.created",
            "occurredAt": "2026-11-12T10:00:00.000Z",
            "metadata": {
                "missionId": "mission_t1",
                "runId": "run_t1",
                "prompt": "First turn",
                "runtime": "codex",
                "model": "gpt-6-astra",
                "cliVersion": "0.153.0",
                "requestedRouteId": "codex:account-default",
                "resolvedRouteId": "codex-account:default",
                "workspaceId": "ws_test",
                "sandbox": "read-only",
                "mode": "ask",
                "executionPolicyVersion": 1,
                "createdAt": "2026-11-12T10:00:00.000Z"
            }
        }),
        serde_json::json!({
            "schemaVersion": 24,
            "recordType": "mission.event",
            "occurredAt": "2026-11-12T10:00:01.000Z",
            "event": {
                "id": "e1",
                "sequence": 1,
                "type": "run.started",
                "sourceAdapter": "codex",
                "occurredAt": "2026-11-12T10:00:01.000Z",
                "payload": {}
            }
        }),
        serde_json::json!({
            "schemaVersion": 24,
            "recordType": "mission.event",
            "occurredAt": "2026-11-12T10:00:05.000Z",
            "event": {
                "id": "e2",
                "sequence": 2,
                "type": "run.failed",
                "sourceAdapter": "codex",
                "occurredAt": "2026-11-12T10:00:05.000Z",
                "payload": { "message": "Quota reached" }
            }
        }),
    ];
    std::fs::write(
        ledger.join("mission_t1.jsonl"),
        format!(
            "{}\n",
            t1_lines
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        ),
    )
    .unwrap();

    let t2_lines = [
        serde_json::json!({
            "schemaVersion": 24,
            "recordType": "mission.created",
            "occurredAt": "2026-11-12T10:01:00.000Z",
            "metadata": {
                "missionId": "mission_t2",
                "runId": "run_t2",
                "prompt": "Carry on",
                "runtime": "claude",
                "model": "claude-sonnet-4-6",
                "requestedRouteId": "claude:account-default",
                "resolvedRouteId": "claude-account:default",
                "workspaceId": "ws_test",
                "sandbox": "workspace-write",
                "mode": "accept-edits",
                "continuesFrom": { "missionId": "mission_t1", "reason": "route-switch" },
                "startedBy": { "kind": "carry-on", "from": "codex" },
                "executionPolicyVersion": 1,
                "createdAt": "2026-11-12T10:01:00.000Z"
            }
        }),
        serde_json::json!({
            "schemaVersion": 24,
            "recordType": "mission.event",
            "occurredAt": "2026-11-12T10:01:01.000Z",
            "event": {
                "id": "e3",
                "sequence": 1,
                "type": "run.started",
                "sourceAdapter": "claude",
                "occurredAt": "2026-11-12T10:01:01.000Z",
                "payload": {}
            }
        }),
        serde_json::json!({
            "schemaVersion": 24,
            "recordType": "mission.event",
            "occurredAt": "2026-11-12T10:01:10.000Z",
            "event": {
                "id": "e4",
                "sequence": 2,
                "type": "run.completed",
                "sourceAdapter": "claude",
                "occurredAt": "2026-11-12T10:01:10.000Z",
                "payload": { "text": "Done", "usage": { "costUsd": 0.0125 } }
            }
        }),
    ];
    std::fs::write(
        ledger.join("mission_t2.jsonl"),
        format!(
            "{}\n",
            t2_lines
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        ),
    )
    .unwrap();

    rt.update(|s, tx| {
        let mate = s.teammates[0].clone();
        begin_ledger(s, "mission_t2".into(), "First turn".into(), mate, None, tx);
        if let Some(pane) = &mut s.ledger
            && let Some(worker) = &mut pane.worker
        {
            pane.snapshot = worker.poll().unwrap();
            pane.items = pane.snapshot.items.clone();
        }
    });
    settle(&mut rt);

    click(&mut rt, "inspector-open");
    settle(&mut rt);

    let panel_text = text_in(&rt, "inspector-panel").join(" ");
    assert!(panel_text.contains("About this reply"), "{panel_text}");
    assert!(!panel_text.to_lowercase().contains("inspector"), "{panel_text}");
    assert!(panel_text.contains("Turn 1 · Codex"), "{panel_text}");
    assert!(panel_text.contains("Turn 2 · Claude Code"), "{panel_text}");

    click(&mut rt, "inspector-turn-0");
    settle(&mut rt);
    let rec = text_in(&rt, "inspector-recovery").join(" ");
    assert!(rec.contains("Carried on to Claude Code"), "{rec}");

    click(&mut rt, "inspector-tab-details");
    settle(&mut rt);
    let details = text_in(&rt, "inspector-panel").join(" ");
    assert!(details.contains("Codex 0.153.0"), "{details}");

    click(&mut rt, "inspector-turn-1");
    settle(&mut rt);
    let details_t2 = text_in(&rt, "inspector-panel").join(" ");
    assert!(details_t2.contains("Claude Code"), "{details_t2}");
    assert!(details_t2.contains("$0.0125"), "{details_t2}");

    on_quit(&mut rt.state, &mut Tx::default());
}

#[test]
fn deleting_a_conversation_moves_every_turn_to_the_trash_and_an_electron_one_says_why_not() {
    let (dirs, store, mut rt) = window();
    let _hook = native::hooks::install(store.clone(), None);
    let ledger = store.ledger_dir().unwrap();
    std::fs::create_dir_all(&ledger).unwrap();
    let turn = |id: &str, at: &str, prompt: &str, parent: Option<&str>| {
        let mut metadata = serde_json::json!({"missionId": id, "runId": format!("run_{id}"), "prompt": prompt, "runtime": "codex",
            "model": "account-default", "workspaceId": "ws_test", "sandbox": "read-only", "executionPolicyVersion": 1, "createdAt": at});
        if let Some(parent) = parent {
            metadata["continuesFrom"] = serde_json::json!({"missionId": parent, "checkpointEpoch": 1, "reason": "follow-up"});
        }
        serde_json::json!({"schemaVersion": 1, "recordType": "mission.created", "occurredAt": at, "metadata": metadata}).to_string() + "\n"
    };
    std::fs::write(ledger.join("mission_first.jsonl"), turn("mission_first", "2026-11-12T10:00:00.000Z", "Fix the cart", None)).unwrap();
    std::fs::write(ledger.join("mission_second.jsonl"), turn("mission_second", "2026-11-12T10:05:00.000Z", "And the tax", Some("mission_first"))).unwrap();
    let electron = dirs.0.join("electron/mission-ledger");
    std::fs::write(electron.join("mission_theirs.jsonl"), turn("mission_theirs", "2026-11-12T09:00:00.000Z", "Electron's own", None)).unwrap();
    rt.update(|s, _| {
        s.layout_pref = "wide".into();
        refresh_recent(s);
    });
    pump(&mut rt, "both conversations listed", |s| {
        s.recent.iter().any(|r| r.id == "mission_second" || r.id == "mission_first") && s.recent.iter().any(|r| r.id == "mission_theirs")
    });
    let row_id = rt.state.recent.iter().find(|r| r.id == "mission_second" || r.id == "mission_first").unwrap().id.clone();

    // The two-turn conversation: both turns go, so it does not come back as its first turn.
    rt.update(|s, _| s.org_menu_id = Some(row_id.clone()));
    settle(&mut rt);
    click(&mut rt, &format!("org-delete-{row_id}"));
    for id in ["mission_first", "mission_second"] {
        assert!(!ledger.join(format!("{id}.jsonl")).exists(), "{id} still live");
        assert!(ledger.join(format!(".trash/{id}.jsonl")).exists(), "{id} not in the trash");
    }
    rt.update(|s, _| refresh_recent(s));
    pump(&mut rt, "the conversation gone from the list", |s| !s.recent.iter().any(|r| r.id == "mission_first" || r.id == "mission_second"));

    // Electron's conversation: read here, never changed, and the window says so.
    rt.update(|s, _| s.org_menu_id = Some("mission_theirs".into()));
    settle(&mut rt);
    click(&mut rt, "org-delete-mission_theirs");
    assert!(electron.join("mission_theirs.jsonl").exists());
    assert_eq!(rt.state.status, locust_core::trash::ELECTRON_ONLY);
    on_quit(&mut rt.state, &mut Tx::default());
}

// --- Routines: a file arriving, a step that waits for OK, a copy run kept or discarded (ROUTINES-NOTES.md) ---

/// The team as the app writes it, so the routine runner can find Codec's route and folder.
fn seed_routine_team(store: &Store, work: &Path, mode: &str) {
    store
        .save_json("teammates.json", &json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_codec", "name": "Codec",
            "hue": "lime", "role": "Code & Migrations", "createdAt": "2026-11-01T00:00:00.000Z",
            "route": {"runtime": "codex", "model": "synthetic-model", "mode": mode}, "folder": work.to_string_lossy()}],
            "missionOwners": {}, "settings": {}}))
        .unwrap();
}

fn seed_routine(store: &Store, fields: Value) -> String {
    let mut routine = json!({"name": "Routine", "teammateId": "tm_codec", "route": {"runtime": "codex", "model": "synthetic-model", "mode": "ask"},
        "learnedFrom": [], "createdAt": "2026-11-01T00:00:00.000Z", "runs": 0});
    for (k, v) in fields.as_object().unwrap() {
        routine[k] = v.clone();
    }
    store.save_json("routines.json", &json!({"schemaVersion": 1, "routines": [routine.clone()]})).unwrap();
    routine["routineId"].as_str().unwrap().to_owned()
}

/// The routine as the store holds it now.
fn routine_now(store: &Store, id: &str) -> Value {
    locust_core::routines::progress::get(store, id).unwrap().unwrap()
}

/// Automations open, looking at its routines on every poll rather than once a minute.
fn open_routines(rt: &mut Runtime<State>) {
    rt.state.routines.cadence = crate::routines_panel::Cadence { tick: Duration::ZERO, watch: Duration::ZERO, ended: Duration::ZERO };
    rt.update(|s, _| crate::routines_panel::open(s));
    settle(rt);
}

/// The window's own poll, for a while.
fn poll_for(rt: &mut Runtime<State>, how_long: Duration) {
    let until = std::time::Instant::now() + how_long;
    while std::time::Instant::now() < until {
        rt.update(|s, tx| {
            poll(s, 0.0, tx);
        });
        rt.frame(0.05);
        std::thread::sleep(Duration::from_millis(15));
    }
}

/// Every ledger's first record that a routine started.
fn routine_turns(store: &Store) -> Vec<Value> {
    if !store.ledger_dir().unwrap().exists() {
        return Vec::new();
    }
    first_records(store).into_iter().filter(|r| r["metadata"]["startedBy"]["kind"] == "routine").collect()
}

#[test]
fn a_file_arriving_in_a_watched_folder_runs_its_routine_once() {
    let (dirs, store, mut rt) = window();
    let work = dirs.0.join("work").canonicalize().unwrap();
    std::fs::create_dir_all(work.join("inbox")).unwrap();
    seed_routine_team(&store, &work, "ask");
    let id = seed_routine(&store, json!({"routineId": "rt_inbox", "name": "Read new invoices", "steps": ["Synthetic prompt"],
        "schedule": {"kind": "files", "folder": "inbox"}}));
    let _g = native::hooks::install(store.clone(), fake("m1-app-server.py", "success"));
    open_routines(&mut rt);
    // The first look is the baseline; then a file lands, and is seen while it may still be changing.
    poll_for(&mut rt, Duration::from_millis(600));
    std::fs::write(work.join("inbox/invoice-1043.pdf"), "an invoice\n").unwrap();
    poll_for(&mut rt, Duration::from_millis(600));
    assert!(routine_turns(&store).is_empty(), "nothing runs for a file that may still be changing");
    // It stops changing: twenty seconds on the watcher's clock, without sleeping through them.
    rt.state.routines.engine.as_ref().unwrap().host.skew(locust_core::routine_files::SETTLE_MS + 1);
    pump(&mut rt, "the routine's run to finish", |s| {
        s.routines.file["routines"][0]["runs"] == 1 && s.routines.file["routines"][0].get("execution").is_none()
    });
    let turns = routine_turns(&store);
    assert_eq!(turns.len(), 1, "one run for the file");
    let meta = &turns[0]["metadata"];
    assert_eq!(meta["startedBy"], json!({"kind": "routine", "routineId": id, "step": 1}));
    assert_eq!(meta["prompt"], "A new file just arrived: inbox/invoice-1043.pdf. This run is for it.\n\nSynthetic prompt");
    settle(&mut rt);
    let card = text_in(&rt, &format!("routine-{id}")).join("\n");
    assert!(card.contains("Routine \"Read new invoices\" finished: 1 step completed."), "{card}");
    // Later looks: the same file never starts it again.
    rt.state.routines.engine.as_ref().unwrap().host.skew(locust_core::routine_files::SETTLE_MS + 1);
    poll_for(&mut rt, Duration::from_millis(800));
    assert_eq!(routine_turns(&store).len(), 1, "the file started its routine once");
    assert_eq!(routine_now(&store, &id)["runs"], 1);
}

#[test]
fn a_step_that_waits_for_your_ok_shows_its_card_and_go_ahead_runs_the_next_step() {
    let (dirs, store, mut rt) = window();
    let work = dirs.0.join("work").canonicalize().unwrap();
    seed_routine_team(&store, &work, "ask");
    let id = seed_routine(&store, json!({"routineId": "rt_ship", "name": "Ship the fix", "steps": ["Read the notes.", "Write the summary."],
        "handOffs": [{}, {"askFirst": true}]}));
    let _g = native::hooks::install(store.clone(), fake("m1-app-server.py", "cards-routine"));
    open_routines(&mut rt);
    click(&mut rt, &format!("routine-run-{id}"));
    pump(&mut rt, "the wait for an OK", |s| s.routines.file["routines"][0]["execution"]["waitsForOk"] == true);
    settle(&mut rt);
    let card = text_in(&rt, &format!("routine-{id}")).join("\n");
    for said in [
        "Waiting for your OK · Step 2 of 2 · Run started",
        "Step 2 waits for your OK. Read what step 1 did, then go ahead or stop here.",
        "Step 2: Write the summary.",
        "Stop here ends this run after step 1; the routine runs again from step 1 when you press Run.",
        "Go ahead with step 2",
        "Stop here",
        "Routine \"Ship the fix\" is waiting for your OK after step 1 of 2. Go ahead with step 2 under Routines.",
    ] {
        assert!(card.contains(said), "{said}\n---\n{card}");
    }
    assert_eq!(routine_turns(&store).len(), 1, "step 2 waits");
    click(&mut rt, &format!("routine-go-ahead-{id}"));
    pump(&mut rt, "the run to finish", |s| s.routines.file["routines"][0]["runs"] == 1);
    let turns = routine_turns(&store);
    assert_eq!(turns.len(), 2);
    let steps: Vec<u64> = turns.iter().map(|t| t["metadata"]["startedBy"]["step"].as_u64().unwrap()).collect();
    assert!(steps.contains(&1) && steps.contains(&2), "{steps:?}");
    let first = turns.iter().find(|t| t["metadata"]["startedBy"]["step"] == 1).unwrap()["metadata"]["missionId"].as_str().unwrap().to_owned();
    let second = turns.iter().find(|t| t["metadata"]["startedBy"]["step"] == 2).unwrap()["metadata"]["missionId"].as_str().unwrap().to_owned();
    assert_eq!(native::real_tip(&first).unwrap(), second, "step 2 continues step 1's conversation");
    assert!(routine_now(&store, &id).get("execution").is_none());
}

#[test]
fn a_copy_runs_changes_are_kept_into_the_folder_or_discarded() {
    let (dirs, store, mut rt) = window_in("accept-edits");
    let work = dirs.0.join("work").canonicalize().unwrap();
    seed_routine_team(&store, &work, "accept-edits");
    let id = seed_routine(&store, json!({"routineId": "rt_notes", "name": "Notes", "steps": ["Write notes.txt."], "inCopy": true,
        "route": {"runtime": "codex", "model": "synthetic-model", "mode": "accept-edits"}}));
    let _g = native::hooks::install(store.clone(), fake("m1-app-server.py", "cards-edit-check"));
    let copy = store.root().join("routine-copies").join(&id);
    open_routines(&mut rt);

    // Kept: the copy's change is written into the folder.
    click(&mut rt, &format!("routine-run-{id}"));
    pump(&mut rt, "its changes to wait", |s| s.routines.file["routines"][0].get("staged").is_some());
    settle(&mut rt);
    assert!(!work.join("notes.txt").exists(), "nothing reaches the folder before Keep");
    assert_eq!(std::fs::read_to_string(copy.join("notes.txt")).unwrap(), "BROKEN\n", "the step ran in the copy");
    let waiting = text_in(&rt, &format!("routine-changes-{id}")).join("\n");
    assert!(waiting.contains("Waiting for you · its last run changed 1 file in its copy"), "{waiting}");
    assert!(waiting.contains("notes.txt") && waiting.contains("Keep writes it into the folder; Discard throws it away."), "{waiting}");
    click(&mut rt, &format!("routine-keep-{id}"));
    pump(&mut rt, "Keep", |s| s.routines.file["routines"][0].get("staged").is_none() && !s.routines.said.is_empty());
    assert_eq!(rt.state.routines.said, "Kept: 1 file written into the folder.");
    assert_eq!(std::fs::read_to_string(work.join("notes.txt")).unwrap(), "BROKEN\n");
    assert!(!copy.exists(), "the copy goes once kept");

    // Discarded: the person's own file stays as they left it.
    std::fs::write(work.join("notes.txt"), "mine\n").unwrap();
    click(&mut rt, &format!("routine-run-{id}"));
    pump(&mut rt, "its changes to wait again", |s| s.routines.file["routines"][0].get("staged").is_some());
    click(&mut rt, &format!("routine-discard-{id}"));
    assert!(text_in(&rt, &format!("routine-changes-{id}")).join("\n").contains("Discard for good?"), "Discard asks first");
    assert!(rt.state.routines.file["routines"][0].get("staged").is_some(), "one press discards nothing");
    click(&mut rt, &format!("routine-discard-{id}"));
    pump(&mut rt, "Discard", |s| s.routines.file["routines"][0].get("staged").is_none() && s.routines.said.starts_with("Discarded"));
    assert_eq!(rt.state.routines.said, "Discarded. Nothing from that run of \"Notes\" reached the folder.");
    assert_eq!(std::fs::read_to_string(work.join("notes.txt")).unwrap(), "mine\n");
    assert!(!copy.exists(), "no copy left behind");
    assert_eq!(routine_now(&store, &id)["runs"], 2);
}

#[test]
fn creating_a_teammate_runs_the_interview() {
    let (_dirs, store, mut rt) = window();
    let _hook = native::hooks::install(store.clone(), None);
    rt.update(|s, _| {
        s.home.home_open = false;
        crate::people_panel::open(s);
        s.people_panel.form_open = true;
        s.people_panel.fields[0].replace_text("Ada");
        s.people_panel.fields[1].replace_text("Research & Briefs");
    });
    settle(&mut rt);
    rt.update(|_, tx| tx.scroll_key_to("team-interview", 0.0));
    settle(&mut rt);
    click(&mut rt, "team-interview");
    settle(&mut rt);

    // Teammate created, interview conversation opened, and first turn started with interview_prompt.
    let ada = rt
        .state
        .teammates
        .iter()
        .find(|t| t.name == "Ada")
        .expect("Ada created")
        .clone();
    assert!(!rt.state.people_panel.open);
    let prompt = rt
        .state
        .flight
        .as_ref()
        .and_then(|f| f.message.as_deref())
        .expect("interview flight started")
        .to_owned();
    assert!(locust_core::interview::is_interview(Some(&prompt)), "{prompt}");

    // Pump until the turn settles (no real binary runs because hook has None), then simulate the finished interview reply.
    pump(&mut rt, "interview send settled", |s| !s.sending.is_pending());
    rt.update(|s, _| {
        s.flight = None;
        let conv = s.conversations.entry(ada.id.clone()).or_default();
        conv.turns = vec![
            ("You".into(), prompt.clone()),
            (
                ada.name.clone(),
                "Thanks, that is enough.\n\n<locust-instructions>\nYou are Ada. Answer in three bullet points at most.\n</locust-instructions>".into(),
            ),
        ];
        crate::instructions_card::sync_state(s);
    });
    settle(&mut rt);

    let card = text_in(&rt, "instructions-card").join(" ");
    assert!(
        card.contains("Ada wrote its own instructions from your answers."),
        "{card}"
    );
    assert!(card.contains("Save as Ada’s instructions"), "{card}");

    click(&mut rt, "instructions-card-save");
    settle(&mut rt);
    let saved_card = text_in(&rt, "instructions-card-saved").join(" ");
    assert!(saved_card.contains("Saved as Ada’s instructions."), "{saved_card}");

    let saved_team = people::team(&store).unwrap();
    let ada_row = saved_team["teammates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["teammateId"] == ada.id)
        .unwrap();
    assert_eq!(
        ada_row["instructions"].as_str(),
        Some("You are Ada. Answer in three bullet points at most.")
    );
    on_quit(&mut rt.state, &mut Tx::default());
}

#[test]
fn a_teammates_setup_opens_from_its_card() {
    let (dirs, store, mut rt) = window();
    let _hook = native::hooks::install(store.clone(), None);
    let work = dirs.0.join("project");
    std::fs::create_dir_all(work.join(".claude/skills/review")).unwrap();
    std::fs::write(work.join("LOCUST.md"), "Run cargo test.\n").unwrap();
    std::fs::write(work.join("CLAUDE.md"), "Keep edits small.\n").unwrap();
    std::fs::write(
        work.join(".claude/skills/review/SKILL.md"),
        "---\nname: review\ndescription: test\n---\nReview carefully.\n",
    )
    .unwrap();
    store
        .save_json(
            "teammates.json",
            &serde_json::json!({
                "schemaVersion": 1,
                "teammates": [{
                    "teammateId": "tm_ada",
                    "name": "Ada",
                    "hue": "teal",
                    "role": "Research & Briefs",
                    "createdAt": "2026-11-12T00:00:00.000Z",
                    "instructions": "Be brief.",
                    "route": {"runtime": "claude", "model": "haiku", "mode": "accept-edits"}
                }],
                "missionOwners": {},
                "settings": {"aboutYou": "I am Colin.", "memoryMode": "off"}
            }),
        )
        .unwrap();
    rt.update(|s, _| {
        s.home.home_open = false;
        s.home.folder_path = Some(work.to_string_lossy().into_owned());
        let team = native::parse_roster(&people::team(&store).unwrap().to_string()).unwrap();
        apply_team(s, team);
        crate::people_panel::open(s);
    });
    settle(&mut rt);

    click(&mut rt, "team-setup-tm_ada");
    settle(&mut rt);

    let setup = text_in(&rt, "setup-card").join(" ");
    assert!(setup.contains("Claude Code / haiku"), "{setup}");
    assert!(setup.contains("Every turn"), "{setup}");
    assert!(setup.contains("1 line, read by Claude Code itself"), "{setup}");
    assert!(setup.contains("1 skill in .claude/skills"), "{setup}");
    assert!(setup.contains("Only when you tune it"), "{setup}");
    assert!(setup.contains("Edit teammate"), "{setup}");

    click(&mut rt, "setup-close");
    settle(&mut rt);
    assert!(rt.find("setup-card").is_none());

    // /setup from the composer also opens it.
    rt.update(|s, _| s.people_panel.open = false);
    settle(&mut rt);
    type_and_send(&mut rt, "/setup");
    settle(&mut rt);
    assert!(rt.find("setup-card").is_some());
    on_quit(&mut rt.state, &mut Tx::default());
}

#[test]
fn a_send_with_the_connectors_off_says_so_first() {
    let (_dirs, store, mut rt) = window();
    let _hook = native::hooks::install(store.clone(), None);
    store
        .save_json(
            "teammates.json",
            &serde_json::json!({
                "schemaVersion": 1,
                "teammates": [{
                    "teammateId": "tm_wren",
                    "name": "Wren",
                    "hue": "lime",
                    "role": "Code & Migrations",
                    "createdAt": "2026-11-12T00:00:00.000Z",
                    "route": {"runtime": "claude", "model": "haiku", "mode": "accept-edits"}
                }],
                "missionOwners": {},
                "settings": {}
            }),
        )
        .unwrap();
    rt.update(|s, _| {
        s.home.home_open = false;
        let team = native::parse_roster(&people::team(&store).unwrap().to_string()).unwrap();
        apply_team(s, team);
        s.connector_names = vec!["claude.ai Gmail".into()];
    });
    settle(&mut rt);

    click(&mut rt, "composer");
    rt.paste("can you check my gmail for the invoice");
    settle(&mut rt);

    let hint = text_in(&rt, "connectors-off-hint").join(" ");
    assert_eq!(
        hint,
        "Wren's connectors are off, so Gmail won't be in this turn. Tick it in Wren's settings, under Connectors, to turn it on."
    );
    on_quit(&mut rt.state, &mut Tx::default());
}

#[test]
fn a_dropped_free_model_is_said_before_the_send() {
    let (_dirs, store, mut rt) = window();
    let _hook = native::hooks::install(store.clone(), None);
    store
        .save_json(
            "teammates.json",
            &serde_json::json!({
                "schemaVersion": 1,
                "teammates": [{
                    "teammateId": "tm_pip",
                    "name": "Pip",
                    "hue": "violet",
                    "role": "Docs & QA",
                    "createdAt": "2026-11-12T00:00:00.000Z",
                    "route": {"runtime": "opencode", "model": "opencode/longcat-2.5-preview-free", "mode": "accept-edits"}
                }],
                "missionOwners": {},
                "settings": {}
            }),
        )
        .unwrap();
    rt.update(|s, _| {
        s.home.home_open = false;
        let team = native::parse_roster(&people::team(&store).unwrap().to_string()).unwrap();
        apply_team(s, team);
        s.models.insert(
            "tm_pip".into(),
            vec![engine::Models {
                teammate: Some("Pip".into()),
                runtime: "opencode".into(),
                model: Some("opencode/longcat-2.5-preview-free".into()),
                effort: None,
                models: vec![engine::Model {
                    id: "opencode/deepseek-v4.1-flash-free".into(),
                    name: "DeepSeek V4.1 Flash Free".into(),
                    efforts: vec![],
                }],
            }],
        );
    });
    settle(&mut rt);

    let offer = text_in(&rt, "model-dropped-offer").join(" ");
    assert!(
        offer.contains("OpenCode no longer offers Longcat 2.5 Preview Free."),
        "{offer}"
    );
    assert!(
        offer.contains("Switch Pip to Deepseek V4.1 Flash Free"),
        "{offer}"
    );

    click(&mut rt, "model-dropped-switch");
    settle(&mut rt);

    assert!(rt.find("model-dropped-offer").is_none());
    let pip = rt.state.teammates.iter().find(|t| t.id == "tm_pip").unwrap();
    assert_eq!(
        pip.model.as_deref(),
        Some("opencode/deepseek-v4.1-flash-free")
    );
    on_quit(&mut rt.state, &mut Tx::default());
}

/// a-new-teammate-interviews-you.test.tsx, "draws no card outside an interview, nor while the turn is still going":
/// an instructions block in an ordinary conversation is only words, and an interview's card waits for its turn to end.
#[test]
fn no_instructions_card_outside_an_interview_nor_while_its_turn_runs() {
    let (_dirs, store, mut rt) = window();
    let _hook = native::hooks::install(store.clone(), None);
    let block = "Done.\n\n<locust-instructions>\nYou are Codec. Answer in three bullet points at most.\n</locust-instructions>";
    let show = |rt: &mut Runtime<State>, first: &str, running: bool| {
        let first = first.to_owned();
        rt.update(move |s, _| {
            let id = s.teammates[0].id.clone();
            let name = s.teammates[0].name.clone();
            s.flight = None;
            let conv = s.conversations.entry(id.clone()).or_default();
            conv.turns = vec![("You".into(), first.clone()), (name, block.into())];
            if running {
                s.flight = Some(Flight { teammate_id: id, conversation_id: None, header: None, after_tip: None, message: Some("next".into()) });
            }
            crate::instructions_card::sync_state(s);
        });
        settle(rt);
    };
    show(&mut rt, "Summarize the folder", false);
    assert!(rt.find("instructions-card").is_none(), "a card under an ordinary conversation");
    let interview = locust_core::interview::interview_prompt("Codec", "Code & Migrations");
    show(&mut rt, &interview, true);
    assert!(rt.find("instructions-card").is_none(), "a card while the interview's turn still runs");
    show(&mut rt, &interview, false);
    assert!(rt.find("instructions-card").is_some(), "no card under the finished interview");
}

#[test]
fn starter_team_makes_its_teammates() {
    let root = std::env::temp_dir().join(format!(
        "locust-drive-starter-{}-{}",
        std::process::id(),
        TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("electron")).unwrap();
    std::fs::create_dir_all(root.join("work")).unwrap();
    let _dirs = Dirs(root.clone());
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    native::set_open_folder(&store, &root.join("work")).unwrap();
    let mut s = State::default();
    s.engine = Some(Kind::Native);
    s.data = Some(store.clone());
    s.home.home_open = true;
    s.home.folder_path = Some(root.join("work").to_string_lossy().into());
    let mut rt = Runtime::new(s, view, (1100.0, 780.0), Env::new(true, 1.0));
    rt.frame(0.0);

    click(&mut rt, "home-template-software");
    let team_file = people::team(&store).unwrap();
    let mates = team_file["teammates"].as_array().unwrap();
    assert_eq!(mates.len(), 3, "software starter team should create 3 teammates");
    assert!(mates.iter().any(|m| m["name"] == "Wren"));
    assert!(mates.iter().any(|m| m["name"] == "Juno"));
    assert!(mates.iter().any(|m| m["name"] == "Atlas"));
}

#[test]
fn team_card_exported_and_imported_brings_routines_and_auto_arrives_in_edit() {
    let (dirs, store, mut rt) = window_in("auto");
    let work = dirs.0.join("work").canonicalize().unwrap();
    native::set_open_folder(&store, &work).unwrap();
    seed_routine_team(&store, &work, "auto");
    let ws = people::workspace_id(&work);
    seed_routine(
        &store,
        json!({
            "routineId": "rt_card",
            "name": "Check build",
            "steps": ["Run tests."],
            "workspaceId": ws,
            "route": {"runtime": "codex", "model": "synthetic-model", "mode": "auto"}
        }),
    );

    rt.update(|s, _| {
        s.home.folder_path = Some(work.to_string_lossy().into());
        crate::people_panel::open(s);
    });
    settle(&mut rt);

    click(&mut rt, "team-share");
    click(&mut rt, "team-card-save");
    let (_kind, _opts) = rt.file_dialog().expect("team card save dialog");
    let card_path = dirs.0.join("team-card.png");
    rt.resolve_file_dialog(Some(card_path.clone())).unwrap();
    settle(&mut rt);
    assert!(card_path.is_file(), "team card PNG should be written");

    // Import into a fresh window and store.
    let dst_root = dirs.0.join("dst");
    std::fs::create_dir_all(dst_root.join("electron")).unwrap();
    std::fs::create_dir_all(dst_root.join("work")).unwrap();
    std::fs::write(
        dst_root.join("electron/workspace.json"),
        serde_json::to_vec(&json!({"schemaVersion": 1, "path": dst_root.join("work").to_string_lossy()})).unwrap(),
    )
    .unwrap();
    let dst_store = Store::new(dst_root.join("native"), dst_root.join("electron")).unwrap();
    native::set_open_folder(&dst_store, &dst_root.join("work")).unwrap();
    let mut dst_state = State::default();
    dst_state.engine = Some(Kind::Native);
    dst_state.data = Some(dst_store.clone());
    dst_state.home.home_open = true;
    dst_state.home.folder_path = Some(dst_root.join("work").to_string_lossy().into());
    let mut dst_rt = Runtime::new(dst_state, view, (1100.0, 780.0), Env::new(true, 1.0));
    dst_rt.frame(0.0);

    click(&mut dst_rt, "home-team-from-image");
    let (_kind, _opts) = dst_rt.file_dialog().expect("team card open dialog");
    dst_rt.resolve_file_dialog(Some(card_path)).unwrap();
    settle(&mut dst_rt);

    let imported_team = people::team(&dst_store).unwrap();
    let imported_mate = &imported_team["teammates"][0];
    assert_eq!(imported_mate["name"], "Codec");
    assert_eq!(imported_mate["route"]["mode"], "accept-edits", "Auto must arrive in Edit (accept-edits)");
    let imported_routines = locust_core::routines::list(&dst_store).unwrap();
    assert_eq!(imported_routines["routines"].as_array().unwrap().len(), 1);
    assert_eq!(imported_routines["routines"][0]["name"], "Check build");
}

#[test]
fn routine_saved_as_markdown_and_opened_again_is_the_same_routine() {
    let (dirs, store, mut rt) = window();
    let work = dirs.0.join("work").canonicalize().unwrap();
    seed_routine_team(&store, &work, "ask");
    let id = seed_routine(
        &store,
        json!({
            "routineId": "rt_md",
            "name": "Weekly notes",
            "steps": ["Read {{topic}}", "Summarize {{topic}}"],
            "inputs": [{"key": "topic", "label": "Topic", "kind": "text", "required": true, "default": "release"}],
            "handOffs": [{}, {"askFirst": true}]
        }),
    );
    rt.update(|s, _| {
        s.home.folder_path = Some(work.to_string_lossy().into());
    });
    open_routines(&mut rt);

    click(&mut rt, &format!("routine-export-{id}"));
    let (_kind, _opts) = rt.file_dialog().expect("routine save dialog");
    let md_path = dirs.0.join("weekly-notes.routine.md");
    rt.resolve_file_dialog(Some(md_path.clone())).unwrap();
    settle(&mut rt);
    let md_text = std::fs::read_to_string(&md_path).unwrap();
    assert!(md_text.contains("Weekly notes"), "{md_text}");

    // Remove the original routine and re-open the Markdown file.
    locust_core::routines::remove(&store, &id).unwrap();
    rt.update(|s, _| {
        crate::routines_panel::open(s);
    });
    settle(&mut rt);

    click(&mut rt, "routine-import-file");
    let (_kind, _opts) = rt.file_dialog().expect("routine open dialog");
    rt.resolve_file_dialog(Some(md_path)).unwrap();
    settle(&mut rt);

    click(&mut rt, "routine-import-confirm");
    settle(&mut rt);

    let list = locust_core::routines::list(&store).unwrap();
    let routines = list["routines"].as_array().unwrap();
    assert_eq!(routines.len(), 1);
    assert_eq!(routines[0]["name"], "Weekly notes");
    assert_eq!(routines[0]["steps"], json!(["Read {{topic}}", "Summarize {{topic}}"]));
    assert_eq!(routines[0]["inputs"][0]["key"], "topic");
    assert_eq!(routines[0]["inputs"][0]["default"], "release");
    assert_eq!(routines[0]["handOffs"][1]["askFirst"], true);
}

#[test]
fn routine_with_inputs_asks_for_them_and_runs_with_them() {
    let (dirs, store, mut rt) = window();
    let work = dirs.0.join("work").canonicalize().unwrap();
    seed_routine_team(&store, &work, "ask");
    let id = seed_routine(
        &store,
        json!({
            "routineId": "rt_inp",
            "name": "Topic summary",
            "steps": ["Summarize {{topic}}"],
            "inputs": [{"key": "topic", "label": "Topic", "kind": "text", "required": true}]
        }),
    );
    let _g = native::hooks::install(store.clone(), fake("m1-app-server.py", "cards-routine"));
    open_routines(&mut rt);

    click(&mut rt, &format!("routine-run-{id}"));
    assert!(rt.state.routines.run_dialog.is_some(), "run dialog should ask for inputs");

    click(&mut rt, "routine-run-input-0");
    rt.paste("Rust port");
    rt.frame(0.016);
    click(&mut rt, "routine-run-submit");

    pump(&mut rt, "routine with inputs to finish", |s| {
        s.routines.file["routines"][0]["runs"] == 1
    });
    let turns = routine_turns(&store);
    assert_eq!(turns.len(), 1);
    let turn_json = turns[0].to_string();
    assert!(turn_json.contains("Summarize Rust port"), "prompt should contain entered input: {turn_json}");
}

#[test]
fn choosing_a_theme_is_kept_after_restart() {
    let (_dirs, store, mut rt) = window();
    rt.update(|s, _| crate::settings_panel::open(s));
    settle(&mut rt);

    click(&mut rt, "settings-theme-light");
    assert_eq!(rt.state.appearance, Some(false));
    assert_eq!(
        rt.state.settings_panel.theme,
        crate::workspace_settings::ThemeChoice::Light
    );
    drop(rt);

    // Simulate a restart on the same Store: startup loads appearance.json.
    let mut s2 = State::default();
    s2.engine = Some(Kind::Native);
    s2.data = Some(store.clone());
    let prefs = crate::workspace_settings::load_appearance(&store);
    s2.appearance = prefs.theme.to_appearance();
    s2.settings_panel.theme = prefs.theme;
    let mut rt2 = Runtime::new(s2, view, (1100.0, 780.0), Env::new(true, 1.0));
    rt2.frame(0.0);
    rt2.update(|s, _| crate::settings_panel::open(s));
    settle(&mut rt2);

    assert_eq!(rt2.state.appearance, Some(false));
    assert_eq!(
        rt2.state.settings_panel.theme,
        crate::workspace_settings::ThemeChoice::Light
    );
    assert!(rt2.find("settings-theme-light").is_some());
}

#[test]
fn quitting_with_a_turn_running_says_so_first() {
    let (_dirs, _store, mut rt) = window();
    rt.update(|s, tx| {
        s.teammates[0].busy = true;
        crate::quit_guard_dialog::request_close(s, tx);
    });
    settle(&mut rt);

    assert!(rt.state.quit_guard.open);
    assert!(!rt.state.quit_guard.quit_requested);
    assert_eq!(rt.state.quit_guard.message, "Codec is still working.");
    assert!(rt.find("quit-guard-keep").is_some());
    assert!(rt.find("quit-guard-quit").is_some());
    assert!(rt.find("quit-guard-cancel").is_some());

    click(&mut rt, "quit-guard-cancel");
    assert!(!rt.state.quit_guard.open);
    assert!(!rt.state.quit_guard.quit_requested);

    rt.update(|s, tx| {
        crate::quit_guard_dialog::request_close(s, tx);
    });
    settle(&mut rt);
    click(&mut rt, "quit-guard-quit");
    assert!(!rt.state.quit_guard.open);
    assert!(rt.state.quit_guard.confirmed);
    assert!(rt.state.quit_guard.quit_requested);
}

#[test]
fn report_shows_what_it_sends_and_sends_nothing_until_pressed() {
    let (_dirs, _store, mut rt) = window();
    rt.update(|s, _| {
        s.feedback_dialog.suppress_os_open = true;
        crate::settings_panel::open(s);
        s.settings_panel.page = Some(crate::settings_pages::SettingsPageId::App);
        s.settings_panel.search.replace_text("Report a problem");
    });
    settle(&mut rt);

    click(&mut rt, "settings-send-feedback");
    assert!(rt.state.feedback_dialog.open);
    assert!(rt.state.feedback_dialog.last_sent.is_none());
    assert!(rt.find("feedback-send").is_some());
    assert!(rt.find("feedback-email").is_some());
    assert!(rt.find("feedback-save").is_some());

    // Attach a conversation and verify it stays unticked and unsent until Send is pressed.
    rt.update(|s, _| {
        s.feedback_dialog.conversation = Some("You: hi\n\nCodec: hello".into());
    });
    settle(&mut rt);
    assert!(rt.find("feedback-include-conversation").is_some());
    assert!(!rt.state.feedback_dialog.include_conversation);
    assert!(rt.state.feedback_dialog.last_sent.is_none());

    let secret = ["sk", "ant", "api03-abcdefghijklmnopqrstuvwxyz0123456789"].join("-");
    click(&mut rt, "feedback-description");
    rt.paste(&format!("Problem with {secret}"));
    rt.frame(0.016);
    assert!(rt.state.feedback_dialog.last_sent.is_none());

    click(&mut rt, "feedback-send");
    match &rt.state.feedback_dialog.last_sent {
        Some(crate::feedback_dialog::FeedbackSent::Issue(url)) => {
            assert!(url.starts_with(locust_core::report_problem::REPORT_DESTINATION));
            assert!(!url.contains(&secret), "secret must be scrubbed from report");
        }
        other => panic!("expected Issue sent after press, got {other:?}"),
    }
}
