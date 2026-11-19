//! The window's half of `whats-new-is-the-whole-changelog.test.ts`: no banner on Home; what changed lives in
//! Settings > What's new, and a big build since the one last seen gets the splash.
use super::*;
use rune::{Env, Runtime};

fn texts(s: State) -> String {
    let mut rt = Runtime::new(s, crate::view, (1200.0, 3000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect::<Vec<_>>().join("\n")
}

#[test]
fn is_gone_what_changed_lives_in_settings_and_a_big_build_gets_the_splash() {
    // A big build since the one last seen: the splash, and no banner on Home.
    let mut s = State::default();
    home_scene(&mut s);
    // Arriving from 0.0.1, an older entry the notes end on, as the What's new scene does.
    s.home.changelog = changelog::entries(&format!("{}\n## 0.0.1 - 2026-11-09\n\nEarlier preview.\n", changelog::NATIVE_CHANGELOG));
    s.home.current_version = s.home.changelog[0].version.clone();
    s.home.splash_entries = changelog::splash_entries(&s.home.changelog, Some("0.0.1"), &s.home.current_version).into_iter().cloned().collect();
    s.home.splash_open = !s.home.splash_entries.is_empty();
    assert!(s.home.splash_open, "the embedded notes mark a big build after 0.0.1");
    let home = texts(s);
    assert!(home.contains(changelog::SPLASH_TITLE), "{home}");
    assert!(!home.contains("Here is what changed"), "{home}");
    // Nothing big since: no splash, and still no banner.
    let mut s = State::default();
    home_scene(&mut s);
    s.home.splash_entries.clear();
    s.home.splash_open = false;
    let quiet = texts(s);
    assert!(!quiet.contains(changelog::SPLASH_TITLE), "{quiet}");
    assert!(!quiet.contains("Here is what changed"), "{quiet}");
    // What changed lives in Settings > What's new: every version, newest first.
    let mut s = State::default();
    s.settings_panel.open = true;
    s.settings_panel.page = Some(crate::settings_pages::SettingsPageId::WhatsNew);
    // What settings_panel::open reads when the notes are not loaded yet.
    s.home.changelog = changelog::entries(&changelog::native_notes(None));
    s.home.changelog_shown = changelog::WHATS_NEW_PAGE;
    let page = texts(s);
    let newest = changelog::version_heading(&changelog::entries(changelog::NATIVE_CHANGELOG)[0]);
    assert!(page.contains(&newest), "{newest} not in: {page}");
}
