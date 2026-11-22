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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub const UPDATE_CHECK_EVERY_MS: u64 = 60 * 60 * 1000;
pub const UPDATE_RETRY_AFTER_RUN_MS: u64 = 15 * 60 * 1000;
pub const DOWNLOADS: &str = "https://dl.locust.lol/";
pub const MAC_RELEASES_API: &str = "https://dl.locust.lol/mac-releases.json";

pub const DOWNLOAD_FAILED_MESSAGE: &str =
    "The update could not be downloaded. Locust will try again at its next check.";
pub const CHECK_FAILED_MESSAGE: &str = "The update check could not complete.";
pub const NOT_READY_MESSAGE: &str = "There is no downloaded update to install yet.";
pub const BUSY_MESSAGE: &str =
    "A mission is still running. Stop it first, then install — restarting now would cut it off mid-run.";
pub const NO_FEED_MESSAGE: &str = "This copy has no updates to look for yet";
pub const INSTALL_TAKES: &str =
    "Locust closes while it installs and opens again by itself in a minute or two.";

/// Which builds an installed Locust takes (`UpdateLane` in `main/update-lane.ts`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UpdateLane {
    pub every_build: bool,
}

pub const TESTER_LANE: UpdateLane = UpdateLane { every_build: false };

/// A saved lane, read defensively: anything unreadable is the tester lane (`updateLaneFrom` in `main/update-lane.ts`).
pub fn update_lane_from(value: Option<&serde_json::Value>) -> UpdateLane {
    let Some(every_build) = value
        .and_then(serde_json::Value::as_object)
        .and_then(|obj| obj.get("everyBuild"))
        .and_then(serde_json::Value::as_bool)
    else {
        return TESTER_LANE;
    };
    UpdateLane { every_build }
}

pub fn installing_line(version: Option<&str>) -> String {
    match version.filter(|v| !v.is_empty()) {
        Some(v) => format!(
            "Installing Locust {v}. It opens again by itself in a minute or two; there is no need to open it."
        ),
        None => "Installing Locust. It opens again by itself in a minute or two; there is no need to open it.".to_owned(),
    }
}

pub fn held_back_line(held_back: Option<HeldBack>) -> &'static str {
    match held_back {
        Some(HeldBack::Development) => {
            "This copy of Locust doesn't update itself. The installed Locust does. A development build never updates."
        }
        Some(HeldBack::NoFeed) => NO_FEED_MESSAGE,
        Some(HeldBack::Driven | HeldBack::Copy) | None => {
            "This copy of Locust doesn't update itself. The installed Locust does."
        }
    }
}

pub fn update_control_line(update: Option<&AppUpdateState>, installing: bool) -> String {
    let Some(update) = update else {
        return "Not checked yet.".to_owned();
    };
    match update.phase {
        AppUpdatePhase::Unsupported => held_back_line(update.held_back).to_owned(),
        AppUpdatePhase::Checking => "Checking…".to_owned(),
        AppUpdatePhase::Current => "Up to date.".to_owned(),
        AppUpdatePhase::Available => {
            let ver = update.available_version.as_deref().unwrap_or("");
            if update.download_url.is_some() {
                format!("Version {ver} is out. Download it, then open the file to install it.")
            } else {
                format!("Version {ver} is available. Downloading it now.")
            }
        }
        AppUpdatePhase::Downloading => {
            let ver = update.available_version.as_deref().unwrap_or("");
            match update.percent {
                Some(pct) => format!("Downloading {ver} · {pct}%"),
                None => format!("Downloading {ver}"),
            }
        }
        AppUpdatePhase::Ready => {
            if installing {
                installing_line(update.available_version.as_deref())
            } else {
                let ver = update.available_version.as_deref().unwrap_or("");
                format!("Version {ver} is downloaded and ready to install. {INSTALL_TAKES}")
            }
        }
        AppUpdatePhase::Failed => update
            .message
            .clone()
            .unwrap_or_else(|| CHECK_FAILED_MESSAGE.to_owned()),
        AppUpdatePhase::Idle => "Not checked yet.".to_owned(),
    }
}

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

