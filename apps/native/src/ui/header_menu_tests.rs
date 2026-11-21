//! The conversation header's `⋯`, drawn in the whole window: Verbose or Normal first, Send feedback with what was
//! said, Delete conversation only once nothing runs.
use super::*;
use rune::{Env, Runtime};

fn window(running: bool) -> Runtime<State> {
    let mut s = State::default();
    let header = Teammate { id: "tm".into(), name: "Codec".into(), role: "Custom".into(), model: None, runtime: Some("claude".into()), folder: None, runs_in: "ask".into(), account: None, busy: running };
    s.teammates.push(header.clone());
    s.conversations.insert("tm".into(), Default::default());
    if running {
        s.opening_teammate = Some("tm".into());
    }
    let items = vec![
        ledger::Item::User("Check the migration.".into()),
        ledger::Item::Assistant { item_id: "a1".into(), text: "The migration is fine.".into(), r#final: true },
    ];
    s.ledger = Some(LedgerPane {
        cold_tip: None,
        header,
        conversation: Conversation { conversation_id: Some("mission_1".into()), turns: Vec::new() },
        items,
        dir: std::path::PathBuf::new(),
        worker: None,
        snapshot: ledger::Snapshot { turn_metas: vec![ledger::Metadata { mission_id: "mission_1".into(), ..Default::default() }], ..Default::default() },
        due: 0.0,
        loading: false,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.0,
    });
    let mut rt = Runtime::new(s, crate::view, (1400.0, 900.0), Env::new(true, 1.0));
    for _ in 0..10 {
        rt.frame(0.05);
    }
    rt
}

fn press(rt: &mut Runtime<State>, key: &str) {
    for _ in 0..5 {
        rt.frame(0.05);
    }
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key} on screen: {:?}", rt.elements().iter().filter_map(|e| e.key.clone()).collect::<Vec<_>>()));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..5 {
        rt.frame(0.05);
    }
}

fn labels(rt: &Runtime<State>) -> Vec<String> {
    rt.state.row_menu.as_ref().map(|m| m.items.iter().map(|i| i.label.clone()).collect()).unwrap_or_default()
}

#[test]
fn the_headers_more_menu_switches_verbose_and_normal() {
    let mut rt = window(false);
    press(&mut rt, "thread-header-more");
    assert_eq!(labels(&rt), vec!["Verbose", "Send feedback", "Delete conversation"]);
    assert_eq!(rt.state.row_menu.as_ref().unwrap().target, "mission_1");
    press(&mut rt, "row-menu-item-verbose");
    assert!(rt.state.row_menu.is_none());
    assert!(rt.state.settings_panel.settings.verbose(), "Verbose was chosen");
    press(&mut rt, "thread-header-more");
    assert_eq!(labels(&rt)[0], "Normal");
    press(&mut rt, "row-menu-item-normal");
    assert!(!rt.state.settings_panel.settings.verbose());
}

#[test]
fn send_feedback_carries_what_was_said_in_this_conversation() {
    let mut rt = window(false);
    press(&mut rt, "thread-header-more");
    press(&mut rt, "row-menu-item-send-feedback");
    assert!(rt.state.feedback_dialog.open);
    let said = rt.state.feedback_dialog.conversation.clone().unwrap_or_default();
    assert!(said.contains("You: Check the migration."), "{said}");
    assert!(said.contains("Codec: The migration is fine."), "{said}");
}

#[test]
fn delete_conversation_is_offered_only_once_nothing_runs() {
    let mut rt = window(true);
    press(&mut rt, "thread-header-more");
    assert_eq!(labels(&rt), vec!["Verbose", "Send feedback"]);
}
