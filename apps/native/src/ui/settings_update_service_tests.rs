//! The window builds its update service through updates.rs (`this_copys_update_service`, Electron's `updater:` in
//! main/index.ts), so the Mac self-updater is reachable from the app: switched off, the default, the service is
//! held back as no-feed and nothing is fetched, downloaded or replaced.
use super::*;
use locust_core::updates::{AppUpdatePhase, HeldBack, NO_FEED_MESSAGE, NoFeedUpdater, SELF_UPDATE_FEED, SELF_UPDATE_SWITCH, app_update_service};
use std::sync::Arc;

fn settle(rt: &mut rune::Runtime<State>) {
    for _ in 0..10 {
        rt.frame(0.016);
    }
}

/// Settings opened as the person opens it, on a copy that says it is `version`.
fn opened(version: &str, before: impl FnOnce(&mut State)) -> rune::Runtime<State> {
    assert!(std::env::var_os(SELF_UPDATE_SWITCH).is_none() && std::env::var_os(SELF_UPDATE_FEED).is_none(), "these tests run with the switch off");
    let mut s = State::default();
    s.home.current_version = version.to_owned();
    before(&mut s);
    let mut rt = rune::Runtime::new(s, |s, _| view(s, &Palette::new(true)), (1200.0, 5000.0), rune::Env::new(true, 1.0));
    rt.update(|s, _| {
        open(s);
        s.settings_panel.page = Some(SettingsPageId::App);
    });
    settle(&mut rt);
    rt
}

fn line(rt: &rune::Runtime<State>) -> Option<String> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some("settings-update-line") && !e.exiting).and_then(|e| e.text)
}

#[test]
fn opening_settings_builds_this_copys_update_service_off_by_default() {
    let rt = opened("0.790.0", |_| {});
    let svc = rt.state.settings_panel.update_service.clone().expect("Settings built the app's update service");
    let state = svc.state();
    assert_eq!(state.current_version, "0.790.0", "the service knows the version the window shows");
    assert_eq!(state.phase, AppUpdatePhase::Unsupported);
    assert_eq!(state.held_back, Some(HeldBack::NoFeed), "switched off, it is held back as no-feed");
    assert_eq!(rt.state.settings_panel.update, state, "the section shows the service's own state");
    assert_eq!(line(&rt).as_deref(), Some(NO_FEED_MESSAGE));
    // Off, Check now asks no updater anything: the service says the same after it.
    let _ = svc.check();
    assert_eq!(svc.state().phase, AppUpdatePhase::Unsupported);
    assert_eq!(svc.state().held_back, Some(HeldBack::NoFeed));
}

#[test]
fn a_copy_that_shows_no_version_gives_the_service_its_build_version() {
    let rt = opened("", |_| {});
    let svc = rt.state.settings_panel.update_service.clone().expect("Settings built the app's update service");
    assert_eq!(svc.state().current_version, env!("CARGO_PKG_VERSION"));
}

#[test]
fn a_service_already_there_is_kept_when_settings_opens_again() {
    let rt = opened("0.790.0", |s| {
        s.settings_panel.update_service = Some(app_update_service(None, |_| None::<NoFeedUpdater>, "9.9.9", Arc::new(|| 0), Arc::new(|_| {})));
    });
    let svc = rt.state.settings_panel.update_service.clone().expect("a service");
    assert_eq!(svc.state().current_version, "9.9.9", "the service already built was not replaced");
}
