//! EVERY MODAL DIALOG HOLDS THE KEYBOARD (`renderer/src/every-modal-dialog-holds-the-keyboard.test.tsx`, design pass
//! 2026-10-19). A dialog drawn over the window takes focus in, keeps Tab inside, closes on Escape and gives the focus back
//! (Electron's `useModal`; here Rune's `.modal`). Driven in the whole window: real Tab, Shift-Tab and Escape presses.
use super::*;
use rune::{Env, Mods, Runtime};

fn window() -> Runtime<State> {
    let mut rt = Runtime::new(State::default(), crate::view, (1400.0, 900.0), Env::new(true, 1.0));
    // The window's own keys, as `main` hands them to the app.
    rt.on_key = Some(crate::on_key);
    settle(&mut rt);
    rt
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..8 {
        rt.frame(0.05);
    }
}

fn key(rt: &mut Runtime<State>, key: Key, shift: bool) {
    settle(rt);
    rt.key(KeyEvent { key, mods: Mods { shift, ..Default::default() } });
    settle(rt);
}

/// The focused element's key, or its path when it has none.
fn focused(rt: &Runtime<State>) -> Option<String> {
    rt.elements().into_iter().find(|e| e.focused).map(|e| e.key.unwrap_or(e.path))
}

/// Whether the focused element sits inside the element keyed `within` (walking up its parents).
fn focus_inside(rt: &Runtime<State>, within: &str) -> bool {
    let all = rt.elements();
    let Some(mut at) = all.iter().position(|e| e.focused) else { return true };
    loop {
        if all[at].key.as_deref() == Some(within) {
            return true;
        }
        match all[at].parent {
            Some(up) => at = up,
            None => return false,
        }
    }
}

struct Dialog {
    /// The name its scrim carries (`dialog-scrim-<name>`), or the Tune card's own backdrop.
    within: &'static str,
    open: fn(&mut State),
    is_open: fn(&State) -> bool,
}

/// Every dialog the window draws over itself, one per `dialog_scrim("…")` in `view`, and the Tune card over the pane.
fn dialogs() -> Vec<Dialog> {
    vec![
        Dialog { within: "dialog-scrim-splash", open: |s| s.home.splash_open = true, is_open: |s| s.home.splash_open },
        Dialog { within: "dialog-scrim-usage", open: |s| s.usage_panel.open = true, is_open: |s| s.usage_panel.open },
        Dialog { within: "dialog-scrim-keyboard-sheet", open: |s| s.keyboard_sheet.open = true, is_open: |s| s.keyboard_sheet.open },
        Dialog { within: "dialog-scrim-team-card", open: |s| s.people_panel.share_card_open = true, is_open: |s| s.people_panel.share_card_open },
        Dialog { within: "dialog-scrim-save-record", open: |s| crate::save_record::open(s, "m1"), is_open: |s| s.save_record.open },
        Dialog { within: "dialog-scrim-import", open: |s| s.import_dialog.open = true, is_open: |s| s.import_dialog.open },
        Dialog { within: "dialog-scrim-scout-invite", open: |s| s.scout.invite_open = true, is_open: |s| s.scout.invite_open },
        Dialog { within: "dialog-scrim-feedback", open: |s| s.feedback_dialog.open = true, is_open: |s| s.feedback_dialog.open },
        Dialog { within: "dialog-scrim-group-settings", open: |s| s.group_settings.open = true, is_open: |s| s.group_settings.open },
        Dialog { within: "dialog-scrim-quit-guard", open: |s| s.quit_guard.open = true, is_open: |s| s.quit_guard.open },
        Dialog { within: "dialog-scrim-palette", open: |s| s.palette_open = true, is_open: |s| s.palette_open },
        Dialog {
            within: "tune-backdrop",
            open: |s| s.tune_card = Some(crate::tune_teammate::sample_card("claude")),
            is_open: |s| s.tune_card.is_some(),
        },
    ]
}

