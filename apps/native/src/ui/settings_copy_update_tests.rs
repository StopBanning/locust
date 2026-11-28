use super::*;
use locust_core::updates::HeldBack;

fn settings(held_back: Option<HeldBack>) -> rune::Runtime<State> {
    let mut s = State::default();
    s.settings_panel.open = true;
    s.settings_panel.page = Some(SettingsPageId::App);
    s.settings_panel.update.held_back = held_back;
    let mut rt = rune::Runtime::new(s, |s, _| view(s, &Palette::new(true)), (1200.0, 1200.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn update_line(rt: &rune::Runtime<State>) -> String {
    rt.elements().into_iter()
        .find(|item| !item.exiting && item.key.as_deref() == Some("settings-update-line"))
        .and_then(|item| item.text)
        .expect("Settings shows the update status")
}

// case: says the installed Locust does, and that a development build never updates
#[test]
fn held_back_copy_shows_how_the_installed_version_updates() {
    for reason in [Some(HeldBack::Driven), Some(HeldBack::Copy), None] {
        assert_eq!(update_line(&settings(reason)), "This copy of Locust doesn't update itself. The installed Locust does.", "{reason:?}");
    }
    assert_eq!(
        update_line(&settings(Some(HeldBack::Development))),
        "This copy of Locust doesn't update itself. The installed Locust does. A development build never updates.",
    );
}

// case: never says a Mac cannot update itself (the old sentence, pinned away)
#[test]
fn settings_never_shows_the_obsolete_mac_update_warning() {
    let rt = settings(Some(HeldBack::Copy));
    let line = update_line(&rt);
    assert!(line.contains("The installed Locust does."));
    let text: Vec<_> = rt.elements().into_iter().filter(|item| !item.exiting).filter_map(|item| item.text).collect();
    let text = text.join(" ");
    assert!(!text.contains("cannot update itself yet"));
    assert!(!text.contains("Download the newest from the releases page and replace it"));
}