impl Default for AppUpdateState {
    fn default() -> Self {
        Self {
            phase: AppUpdatePhase::Unsupported,
            current_version: "0.1.0".into(),
            every_build: false,
            available_version: None,
            download_url: None,
            percent: None,
            message: None,
            held_back: Some(HeldBack::NoFeed),
        }
    }
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

impl<U: UpdaterLike + ?Sized> UpdaterLike for Box<U> {
    fn auto_download(&self) -> bool {
        (**self).auto_download()
    }
    fn set_auto_download(&mut self, value: bool) {
        (**self).set_auto_download(value);
    }
    fn allow_prerelease(&self) -> bool {
        (**self).allow_prerelease()
    }
    fn set_allow_prerelease(&mut self, value: bool) {
        (**self).set_allow_prerelease(value);
    }
    fn channel(&self) -> Option<String> {
        (**self).channel()
    }
    fn set_channel(&mut self, value: Option<&str>) {
        (**self).set_channel(value);
    }
    fn allow_downgrade(&self) -> Option<bool> {
        (**self).allow_downgrade()
    }
    fn set_allow_downgrade(&mut self, value: bool) {
        (**self).set_allow_downgrade(value);
    }
    fn auto_install_on_app_quit(&self) -> bool {
        (**self).auto_install_on_app_quit()
    }
    fn set_auto_install_on_app_quit(&mut self, value: bool) {
        (**self).set_auto_install_on_app_quit(value);
    }
    fn check_for_updates(&mut self) -> Result<Option<CheckForUpdatesResult>, String> {
        (**self).check_for_updates()
    }
    fn download_update(&mut self) -> Result<(), String> {
        (**self).download_update()
    }
    fn quit_and_install(&mut self, is_silent: Option<bool>, is_force_run_after: Option<bool>) {
        (**self).quit_and_install(is_silent, is_force_run_after);
    }
    fn on(&mut self, event: &str, listener: EventListener) {
        (**self).on(event, listener);
    }
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

/// Whether a periodic look should happen now (`periodicCheckDue` in `main/updates.ts`): never while a teammate is
/// working, and never while a check is under way or a version is found or coming down.
pub fn periodic_check_due(state: &AppUpdateState, live_missions: usize) -> bool {
    if live_missions > 0 {
        return false;
    }
    matches!(
        state.phase,
        AppUpdatePhase::Idle
            | AppUpdatePhase::Current
            | AppUpdatePhase::Failed
            | AppUpdatePhase::Ready
    )
}

struct ScheduledTimer {
    id: u64,
    due_ms: u64,
    task: Box<dyn FnOnce() + Send>,
}

#[derive(Default)]
struct ClockInner {
    now_ms: u64,
    next_id: u64,
    timers: Vec<ScheduledTimer>,
}

/// Deterministic timer queue for periodic update checks and delayed feed downloads.
#[derive(Clone, Default)]
pub struct UpdateClock {
    inner: Arc<Mutex<ClockInner>>,
}

impl UpdateClock {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn schedule_after<F>(&self, after_ms: u64, task: F) -> u64
    where
        F: FnOnce() + Send + 'static,
    {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.next_id += 1;
        let id = guard.next_id;
        let due_ms = guard.now_ms.saturating_add(after_ms);
        guard.timers.push(ScheduledTimer {
            id,
            due_ms,
            task: Box::new(task),
        });
        id
    }

    pub fn cancel(&self, id: u64) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.timers.retain(|t| t.id != id);
    }

    pub fn advance_by(&self, delta_ms: u64) {
        let target_ms = {
            let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            guard.now_ms.saturating_add(delta_ms)
        };
        loop {
            let next_timer = {
                let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                let best_idx = guard
                    .timers
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| t.due_ms <= target_ms)
                    .min_by_key(|(_, t)| (t.due_ms, t.id))
                    .map(|(idx, _)| idx);
                match best_idx {
                    Some(idx) => {
                        let timer = guard.timers.remove(idx);
                        guard.now_ms = timer.due_ms;
                        Some(timer)
                    }
                    None => {
                        guard.now_ms = target_ms;
                        None
                    }
                }
            };
            match next_timer {
                Some(timer) => (timer.task)(),
                None => break,
            }
        }
    }
}

pub struct PeriodicChecks {
    stopped: Arc<AtomicBool>,
    timer_id: Arc<Mutex<Option<u64>>>,
    clock: UpdateClock,
}

impl PeriodicChecks {
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        if let Some(id) = self.timer_id.lock().unwrap_or_else(|e| e.into_inner()).take() {
            self.clock.cancel(id);
        }
    }
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

