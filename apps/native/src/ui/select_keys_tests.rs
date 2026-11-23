//! A SELECT MOVES, THEN CHOOSES (`a-select-moves-and-chooses-by-the-keyboard.test.tsx`) and 1-9 PICKS A ROW IN AN OPEN
//! MENU (`menuDigit.test.ts`), pressed in the whole window: the sidebar's Conversation source select (Sidebar.tsx's
//! `<Select>`) and the message box's Permission mode menu (Composer.tsx). Electron's own tables are in
//! `look/select_keys.rs`.
use super::*;
use crate::conversation_list::ConversationSource;
use rune::{Env, Mods, Runtime};

const TRIGGER: &str = "sidebar-source-trigger";

fn window() -> Runtime<State> {
    // No organization policy, on this test's thread alone: another test may set one for the whole process meanwhile,
    // and the mode menu reads it.
    locust_core::org_policy::use_policy_on_this_thread(Some(locust_core::org_policy::PolicyState {
        path: "/test/policy.json".into(),
        present: false,
        policy: Default::default(),
        error: None,
    }));
    let mut s = State::default();
    // Narrowed to "Started here", so the sidebar offers the select (Sidebar.tsx shows it while it narrows).
    s.sidebar_source = ConversationSource::Person;
    let mate = Teammate {
        id: "tm".into(),
        name: "Codec".into(),
        role: "Custom".into(),
        model: None,
        runtime: Some("claude".into()),
        folder: None,
        runs_in: "ask".into(),
        account: None,
        busy: false,
    };
    s.teammates.push(mate);
    s.conversations.insert("tm".into(), Default::default());
    let mut rt = Runtime::new(s, crate::view, (1400.0, 900.0), Env::new(true, 1.0));
    // The window's own keys, as `main` hands them to the app.
    rt.on_key = Some(crate::on_key);
    settle(&mut rt);
    rt
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..6 {
        rt.frame(0.05);
    }
}

fn press_with(rt: &mut Runtime<State>, key: Key, mods: Mods) {
    settle(rt);
    rt.key(KeyEvent { key, mods });
    settle(rt);
}

fn press(rt: &mut Runtime<State>, key: Key) {
    press_with(rt, key, Mods::default());
}

fn typed(rt: &mut Runtime<State>, what: &str) {
    press(rt, Key::Text(what.into()));
}

fn focused(rt: &Runtime<State>) -> Option<String> {
    rt.elements().into_iter().find(|e| e.focused).map(|e| e.key.unwrap_or(e.path))
}

/// Tab to the closed select, as a person would.
fn tab_to_the_select(rt: &mut Runtime<State>) {
    for _ in 0..200 {
        if focused(rt).as_deref() == Some(TRIGGER) {
            return;
        }
        press(rt, Key::Tab);
    }
    panic!("Tab never reached the source select; last focus {:?}", focused(rt));
}

/// (open, highlighted row, chosen source)
fn now(rt: &Runtime<State>) -> (bool, i64, ConversationSource) {
    (rt.state.sidebar_source_open, rt.state.sidebar_source_active, rt.state.sidebar_source)
}

const ALL: ConversationSource = ConversationSource::All;
const PERSON: ConversationSource = ConversationSource::Person;
const APP: ConversationSource = ConversationSource::App;

#[test]
fn the_sidebar_select_opens_on_the_chosen_row_and_arrows_move_without_choosing() {
    // case: opens on the chosen row, and arrows move without choosing
    let mut rt = window();
    tab_to_the_select(&mut rt);
    press(&mut rt, Key::Enter);
    assert_eq!(now(&rt), (true, 1, PERSON), "Enter opens on the chosen row");
    assert!(rt.find("sidebar-source-content").is_some(), "the list is drawn");
    press(&mut rt, Key::Down);
    assert_eq!(now(&rt), (true, 2, PERSON), "Down moves, chooses nothing");
    press(&mut rt, Key::Down);
    assert_eq!(now(&rt).1, 2, "and stops at the end");
    press(&mut rt, Key::Up);
    press(&mut rt, Key::Up);
    press(&mut rt, Key::Up);
    assert_eq!(now(&rt), (true, 0, PERSON), "Up stops at the top");
    press(&mut rt, Key::End);
    assert_eq!(now(&rt), (true, 2, PERSON));
    press(&mut rt, Key::Home);
    assert_eq!(now(&rt), (true, 0, PERSON));
    typed(&mut rt, "9");
    assert_eq!(now(&rt), (true, 0, PERSON), "a digit past the rows chooses nothing");
    typed(&mut rt, "3");
    assert_eq!(now(&rt), (false, 2, APP), "3 chooses the third row and closes");
    // Opened by the pointer, it opens on the chosen row too: Down from Via apps stays on Via apps, the last row.
    rt.update(|s, _| s.sidebar_source = PERSON);
    settle(&mut rt);
    let (x, y) = rt.find(TRIGGER).expect("the select is drawn");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(&mut rt);
    assert_eq!(now(&rt), (true, 1, PERSON), "a click opens on the chosen row");
    press(&mut rt, Key::Down);
    assert_eq!(now(&rt), (true, 2, PERSON), "and Down moves on from there");
}

