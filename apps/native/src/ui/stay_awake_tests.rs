//! Electron's `a-plugged-in-computer-stays-awake-in-settings.test.ts`, drawn headless through
//! `quit_guard_dialog::startup_section`: the switch named with Electron's whole sentence, and what it leaves alone said
//! only while it is on.
use super::*;
use rune::{Cx, Env, Runtime};

fn view(s: &State, _: &mut Cx<State>) -> El {
    col().w_full().child(startup_section(s, &Palette::new(true)))
}
fn drawn(stay_awake: bool) -> (String, Option<bool>, Option<String>) {
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
    let name = all.iter().find(|n| n.key.as_deref() == Some("settings-stay-awake")).and_then(|n| n.label.clone());
    (words, checked, name)
}

#[test]
fn sits_in_settings_under_startup_and_says_what_it_leaves_alone() {
    let named = "Keep this computer awake while it is plugged in, so routines and Scout run on time.";
    let (off_words, off_checked, off_name) = drawn(false);
    assert_eq!(off_checked, Some(false));
    assert_eq!(off_name.as_deref(), Some(named));
    assert!(!off_words.contains("On battery it sleeps as usual."), "{off_words}");

    let (on_words, on_checked, on_name) = drawn(true);
    assert_eq!(on_checked, Some(true));
    assert_eq!(on_name.as_deref(), Some(named));
    assert!(
        on_words.contains("The screen still turns off, and Sleep and closing the lid work as they always do. On battery it sleeps as usual."),
        "{on_words}"
    );
}