/// A Mac release with this chip's disk image (`MacRelease` in `main/mac-release.ts`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MacRelease {
    pub version: String,
    pub url: String,
    pub size: Option<u64>,
    pub sha256: Option<String>,
}

fn parse_exact_semver(text: &str) -> Option<[u64; 3]> {
    let trimmed = text.trim();
    let rest = trimmed.strip_prefix('v').unwrap_or(trimmed);
    let mut parts = rest.split('.');
    let major: u64 = parts.next()?.parse().ok()?;
    let minor: u64 = parts.next()?.parse().ok()?;
    let patch_str = parts.next()?;
    if parts.next().is_some() || patch_str.is_empty() || !patch_str.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let patch: u64 = patch_str.parse().ok()?;
    Some([major, minor, patch])
}

fn mac_release_is_newer(candidate: &str, current: &str) -> bool {
    let current_base = current.split('-').next().unwrap_or(current);
    let (Some(a), Some(b)) = (parse_exact_semver(candidate), parse_exact_semver(current_base)) else {
        return false;
    };
    for i in 0..3 {
        if a[i] != b[i] {
            return a[i] > b[i];
        }
    }
    false
}

/// The newest published release with this chip's disk image, when it is newer than `current`
/// (`newerMacRelease` in `main/mac-release.ts`).
pub fn newer_mac_release(
    releases: &serde_json::Value,
    current: &str,
    arch: &str,
) -> Option<MacRelease> {
    let entries = releases.as_array()?;
    let suffix = if arch == "arm64" {
        "-mac-arm64.dmg"
    } else {
        "-mac-x64.dmg"
    };
    for entry in entries {
        let Some(release) = entry.as_object() else {
            continue;
        };
        if release.get("draft").and_then(serde_json::Value::as_bool) == Some(true)
            || release.get("prerelease").and_then(serde_json::Value::as_bool) == Some(true)
        {
            continue;
        };
        let Some(tag_name) = release.get("tag_name").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let assets = release
            .get("assets")
            .and_then(serde_json::Value::as_array);
        let image = assets.and_then(|list| {
            list.iter().find_map(|asset| {
                let obj = asset.as_object()?;
                let name = obj.get("name")?.as_str()?;
                let url = obj.get("browser_download_url")?.as_str()?;
                if name.ends_with(suffix) {
                    Some((obj, url))
                } else {
                    None
                }
            })
        });
        let Some((image_obj, url)) = image else {
            continue;
        };
        if !url.starts_with(DOWNLOADS) || !url.ends_with(".dmg") {
            continue;
        }
        let version = tag_name.strip_prefix('v').unwrap_or(tag_name);
        if !mac_release_is_newer(version, current) {
            return None;
        }
        let size = image_obj.get("size").and_then(serde_json::Value::as_u64);
        let sha256 = image_obj
            .get("digest")
            .and_then(serde_json::Value::as_str)
            .and_then(|d| d.strip_prefix("sha256:"))
            .filter(|hex| {
                hex.len() == 64 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
            })
            .map(str::to_owned);
        return Some(MacRelease {
            version: version.to_owned(),
            url: url.to_owned(),
            size,
            sha256,
        });
    }
    None
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
    clock: UpdateClock,
}

impl<U: UpdaterLike> Clone for UpdateService<U> {
    fn clone(&self) -> Self {
        Self {
            updater: Arc::clone(&self.updater),
            current_version: self.current_version.clone(),
            supported: self.supported,
            held_back: self.held_back,
            package_url: self.package_url.clone(),
            live_mission_count: Arc::clone(&self.live_mission_count),
            request_quit: Arc::clone(&self.request_quit),
            on_state_change: self.on_state_change.clone(),
            inner: Arc::clone(&self.inner),
            clock: self.clock.clone(),
        }
    }
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
        clock: UpdateClock::new(),
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

    pub fn clock(&self) -> &UpdateClock {
        &self.clock
    }

    pub fn advance_timers_by_ms(&self, delta_ms: u64) {
        self.clock.advance_by(delta_ms);
    }

