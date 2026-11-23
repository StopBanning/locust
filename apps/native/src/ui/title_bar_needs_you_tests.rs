//! `the-title-bar-says-what-needs-you.test.tsx` (2 cases): the title bar shows how many things wait on you as a
//! button that opens their list, before what is running, and says nothing when nothing waits or there is no list.
use super::*;
use rune::{Env, Runtime};
use serde_json::json;

fn state_with_two_waiting() -> State {
    let mut s = State::default();
    s.home.folder_path = Some("/work/tides".into());
    s.teammates.push(Teammate {
        id: "tm_wren".into(),
        name: "Wren".into(),
        role: "Engineer".into(),
        runtime: None,
        model: None,
        account: None,
        busy: false,
        runs_in: "local".into(),
        folder: None,
    });
    s.recent.push(locust_core::engine::ConversationRow {
        id: "m_w".into(),
        teammate: "tm_wren".into(),
        title: "Fix tests".into(),
        started_by: String::new(),
        state: "running".into(),
        updated: "2026-11-23T12:00:00Z".into(),
    });
    s.pending_approval = Some(locust_core::approval::Request {
        approval_id: "a1".into(),
        mission_id: "m_w".into(),
        runtime: "codex".into(),
        kind: locust_core::approval::Kind::Command,
        summary: "Run a command".into(),
        detail: "npm test".into(),
        cwd: None,
        requested_at: "2026-11-23T12:00:00Z".into(),
        always_covers: None,
        always_key: None,
        data_sent_says: None,
        reversible_says: None,
        patch: None,
        claude_input: None,
        skippable: false,
        answer_in: None,
        questions: vec![],
    });
    s.people_panel.memories = json!({
        "memories": [
            { "id": "mem_1", "text": "Use port 3001", "status": "proposed" }
        ]
    });
    s
}

// case: says how many things wait on you, as a button that opens their list, before what is running
#[test]
fn says_how_many_things_wait_on_you_as_a_button_that_opens_their_list_before_what_is_running() {
    let s = state_with_two_waiting();
    let mut rt = Runtime::new(
        s,
        |s, _| col().w_full().child(title_bar(s, &Palette::new(true), false, 1)),
        (900.0, 200.0),
        Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt.frame(0.05);
    let els = rt.elements();
    let needs_btn = els
        .iter()
        .find(|e| e.key.as_deref() == Some("titlebar-needs-you"))
        .expect("titlebar-needs-you button");
    assert_eq!(needs_btn.role, "button");
    assert_eq!(needs_btn.label.as_deref(), Some("2 need you"));
    let needs_idx = els.iter().position(|e| e.text.as_deref() == Some("2 need you")).expect("2 need you text");
    let running_idx = els.iter().position(|e| e.text.as_deref() == Some("1 running")).expect("1 running text");
    assert!(needs_idx < running_idx, "2 need you must appear before 1 running");
    assert!(els[needs_idx].rect[0] < els[running_idx].rect[0], "2 need you must sit left of 1 running");
    // Pressing the chip opens the Waiting for you menu with both items.
    let (x, y) = rt.find("titlebar-needs-you").expect("titlebar-needs-you hit target");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
    let menu = rt.state.row_menu.as_ref().expect("pressing needs-you opens the menu");
    assert_eq!(menu.title, "Waiting for you");
    let labels: Vec<&str> = menu.items.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(labels, ["Wren wants to run: npm test", "1 memory suggestion waiting"]);
}

// case: says nothing when nothing waits, or when there is no list to open
#[test]
fn says_nothing_when_nothing_waits_or_when_there_is_no_list_to_open() {
    let mut s = State::default();
    s.home.folder_path = Some("/work/tides".into());
    let mut rt_zero = Runtime::new(
        s,
        |s, _| col().w_full().child(title_bar_with_needs_you(s, &Palette::new(true), false, 1, 0, true)),
        (900.0, 200.0),
        Env::new(true, 1.0),
    );
    rt_zero.frame(0.0);
    let text_zero: String = rt_zero.elements().iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n");
    assert!(!text_zero.contains("need"), "{text_zero}");
    assert!(rt_zero.find("titlebar-needs-you").is_none());

    let mut s2 = State::default();
    s2.home.folder_path = Some("/work/tides".into());
    let mut rt_no_list = Runtime::new(
        s2,
        |s, _| col().w_full().child(title_bar_with_needs_you(s, &Palette::new(true), false, 1, 2, false)),
        (900.0, 200.0),
        Env::new(true, 1.0),
    );
    rt_no_list.frame(0.0);
    let text_no_list: String = rt_no_list.elements().iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n");
    assert!(!text_no_list.contains("need"), "{text_no_list}");
    assert!(rt_no_list.find("titlebar-needs-you").is_none());
}
