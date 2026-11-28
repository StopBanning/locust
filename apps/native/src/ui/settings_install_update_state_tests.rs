use super::*;
use locust_core::updates::{
    AppUpdatePhase, AppUpdateState, CheckForUpdatesResult, EventListener, EventPayload,
    UpdateInfo, UpdateServiceOptions, UpdaterLike, create_update_service,
};
use rune::{Env, Runtime};
use std::sync::{Arc, Mutex, atomic::{AtomicUsize, Ordering}};

#[derive(Default)]
struct InnerUpdater {
    auto_download: bool,
    allow_prerelease: bool,
    auto_install_on_app_quit: bool,
    downloaded: Option<EventListener>,
}

#[derive(Clone, Default)]
struct FakeUpdater(Arc<Mutex<InnerUpdater>>);

impl FakeUpdater {
    fn emit_downloaded(&self, version: &str) {
        if let Some(cb) = self.0.lock().unwrap().downloaded.clone() {
            cb(Some(EventPayload::version(version)));
        }
    }
}

impl UpdaterLike for FakeUpdater {
    fn auto_download(&self) -> bool { self.0.lock().unwrap().auto_download }
    fn set_auto_download(&mut self, value: bool) { self.0.lock().unwrap().auto_download = value; }
    fn allow_prerelease(&self) -> bool { self.0.lock().unwrap().allow_prerelease }
    fn set_allow_prerelease(&mut self, value: bool) { self.0.lock().unwrap().allow_prerelease = value; }
    fn auto_install_on_app_quit(&self) -> bool { self.0.lock().unwrap().auto_install_on_app_quit }
    fn set_auto_install_on_app_quit(&mut self, value: bool) { self.0.lock().unwrap().auto_install_on_app_quit = value; }
    fn check_for_updates(&mut self) -> Result<Option<CheckForUpdatesResult>, String> {
        Ok(Some(CheckForUpdatesResult {
            update_info: UpdateInfo { version: "0.406.0".into(), files: None },
            is_update_available: Some(true),
            download_result: None,
        }))
    }
    fn download_update(&mut self) -> Result<(), String> { Ok(()) }
    fn quit_and_install(&mut self, _: Option<bool>, _: Option<bool>) {}
    fn on(&mut self, event: &str, listener: EventListener) {
        if event == "update-downloaded" {
            self.0.lock().unwrap().downloaded = Some(listener);
        }
    }
}

#[test]
fn settings_install_updates_displayed_version_from_service_response() {
    let updater = FakeUpdater::default();
    let quits = Arc::new(AtomicUsize::new(0));
    let quits_cb = quits.clone();
    let svc = create_update_service(UpdateServiceOptions {
        updater: Box::new(updater.clone()) as Box<dyn UpdaterLike>,
        current_version: "0.405.0".into(),
        supported: true,
        held_back: None,
        check_only: false,
        package_url: None,
        live_mission_count: Arc::new(|| 0),
        request_quit: Arc::new(move |_| { quits_cb.fetch_add(1, Ordering::SeqCst); }),
        on_state_change: None,
        every_build: false,
    });
    let _ = svc.check();
    updater.emit_downloaded("0.406.0");
    assert_eq!(svc.state().phase, AppUpdatePhase::Ready);

    // Settings still shows an older cached Ready version while the service holds 0.406.0.
    let mut s = State::default();
    scene(&mut s);
    s.settings_panel.page = Some(SettingsPageId::App);
    s.settings_panel.update = AppUpdateState {
        phase: AppUpdatePhase::Ready,
        current_version: "0.405.0".into(),
        available_version: Some("0.405.1".into()),
        held_back: None,
        ..AppUpdateState::default()
    };
    s.settings_panel.update_service = Some(svc);

    let mut rt = Runtime::new(s, |s, _| view(s, &Palette::new(true)), (1200., 5000.), Env::new(true, 1.));
    for _ in 0..8 {
        rt.frame(0.05);
    }

    let (x, y) = rt.find("settings-update-install").expect("Install and restart button");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..8 {
        rt.frame(0.05);
    }

    assert_eq!(quits.load(Ordering::SeqCst), 1);
    assert_eq!(rt.state.settings_panel.update.available_version.as_deref(), Some("0.406.0"));
    let line = rt
        .elements()
        .into_iter()
        .find(|el| !el.exiting && el.key.as_deref() == Some("settings-update-line"))
        .and_then(|el| el.text)
        .expect("settings-update-line");
    assert_eq!(
        line,
        "Installing Locust 0.406.0. It opens again by itself in a minute or two; there is no need to open it."
    );
}
