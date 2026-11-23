//! The self-update switch: the Mac self-updater (`mac_self_update.rs`, Electron `main/mac-self-update.ts`) runs in
//! `NoFeedUpdater`'s place only when switched on with a feed, and off it is never even made.
use super::*;
use crate::mac_self_update::{MacUpdater, MacUpdaterOptions, create_mac_updater};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;

fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<String, String> = pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
    move |name| map.get(name).cloned()
}

fn idle() -> (LiveMissionCountFn, RequestQuitFn) {
    (Arc::new(|| 0), Arc::new(|_| {}))
}

/// A self-updater on a fake feed: every fetch counted, nothing downloaded, run or swapped (those seams are unset).
fn fake_mac(fetches: Arc<AtomicUsize>) -> MacUpdater {
    let cache = std::env::temp_dir().join(format!("locust-switch-test-{}", std::process::id()));
    create_mac_updater(MacUpdaterOptions {
        current_version: "0.516.0".into(),
        arch: "arm64".into(),
        bundle: PathBuf::from("/Applications/Locust.app"),
        cache_folder: cache,
        quit: Arc::new(|| {}),
        pid: None,
        fetch_releases: Some(Arc::new(move || {
            fetches.fetch_add(1, Ordering::SeqCst);
            Ok(serde_json::json!([{
                "tag_name": "0.517.0",
                "assets": [{
                    "name": "Locust-0.517.0-mac-arm64.dmg",
                    "browser_download_url": "https://dl.locust.lol/Locust-0.517.0-mac-arm64.dmg",
                    "size": 4,
                    "digest": "sha256:00",
                }]
            }]))
        })),
        download: None,
        run: None,
        start_swap: None,
        test_image: None,
        relaunch: Vec::new(),
    })
}

#[test]
fn the_switch_is_off_unless_switched_on_with_a_feed() {
    assert_eq!(self_update_feed(env(&[])), None, "off by default");
    assert_eq!(self_update_feed(env(&[(SELF_UPDATE_SWITCH, "1")])), None, "on with no feed set");
    assert_eq!(self_update_feed(env(&[(SELF_UPDATE_SWITCH, "1"), (SELF_UPDATE_FEED, "  ")])), None, "an empty feed");
    assert_eq!(self_update_feed(env(&[(SELF_UPDATE_FEED, "https://feed.test/r.json")])), None, "a feed, switch off");
    assert_eq!(self_update_feed(env(&[(SELF_UPDATE_SWITCH, "0"), (SELF_UPDATE_FEED, "https://feed.test/r.json")])), None);
    assert_eq!(
        self_update_feed(env(&[(SELF_UPDATE_SWITCH, "1"), (SELF_UPDATE_FEED, " https://feed.test/r.json ")])).as_deref(),
        Some("https://feed.test/r.json")
    );
}

#[test]
fn switched_off_the_service_never_makes_the_self_updater_and_checks_nothing() {
    let made = Arc::new(AtomicUsize::new(0));
    let fetches = Arc::new(AtomicUsize::new(0));
    let (live, quit) = idle();
    let (m, f) = (made.clone(), fetches.clone());
    let service = app_update_service(None, move |_| {
        m.fetch_add(1, Ordering::SeqCst);
        Some(fake_mac(f))
    }, "0.516.0", live, quit);
    let _ = service.check();
    assert_eq!(made.load(Ordering::SeqCst), 0, "no self-updater is made while the switch is off");
    assert_eq!(fetches.load(Ordering::SeqCst), 0);
    let state = service.state();
    assert_eq!(state.phase, AppUpdatePhase::Unsupported);
    assert_eq!(state.held_back, Some(HeldBack::NoFeed));
    assert_eq!(update_control_line(Some(&state), false), held_back_line(Some(HeldBack::NoFeed)));
}

#[test]
fn switched_on_with_a_feed_the_self_updater_takes_no_feeds_place() {
    let asked = Arc::new(Mutex::new(Vec::<String>::new()));
    let fetches = Arc::new(AtomicUsize::new(0));
    let (live, quit) = idle();
    let (a, f) = (asked.clone(), fetches.clone());
    let service = app_update_service(Some("https://feed.test/r.json"), move |feed| {
        a.lock().unwrap().push(feed.to_owned());
        Some(fake_mac(f))
    }, "0.516.0", live, quit);
    assert_eq!(*asked.lock().unwrap(), vec!["https://feed.test/r.json".to_owned()]);
    assert_ne!(service.state().phase, AppUpdatePhase::Unsupported);
    let _ = service.check();
    assert_eq!(fetches.load(Ordering::SeqCst), 1, "the check read the switched-on feed");
    assert_eq!(service.state().available_version.as_deref(), Some("0.517.0"));
    assert_ne!(service.state().phase, AppUpdatePhase::Ready, "nothing was downloaded: the download seam is unset");
}

#[test]
fn a_copy_the_self_updater_cannot_replace_stays_on_no_feed() {
    let (live, quit) = idle();
    let service = app_update_service(Some("https://feed.test/r.json"), |_| None::<MacUpdater>, "0.516.0", live, quit);
    assert_eq!(service.state().phase, AppUpdatePhase::Unsupported);
    assert_eq!(service.state().held_back, Some(HeldBack::NoFeed));
}