    fn schedule_tick(
        service: &UpdateService<U>,
        stopped: &Arc<AtomicBool>,
        timer_id: &Arc<Mutex<Option<u64>>>,
        every_ms: u64,
        retry_ms: u64,
        after_ms: u64,
    ) {
        let svc = service.clone();
        let stopped_flag = Arc::clone(stopped);
        let timer_slot = Arc::clone(timer_id);
        let id = service.clock.schedule_after(after_ms, move || {
            if stopped_flag.load(Ordering::SeqCst) || !svc.supported {
                return;
            }
            let live = (svc.live_mission_count)();
            if live > 0 {
                Self::schedule_tick(&svc, &stopped_flag, &timer_slot, every_ms, retry_ms, retry_ms);
                return;
            }
            if periodic_check_due(&svc.state(), live) {
                svc.look(true);
            }
            Self::schedule_tick(&svc, &stopped_flag, &timer_slot, every_ms, retry_ms, every_ms);
        });
        *timer_id.lock().unwrap_or_else(|e| e.into_inner()) = Some(id);
    }

    pub fn start_periodic_checks(&self) -> PeriodicChecks {
        self.start_periodic_checks_with(UPDATE_CHECK_EVERY_MS, UPDATE_RETRY_AFTER_RUN_MS)
    }

