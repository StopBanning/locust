//! the-look-is-offered-only-where-it-can-be-drawn.test.tsx, drawn: the Look row is nothing until the window says the
//! glass is offered, and once offered it sits under the theme on the Appearance page.
use super::*;
use rune::{Env, Runtime};

fn appearance(offered: bool) -> Vec<rune::ElementInfo> {
    let mut s = State::default();
    s.settings_panel.open = true;
    s.settings_panel.page = Some(SettingsPageId::Appearance);
    s.settings_panel.liquid_offered = offered;
    let mut rt = Runtime::new(s, crate::view, (1200.0, 4000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().filter(|e| !e.exiting).collect()
}

#[test]
fn draws_nothing_before_the_window_has_said_the_glass_is_offered() {
    let all = appearance(false);
    assert!(!all.iter().any(|e| e.key.as_deref() == Some("settings-look-liquid")));
    assert!(!all.iter().any(|e| e.text.as_deref() == Some("Look")));
    // The control: offered, it is drawn.
    assert!(appearance(true).iter().any(|e| e.key.as_deref() == Some("settings-look-liquid")));
}

// Rust has no browser view of Settings, so "hidden in a browser" has nothing to hold; the place under the theme does.
#[test]
fn is_hidden_in_a_browser_and_sits_under_the_theme_on_the_appearance_page() {
    let all = appearance(true);
    let top = |key: &str| all.iter().find(|e| e.key.as_deref() == Some(key)).map(|e| e.rect[1]).unwrap_or_else(|| panic!("no {key}"));
    assert!(top("settings-look-standard") > top("settings-theme-dark"), "the look sits under the theme");
}
