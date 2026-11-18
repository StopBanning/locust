//! Electron's `a-plugged-in-computer-stays-awake-in-settings.test.ts`, drawn headless through
//! `quit_guard_dialog::startup_section`. Rust draws the switch with `aria-label="Keep this computer awake while it is plugged in"`
//! and the subtitle "Prevents idle sleep on AC power so scheduled routines and Scout run on time." (without Electron's
//! "The screen still turns off, and Sleep and closing the lid work as they always do. On battery it sleeps as usual.").
use super::*;
use rune::{Cx, Env, Runtime};

fn view(s: &State, _: &mut Cx<State>) -> El {
    col().w_full().child(startup_section(s, &Palette::new(true)))
}
fn drawn(stay_awake: bool) -> (String, Option<bool>) {
    let mut state = State::default();
    state.settings_panel.stay_awake = stay_awake;
    let mut rt = Runtime::new(state, view, (800.0, 400.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let all = rt.elements();
    let look = crate::look::controls::Look::new(true);
    let words = all.iter().filter_map(|n| n.text.clone()).collect::<Vec<_>>().join("\n");
    let checked = all
        .iter()
        .find(|n| n.key.as_deref() == Some("settings-stay-awake"))
        .map(|n| n.fill == look.c("switch-on"));
    (words, checked)
}

#[test]
fn sits_in_settings_under_startup_with_its_checked_state() {
    let (off_words, off_checked) = drawn(false);
    assert_eq!(off_checked, Some(false));
    assert!(off_words.contains("Keep this computer awake while it is plugged in"), "{off_words}");
    assert!(!off_words.contains("On battery it sleeps as usual."));

    let (on_words, on_checked) = drawn(true);
    assert_eq!(on_checked, Some(true));
    assert!(on_words.contains("Keep this computer awake while it is plugged in"), "{on_words}");
}