#[test]
fn the_sidebar_select_chooses_the_highlight_with_enter_or_space_and_escape_leaves_the_value() {
    // case: chooses the highlight with Enter or Space, and Escape leaves the value
    let mut rt = window();
    tab_to_the_select(&mut rt);
    press(&mut rt, Key::Enter);
    press(&mut rt, Key::Down);
    press(&mut rt, Key::Escape);
    assert_eq!(now(&rt), (false, 1, PERSON), "Escape closes and leaves the value");
    tab_to_the_select(&mut rt);
    press(&mut rt, Key::Enter);
    press(&mut rt, Key::Down);
    press(&mut rt, Key::Tab);
    assert_eq!((now(&rt).0, now(&rt).2), (false, PERSON), "Tab closes and leaves the value");
    tab_to_the_select(&mut rt);
    press(&mut rt, Key::Down);
    assert_eq!(now(&rt), (true, 2, PERSON), "Down on the closed select opens it on the next row");
    press(&mut rt, Key::Enter);
    assert_eq!(now(&rt), (false, 2, APP), "Enter chooses the highlight");
    tab_to_the_select(&mut rt);
    press(&mut rt, Key::Enter);
    press(&mut rt, Key::Home);
    typed(&mut rt, " ");
    assert_eq!(now(&rt), (false, 0, ALL), "Space chooses the highlight");
}

#[test]
fn the_sidebar_select_jumps_by_a_letter_and_leaves_a_chord_alone() {
    // case: skips a disabled row, and a letter jumps, then the same letter walks
    let mut rt = window();
    tab_to_the_select(&mut rt);
    press(&mut rt, Key::Enter);
    typed(&mut rt, "v");
    assert_eq!(now(&rt), (true, 2, PERSON), "v jumps to Via apps without choosing");
    typed(&mut rt, "z");
    assert_eq!(now(&rt), (true, 2, PERSON), "a letter nothing starts with stays");
    press(&mut rt, Key::Escape);
    tab_to_the_select(&mut rt);
    press_with(&mut rt, Key::Text("a".into()), Mods { ctrl: true, ..Default::default() });
    assert_eq!(now(&rt), (false, 1, PERSON), "a chord is left to the window");
    // This select has no row that cannot be chosen; the skip is Electron's table in `look/select_keys.rs`.
}

#[test]
fn the_sidebar_select_is_a_combobox_whose_list_names_the_chosen_row() {
    // case: is a combobox whose list names the chosen row, a description, and a disabled row
    let mut rt = window();
    let trigger = rt.elements().into_iter().find(|e| e.key.as_deref() == Some(TRIGGER)).expect("the select is drawn");
    assert_eq!(trigger.role, "combobox");
    assert_eq!(trigger.label.as_deref(), Some("Conversation source"));
    assert_eq!(trigger.expanded, Some(false));
    assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some("Started here")), "the chosen row's words show");
    tab_to_the_select(&mut rt);
    press(&mut rt, Key::Enter);
    let all = rt.elements();
    let trigger = all.iter().find(|e| e.key.as_deref() == Some(TRIGGER)).unwrap();
    assert_eq!(trigger.expanded, Some(true));
    let list = all.iter().find(|e| e.role == "listbox").expect("a listbox");
    assert_eq!(list.label.as_deref(), Some("Conversation source"));
    let chosen: Vec<_> = all.iter().filter(|e| e.role == "option" && e.item_selected).filter_map(|e| e.label.clone()).collect();
    assert_eq!(chosen, vec!["Started here".to_owned()], "the list names the chosen row");
}

#[test]
fn draws_a_disabled_control_as_a_disabled_combobox() {
    // case: draws a disabled control as a disabled combobox
    fn view(_: &bool, _: &mut Cx<bool>) -> El {
        let l = crate::look::controls::Look::new(true);
        let field = rune_kit::Select::new("hours", &["4 hours"])
            .label("Hours between runs")
            .current(Some("4 hours"))
            .disabled(true)
            .build(&l.k, &mut Vec::new());
        col().size(400.0, 200.0).child(field)
    }
    let mut rt = Runtime::new(false, view, (400.0, 200.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let all = rt.elements();
    let combo = all.iter().find(|e| e.role == "combobox").expect("a combobox");
    assert!(!combo.focusable, "a disabled select takes no focus");
    assert!(combo.inert || !combo.focusable);
    assert!(all.iter().any(|e| e.text.as_deref() == Some("4 hours")), "it still says its value");
}
