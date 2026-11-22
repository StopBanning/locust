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

// ---- Save conversation as routine (Electron `renderer/src/savable-conversations.test.ts`, the header's half) --------

/// A finished conversation of the person's: "Check the migration." typed, its turn ended as `phase`.
fn finished(phase: &str, typed: &str) -> Runtime<State> {
    let mut rt = window(false);
    {
        let s = &mut rt.state;
        s.routines.read_saved = |_| None;
        let pane = s.ledger.as_mut().unwrap();
        pane.snapshot.turn_metas[0].prompt = typed.into();
        pane.snapshot.tip = "mission_1".into();
        pane.snapshot.failed = phase == "failed";
        pane.snapshot.running = phase == "running";
        s.ledger_summaries.insert("mission_1".into(), ledger::Summary { mission_id: "mission_1".into(), cancelled: phase == "cancelled", failed: phase == "failed", ..Default::default() });
    }
    for _ in 0..5 {
        rt.frame(0.05);
    }
    rt
}

#[test]
fn save_as_routine_is_there_on_a_finished_conversation_with_turns_of_yours_in_it() {
    // case: is there on a finished conversation with turns of yours in it
    let mut rt = finished("completed", "Check the migration.");
    press(&mut rt, "thread-header-more");
    assert_eq!(labels(&rt), vec!["Verbose", "Save conversation as routine", "Send feedback", "Delete conversation"]);
    // Pressed, the Routines screen opens with the editor on its draft: the words the person typed, for its teammate.
    rt.state.routines.read_saved = |tip| {
        assert_eq!(tip, "mission_1");
        Some(locust_core::routine_draft::Saved {
            turns: 1,
            draft: Some(locust_core::routine_draft::RoutineDraft { name: "Check the migration.".into(), steps: vec!["Check the migration.".into()], learned_from: vec!["mission_1".into()], truncated: false }),
            chain: vec!["mission_1".into()],
        })
    };
    press(&mut rt, "row-menu-item-save-conversation-as-routine");
    assert!(rt.state.routines.open, "the Routines screen opened");
    assert_eq!(rt.state.routines.steps.iter().map(|f| f.text.clone()).collect::<Vec<_>>(), ["Check the migration."]);
    assert_eq!(rt.state.routines.fields[0].text, "Check the migration.");
    assert_eq!(rt.state.routines.entry["teammateId"], "tm", "the conversation's own teammate runs it");
}

#[test]
fn save_as_routine_is_absent_while_the_run_is_going() {
    // case: is absent while the run is going
    let mut rt = finished("running", "Check the migration.");
    press(&mut rt, "thread-header-more");
    assert!(!labels(&rt).contains(&"Save conversation as routine".to_owned()), "{:?}", labels(&rt));
    // Nor while a turn the window started is still running.
    let mut rt = finished("completed", "Check the migration.");
    rt.state.opening_teammate = Some("tm".into());
    rt.state.teammates[0].busy = true;
    assert!(save_as_routine_id(&rt.state).is_none());
}

#[test]
fn save_as_routine_is_absent_on_a_run_that_failed_or_was_cancelled() {
    // case: is absent on a run that failed, was cancelled or was interrupted
    for phase in ["failed", "cancelled", "running"] {
        let mut rt = finished(phase, "Check the migration.");
        press(&mut rt, "thread-header-more");
        assert!(!labels(&rt).contains(&"Save conversation as routine".to_owned()), "{phase}: {:?}", labels(&rt));
    }
}

#[test]
fn save_as_routine_is_absent_when_nothing_in_the_conversation_was_typed_by_the_person() {
    // case: is absent when nothing in the conversation was typed by the person
    let mut rt = finished("completed", "   ");
    press(&mut rt, "thread-header-more");
    assert!(!labels(&rt).contains(&"Save conversation as routine".to_owned()), "{:?}", labels(&rt));
    // A turn the host started (a relayed reply) is not the person's words either.
    let mut rt = finished("completed", "Check the migration.");
    rt.state.ledger.as_mut().unwrap().snapshot.turn_metas[0].started_by_kind = Some("relay".into());
    assert!(save_as_routine_id(&rt.state).is_none());
}

#[test]
fn is_absent_on_a_conversation_whose_first_receipt_has_not_landed() {
    // case: is absent on a conversation whose first receipt has not landed
    let mut rt = finished("completed", "Check the migration.");
    rt.state.ledger.as_mut().unwrap().snapshot.turn_metas[0].mission_id = "pending:3".into();
    press(&mut rt, "thread-header-more");
    assert!(!labels(&rt).contains(&"Save conversation as routine".to_owned()));
    assert!(locust_core::routine_draft::savable_mission_id(None, Some("completed"), false, true).is_none());
}
