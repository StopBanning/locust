//! Updates (Electron `main/updates.ts`).
//!
//! The rules this follows, and why:
//! - It checks automatically and installs only when asked. A desktop agent can be holding a running mission and a
//!   durable ledger writer; swapping the binary underneath that is not something to do while someone is away from the
//!   keyboard.
//! - It never installs while a mission is running.
//! - It says what it knows and no more. "Up to date" is only said after a check actually completed; a check that could
//!   not run says so.
//! - The Rust app has no release feed of its own yet, so its default updater (`NoFeedUpdater`) never reaches the
//!   network and says `"This copy has no updates to look for yet"`.

use std::sync::{Arc, Mutex};

pub const DOWNLOAD_FAILED_MESSAGE: &str =
    "The update could not be downloaded. Locust will try again at its next check.";
pub const CHECK_FAILED_MESSAGE: &str = "The update check could not complete.";
pub const NOT_READY_MESSAGE: &str = "There is no downloaded update to install yet.";
pub const BUSY_MESSAGE: &str =
    "A mission is still running. Stop it first, then install — restarting now would cut it off mid-run.";
pub const NO_FEED_MESSAGE: &str = "This copy has no updates to look for yet";

/// Where an update stands (`shared/ipc.ts` `AppUpdatePhase`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppUpdatePhase {
    Idle,
    Checking,
    Current,
    Available,
    Downloading,
    Ready,
    Failed,
    Unsupported,
}

impl AppUpdatePhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Checking => "checking",
            Self::Current => "current",
            Self::Available => "available",
            Self::Downloading => "downloading",
            Self::Ready => "ready",
            Self::Failed => "failed",
            Self::Unsupported => "unsupported",
        }
    }
}

/// Why a copy is in the `Unsupported` phase (`shared/ipc.ts` `heldBack`, plus `NoFeed` when no feed exists yet).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeldBack {
    Development,
    Driven,
    Copy,
    NoFeed,
}

impl HeldBack {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Driven => "driven",
            Self::Copy => "copy",
            Self::NoFeed => "no-feed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppUpdateState {
    pub phase: AppUpdatePhase,
    pub current_version: String,
    pub every_build: bool,
    pub available_version: Option<String>,
    pub download_url: Option<String>,
    pub percent: Option<u8>,
    pub message: Option<String>,
    pub held_back: Option<HeldBack>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppUpdateErrorCode {
    UpdateNotReady,
    UpdateBusy,
    InternalError,
}

impl AppUpdateErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UpdateNotReady => "UPDATE_NOT_READY",
            Self::UpdateBusy => "UPDATE_BUSY",
            Self::InternalError => "INTERNAL_ERROR",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppUpdateError {
    pub code: AppUpdateErrorCode,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppUpdateResponse {
    Ok(AppUpdateState),
    Err(AppUpdateError),
}

impl AppUpdateResponse {
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok(_))
    }

    pub fn data(&self) -> Option<&AppUpdateState> {
        match self {
            Self::Ok(state) => Some(state),
            Self::Err(_) => None,
        }
    }

    pub fn error(&self) -> Option<&AppUpdateError> {
        match self {
            Self::Ok(_) => None,
            Self::Err(err) => Some(err),
        }
    }
}