    pub fn start_periodic_checks_with(&self, every_ms: u64, retry_ms: u64) -> PeriodicChecks {
        let stopped = Arc::new(AtomicBool::new(false));
        let timer_id = Arc::new(Mutex::new(None));
        Self::schedule_tick(self, &stopped, &timer_id, every_ms, retry_ms, every_ms);
        PeriodicChecks {
            stopped,
            timer_id,
            clock: self.clock.clone(),
        }
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

#[cfg(test)]
mod ready_means_this_version_is_on_disk_tests {
    use super::test_support::*;
    use super::*;

    fn fake_versions(versions: &[&str]) -> FakeUpdater {
        let list: Vec<String> = versions.iter().map(|s| (*s).to_owned()).collect();
        let mut at = 0usize;
        FakeUpdater::with_check(move || {
            let idx = at.min(list.len().saturating_sub(1));
            let version = list.get(idx).cloned().unwrap_or_else(|| "0.0.0".into());
            at += 1;
            Ok(Some(CheckForUpdatesResult {
                update_info: UpdateInfo {
                    version,
                    files: None,
                },
                is_update_available: None,
                download_result: None,
            }))
        })
    }

    fn service_on(updater: FakeUpdater) -> UpdateService<FakeUpdater> {
        let mut opts = default_options(updater, "0.225.0", true);
        opts.request_quit = Arc::new(|finalise| {
            if let Some(f) = finalise {
                f();
            }
        });
        create_update_service(opts)
    }

    // case: stops calling a newer version downloaded when an older one was
    #[test]
    fn stops_calling_a_newer_version_downloaded_when_an_older_one_was() {
        let updater = fake_versions(&["0.226.0", "0.228.0"]);
        let service = service_on(updater.clone());

        service.check();
        updater.emit("update-downloaded", Some(EventPayload::version("0.226.0")));
        assert_eq!(service.state().phase, AppUpdatePhase::Ready);
        assert_eq!(service.state().available_version.as_deref(), Some("0.226.0"));

        let after = service.check();
        let data = after.data().expect("check succeeded");
        assert_eq!(data.phase, AppUpdatePhase::Available);
        assert_eq!(data.available_version.as_deref(), Some("0.228.0"));
    }

    // case: stays ready when the check finds the version that is on disk
    #[test]
    fn stays_ready_when_the_check_finds_the_version_that_is_on_disk() {
        let updater = fake_versions(&["0.226.0", "0.226.0"]);
        let service = service_on(updater.clone());
        service.check();
        updater.emit("update-downloaded", Some(EventPayload::version("0.226.0")));
        let after = service.check();
        let data = after.data().expect("check succeeded");
        assert_eq!(data.phase, AppUpdatePhase::Ready);
    }

    // case: refuses to install once the downloaded one is superseded
    #[test]
    fn refuses_to_install_once_the_downloaded_one_is_superseded() {
        let updater = fake_versions(&["0.226.0", "0.228.0"]);
        let service = service_on(updater.clone());
        service.check();
        updater.emit("update-downloaded", Some(EventPayload::version("0.226.0")));
        service.check();
        let result = service.install();
        assert!(!result.is_ok());
        assert!(updater.installs().is_empty());
        // And an error after the newer check does not revive the superseded 0.226.0 as ready.
        updater.emit("error", None);
        assert_eq!(service.state().phase, AppUpdatePhase::Failed);
        assert!(!service.install().is_ok());
    }

    // case: falls back to the version it was looking for when the event carries none
    #[test]
    fn falls_back_to_the_version_it_was_looking_for_when_the_event_carries_none() {
        let updater = fake_versions(&["0.226.0", "0.226.0"]);
        let service = service_on(updater.clone());
        service.check();
        updater.emit("update-downloaded", None);
        assert_eq!(service.state().available_version.as_deref(), Some("0.226.0"));
        let after = service.check();
        let data = after.data().expect("check succeeded");
        assert_eq!(data.phase, AppUpdatePhase::Ready);
    }
}

#[cfg(test)]
mod an_open_locust_keeps_looking_for_updates_tests {
    use super::test_support::*;
    use super::*;
    use std::sync::atomic::AtomicUsize;

    const DOWNLOAD_MS: u64 = 1_000;

    fn feed(latest: Arc<Mutex<String>>, clock: UpdateClock) -> FakeUpdater {
        let holder: Arc<Mutex<Option<FakeUpdater>>> = Arc::new(Mutex::new(None));
        let holder_in = Arc::clone(&holder);
        let updater = FakeUpdater::with_check(move || {
            let version = latest.lock().unwrap().clone();
            if let Some(u) = holder_in.lock().unwrap().clone() {
                let v = version.clone();
                clock.schedule_after(DOWNLOAD_MS, move || {
                    u.emit("update-downloaded", Some(EventPayload::version(v)));
                });
            }
            Ok(Some(CheckForUpdatesResult {
                update_info: UpdateInfo {
                    version,
                    files: None,
                },
                is_update_available: Some(true),
                download_result: None,
            }))
        });
        *holder.lock().unwrap() = Some(updater.clone());
        updater
    }

    fn bare_state(phase: AppUpdatePhase) -> AppUpdateState {
        AppUpdateState {
            phase,
            current_version: "0.366.0".into(),
            every_build: false,
            available_version: None,
            download_url: None,
            percent: None,
            message: None,
            held_back: None,
        }
    }

    // case: looks again every hour
    #[test]
    fn looks_again_every_hour() {
        assert_eq!(UPDATE_CHECK_EVERY_MS, 60 * 60 * 1000);
        let updater = FakeUpdater::new();
        let service = create_update_service(default_options(updater.clone(), "0.366.0", true));
        let stop = service.start_periodic_checks();
        service.advance_timers_by_ms(UPDATE_CHECK_EVERY_MS - 1);
        assert_eq!(updater.check_calls(), 0);
        service.advance_timers_by_ms(1);
        assert_eq!(updater.check_calls(), 1);
        service.advance_timers_by_ms(UPDATE_CHECK_EVERY_MS);
        assert_eq!(updater.check_calls(), 2);
        stop.stop();
        service.advance_timers_by_ms(UPDATE_CHECK_EVERY_MS * 3);
        assert_eq!(updater.check_calls(), 2);
    }

    // case: waits while a teammate is working, and tries again soon rather than an hour later
    #[test]
    fn waits_while_a_teammate_is_working_and_tries_again_soon_rather_than_an_hour_later() {
        let updater = FakeUpdater::new();
        let live = Arc::new(AtomicUsize::new(1));
        let live_read = Arc::clone(&live);
        let mut opts = default_options(updater.clone(), "0.366.0", true);
        opts.live_mission_count = Arc::new(move || live_read.load(Ordering::SeqCst));
        let service = create_update_service(opts);
        let _stop = service.start_periodic_checks();
        service.advance_timers_by_ms(UPDATE_CHECK_EVERY_MS);
        assert_eq!(updater.check_calls(), 0);
        live.store(0, Ordering::SeqCst);
        service.advance_timers_by_ms(UPDATE_RETRY_AFTER_RUN_MS);
        assert_eq!(updater.check_calls(), 1);
    }

    // case: never looks in a build that cannot update itself
    #[test]
    fn never_looks_in_a_build_that_cannot_update_itself() {
        let updater = FakeUpdater::new();
        let live_polls = Arc::new(AtomicUsize::new(0));
        let live_polls_in = Arc::clone(&live_polls);
        let mut opts = default_options(updater.clone(), "0.366.0", false);
        opts.live_mission_count = Arc::new(move || {
            live_polls_in.fetch_add(1, Ordering::SeqCst);
            0
        });
        let service = create_update_service(opts);
        let _stop = service.start_periodic_checks();
        service.advance_timers_by_ms(UPDATE_CHECK_EVERY_MS * 2);
        assert_eq!(updater.check_calls(), 0);
        assert_eq!(live_polls.load(Ordering::SeqCst), 0);
    }

    // case: keeps looking while a downloaded version waits, and a newer one takes its place
    #[test]
    fn keeps_looking_while_a_downloaded_version_waits_and_a_newer_one_takes_its_place() {
        let latest = Arc::new(Mutex::new("0.817.0".to_owned()));
        let said: Arc<Mutex<Vec<(AppUpdatePhase, Option<String>)>>> =
            Arc::new(Mutex::new(Vec::new()));
        let said_cb = Arc::clone(&said);
        let probe = create_update_service(default_options(FakeUpdater::new(), "0.814.0", true));
        let clock = probe.clock().clone();
        let updater = feed(Arc::clone(&latest), clock.clone());
        let mut opts = default_options(updater, "0.814.0", true);
        opts.on_state_change = Some(Arc::new(move |state| {
            said_cb
                .lock()
                .unwrap()
                .push((state.phase, state.available_version.clone()));
        }));
        let mut service = create_update_service(opts);
        service.clock = clock;

        service.check();
        service.advance_timers_by_ms(DOWNLOAD_MS);
        assert_eq!(service.state().phase, AppUpdatePhase::Ready);
        assert_eq!(service.state().available_version.as_deref(), Some("0.817.0"));

        let _stop = service.start_periodic_checks();
        said.lock().unwrap().clear();

        service.advance_timers_by_ms(UPDATE_CHECK_EVERY_MS);
        assert_eq!(service.state().phase, AppUpdatePhase::Ready);
        assert_eq!(service.state().available_version.as_deref(), Some("0.817.0"));
        service.advance_timers_by_ms(DOWNLOAD_MS);
        assert_eq!(service.state().phase, AppUpdatePhase::Ready);
        assert_eq!(service.state().available_version.as_deref(), Some("0.817.0"));
        assert!(
            said.lock()
                .unwrap()
                .iter()
                .all(|(phase, ver)| *phase == AppUpdatePhase::Ready
                    && ver.as_deref() == Some("0.817.0"))
        );

        *latest.lock().unwrap() = "0.819.0".to_owned();
        said.lock().unwrap().clear();
        service.advance_timers_by_ms(UPDATE_CHECK_EVERY_MS - DOWNLOAD_MS);
        assert_eq!(service.state().phase, AppUpdatePhase::Available);
        assert_eq!(service.state().available_version.as_deref(), Some("0.819.0"));
        service.advance_timers_by_ms(DOWNLOAD_MS);
        assert_eq!(service.state().phase, AppUpdatePhase::Ready);
        assert_eq!(service.state().available_version.as_deref(), Some("0.819.0"));
        assert_eq!(
            *said.lock().unwrap(),
            vec![
                (AppUpdatePhase::Available, Some("0.819.0".to_owned())),
                (AppUpdatePhase::Ready, Some("0.819.0".to_owned())),
            ]
        );
    }

    // case: still says Checking when the person presses Check now
    #[test]
    fn still_says_checking_when_the_person_presses_check_now() {
        let said: Arc<Mutex<Vec<AppUpdatePhase>>> = Arc::new(Mutex::new(Vec::new()));
        let said_cb = Arc::clone(&said);
        let updater = feed(
            Arc::new(Mutex::new("0.817.0".to_owned())),
            UpdateClock::new(),
        );
        let mut opts = default_options(updater, "0.814.0", true);
        opts.on_state_change = Some(Arc::new(move |state| {
            said_cb.lock().unwrap().push(state.phase);
        }));
        let service = create_update_service(opts);
        service.check();
        assert_eq!(said.lock().unwrap().first().copied(), Some(AppUpdatePhase::Checking));
    }

    // case: is, when nothing is found yet, the last look could not complete, or a downloaded version waits
    #[test]
    fn is_when_nothing_is_found_yet_the_last_look_could_not_complete_or_a_downloaded_version_waits() {
        for phase in [
            AppUpdatePhase::Idle,
            AppUpdatePhase::Current,
            AppUpdatePhase::Failed,
            AppUpdatePhase::Ready,
        ] {
            assert!(periodic_check_due(&bare_state(phase), 0));
        }
    }

    // case: is not while a check runs or a version is found or coming down -- or while a teammate works
    #[test]
    fn is_not_while_a_check_runs_or_a_version_is_found_or_coming_down_or_while_a_teammate_works() {
        for phase in [
            AppUpdatePhase::Checking,
            AppUpdatePhase::Available,
            AppUpdatePhase::Downloading,
            AppUpdatePhase::Unsupported,
        ] {
            assert!(!periodic_check_due(&bare_state(phase), 0));
        }
        assert!(!periodic_check_due(&bare_state(AppUpdatePhase::Current), 2));
        assert!(!periodic_check_due(&bare_state(AppUpdatePhase::Ready), 1));
    }
}

#[cfg(test)]
mod a_mac_says_when_a_new_locust_is_out_tests {
    use super::*;
    use serde_json::{Value, json};

    const BASE: &str = "https://dl.locust.lol";

    fn release(tag: &str, assets: &[&str], draft: bool, prerelease: bool) -> Value {
        let asset_list: Vec<Value> = assets
            .iter()
            .map(|name| {
                json!({
                    "name": name,
                    "browser_download_url": format!("{BASE}/{name}"),
                })
            })
            .collect();
        json!({
            "tag_name": tag,
            "draft": draft,
            "prerelease": prerelease,
            "assets": asset_list,
        })
    }

    // case: offers the newest release that has this chip's disk image
    #[test]
    fn offers_the_newest_release_that_has_this_chips_disk_image() {
        let releases = json!([
            release("0.515.0", &["Locust-0.515.0-setup.exe"], false, false),
            release(
                "0.514.0",
                &[
                    "Locust-0.514.0-setup.exe",
                    "Locust-0.514.0-mac-arm64.dmg",
                    "Locust-0.514.0-mac-x64.dmg"
                ],
                false,
                false
            ),
        ]);
        assert_eq!(
            newer_mac_release(&releases, "0.512.0", "arm64"),
            Some(MacRelease {
                version: "0.514.0".into(),
                url: format!("{BASE}/Locust-0.514.0-mac-arm64.dmg"),
                size: None,
                sha256: None,
            })
        );
        assert_eq!(
            newer_mac_release(&releases, "0.512.0", "x64").map(|r| r.url),
            Some(format!("{BASE}/Locust-0.514.0-mac-x64.dmg"))
        );
    }

    // case: offers nothing when this copy is already that version or newer
    #[test]
    fn offers_nothing_when_this_copy_is_already_that_version_or_newer() {
        let releases = json!([release(
            "0.514.0",
            &["Locust-0.514.0-mac-arm64.dmg"],
            false,
            false
        )]);
        assert_eq!(newer_mac_release(&releases, "0.514.0", "arm64"), None);
        assert_eq!(newer_mac_release(&releases, "0.515.0", "arm64"), None);
    }

    // case: skips drafts and pre-releases, and never offers a link off Locust's own downloads
    #[test]
    fn skips_drafts_and_pre_releases_and_never_offers_a_link_off_locusts_own_downloads() {
        let draft = json!([release(
            "0.520.0",
            &["Locust-0.520.0-mac-arm64.dmg"],
            true,
            false
        )]);
        assert_eq!(newer_mac_release(&draft, "0.514.0", "arm64"), None);

        let pre = json!([release(
            "0.520.0",
            &["Locust-0.520.0-mac-arm64.dmg"],
            false,
            true
        )]);
        assert_eq!(newer_mac_release(&pre, "0.514.0", "arm64"), None);

        let elsewhere = json!([{
            "tag_name": "0.520.0",
            "assets": [{
                "name": "Locust-0.520.0-mac-arm64.dmg",
                "browser_download_url": "https://example.com/Locust.dmg"
            }]
        }]);
        assert_eq!(newer_mac_release(&elsewhere, "0.514.0", "arm64"), None);
    }

    // case: answers nothing for an answer that is not a list of releases
    #[test]
    fn answers_nothing_for_an_answer_that_is_not_a_list_of_releases() {
        assert_eq!(newer_mac_release(&Value::Null, "0.514.0", "arm64"), None);
        assert_eq!(
            newer_mac_release(
                &json!({ "message": "API rate limit exceeded" }),
                "0.514.0",
                "arm64"
            ),
            None
        );
    }
}

#[cfg(test)]
mod testers_get_one_build_a_day_tests {
    use super::test_support::*;
    use super::*;
    use serde_json::json;

    fn updater() -> FakeUpdater {
        let mut u = FakeUpdater::new();
        u.set_auto_download(false);
        u.set_allow_prerelease(true);
        u.set_auto_install_on_app_quit(false);
        u
    }

    fn service(fake: FakeUpdater, every_build: Option<bool>) -> UpdateService<FakeUpdater> {
        let mut opts = default_options(fake, "0.307.0", true);
        if let Some(eb) = every_build {
            opts.every_build = eb;
        }
        create_update_service(opts)
    }

    // case: is the tester lane unless every build was chosen, whatever the file says
    #[test]
    fn is_the_tester_lane_unless_every_build_was_chosen_whatever_the_file_says() {
        assert_eq!(update_lane_from(None), TESTER_LANE);
        assert_eq!(
            update_lane_from(Some(&json!({ "everyBuild": "yes" }))),
            TESTER_LANE
        );
        assert_eq!(
            update_lane_from(Some(&json!({ "everyBuild": true }))),
            UpdateLane { every_build: true }
        );
        assert!(!TESTER_LANE.every_build);
    }

    // case: takes no prerelease by default -- even from an updater that would
    #[test]
    fn takes_no_prerelease_by_default_even_from_an_updater_that_would() {
        let fake = updater();
        let updates = service(fake.clone(), None);
        assert!(!fake.allow_prerelease());
        assert!(!updates.state().every_build);
    }

    // case: takes every build once chosen, and says so in every state after
    #[test]
    fn takes_every_build_once_chosen_and_says_so_in_every_state_after() {
        let fake = updater();
        let updates = service(fake.clone(), None);
        assert!(updates.set_every_build(true).every_build);
        assert!(fake.allow_prerelease());
        let checked = updates.check();
        assert_eq!(checked.data().map(|d| d.every_build), Some(true));
        updates.set_every_build(false);
        assert!(!fake.allow_prerelease());
    }

    // case: calls an older latest up to date, not an update
    #[test]
    fn calls_an_older_latest_up_to_date_not_an_update() {
        let fake = FakeUpdater::with_check(|| {
            Ok(Some(CheckForUpdatesResult {
                update_info: UpdateInfo {
                    version: "0.306.0".into(),
                    files: None,
                },
                is_update_available: Some(false),
                download_result: None,
            }))
        });
        let updates = service(fake, None);
        let checked = updates.check();
        assert_eq!(checked.data().map(|d| d.phase), Some(AppUpdatePhase::Current));

        let plain = service(
            FakeUpdater::with_check(|| {
                Ok(Some(CheckForUpdatesResult {
                    update_info: UpdateInfo {
                        version: "0.306.0".into(),
                        files: None,
                    },
                    is_update_available: None,
                    download_result: None,
                }))
            }),
            None,
        );
        let again = plain.check();
        assert_eq!(again.data().map(|d| d.phase), Some(AppUpdatePhase::Current));

        let newer = service(
            FakeUpdater::with_check(|| {
                Ok(Some(CheckForUpdatesResult {
                    update_info: UpdateInfo {
                        version: "0.308.0".into(),
                        files: None,
                    },
                    is_update_available: None,
                    download_result: None,
                }))
            }),
            None,
        );
        let found = newer.check();
        assert_eq!(
            found.data().map(|d| d.phase),
            Some(AppUpdatePhase::Available)
        );
    }

    // case: reads its own feed file, and never steps back for it
    #[test]
    fn reads_its_own_feed_file_and_never_steps_back_for_it() {
        let fake = updater();
        fake.set_channel_allows_downgrade(true);
        let updates = service(fake.clone(), None);
        assert_eq!(
            (fake.channel().as_deref(), fake.allow_downgrade()),
            (Some("latest"), Some(false))
        );
        updates.set_every_build(true);
        assert_eq!(
            (fake.channel().as_deref(), fake.allow_downgrade()),
            (Some("beta"), Some(false))
        );
        updates.set_every_build(false);
        assert_eq!(
            (fake.channel().as_deref(), fake.allow_downgrade()),
            (Some("latest"), Some(false))
        );
    }

    // case: starts on the saved lane
    #[test]
    fn starts_on_the_saved_lane() {
        let fake = updater();
        let _updates = service(fake.clone(), Some(true));
        assert!(fake.allow_prerelease());
        assert_eq!(fake.channel().as_deref(), Some("beta"));
    }
}
