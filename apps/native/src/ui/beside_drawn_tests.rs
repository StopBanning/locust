//! Electron's `a-conversation-opens-beside-another.test.tsx` through the window: the conversation beside as the whole
//! window draws it (`crate::view`, whose panes are the file viewer, the conversation beside, Review changes and the
//! inspector), its lines from a turn's items (`beside_lines`, what the sidebar's Open beside reads), and the right-click
//! menu that offers it (`context_menu::conversation_menu`). Events and words are Electron's, written out.
use super::*;
use serde_json::json;

/// Electron's run, read by the ledger reader: what was asked, then "1\n2\n3" come back so far.
fn sables_turn() -> Vec<ledger::Item> {
    let mut t = ledger::Transcript::default();
    t.apply(vec![
        json!({"recordType": "mission.event", "event": {"type": "run.started", "sequence": 1, "occurredAt": "2026-09-27T10:00:00.000Z", "sourceAdapter": "opencode", "payload": {}}}),
        json!({"recordType": "mission.event", "event": {"type": "message.delta", "sequence": 2, "occurredAt": "2026-09-27T10:00:01.000Z", "sourceAdapter": "opencode",
            "payload": {"itemId": "m", "operation": "append", "text": "1\n2\n3", "final": false}}}),
    ]);
    let mut items = vec![ledger::Item::User("Count from 1 to 150.".into())];
    items.extend(t.items);
    items
}