/// One entry in the feed's file list (`updateInfo.files`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateFile {
    pub url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateInfo {
    pub version: String,
    pub files: Option<Vec<UpdateFile>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckForUpdatesResult {
    pub update_info: UpdateInfo,
    pub is_update_available: Option<bool>,
    /// Outcome of the background download `autoDownload` started, if any.
    pub download_result: Option<Result<(), String>>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EventPayload {
    pub version: Option<String>,
    pub percent: Option<f64>,
    pub error: Option<String>,
}

impl EventPayload {
    pub fn version(v: impl Into<String>) -> Self {
        Self {
            version: Some(v.into()),
            percent: None,
            error: None,
        }
    }

    pub fn percent(p: f64) -> Self {
        Self {
            version: None,
            percent: Some(p),
            error: None,
        }
    }

    pub fn error(e: impl Into<String>) -> Self {
        Self {
            version: None,
            percent: None,
            error: Some(e.into()),
        }
    }
}

pub type EventListener = Arc<dyn Fn(Option<EventPayload>) + Send + Sync>;

/// What an updater must provide (`UpdaterLike` in `main/updates.ts`). Injected so the service is testable and never
/// touches a real release feed unless given a fake in tests.
pub trait UpdaterLike: Send + Sync {
    fn auto_download(&self) -> bool;
    fn set_auto_download(&mut self, value: bool);

    fn allow_prerelease(&self) -> bool;
    fn set_allow_prerelease(&mut self, value: bool);

    fn channel(&self) -> Option<String> {
        None
    }
    fn set_channel(&mut self, _value: Option<&str>) {}

    fn allow_downgrade(&self) -> Option<bool> {
        None
    }
    fn set_allow_downgrade(&mut self, _value: bool) {}

    fn auto_install_on_app_quit(&self) -> bool;
    fn set_auto_install_on_app_quit(&mut self, value: bool);

    fn check_for_updates(&mut self) -> Result<Option<CheckForUpdatesResult>, String>;
    fn download_update(&mut self) -> Result<(), String>;
    fn quit_and_install(&mut self, is_silent: Option<bool>, is_force_run_after: Option<bool>);
    fn on(&mut self, event: &str, listener: EventListener);
}

impl<U: UpdaterLike + ?Sized> UpdaterLike for Arc<Mutex<U>> {
    fn auto_download(&self) -> bool {
        self.lock().unwrap_or_else(|e| e.into_inner()).auto_download()
    }
    fn set_auto_download(&mut self, value: bool) {
        self.lock().unwrap_or_else(|e| e.into_inner()).set_auto_download(value);
    }
    fn allow_prerelease(&self) -> bool {
        self.lock().unwrap_or_else(|e| e.into_inner()).allow_prerelease()
    }
    fn set_allow_prerelease(&mut self, value: bool) {
        self.lock().unwrap_or_else(|e| e.into_inner()).set_allow_prerelease(value);
    }
    fn channel(&self) -> Option<String> {
        self.lock().unwrap_or_else(|e| e.into_inner()).channel()
    }
    fn set_channel(&mut self, value: Option<&str>) {
        self.lock().unwrap_or_else(|e| e.into_inner()).set_channel(value);
    }
    fn allow_downgrade(&self) -> Option<bool> {
        self.lock().unwrap_or_else(|e| e.into_inner()).allow_downgrade()
    }
    fn set_allow_downgrade(&mut self, value: bool) {
        self.lock().unwrap_or_else(|e| e.into_inner()).set_allow_downgrade(value);
    }
    fn auto_install_on_app_quit(&self) -> bool {
        self.lock().unwrap_or_else(|e| e.into_inner()).auto_install_on_app_quit()
    }
    fn set_auto_install_on_app_quit(&mut self, value: bool) {
        self.lock().unwrap_or_else(|e| e.into_inner()).set_auto_install_on_app_quit(value);
    }
    fn check_for_updates(&mut self) -> Result<Option<CheckForUpdatesResult>, String> {
        self.lock().unwrap_or_else(|e| e.into_inner()).check_for_updates()
    }
    fn download_update(&mut self) -> Result<(), String> {
        self.lock().unwrap_or_else(|e| e.into_inner()).download_update()
    }
    fn quit_and_install(&mut self, is_silent: Option<bool>, is_force_run_after: Option<bool>) {
        self.lock()
            .unwrap_or_else(|e| e.into_inner())
            .quit_and_install(is_silent, is_force_run_after);
    }
    fn on(&mut self, event: &str, listener: EventListener) {
        self.lock().unwrap_or_else(|e| e.into_inner()).on(event, listener);
    }
}

/// The Rust app's default updater: has no release feed and never touches the network or disk.
#[derive(Clone, Debug, Default)]
pub struct NoFeedUpdater {
    pub auto_download: bool,
    pub allow_prerelease: bool,
    pub channel: Option<String>,
    pub allow_downgrade: Option<bool>,
    pub auto_install_on_app_quit: bool,
}

impl UpdaterLike for NoFeedUpdater {
    fn auto_download(&self) -> bool {
        self.auto_download
    }
    fn set_auto_download(&mut self, value: bool) {
        self.auto_download = value;
    }
    fn allow_prerelease(&self) -> bool {
        self.allow_prerelease
    }
    fn set_allow_prerelease(&mut self, value: bool) {
        self.allow_prerelease = value;
    }
    fn channel(&self) -> Option<String> {
        self.channel.clone()
    }
    fn set_channel(&mut self, value: Option<&str>) {
        self.channel = value.map(str::to_owned);
    }
    fn allow_downgrade(&self) -> Option<bool> {
        self.allow_downgrade
    }
    fn set_allow_downgrade(&mut self, value: bool) {
        self.allow_downgrade = Some(value);
    }
    fn auto_install_on_app_quit(&self) -> bool {
        self.auto_install_on_app_quit
    }
    fn set_auto_install_on_app_quit(&mut self, value: bool) {
        self.auto_install_on_app_quit = value;
    }
    fn check_for_updates(&mut self) -> Result<Option<CheckForUpdatesResult>, String> {
        Err(NO_FEED_MESSAGE.to_owned())
    }
    fn download_update(&mut self) -> Result<(), String> {
        Err(NO_FEED_MESSAGE.to_owned())
    }
    fn quit_and_install(&mut self, _is_silent: Option<bool>, _is_force_run_after: Option<bool>) {}
    fn on(&mut self, _event: &str, _listener: EventListener) {}
}

fn parse_semver_prefix(text: &str) -> Option<[u64; 3]> {
    let trimmed = text.trim();
    let rest = trimmed.strip_prefix('v').unwrap_or(trimmed);
    let mut parts = rest.splitn(3, '.');
    let major: u64 = parts.next()?.parse().ok()?;
    let minor: u64 = parts.next()?.parse().ok()?;
    let third = parts.next()?;
    let digits_len = third.bytes().take_while(u8::is_ascii_digit).count();
    if digits_len == 0 {
        return None;
    }
    let patch: u64 = third[..digits_len].parse().ok()?;
    Some([major, minor, patch])
}

/// `a` is a later version than `b`, by major.minor.patch (`isNewer` in `main/updates.ts`).
pub fn is_newer(a: &str, b: &str) -> bool {
    let (Some(x), Some(y)) = (parse_semver_prefix(a), parse_semver_prefix(b)) else {
        return a != b;
    };
    for index in 0..3 {
        if x[index] != y[index] {
            return x[index] > y[index];
        }
    }
    false
}

pub type PackageUrlFn = Arc<dyn Fn(Option<&[UpdateFile]>) -> Option<String> + Send + Sync>;
pub type LiveMissionCountFn = Arc<dyn Fn() -> usize + Send + Sync>;
pub type RequestQuitFn = Arc<dyn Fn(Option<Box<dyn FnOnce() + Send>>) + Send + Sync>;
pub type OnStateChangeFn = Arc<dyn Fn(&AppUpdateState) + Send + Sync>;

pub struct UpdateServiceOptions<U: UpdaterLike> {
    pub updater: U,
    pub current_version: String,
    pub supported: bool,
    pub held_back: Option<HeldBack>,
    pub check_only: bool,
    pub package_url: Option<PackageUrlFn>,
    pub live_mission_count: LiveMissionCountFn,
    pub request_quit: RequestQuitFn,
    pub on_state_change: Option<OnStateChangeFn>,
    pub every_build: bool,
}

struct ServiceInner {
    state: AppUpdateState,
    every_build: bool,
    downloaded_version: Option<String>,
}

#[derive(Clone)]
pub struct UpdateService<U: UpdaterLike> {
    updater: Arc<Mutex<U>>,
    current_version: String,
    supported: bool,
    held_back: Option<HeldBack>,
    package_url: Option<PackageUrlFn>,
    live_mission_count: LiveMissionCountFn,
    request_quit: RequestQuitFn,
    on_state_change: Option<OnStateChangeFn>,
    inner: Arc<Mutex<ServiceInner>>,
}

pub fn create_update_service<U: UpdaterLike + 'static>(
    mut options: UpdateServiceOptions<U>,
) -> UpdateService<U> {
    let every_build = options.every_build;
    options.updater.set_allow_prerelease(every_build);
    options
        .updater
        .set_channel(Some(if every_build { "beta" } else { "latest" }));
    options.updater.set_allow_downgrade(false);

    let initial_state = AppUpdateState {
        phase: if options.supported {
            AppUpdatePhase::Idle
        } else {
            AppUpdatePhase::Unsupported
        },
        current_version: options.current_version.clone(),
        every_build,
        available_version: None,
        download_url: None,
        percent: None,
        message: None,
        held_back: if options.supported { None } else { options.held_back },
    };

    let inner = Arc::new(Mutex::new(ServiceInner {
        state: initial_state,
        every_build,
        downloaded_version: None,
    }));

    let may_install = options.supported && !options.check_only && options.package_url.is_none();
    if !may_install {
        options.updater.set_auto_download(false);
        options.updater.set_auto_install_on_app_quit(false);
    }
    if may_install {
        options.updater.set_auto_download(true);
        options.updater.set_auto_install_on_app_quit(true);

        {
            let inner = Arc::clone(&inner);
            let current_version = options.current_version.clone();
            let on_state_change = options.on_state_change.clone();
            options.updater.on(
                "download-progress",
                Arc::new(move |payload| {
                    let next = {
                        let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
                        if guard.state.phase != AppUpdatePhase::Downloading
                            && guard.state.phase != AppUpdatePhase::Available
                        {
                            return;
                        }
                        let percent = payload
                            .and_then(|p| p.percent)
                            .map(|p| (p.round() as i64).clamp(0, 100) as u8);
                        let next = AppUpdateState {
                            phase: AppUpdatePhase::Downloading,
                            current_version: current_version.clone(),
                            every_build: guard.every_build,
                            available_version: guard.state.available_version.clone(),
                            download_url: None,
                            percent,
                            message: None,
                            held_back: None,
                        };
                        guard.state = next.clone();
                        next
                    };
                    if let Some(cb) = &on_state_change {
                        cb(&next);
                    }
                }),
            );
        }

        {
            let inner = Arc::clone(&inner);
            let current_version = options.current_version.clone();
            let on_state_change = options.on_state_change.clone();
            options.updater.on(
                "update-downloaded",
                Arc::new(move |payload| {
                    let next = {
                        let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
                        let version = payload
                            .and_then(|p| p.version)
                            .or_else(|| guard.state.available_version.clone());
                        guard.downloaded_version = version.clone();
                        let next = AppUpdateState {
                            phase: AppUpdatePhase::Ready,
                            current_version: current_version.clone(),
                            every_build: guard.every_build,
                            available_version: version,
                            download_url: None,
                            percent: None,
                            message: None,
                            held_back: None,
                        };
                        guard.state = next.clone();
                        next
                    };
                    if let Some(cb) = &on_state_change {
                        cb(&next);
                    }
                }),
            );
        }

        {
            let inner = Arc::clone(&inner);
            let current_version = options.current_version.clone();
            let on_state_change = options.on_state_change.clone();
            options.updater.on(
                "error",
                Arc::new(move |_payload| {
                    let next = {
                        let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
                        if let Some(version) = guard.downloaded_version.clone() {
                            let next = AppUpdateState {
                                phase: AppUpdatePhase::Ready,
                                current_version: current_version.clone(),
                                every_build: guard.every_build,
                                available_version: Some(version),
                                download_url: None,
                                percent: None,
                                message: None,
                                held_back: None,
                            };
                            guard.state = next.clone();
                            next
                        } else {
                            let downloading = guard.state.phase == AppUpdatePhase::Available
                                || guard.state.phase == AppUpdatePhase::Downloading;
                            let next = AppUpdateState {
                                phase: AppUpdatePhase::Failed,
                                current_version: current_version.clone(),
                                every_build: guard.every_build,
                                available_version: None,
                                download_url: None,
                                percent: None,
                                message: Some(
                                    if downloading {
                                        DOWNLOAD_FAILED_MESSAGE
                                    } else {
                                        CHECK_FAILED_MESSAGE
                                    }
                                    .to_owned(),
                                ),
                                held_back: None,
                            };
                            guard.state = next.clone();
                            next
                        }
                    };
                    if let Some(cb) = &on_state_change {
                        cb(&next);
                    }
                }),
            );
        }
    }

    UpdateService {
        updater: Arc::new(Mutex::new(options.updater)),
        current_version: options.current_version,
        supported: options.supported,
        held_back: options.held_back,
        package_url: options.package_url,
        live_mission_count: options.live_mission_count,
        request_quit: options.request_quit,
        on_state_change: options.on_state_change,
        inner,
    }
}

impl<U: UpdaterLike + 'static> UpdateService<U> {
    fn publish(&self, mut next: AppUpdateState) -> AppUpdateState {
        {
            let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            next.every_build = guard.every_build;
            guard.state = next.clone();
        }
        if let Some(cb) = &self.on_state_change {
            cb(&next);
        }
        next
    }

    pub fn state(&self) -> AppUpdateState {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).state.clone()
    }

    pub fn set_every_build(&self, next: bool) -> AppUpdateState {
        {
            let mut u = self.updater.lock().unwrap_or_else(|e| e.into_inner());
            u.set_allow_prerelease(next);
            u.set_channel(Some(if next { "beta" } else { "latest" }));
            u.set_allow_downgrade(false);
        }
        let state = {
            let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            guard.every_build = next;
            guard.state.clone()
        };
        self.publish(state)
    }

    pub(crate) fn look(&self, quiet: bool) -> AppUpdateResponse {
        if !self.supported {
            return AppUpdateResponse::Ok(self.publish(AppUpdateState {
                phase: AppUpdatePhase::Unsupported,
                current_version: self.current_version.clone(),
                every_build: false,
                available_version: None,
                download_url: None,
                percent: None,
                message: None,
                held_back: self.held_back,
            }));
        }
        if !quiet {
            self.publish(AppUpdateState {
                phase: AppUpdatePhase::Checking,
                current_version: self.current_version.clone(),
                every_build: false,
                available_version: None,
                download_url: None,
                percent: None,
                message: None,
                held_back: None,
            });
        }
        let checked = self
            .updater
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .check_for_updates();
        match checked {
            Ok(result) => {
                if let Some(res) = &result {
                    let _ = &res.download_result;
                }
                let version = result.as_ref().map(|r| r.update_info.version.as_str());
                let is_available = result.as_ref().and_then(|r| r.is_update_available);
                let Some(version) = version else {
                    return AppUpdateResponse::Ok(self.publish(AppUpdateState {
                        phase: AppUpdatePhase::Current,
                        current_version: self.current_version.clone(),
                        every_build: false,
                        available_version: None,
                        download_url: None,
                        percent: None,
                        message: None,
                        held_back: None,
                    }));
                };
                if is_available == Some(false) || !is_newer(version, &self.current_version) {
                    return AppUpdateResponse::Ok(self.publish(AppUpdateState {
                        phase: AppUpdatePhase::Current,
                        current_version: self.current_version.clone(),
                        every_build: false,
                        available_version: None,
                        download_url: None,
                        percent: None,
                        message: None,
                        held_back: None,
                    }));
                }
                let on_disk = {
                    let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                    let matches = guard.downloaded_version.as_deref() == Some(version);
                    if !matches {
                        guard.downloaded_version = None;
                    }
                    matches
                };
                let files = result.as_ref().and_then(|r| r.update_info.files.as_deref());
                let download_url = self.package_url.as_ref().and_then(|f| f(files));
                AppUpdateResponse::Ok(self.publish(AppUpdateState {
                    phase: if on_disk {
                        AppUpdatePhase::Ready
                    } else {
                        AppUpdatePhase::Available
                    },
                    current_version: self.current_version.clone(),
                    every_build: false,
                    available_version: Some(version.to_owned()),
                    download_url,
                    percent: None,
                    message: None,
                    held_back: None,
                }))
            }
            Err(_) => {
                let downloaded = self
                    .inner
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .downloaded_version
                    .clone();
                if let Some(version) = downloaded {
                    return AppUpdateResponse::Ok(self.publish(AppUpdateState {
                        phase: AppUpdatePhase::Ready,
                        current_version: self.current_version.clone(),
                        every_build: false,
                        available_version: Some(version),
                        download_url: None,
                        percent: None,
                        message: None,
                        held_back: None,
                    }));
                }
                AppUpdateResponse::Ok(self.publish(AppUpdateState {
                    phase: AppUpdatePhase::Failed,
                    current_version: self.current_version.clone(),
                    every_build: false,
                    available_version: None,
                    download_url: None,
                    percent: None,
                    message: Some(CHECK_FAILED_MESSAGE.to_owned()),
                    held_back: None,
                }))
            }
        }
    }

    pub fn check(&self) -> AppUpdateResponse {
        self.look(false)
    }

    pub fn hold_install_for_relaunch(&self) {
        self.updater
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .set_auto_install_on_app_quit(false);
    }

    pub fn install(&self) -> AppUpdateResponse {
        let current = self.state();
        if current.phase != AppUpdatePhase::Ready {
            return AppUpdateResponse::Err(AppUpdateError {
                code: AppUpdateErrorCode::UpdateNotReady,
                message: NOT_READY_MESSAGE.to_owned(),
            });
        }
        if (self.live_mission_count)() > 0 {
            return AppUpdateResponse::Err(AppUpdateError {
                code: AppUpdateErrorCode::UpdateBusy,
                message: BUSY_MESSAGE.to_owned(),
            });
        }
        let updater = Arc::clone(&self.updater);
        (self.request_quit)(Some(Box::new(move || {
            updater
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .quit_and_install(Some(true), Some(true));
        })));
        AppUpdateResponse::Ok(current)
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::collections::HashMap;

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct InstallCall {
        pub is_silent: Option<bool>,
        pub is_force_run_after: Option<bool>,
    }

    type CheckFn = Box<dyn FnMut() -> Result<Option<CheckForUpdatesResult>, String> + Send + Sync>;

    struct FakeUpdaterState {
        auto_download: bool,
        allow_prerelease: bool,
        channel: Option<String>,
        allow_downgrade: Option<bool>,
        channel_sets_downgrade: bool,
        auto_install_on_app_quit: bool,
        listeners: HashMap<String, Vec<EventListener>>,
        installs: Vec<InstallCall>,
        quits: Vec<usize>,
        check_calls: usize,
        check_fn: CheckFn,
    }

    #[derive(Clone)]
    pub struct FakeUpdater {
        state: Arc<Mutex<FakeUpdaterState>>,
    }

    impl Default for FakeUpdater {
        fn default() -> Self {
            Self::new()
        }
    }

    impl FakeUpdater {
        pub fn new() -> Self {
            Self {
                state: Arc::new(Mutex::new(FakeUpdaterState {
                    auto_download: false,
                    allow_prerelease: false,
                    channel: None,
                    allow_downgrade: None,
                    channel_sets_downgrade: false,
                    auto_install_on_app_quit: true,
                    listeners: HashMap::new(),
                    installs: Vec::new(),
                    quits: Vec::new(),
                    check_calls: 0,
                    check_fn: Box::new(|| Ok(None)),
                })),
            }
        }

        pub fn with_check<F>(f: F) -> Self
        where
            F: FnMut() -> Result<Option<CheckForUpdatesResult>, String> + Send + Sync + 'static,
        {
            let u = Self::new();
            u.state.lock().unwrap().check_fn = Box::new(f);
            u
        }

        pub fn emit(&self, event: &str, payload: Option<EventPayload>) {
            let listeners = self
                .state
                .lock()
                .unwrap()
                .listeners
                .get(event)
                .cloned()
                .unwrap_or_default();
            for listener in listeners {
                listener(payload.clone());
            }
        }

        pub fn has_listener(&self, event: &str) -> bool {
            self.state
                .lock()
                .unwrap()
                .listeners
                .get(event)
                .is_some_and(|l| !l.is_empty())
        }

        pub fn installs(&self) -> Vec<InstallCall> {
            self.state.lock().unwrap().installs.clone()
        }

        pub fn push_quit(&self) {
            self.state.lock().unwrap().quits.push(1);
        }

        pub fn quits(&self) -> Vec<usize> {
            self.state.lock().unwrap().quits.clone()
        }

        pub fn check_calls(&self) -> usize {
            self.state.lock().unwrap().check_calls
        }

        pub fn set_channel_allows_downgrade(&self, enabled: bool) {
            self.state.lock().unwrap().channel_sets_downgrade = enabled;
        }
    }

    impl UpdaterLike for FakeUpdater {
        fn auto_download(&self) -> bool {
            self.state.lock().unwrap().auto_download
        }
        fn set_auto_download(&mut self, value: bool) {
            self.state.lock().unwrap().auto_download = value;
        }
        fn allow_prerelease(&self) -> bool {
            self.state.lock().unwrap().allow_prerelease
        }
        fn set_allow_prerelease(&mut self, value: bool) {
            self.state.lock().unwrap().allow_prerelease = value;
        }
        fn channel(&self) -> Option<String> {
            self.state.lock().unwrap().channel.clone()
        }
        fn set_channel(&mut self, value: Option<&str>) {
            let mut s = self.state.lock().unwrap();
            s.channel = value.map(str::to_owned);
            if s.channel_sets_downgrade {
                s.allow_downgrade = Some(true);
            }
        }
        fn allow_downgrade(&self) -> Option<bool> {
            self.state.lock().unwrap().allow_downgrade
        }
        fn set_allow_downgrade(&mut self, value: bool) {
            self.state.lock().unwrap().allow_downgrade = Some(value);
        }
        fn auto_install_on_app_quit(&self) -> bool {
            self.state.lock().unwrap().auto_install_on_app_quit
        }
        fn set_auto_install_on_app_quit(&mut self, value: bool) {
            self.state.lock().unwrap().auto_install_on_app_quit = value;
        }
        fn check_for_updates(&mut self) -> Result<Option<CheckForUpdatesResult>, String> {
            let mut s = self.state.lock().unwrap();
            s.check_calls += 1;
            (s.check_fn)()
        }
        fn download_update(&mut self) -> Result<(), String> {
            Ok(())
        }
        fn quit_and_install(&mut self, is_silent: Option<bool>, is_force_run_after: Option<bool>) {
            self.state.lock().unwrap().installs.push(InstallCall {
                is_silent,
                is_force_run_after,
            });
        }
        fn on(&mut self, event: &str, listener: EventListener) {
            self.state
                .lock()
                .unwrap()
                .listeners
                .entry(event.to_owned())
                .or_default()
                .push(listener);
        }
    }

    pub fn default_options(updater: FakeUpdater, current_version: &str, supported: bool) -> UpdateServiceOptions<FakeUpdater> {
        let quit_updater = updater.clone();
        UpdateServiceOptions {
            updater,
            current_version: current_version.to_owned(),
            supported,
            held_back: None,
            check_only: false,
            package_url: None,
            live_mission_count: Arc::new(|| 0),
            request_quit: Arc::new(move |_| quit_updater.push_quit()),
            on_state_change: None,
            every_build: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    // case: downloads on its own, and installs only on a quit the app itself makes
    #[test]
    fn downloads_on_its_own_and_installs_only_on_a_quit_the_app_itself_makes() {
        let updater = FakeUpdater::new();
        create_update_service(default_options(updater.clone(), "0.5.0", true));
        assert!(updater.auto_download());
        assert!(updater.auto_install_on_app_quit());
    }

    // case: keeps a downloaded update installable when a later check fails
    #[test]
    fn keeps_a_downloaded_update_installable_when_a_later_check_fails() {
        let fail = Arc::new(AtomicBool::new(false));
        let fail_check = Arc::clone(&fail);
        let updater = FakeUpdater::with_check(move || {
            if fail_check.load(Ordering::SeqCst) {
                return Err("offline".into());
            }
            Ok(Some(CheckForUpdatesResult {
                update_info: UpdateInfo {
                    version: "0.6.0".into(),
                    files: None,
                },
                is_update_available: Some(true),
                download_result: None,
            }))
        });
        let service = create_update_service(default_options(updater.clone(), "0.5.0", true));
        service.check();
        updater.emit("update-downloaded", Some(EventPayload::version("0.6.0")));
        assert_eq!(service.state().phase, AppUpdatePhase::Ready);

        fail.store(true, Ordering::SeqCst);
        service.check();
        assert_eq!(service.state().phase, AppUpdatePhase::Ready);
        assert_eq!(service.state().available_version.as_deref(), Some("0.6.0"));

        updater.emit("error", None);
        assert_eq!(service.state().phase, AppUpdatePhase::Ready);
        assert!(service.install().is_ok());
    }

    // case: does not install on a quit that is a relaunch (M19)
    #[test]
    fn does_not_install_on_a_quit_that_is_a_relaunch_m19() {
        let updater = FakeUpdater::new();
        let service = create_update_service(default_options(updater.clone(), "0.5.0", true));
        updater.emit("update-downloaded", Some(EventPayload::version("0.6.0")));
        service.hold_install_for_relaunch();
        assert!(!updater.auto_install_on_app_quit());
        assert!(updater.installs().is_empty());
        assert_eq!(service.state().phase, AppUpdatePhase::Ready);
    }

    // case: refuses to install while a mission is running, and says why
    #[test]
    fn refuses_to_install_while_a_mission_is_running_and_says_why() {
        let updater = FakeUpdater::with_check(|| {
            Ok(Some(CheckForUpdatesResult {
                update_info: UpdateInfo {
                    version: "0.6.0".into(),
                    files: None,
                },
                is_update_available: None,
                download_result: None,
            }))
        });
        let mut opts = default_options(updater.clone(), "0.5.0", true);
        opts.live_mission_count = Arc::new(|| 1);
        let service = create_update_service(opts);
        updater.emit("update-downloaded", None);

        let response = service.install();
        assert_eq!(
            response.error().map(|e| e.code),
            Some(AppUpdateErrorCode::UpdateBusy)
        );
        assert_eq!(
            response.error().map(|e| e.message.as_str()),
            Some(BUSY_MESSAGE)
        );
        assert!(updater.installs().is_empty());
        assert!(updater.quits().is_empty());
    }

    // case: installs by asking the app to quit, so the shutdown flush runs first and the updater installs on the real quit
    #[test]
    fn installs_by_asking_the_app_to_quit_so_the_shutdown_flush_runs_first_and_the_updater_installs_on_the_real_quit() {
        let finaliser: Arc<Mutex<Option<Box<dyn FnOnce() + Send>>>> = Arc::new(Mutex::new(None));
        let finaliser_slot = Arc::clone(&finaliser);
        let updater = FakeUpdater::new();
        let quit_updater = updater.clone();
        let mut opts = default_options(updater.clone(), "0.5.0", true);
        opts.request_quit = Arc::new(move |on_flushed| {
            quit_updater.push_quit();
            *finaliser_slot.lock().unwrap() = on_flushed;
        });
        let service = create_update_service(opts);
        updater.emit("update-downloaded", None);

        assert!(service.install().is_ok());
        assert!(updater.installs().is_empty());
        assert_eq!(updater.quits().len(), 1);
        let run_finaliser = finaliser.lock().unwrap().take().expect("finaliser handed over");
        run_finaliser();
        assert_eq!(
            updater.installs(),
            vec![InstallCall {
                is_silent: Some(true),
                is_force_run_after: Some(true),
            }]
        );
    }

    // case: refuses to install what it has not downloaded
    #[test]
    fn refuses_to_install_what_it_has_not_downloaded() {
        let updater = FakeUpdater::new();
        let service = create_update_service(default_options(updater.clone(), "0.5.0", true));
        let response = service.install();
        assert_eq!(
            response.error().map(|e| e.code),
            Some(AppUpdateErrorCode::UpdateNotReady)
        );
        assert_eq!(
            response.error().map(|e| e.message.as_str()),
            Some(NOT_READY_MESSAGE)
        );
        assert!(updater.installs().is_empty());
    }

    // case: says up to date only after a check that finished
    #[test]
    fn says_up_to_date_only_after_a_check_that_finished() {
        let updater = FakeUpdater::with_check(|| Err("offline".into()));
        let service = create_update_service(default_options(updater, "0.5.0", true));
        let response = service.check();
        assert_eq!(
            response.data().map(|d| d.phase),
            Some(AppUpdatePhase::Failed)
        );
        assert_eq!(
            response.data().and_then(|d| d.message.as_deref()),
            Some(CHECK_FAILED_MESSAGE)
        );
    }

    // case: names the version it found, and calls a matching one current
    #[test]
    fn names_the_version_it_found_and_calls_a_matching_one_current() {
        let newer = create_update_service(default_options(
            FakeUpdater::with_check(|| {
                Ok(Some(CheckForUpdatesResult {
                    update_info: UpdateInfo {
                        version: "0.6.0".into(),
                        files: None,
                    },
                    is_update_available: None,
                    download_result: None,
                }))
            }),
            "0.5.0",
            true,
        ));
        let checked_newer = newer.check();
        assert_eq!(
            checked_newer.data().map(|d| d.phase),
            Some(AppUpdatePhase::Available)
        );
        assert_eq!(
            checked_newer
                .data()
                .and_then(|d| d.available_version.as_deref()),
            Some("0.6.0")
        );

        let same = create_update_service(default_options(
            FakeUpdater::with_check(|| {
                Ok(Some(CheckForUpdatesResult {
                    update_info: UpdateInfo {
                        version: "0.5.0".into(),
                        files: None,
                    },
                    is_update_available: None,
                    download_result: None,
                }))
            }),
            "0.5.0",
            true,
        ));
        let checked_same = same.check();
        assert_eq!(
            checked_same.data().map(|d| d.phase),
            Some(AppUpdatePhase::Current)
        );
    }

    // case: says a build that cannot update itself cannot, rather than that it is current
    #[test]
    fn says_a_build_that_cannot_update_itself_cannot_rather_than_that_it_is_current() {
        let updater = FakeUpdater::new();
        let service = create_update_service(default_options(updater.clone(), "0.5.0-dev", false));
        let response = service.check();
        assert_eq!(
            response.data().map(|d| d.phase),
            Some(AppUpdatePhase::Unsupported)
        );
        assert_eq!(updater.check_calls(), 0);
    }

    // case: never installs anything at quit from a copy that is not the installed one (0.507)
    #[test]
    fn never_installs_anything_at_quit_from_a_copy_that_is_not_the_installed_one_0_507() {
        let updater = FakeUpdater::new();
        create_update_service(default_options(updater.clone(), "0.504.0", false));
        assert!(!updater.auto_install_on_app_quit());
        assert!(!updater.auto_download());
    }

    // case: a copy that only asks reads the feed and still never downloads or installs (0.507)
    #[test]
    fn a_copy_that_only_asks_reads_the_feed_and_still_never_downloads_or_installs_0_507() {
        let updater = FakeUpdater::with_check(|| {
            Ok(Some(CheckForUpdatesResult {
                update_info: UpdateInfo {
                    version: "0.6.0".into(),
                    files: None,
                },
                is_update_available: Some(true),
                download_result: None,
            }))
        });
        let mut opts = default_options(updater.clone(), "0.5.0", true);
        opts.check_only = true;
        let service = create_update_service(opts);
        let response = service.check();
        assert_eq!(
            response.data().map(|d| d.phase),
            Some(AppUpdatePhase::Available)
        );
        assert_eq!(
            response.data().and_then(|d| d.available_version.as_deref()),
            Some("0.6.0")
        );
        assert_eq!(updater.check_calls(), 1);
        assert!(!updater.auto_download());
        assert!(!updater.auto_install_on_app_quit());
        assert!(!updater.has_listener("update-downloaded"));
    }

    // case: never shows the provider its own error text
    #[test]
    fn never_shows_the_provider_its_own_error_text() {
        let updater = FakeUpdater::new();
        let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let seen_cb = Arc::clone(&seen);
        let mut opts = default_options(updater.clone(), "0.5.0", true);
        opts.on_state_change = Some(Arc::new(move |state| {
            if let Some(msg) = &state.message {
                seen_cb.lock().unwrap().push(msg.clone());
            }
        }));
        create_update_service(opts);

        updater.emit(
            "error",
            Some(EventPayload::error(
                "ENOTFOUND https://example.invalid/token=secret",
            )),
        );

        let recorded = seen.lock().unwrap().clone();
        assert_eq!(recorded, vec![CHECK_FAILED_MESSAGE.to_owned()]);
        assert!(!recorded.join(" ").contains("secret"));
    }

    // case: is said as a download that failed, and nothing is left unhandled
    #[test]
    fn is_said_as_a_download_that_failed_and_nothing_is_left_unhandled() {
        let updater = FakeUpdater::with_check(|| {
            Ok(Some(CheckForUpdatesResult {
                update_info: UpdateInfo {
                    version: "0.6.0".into(),
                    files: None,
                },
                is_update_available: Some(true),
                download_result: Some(Err("Cannot download ... status 404".into())),
            }))
        });
        let service = create_update_service(default_options(updater.clone(), "0.5.0", true));
        service.check();
        updater.emit(
            "error",
            Some(EventPayload::error("Cannot download ... status 404")),
        );
        assert_eq!(service.state().phase, AppUpdatePhase::Failed);
        assert_eq!(
            service.state().message.as_deref(),
            Some(DOWNLOAD_FAILED_MESSAGE)
        );
    }

    // case: keeps the check's own sentence for a check that failed
    #[test]
    fn keeps_the_checks_own_sentence_for_a_check_that_failed() {
        let updater = FakeUpdater::new();
        let service = create_update_service(default_options(updater.clone(), "0.5.0", true));
        updater.emit("error", Some(EventPayload::error("offline")));
        assert_eq!(service.state().phase, AppUpdatePhase::Failed);
        assert_eq!(
            service.state().message.as_deref(),
            Some(CHECK_FAILED_MESSAGE)
        );
    }

    #[test]
    fn default_no_feed_updater_refuses_to_check_or_download() {
        let mut unfed = NoFeedUpdater::default();
        assert_eq!(unfed.check_for_updates(), Err(NO_FEED_MESSAGE.to_owned()));
        assert_eq!(unfed.download_update(), Err(NO_FEED_MESSAGE.to_owned()));
    }
}