#[test]
fn main_window_a_modal_dialog_is_found_where_this_test_looks() {
    // case: is found where this test looks
    // Every dialog `view` draws over a scrim is in this test's list, so a new one cannot skip the keyboard checks below.
    let source = include_str!("../main.rs");
    let drawn: Vec<String> = source
        .split("dialog_scrim(\"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .filter(|name| !name.is_empty() && !name.contains('{'))
        .map(|name| format!("dialog-scrim-{name}"))
        .collect();
    let listed: Vec<&str> = dialogs().iter().map(|d| d.within).collect();
    assert!(drawn.len() >= 11, "the dialogs drawn over the window: {drawn:?}");
    for name in &drawn {
        assert!(listed.contains(&name.as_str()), "{name} is drawn over the window and is not checked here");
    }
    for wanted in ["dialog-scrim-team-card", "dialog-scrim-save-record"] {
        assert!(listed.contains(&wanted), "{wanted}");
    }
    // And each one, opened in the window, is there.
    for d in dialogs() {
        let mut rt = window();
        rt.update(|s, _| (d.open)(s));
        settle(&mut rt);
        assert!(rt.find(d.within).is_some(), "{} opens in the window", d.within);
    }
}

#[test]
fn main_window_every_dialog_over_it_closes_on_escape_and_keeps_tab_inside() {
    // case: closes on Escape and keeps Tab inside
    for d in dialogs() {
        let mut rt = window();
        rt.update(|s, _| (d.open)(s));
        settle(&mut rt);
        assert!((d.is_open)(&rt.state), "{} opened", d.within);
        let mut reached = Vec::new();
        for i in 0..14 {
            key(&mut rt, Key::Tab, i >= 10);
            assert!(focus_inside(&rt, d.within), "{}: Tab {i} walked out to {:?}", d.within, focused(&rt));
            reached.extend(focused(&rt));
        }
        assert!(!reached.is_empty(), "{}: Tab reaches something inside it", d.within);
        assert!((d.is_open)(&rt.state), "{}: Tab does not close it", d.within);
        key(&mut rt, Key::Escape, false);
        assert!(!(d.is_open)(&rt.state), "{}: Escape closes it", d.within);
        assert!(rt.find(d.within).is_none(), "{}: and it is gone from the window", d.within);
    }
}

#[test]
fn people_panel_share_your_team_gives_the_focus_back_to_the_button_that_opened_it() {
    let mut rt = window();
    rt.update(|s, _| crate::people_panel::open(s));
    settle(&mut rt);
    // From the keyboard: Tab to the button, Space opens the card.
    let mut on_button = false;
    for _ in 0..120 {
        key(&mut rt, Key::Tab, false);
        if focused(&rt).as_deref() == Some("team-share") {
            on_button = true;
            break;
        }
    }
    assert!(on_button, "Tab reaches Share your team");
    key(&mut rt, Key::Text(" ".into()), false);
    assert!(rt.state.people_panel.share_card_open);
    key(&mut rt, Key::Tab, false);
    assert!(focus_inside(&rt, "dialog-scrim-team-card"), "focus goes in: {:?}", focused(&rt));
    key(&mut rt, Key::Escape, false);
    assert!(!rt.state.people_panel.share_card_open);
    assert_eq!(focused(&rt).as_deref(), Some("team-share"), "the focus is given back");
}

#[test]
fn tune_teammate_card_escape_does_what_cancel_does() {
    let mut rt = window();
    rt.update(|s, _| s.tune_card = Some(crate::tune_teammate::sample_card("claude")));
    settle(&mut rt);
    assert!(rt.find("tune-cancel").is_some());
    key(&mut rt, Key::Escape, false);
    assert!(rt.state.tune_card.is_none(), "Escape cancels the tune");
}

#[test]
fn save_record_dialog_save_the_record_has_an_x() {
    // case: Save the record has an x
    let mut rt = window();
    rt.update(|s, _| crate::save_record::open(s, "m1"));
    settle(&mut rt);
    let x = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("save-record-x")).expect("the x is drawn");
    assert_eq!(x.role, "button");
    assert_eq!(x.label.as_deref(), Some("Close"));
    assert!(x.focusable, "the keyboard reaches the x");
    // Tab walks to it, and Space on it closes the dialog.
    let mut on_x = false;
    for _ in 0..8 {
        key(&mut rt, Key::Tab, false);
        if focused(&rt).as_deref() == Some("save-record-x") {
            on_x = true;
            break;
        }
    }
    assert!(on_x, "Tab reaches the x");
    key(&mut rt, Key::Text(" ".into()), false);
    assert!(!rt.state.save_record.open, "the x closes it");
}