/// The window with Wren's conversation `middle` in the middle and Sable's `mission_sable` beside it.
fn window_with_beside(middle: &str, prep: impl FnOnce(&mut State)) -> rune::Runtime<State> {
    let mut s = State::default();
    s.home.home_open = false;
    s.teammates.push(Teammate { id: "tm_wren".into(), name: "Wren".into(), role: String::new(), runtime: Some("claude".into()), model: None, account: None, busy: false, runs_in: "ask".into(), folder: None });
    s.conversations.insert("tm_wren".into(), Conversation { conversation_id: Some(middle.into()), turns: vec![] });
    s.beside = Some(BesideState {
        mission_id: "mission_sable".into(),
        title: "Count from 1 to 150.".into(),
        owner_name: Some("Sable".into()),
        prompt: String::new(),
        reply_lines: beside_lines(&sables_turn()),
    });
    prep(&mut s);
    let mut rt = rune::Runtime::new(s, crate::view, (1800.0, 1000.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn has_key<T>(rt: &rune::Runtime<T>, key: &str) -> bool {
    rt.elements().iter().any(|e| !e.exiting && e.key.as_deref() == Some(key))
}

/// The elements drawn inside the panel beside.
fn in_panel<T>(rt: &rune::Runtime<T>) -> Vec<rune::ElementInfo> {
    let els = rt.elements();
    let Some(panel) = els.iter().find(|e| e.key.as_deref() == Some("beside-panel")).map(|e| format!("{}/", e.path)) else { return vec![] };
    els.into_iter().filter(|e| !e.exiting && e.path.starts_with(&panel)).collect()
}

// case: names whose conversation it is, and offers to open it here or close it
#[test]
fn the_panel_beside_names_whose_it_is_and_offers_open_here_and_close() {
    let rt = window_with_beside("mission_wren", |_| {});
    let panel = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("beside-panel")).expect("the panel beside");
    assert_eq!(panel.label.as_deref(), Some("Beside: Count from 1 to 150."));
    let inside = in_panel(&rt);
    assert!(inside.iter().any(|e| e.text.as_deref() == Some("Sable")), "whose it is");
    assert!(inside.iter().any(|e| e.role == "button" && e.label.as_deref() == Some("Open it here")));
    assert!(inside.iter().any(|e| e.role == "button" && e.label.as_deref() == Some("Close")));
}

// case: draws the conversation itself -- what was asked and what has come back so far
#[test]
fn the_panel_beside_draws_what_was_asked_and_what_came_back() {
    let rt = window_with_beside("mission_wren", |_| {});
    let words: Vec<String> = in_panel(&rt).into_iter().filter_map(|e| e.text).collect();
    assert!(words.iter().any(|w| w == "Count from 1 to 150."), "{words:?}");
    assert!(words.iter().any(|w| regex::Regex::new(r"^1\s*2\s*3$").unwrap().is_match(w)), "{words:?}");
}

// case: has no chat box of its own: a message goes to the conversation in the middle
#[test]
fn the_panel_beside_has_no_chat_box() {
    let rt = window_with_beside("mission_wren", |_| {});
    let inside = in_panel(&rt);
    assert!(!inside.is_empty());
    assert!(!inside.iter().any(|e| e.role == "textbox" || e.role == "textfield" || e.role == "text field"), "{:?}", inside.iter().map(|e| e.role).collect::<Vec<_>>());
    assert!(!inside.iter().any(|e| e.key.as_deref().is_some_and(|k| k.contains("composer") || k.contains("command-dock"))));
}

// case: is offered on a conversation's right-click menu
#[test]
fn open_beside_is_on_a_conversations_right_click_menu() {
    let mut s = State::default();
    s.recent.push(ConversationRow { id: "mission_sable".into(), title: "Count from 1 to 150.".into(), teammate: "Sable".into(), started_by: "native".into(), state: "running".into(), updated: "2026-09-27T10:00:01.000Z".into() });
    let menu = crate::context_menu::conversation_menu(&s, "mission_sable", "Count from 1 to 150.", (0.0, 0.0));
    let item = menu.items.iter().find(|item| item.label == "Open beside").expect("Open beside is offered");
    assert_eq!(item.shortcut, Some('b'));
    assert_eq!(item.action, "beside");
}

// case: shares the file panel: an open file wins, then the conversation beside, then Review changes (0.439), then the inspector
#[test]
fn the_file_panel_holds_a_file_then_beside_then_review_then_the_inspector() {
    let review = || crate::review_changes::Review {
        teammate_id: "tm_wren".into(), name: "Wren".into(), folder: std::path::PathBuf::from("/work"), review: None, preview: None, problem: None,
        showing: None, turn_text: None, note: None, editing: false, message: TextEdit::default(), working: false, was_running: false,
    };
    // An open file wins: nothing beside, no review, no inspector.
    let rt = window_with_beside("mission_wren", |s| {
        s.file_view = Some((crate::reply_actions::FileTarget { path: "notes.md".into(), start: 1, end: 1 }, Ok("notes\n".into())));
        s.review = Some(review());
        s.inspector_open = true;
    });
    assert!(!has_key(&rt, "beside-panel") && !has_key(&rt, "review-close") && !has_key(&rt, "inspector-panel"));
    // Then the conversation beside, before Review changes and the inspector.
    let rt = window_with_beside("mission_wren", |s| {
        s.review = Some(review());
        s.inspector_open = true;
    });
    assert!(has_key(&rt, "beside-panel") && !has_key(&rt, "review-close") && !has_key(&rt, "inspector-panel"));
    // Then Review changes, before the inspector.
    let rt = window_with_beside("mission_wren", |s| {
        s.beside = None;
        s.review = Some(review());
        s.inspector_open = true;
    });
    assert!(has_key(&rt, "review-close") && !has_key(&rt, "inspector-panel"));
    // And the inspector when it is the only one asked for.
    let rt = window_with_beside("mission_wren", |s| {
        s.beside = None;
        s.inspector_open = true;
    });
    assert!(has_key(&rt, "inspector-panel"));
}

// case: never draws the conversation already in the middle a second time
#[test]
fn the_conversation_in_the_middle_is_never_drawn_beside_it() {
    assert!(has_key(&window_with_beside("mission_wren", |_| {}), "beside-panel"));
    // Opened in the middle after it was put beside: it is drawn once, in the middle.
    assert!(!has_key(&window_with_beside("mission_sable", |_| {}), "beside-panel"));
    // Nor when the middle holds a later turn of the same conversation.
    let rt = window_with_beside("mission_sable_2", |s| {
        for (id, parent) in [("mission_sable", None), ("mission_sable_2", Some("mission_sable"))] {
            s.recent.push(ConversationRow { id: id.into(), title: "Count from 1 to 150.".into(), teammate: "Sable".into(), started_by: "native".into(), state: "completed".into(), updated: "2026-09-27T10:00:01.000Z".into() });
            s.ledger_summaries.insert(id.into(), ledger::Summary { mission_id: id.into(), title: "Count from 1 to 150.".into(), parent: parent.map(Into::into), created_at: "2026-09-27T10:00:00.000Z".into(), ..Default::default() });
        }
    });
    assert!(!has_key(&rt, "beside-panel"));
}
