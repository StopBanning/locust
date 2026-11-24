//! Electron `renderer/src/an-update-says-it-reopens-by-itself.test.tsx`, in the window: a version the app's update
//! service has downloaded shows above the composer, saying before the click that Locust closes while it installs and
//! opens again by itself, and after Restart and install that it is installing and there is no need to open it. The
//! service runs a stand-in updater: nothing is fetched, downloaded or installed.

use crate::*;
use locust_core::updates::{
    AppUpdatePhase, AppUpdateState, CheckForUpdatesResult, EventListener, EventPayload, INSTALL_TAKES, UpdateInfo, UpdateService, UpdateServiceOptions,
    UpdaterLike, create_update_service, installing_line,
};
use rune::{Env, Runtime};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// A release feed with 0.406.0 on it, that "downloads" when told to.
#[derive(Clone, Default)]
struct Feed(Arc<Mutex<Option<EventListener>>>);

impl Feed {
    fn downloaded(&self, version: &str) {
        let listener = self.0.lock().unwrap().clone();
        if let Some(cb) = listener {
            cb(Some(EventPayload::version(version)));
        }
    }
}

impl UpdaterLike for Feed {
    fn auto_download(&self) -> bool {
        true
    }
    fn set_auto_download(&mut self, _: bool) {}
    fn allow_prerelease(&self) -> bool {
        false
    }
    fn set_allow_prerelease(&mut self, _: bool) {}
    fn auto_install_on_app_quit(&self) -> bool {
        false
    }
    fn set_auto_install_on_app_quit(&mut self, _: bool) {}
    fn check_for_updates(&mut self) -> Result<Option<CheckForUpdatesResult>, String> {
        Ok(Some(CheckForUpdatesResult {
            update_info: UpdateInfo { version: "0.406.0".into(), files: None },
            is_update_available: Some(true),
            download_result: None,
        }))
    }
    fn download_update(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn quit_and_install(&mut self, _: Option<bool>, _: Option<bool>) {}
    fn on(&mut self, event: &str, listener: EventListener) {
        if event == "update-downloaded" {
            *self.0.lock().unwrap() = Some(listener);
        }
    }
}

/// The app's update service on 0.405.0, which has found 0.406.0 and downloaded it in the background.
fn ready_service(live: Arc<AtomicUsize>, quits: Arc<AtomicUsize>) -> UpdateService<Box<dyn UpdaterLike>> {
    let feed = Feed::default();
    let svc = create_update_service(UpdateServiceOptions {
        updater: Box::new(feed.clone()) as Box<dyn UpdaterLike>,
        current_version: "0.405.0".into(),
        supported: true,
        held_back: None,
        check_only: false,
        package_url: None,
        live_mission_count: Arc::new(move || live.load(Ordering::SeqCst)),
        request_quit: Arc::new(move |_| {
            quits.fetch_add(1, Ordering::SeqCst);
        }),
        on_state_change: None,
        every_build: false,
    });
    let _ = svc.check();
    feed.downloaded("0.406.0");
    svc
}

fn window(svc: UpdateService<Box<dyn UpdaterLike>>) -> Runtime<State> {
    // The window's own copy still says what it said at launch; only the service knows the download finished.
    window_with(|s| s.settings_panel.update_service = Some(svc))
}

/// The conversation window with Codec on the team, after `setup` has had its say about the update.
fn window_with(setup: impl FnOnce(&mut State)) -> Runtime<State> {
    let mut s = State::default();
    apply_team(
        &mut s,
        vec![Teammate {
            id: "tm_codec".into(),
            name: "Codec".into(),
            role: "Code & Migrations".into(),
            runtime: Some("codex".into()),
            model: Some("synthetic-model".into()),
            account: None,
            busy: false,
            runs_in: "ask".into(),
            folder: None,
        }],
    );
    setup(&mut s);
    let mut rt = Runtime::new(s, view, (1100.0, 780.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn tick(rt: &mut Runtime<State>) {
    rt.update(|s, tx| {
        poll(s, 0.0, tx);
    });
    for _ in 0..10 {
        rt.frame(0.05);
    }
}

fn text_of(rt: &Runtime<State>, key: &str) -> Option<String> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).and_then(|e| e.text)
}

fn texts(rt: &Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect()
}

// case: says, before the click, that Locust closes while it installs and opens again by itself
#[test]
fn says_before_the_click_that_locust_closes_while_it_installs_and_opens_again_by_itself() {
    let mut rt = window(ready_service(Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0))));
    assert!(rt.find("composer").is_some(), "the conversation's composer is on screen");
    tick(&mut rt);
    assert!(rt.find("update-banner").is_some(), "the banner shows once the service has the download: {:?}", texts(&rt));
    assert_eq!(
        text_of(&rt, "update-banner-line").as_deref(),
        Some("Locust 0.406.0 is downloaded and ready. Locust closes while it installs and opens again by itself in a minute or two.")
    );
    assert!(texts(&rt).iter().any(|t| t == "Restart and install"), "{:?}", texts(&rt));
    // Above the composer, as Electron draws it.
    let (_, banner_y) = rt.find("update-banner").unwrap();
    let (_, composer_y) = rt.find("composer").unwrap();
    assert!(banner_y < composer_y, "banner {banner_y} above composer {composer_y}");
}

