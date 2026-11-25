//! THE SHORTCUTS THE SHEET LISTS ARE BOUND (keyboardShortcuts.ts, App.tsx's window `keydown`): each chord pressed in the
//! whole window, through the window's own `on_key`, and what it opened read back from the state and the drawn window.
//! Electron's chords take Ctrl or Cmd (`accel`); here that is `Key::Command`, as Rune hands it to `on_key`.
use super::*;
use rune::{Env, Mods, Runtime};

fn window() -> Runtime<State> {
    locust_core::org_policy::use_policy_on_this_thread(None);
    let mut s = State::default();
    s.teammates.push(Teammate {
        id: "tm".into(),
        name: "Codec".into(),
        role: "Custom".into(),
        model: None,
        runtime: Some("claude".into()),
        folder: None,
        runs_in: "ask".into(),
        account: None,
        busy: false,
    });
    s.conversations.insert("tm".into(), Default::default());
    let mut rt = Runtime::new(s, crate::view, (1400.0, 900.0), Env::new(true, 1.0));
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

/// Ctrl or Cmd with `c`: the window's primary chord.
fn accel(rt: &mut Runtime<State>, c: char) {
    press_with(rt, Key::Command(c), Mods { cmd: true, ..Default::default() });
}

fn drawn(rt: &Runtime<State>, key: &str) -> bool {
    rt.elements().iter().any(|e| e.key.as_deref() == Some(key))
}

#[test]
fn ctrl_2_opens_teammates_over_whatever_screen_was_up() {
    let mut rt = window();
    accel(&mut rt, '3');
    assert!(rt.state.settings_panel.open);
    rt.state.people_panel.tab = 1;
    rt.state.people_panel.memory_tab = true;
    accel(&mut rt, '2');
    assert!(rt.state.people_panel.open && !rt.state.people_panel.memory_tab && rt.state.people_panel.tab == 0);
    assert!(!rt.state.settings_panel.open, "one screen at a time, as setScreen");
    assert!(drawn(&rt, "team-share"), "the Teammates tab is what the window draws");
    assert!(!drawn(&rt, "memory-about"));
    assert!(rt.state.selected == 0, "Ctrl 2 is a screen, not the second teammate");
}

#[test]
fn ctrl_3_opens_settings_on_general() {
    let mut rt = window();
    accel(&mut rt, '4');
    accel(&mut rt, '3');
    assert!(rt.state.settings_panel.open);
    assert_eq!(rt.state.settings_panel.page, crate::settings_pages::SettingsPageId::from_str("app"));
    assert_eq!(rt.state.rooms.active, room_screen::RoomView::None);
    assert!(drawn(&rt, "settings-search"), "Settings is what the window draws");
}

#[test]
fn ctrl_4_opens_the_rooms_list() {
    let mut rt = window();
    accel(&mut rt, '2');
    accel(&mut rt, '4');
    assert_eq!(rt.state.rooms.active, room_screen::RoomView::List);
    assert!(!rt.state.people_panel.open);
    assert!(drawn(&rt, "rooms-screen"));
}

#[test]
fn ctrl_5_opens_memory() {
    let mut rt = window();
    accel(&mut rt, '1');
    assert!(rt.state.missions.open);
    accel(&mut rt, '5');
    assert!(rt.state.people_panel.open && rt.state.people_panel.memory_tab && rt.state.people_panel.tab == 1);
    assert!(!rt.state.missions.open);
    assert!(drawn(&rt, "memory-about"), "the Memory screen is what the window draws");
}

#[test]
fn ctrl_backtick_opens_and_closes_the_terminal() {
    let mut rt = window();
    // Control itself on every computer (control_on_mac): on a Mac it types a backtick with Ctrl held.
    press_with(&mut rt, Key::Text("`".into()), Mods { ctrl: true, ..Default::default() });
    assert!(rt.state.terminal.open);
    press_with(&mut rt, Key::Text("`".into()), Mods { ctrl: true, ..Default::default() });
    assert!(!rt.state.terminal.open);
}

#[test]
fn ctrl_slash_opens_and_closes_the_keyboard_shortcuts_sheet() {
    let mut rt = window();
    accel(&mut rt, '/');
    assert!(rt.state.keyboard_sheet.open);
    assert!(drawn(&rt, "keyboard-sheet"));
    accel(&mut rt, '/');
    assert!(!rt.state.keyboard_sheet.open);
    assert!(!drawn(&rt, "keyboard-sheet"));
}

#[test]
fn ctrl_i_opens_and_closes_the_inspector() {
    let mut rt = window();
    rt.state.terminal.open = true;
    accel(&mut rt, 'i');
    assert!(rt.state.inspector_open);
    assert!(!rt.state.terminal.open, "as the header's Activity button: the inspector takes the side");
    assert!(drawn(&rt, "inspector-title"));
    accel(&mut rt, 'i');
    assert!(!rt.state.inspector_open);
    assert!(!drawn(&rt, "inspector-title"));
}

#[test]
fn a_chord_never_eats_a_plain_key_typed_into_the_message_box() {
    let mut rt = window();
    // Clicked into, as a person does.
    settle(&mut rt);
    let (x, y) = rt.find("composer").expect("the message box is drawn");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for typed in ["2", "3", "4", "5", "i", "/", "`"] {
        press_with(&mut rt, Key::Text(typed.into()), Mods::default());
    }
    assert_eq!(rt.state.draft.text, "2345i/`");
    assert!(!rt.state.people_panel.open && !rt.state.settings_panel.open && !rt.state.keyboard_sheet.open);
    assert!(!rt.state.inspector_open && !rt.state.terminal.open);
    assert_eq!(rt.state.rooms.active, room_screen::RoomView::None);
}

/// Ctrl+Shift (Cmd+Shift on a Mac) with `c`.
fn accel_shift(rt: &mut Runtime<State>, c: char) {
    press_with(rt, Key::Command(c), Mods { cmd: true, shift: true, ..Default::default() });
}

#[test]
fn ctrl_shift_m_opens_the_permission_mode_menu_in_place_of_the_model_picker() {
    let mut rt = window();
    accel_shift(&mut rt, 'i');
    assert_eq!(rt.state.model_menu.as_deref(), Some("tm"));
    accel_shift(&mut rt, 'm');
    assert!(rt.state.mode_menu);
    assert_eq!(rt.state.model_menu, None, "one menu at a time");
    assert!(drawn(&rt, "mode-menu") && !drawn(&rt, "model-menu"));
    accel_shift(&mut rt, 'm');
    assert!(rt.state.mode_menu, "the chord opens its menu and never closes it");
}

#[test]
fn ctrl_shift_i_opens_the_model_picker_in_place_of_the_mode_menu() {
    let mut rt = window();
    accel_shift(&mut rt, 'm');
    assert!(rt.state.mode_menu);
    accel_shift(&mut rt, 'i');
    assert_eq!(rt.state.model_menu.as_deref(), Some("tm"));
    assert!(!rt.state.mode_menu, "one menu at a time");
    assert!(drawn(&rt, "model-menu") && !drawn(&rt, "mode-menu"));
    accel_shift(&mut rt, 'i');
    assert_eq!(rt.state.model_menu.as_deref(), Some("tm"), "the chord opens its menu and never closes it");
    assert!(!rt.state.inspector_open, "Ctrl+Shift+I is the model, Ctrl+I the inspector");
}

#[test]
fn ctrl_shift_e_opens_the_model_picker_for_its_effort() {
    let mut rt = window();
    accel_shift(&mut rt, 'm');
    accel_shift(&mut rt, 'e');
    assert_eq!(rt.state.model_menu.as_deref(), Some("tm"));
    assert!(!rt.state.mode_menu);
    assert!(drawn(&rt, "model-menu"));
    accel_shift(&mut rt, 'e');
    assert_eq!(rt.state.model_menu.as_deref(), Some("tm"), "the chord opens its menu and never closes it");
}

#[test]
fn the_menu_chords_work_from_the_message_box_and_type_nothing_into_it() {
    let mut rt = window();
    settle(&mut rt);
    let (x, y) = rt.find("composer").expect("the message box is drawn");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    press_with(&mut rt, Key::Text("so".into()), Mods::default());
    accel_shift(&mut rt, 'm');
    assert!(rt.state.mode_menu);
    accel_shift(&mut rt, 'e');
    assert_eq!(rt.state.model_menu.as_deref(), Some("tm"));
    assert_eq!(rt.state.draft.text, "so");
    // Without the chord's keys held, the letters type.
    rt.state.model_menu = None;
    settle(&mut rt);
    let (x, y) = rt.find("composer").expect("the message box is drawn");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    press_with(&mut rt, Key::Text("M".into()), Mods { shift: true, ..Default::default() });
    assert!(rt.state.draft.text.ends_with('M'), "{}", rt.state.draft.text);
}
