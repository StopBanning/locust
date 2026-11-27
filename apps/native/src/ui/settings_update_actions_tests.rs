use super::*;
use locust_core::updates::{AppUpdateState, HeldBack};
use std::sync::atomic::{AtomicUsize, Ordering};

fn settings_with(update: AppUpdateState, service: Option<locust_core::updates::UpdateService<Box<dyn UpdaterLike>>>) -> rune::Runtime<State> {
    let mut s = State::default();
    scene(&mut s);
    s.settings_panel.page = Some(SettingsPageId::App);
    s.settings_panel.update = update;
    s.settings_panel.update_service = service;
    general_window(s)
}

fn service(supported: bool, held_back: Option<HeldBack>, quits: Arc<AtomicUsize>, live: Arc<AtomicUsize>)
    -> (SharedWindowUpdater, locust_core::updates::UpdateService<Box<dyn UpdaterLike>>)
{
    let fake = SharedWindowUpdater::new("0.406.0", None);
    let svc = create_update_service(UpdateServiceOptions {
        updater: Box::new(fake.clone()) as Box<dyn UpdaterLike>,
        current_version: "0.405.0".into(),
        supported,
        held_back,
        check_only: false,
        package_url: None,
        live_mission_count: Arc::new(move || live.load(Ordering::SeqCst)),
        request_quit: Arc::new(move |then| {
            assert!(then.is_some(), "request quit with the install callback");
            quits.fetch_add(1, Ordering::SeqCst);
        }),
        on_state_change: None,
        every_build: false,
    });
    (fake, svc)
}

fn ready_window(live: Arc<AtomicUsize>, quits: Arc<AtomicUsize>) -> rune::Runtime<State> {
    let (updater, service) = service(true, None, quits, live);
    let _ = service.check();
    updater.emit_downloaded("0.406.0");
    assert_eq!(service.state().phase, AppUpdatePhase::Ready);
    settings_with(service.state(), Some(service))
}

fn focusable(rt: &rune::Runtime<State>, key: &str) -> bool {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).expect(key).focusable
}

// Settings' install control, not the banner tested in update_banner_tests.rs.
#[test]
fn settings_install_requests_one_restart_only_after_the_running_turn_ends() {
    let live = Arc::new(AtomicUsize::new(1));
    let quits = Arc::new(AtomicUsize::new(0));
    let mut rt = ready_window(live.clone(), quits.clone());
    press(&mut rt, "settings-update-install");
    assert_eq!(quits.load(Ordering::SeqCst), 0);
    assert_eq!(text_of(&rt, "settings-update-refusal").as_deref(),
        Some("A mission is still running. Stop it first, then install — restarting now would cut it off mid-run."));
    live.store(0, Ordering::SeqCst);
    press(&mut rt, "settings-update-install");
    assert_eq!(quits.load(Ordering::SeqCst), 1, "the real Settings press requests one restart");
    assert_eq!(text_of(&rt, "settings-update-refusal"), None);
    assert_eq!(text_of(&rt, "settings-update-line").as_deref(),
        Some("Installing Locust 0.406.0. It opens again by itself in a minute or two; there is no need to open it."));
    assert!(!focusable(&rt, "settings-update-install"), "the install button is disabled during the restart");
    press(&mut rt, "settings-update-install");
    assert_eq!(quits.load(Ordering::SeqCst), 1, "a second press must not request another restart");
    // A callback queued against the old frame also cannot ask twice.
    install_ready_update(&mut rt.state);
    assert_eq!(quits.load(Ordering::SeqCst), 1, "a stale press must not request another restart");
}

#[test]
fn settings_refuses_a_stale_ready_display_when_the_service_is_gone() {
    let mut rt = settings_with(AppUpdateState { phase: AppUpdatePhase::Ready, available_version: Some("0.406.0".into()),
        ..AppUpdateState::default() }, None);
    press(&mut rt, "settings-update-install");
    assert!(!rt.state.settings_panel.update_installing, "nothing installed without its service");
    assert_eq!(text_of(&rt, "settings-update-refusal").as_deref(), Some("There is no downloaded update to install yet."));
}