// case: has the words for after the click: installing, and no need to open it
#[test]
fn has_the_words_for_after_the_click_installing_and_no_need_to_open_it() {
    assert_eq!(INSTALL_TAKES, "Locust closes while it installs and opens again by itself in a minute or two.");
    assert_eq!(installing_line(Some("0.406.0")), "Installing Locust 0.406.0. It opens again by itself in a minute or two; there is no need to open it.");
    assert_eq!(installing_line(None), "Installing Locust. It opens again by itself in a minute or two; there is no need to open it.");

    // In the window: with a teammate working the service refuses and the banner says why; once it is done, Restart and
    // install asks the app to quit and the banner says it is installing.
    let live = Arc::new(AtomicUsize::new(1));
    let quits = Arc::new(AtomicUsize::new(0));
    let mut rt = window(ready_service(live.clone(), quits.clone()));
    tick(&mut rt);
    let (x, y) = rt.find("update-banner-install").expect("Restart and install");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    tick(&mut rt);
    assert_eq!(quits.load(Ordering::SeqCst), 0, "nothing is installed while a mission runs");
    assert_eq!(
        text_of(&rt, "update-banner-refusal").as_deref(),
        Some("A mission is still running. Stop it first, then install — restarting now would cut it off mid-run.")
    );

    live.store(0, Ordering::SeqCst);
    let (x, y) = rt.find("update-banner-install").expect("Restart and install");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    tick(&mut rt);
    assert_eq!(quits.load(Ordering::SeqCst), 1, "the install asks the app to quit");
    assert_eq!(
        text_of(&rt, "update-banner-line").as_deref(),
        Some("Installing Locust 0.406.0. It opens again by itself in a minute or two; there is no need to open it.")
    );
    assert_eq!(text_of(&rt, "update-banner-refusal"), None);
}

/// Not while a turn runs (Electron App.tsx draws `UpdateBanner` only when `!running`); there again once it ends.
#[test]
fn is_not_drawn_while_a_turn_runs_and_is_there_again_the_moment_it_ends() {
    let mut rt = window(ready_service(Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0))));
    rt.update(|s, _| {
        s.flight = Some(Flight {
            teammate_id: "tm_codec".into(),
            conversation_id: None,
            header: None,
            after_tip: None,
            message: None,
        });
    });
    tick(&mut rt);
    assert!(rt.find("composer").is_some(), "the conversation's composer is on screen");
    assert!(rt.find("update-banner").is_none(), "no banner while Codec's turn runs: {:?}", texts(&rt));
    assert!(!texts(&rt).iter().any(|t| t == "Restart and install"), "{:?}", texts(&rt));

    rt.update(|s, _| s.flight = None);
    tick(&mut rt);
    assert!(rt.find("update-banner").is_some(), "the banner is back once the turn ends: {:?}", texts(&rt));
    assert_eq!(
        text_of(&rt, "update-banner-line").as_deref(),
        Some("Locust 0.406.0 is downloaded and ready. Locust closes while it installs and opens again by itself in a minute or two.")
    );
}

/// A second press of Restart and install while it installs asks the service for nothing more.
#[test]
fn a_second_press_while_it_installs_asks_for_nothing_more() {
    let quits = Arc::new(AtomicUsize::new(0));
    let mut rt = window(ready_service(Arc::new(AtomicUsize::new(0)), quits.clone()));
    tick(&mut rt);
    for _ in 0..2 {
        let (x, y) = rt.find("update-banner-install").expect("Restart and install");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        tick(&mut rt);
    }
    assert_eq!(quits.load(Ordering::SeqCst), 1, "the service is asked to install once");
    assert_eq!(
        text_of(&rt, "update-banner-line").as_deref(),
        Some("Installing Locust 0.406.0. It opens again by itself in a minute or two; there is no need to open it.")
    );
}

/// Nothing until the version is downloaded and ready (Electron `UpdateBanner`: `update.phase !== 'ready'` draws nothing).
#[test]
fn says_nothing_until_the_version_is_downloaded_and_ready() {
    let at = |phase: AppUpdatePhase| {
        window_with(move |s| {
            s.settings_panel.update = AppUpdateState {
                phase,
                current_version: "0.405.0".into(),
                available_version: Some("0.406.0".into()),
                ..AppUpdateState::default()
            };
        })
    };
    for phase in [
        AppUpdatePhase::Idle,
        AppUpdatePhase::Checking,
        AppUpdatePhase::Current,
        AppUpdatePhase::Available,
        AppUpdatePhase::Downloading,
        AppUpdatePhase::Failed,
        AppUpdatePhase::Unsupported,
    ] {
        let rt = at(phase);
        assert!(rt.find("composer").is_some(), "the conversation's composer is on screen");
        assert!(rt.find("update-banner").is_none(), "no banner at {phase:?}: {:?}", texts(&rt));
        assert_eq!(text_of(&rt, "update-banner-line"), None, "no line at {phase:?}");
    }
    let rt = at(AppUpdatePhase::Ready);
    assert_eq!(
        text_of(&rt, "update-banner-line").as_deref(),
        Some("Locust 0.406.0 is downloaded and ready. Locust closes while it installs and opens again by itself in a minute or two.")
    );
}
