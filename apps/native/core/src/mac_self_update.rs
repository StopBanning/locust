//! A Mac Locust updates itself (Electron `main/mac-self-update.ts`).
//!
//! Safety rule: The Rust app has no release feed of its own yet, so the default seams never fetch from the network,
//! mount a real disk image, or touch `/Applications`. Every network fetch, download, command run and swap script launch
//! goes through `MacUpdaterOptions` seams that tests provide.

use crate::updates::{
    CheckForUpdatesResult, EventListener, EventPayload, NO_FEED_MESSAGE, UpdateInfo, UpdaterLike,
    newer_mac_release,
};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// The swap, as the shell runs it once Locust has gone (`SWAP_SCRIPT` in `main/mac-self-update.ts`).
pub const SWAP_SCRIPT: &str = "\
#!/bin/sh
# Locust, updating itself on a Mac (mac-self-update.ts). Waits for the old app to exit,
# puts the new one in its place, puts the old one back if that fails, and opens it.
PID=\"$1\"; APP=\"$2\"; NEW=\"$3\"; OLD=\"$APP.replaced\"
i=0
while kill -0 \"$PID\" 2>/dev/null && [ \"$i\" -lt 600 ]; do sleep 0.2; i=$((i+1)); done
rm -rf \"$OLD\"
if mv \"$APP\" \"$OLD\"; then
  if mv \"$NEW\" \"$APP\"; then rm -rf \"$OLD\"; else mv \"$OLD\" \"$APP\"; fi
fi
# Only the Mac CI check passes more: its copy reopens on its own profile, behind the windows in front.
shift 3
if [ \"$#\" -gt 0 ]; then open -g \"$APP\" --args \"$@\"; else open \"$APP\"; fi
";

fn posix_dirname(path: &str) -> &str {
    let trimmed = path.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(0) => "/",
        Some(idx) => &trimmed[..idx],
        None => ".",
    }
}

/// The `Locust.app` this process runs from, when it can replace itself there (`macSelfUpdateTarget` in
/// `main/mac-self-update.ts`).
pub fn mac_self_update_target<F>(exec_path: &str, can_write: F) -> Option<String>
where
    F: Fn(&str) -> bool,
{
    let bundle = posix_dirname(posix_dirname(posix_dirname(exec_path)));
    if !bundle.ends_with(".app") {
        return None;
    }
    if bundle.starts_with("/Volumes/") || bundle.contains("/AppTranslocation/") {
        return None;
    }
    if can_write(posix_dirname(bundle)) {
        Some(bundle.to_owned())
    } else {
        None
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandOutput {
    pub code: Option<i32>,
    pub output: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestImage {
    pub path: PathBuf,
    pub version: String,
}

pub type FetchReleasesFn = Arc<dyn Fn() -> Result<serde_json::Value, String> + Send + Sync>;
pub type DownloadFn =
    Arc<dyn Fn(&str, &Path, &dyn Fn(u8)) -> Result<(), String> + Send + Sync>;
pub type RunCommandFn = Arc<dyn Fn(&str, &[&str]) -> CommandOutput + Send + Sync>;
pub type StartSwapFn = Arc<dyn Fn(&Path, &[String]) + Send + Sync>;
pub type QuitFn = Arc<dyn Fn() + Send + Sync>;

pub struct MacUpdaterOptions {
    pub current_version: String,
    pub arch: String,
    pub bundle: PathBuf,
    pub cache_folder: PathBuf,
    pub quit: QuitFn,
    pub pid: Option<u32>,
    pub fetch_releases: Option<FetchReleasesFn>,
    pub download: Option<DownloadFn>,
    pub run: Option<RunCommandFn>,
    pub start_swap: Option<StartSwapFn>,
    pub test_image: Option<TestImage>,
    pub relaunch: Vec<String>,
}

fn parse_strict_triple(version: &str) -> Option<[u128; 3]> {
    let mut parts = version.split('.');
    let major = parts.next()?;
    let minor = parts.next()?;
    let patch = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    for part in [major, minor, patch] {
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
    }
    Some([
        major.parse().ok()?,
        minor.parse().ok()?,
        patch.parse().ok()?,
    ])
}

fn parse_mac_dmg_version(file_name: &str) -> Option<[u128; 3]> {
    let rest = file_name.strip_prefix("Locust-")?;
    let version = rest
        .strip_suffix("-mac-arm64.dmg")
        .or_else(|| rest.strip_suffix("-mac-x64.dmg"))?;
    parse_strict_triple(version)
}

/// Only at startup, before an update can be downloaded or installed (`removeInstalledImages` in
/// `main/mac-self-update.ts`).
pub fn remove_installed_images(folder: &Path, current_version: &str) {
    let Some(current) = parse_strict_triple(current_version) else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_file() {
            continue;
        }
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else {
            continue;
        };
        let Some(version) = parse_mac_dmg_version(name_str) else {
            continue;
        };
        if version > current {
            continue;
        }
        let _ = std::fs::remove_file(entry.path());
    }
}

static COUNTER: AtomicU64 = AtomicU64::new(1);

fn unique_temp_path(prefix: &str, suffix: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("{prefix}{}-{nanos}-{n}{suffix}", std::process::id()))
}

struct MacUpdaterInner {
    auto_download: bool,
    allow_prerelease: bool,
    channel: Option<String>,
    allow_downgrade: Option<bool>,
    auto_install_on_app_quit: bool,
    listeners: HashMap<String, Vec<EventListener>>,
    found: Option<crate::updates::MacRelease>,
    ready: Option<String>,
}

#[derive(Clone)]
pub struct MacUpdater {
    options: Arc<MacUpdaterOptions>,
    staged: PathBuf,
    inner: Arc<Mutex<MacUpdaterInner>>,
}

pub fn create_mac_updater(options: MacUpdaterOptions) -> MacUpdater {
    remove_installed_images(&options.cache_folder, &options.current_version);
    let staged = options
        .bundle
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(".Locust-update.app");
    MacUpdater {
        options: Arc::new(options),
        staged,
        inner: Arc::new(Mutex::new(MacUpdaterInner {
            auto_download: false,
            allow_prerelease: false,
            channel: None,
            allow_downgrade: None,
            auto_install_on_app_quit: false,
            listeners: HashMap::new(),
            found: None,
            ready: None,
        })),
    }
}

impl MacUpdater {
    fn emit(&self, event: &str, payload: Option<EventPayload>) {
        let listeners = self
            .inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .listeners
            .get(event)
            .cloned()
            .unwrap_or_default();
        for listener in listeners {
            listener(payload.clone());
        }
    }

    fn run_cmd(&self, command: &str, args: &[&str]) -> CommandOutput {
        match &self.options.run {
            Some(run) => run(command, args),
            None => CommandOutput {
                code: None,
                output: NO_FEED_MESSAGE.to_owned(),
            },
        }
    }

    fn prepare(&self, release: &crate::updates::MacRelease) -> Result<(), String> {
        let image = if let Some(test_image) = &self.options.test_image {
            test_image.path.clone()
        } else {
            std::fs::create_dir_all(&self.options.cache_folder).map_err(|e| e.to_string())?;
            let img = self.options.cache_folder.join(format!(
                "Locust-{}-mac-{}.dmg",
                release.version, self.options.arch
            ));
            let download = self
                .options
                .download
                .as_ref()
                .ok_or_else(|| NO_FEED_MESSAGE.to_owned())?;
            download(&release.url, &img, &|percent| {
                self.emit(
                    "download-progress",
                    Some(EventPayload::percent(f64::from(percent))),
                );
            })?;
            let facts = std::fs::metadata(&img).map_err(|e| e.to_string())?;
            if let Some(expected_size) = release.size
                && facts.len() != expected_size
            {
                return Err("The download is not the size the release states.".into());
            }
            let Some(expected_sha256) = &release.sha256 else {
                return Err(
                    "The release does not say what the download should be, so it was not installed."
                        .into(),
                );
            };
            let bytes = std::fs::read(&img).map_err(|e| e.to_string())?;
            let digest = format!("{:x}", Sha256::digest(&bytes));
            if &digest != expected_sha256 {
                return Err("The download does not match the release it came from.".into());
            }
            img
        };

        let mount = unique_temp_path("locust-update-mount-", "");
        std::fs::create_dir_all(&mount).map_err(|e| e.to_string())?;
        let image_str = image.to_string_lossy().into_owned();
        let mount_str = mount.to_string_lossy().into_owned();
        let attached = self.run_cmd(
            "hdiutil",
            &[
                "attach",
                &image_str,
                "-nobrowse",
                "-readonly",
                "-noautoopen",
                "-mountpoint",
                &mount_str,
            ],
        );
        if attached.code != Some(0) {
            let _ = std::fs::remove_dir_all(&mount);
            let detail: String = attached.output.trim().chars().take(200).collect();
            return Err(format!("The disk image would not open: {detail}"));
        }

        let stage_result = (|| -> Result<(), String> {
            let source = mount.join("Locust.app");
            if !source.exists() {
                return Err("The disk image holds no Locust.app.".into());
            }
            let _ = std::fs::remove_dir_all(&self.staged);
            let source_str = source.to_string_lossy().into_owned();
            let staged_str = self.staged.to_string_lossy().into_owned();
            let copied = self.run_cmd("ditto", &[&source_str, &staged_str]);
            if copied.code != Some(0) {
                let detail: String = copied.output.trim().chars().take(200).collect();
                return Err(format!("The new Locust could not be copied: {detail}"));
            }
            let _ = self.run_cmd("xattr", &["-dr", "com.apple.quarantine", &staged_str]);
            Ok(())
        })();

        let _ = self.run_cmd("hdiutil", &["detach", &mount_str, "-force"]);
        let _ = std::fs::remove_dir_all(&mount);

        stage_result?;
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).ready = Some(release.version.clone());
        self.emit(
            "update-downloaded",
            Some(EventPayload::version(release.version.clone())),
        );
        Ok(())
    }
}

impl UpdaterLike for MacUpdater {
    fn auto_download(&self) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .auto_download
    }
    fn set_auto_download(&mut self, value: bool) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .auto_download = value;
    }
    fn allow_prerelease(&self) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .allow_prerelease
    }
    fn set_allow_prerelease(&mut self, value: bool) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .allow_prerelease = value;
    }
    fn channel(&self) -> Option<String> {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .channel
            .clone()
    }
    fn set_channel(&mut self, value: Option<&str>) {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).channel = value.map(str::to_owned);
    }
    fn allow_downgrade(&self) -> Option<bool> {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .allow_downgrade
    }
    fn set_allow_downgrade(&mut self, value: bool) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .allow_downgrade = Some(value);
    }
    fn auto_install_on_app_quit(&self) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .auto_install_on_app_quit
    }
    fn set_auto_install_on_app_quit(&mut self, value: bool) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .auto_install_on_app_quit = value;
    }

    fn check_for_updates(&mut self) -> Result<Option<CheckForUpdatesResult>, String> {
        let release = if let Some(test_image) = &self.options.test_image {
            Some(crate::updates::MacRelease {
                version: test_image.version.clone(),
                url: String::new(),
                size: None,
                sha256: None,
            })
        } else {
            let fetch = self
                .options
                .fetch_releases
                .as_ref()
                .ok_or_else(|| NO_FEED_MESSAGE.to_owned())?;
            let list = fetch()?;
            newer_mac_release(&list, &self.options.current_version, &self.options.arch)
        };
        let Some(release) = release else {
            return Ok(Some(CheckForUpdatesResult {
                update_info: UpdateInfo {
                    version: self.options.current_version.clone(),
                    files: None,
                },
                is_update_available: Some(false),
                download_result: None,
            }));
        };
        let (already_ready, auto_download) = {
            let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            guard.found = Some(release.clone());
            (
                guard.ready.as_deref() == Some(release.version.as_str()),
                guard.auto_download,
            )
        };
        if already_ready {
            self.emit(
                "update-downloaded",
                Some(EventPayload::version(release.version.clone())),
            );
            return Ok(Some(CheckForUpdatesResult {
                update_info: UpdateInfo {
                    version: release.version,
                    files: None,
                },
                is_update_available: Some(true),
                download_result: None,
            }));
        }
        let download_result = if auto_download {
            Some(self.download_update())
        } else {
            None
        };
        Ok(Some(CheckForUpdatesResult {
            update_info: UpdateInfo {
                version: release.version,
                files: None,
            },
            is_update_available: Some(true),
            download_result,
        }))
    }

    fn download_update(&mut self) -> Result<(), String> {
        let found = self
            .inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .found
            .clone();
        let Some(release) = found else {
            return Err("There is nothing to download.".into());
        };
        if let Err(err) = self.prepare(&release) {
            self.emit("error", Some(EventPayload::error(err.clone())));
            return Err(err);
        }
        Ok(())
    }

    fn quit_and_install(&mut self, _is_silent: Option<bool>, _is_force_run_after: Option<bool>) {
        let ready = self
            .inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .ready
            .clone();
        if ready.is_none() || !self.staged.exists() {
            return;
        }
        let script = unique_temp_path("locust-swap-", ".sh");
        if std::fs::write(&script, SWAP_SCRIPT).is_err() {
            return;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755));
        }
        let mut args = vec![
            self.options.pid.unwrap_or_else(std::process::id).to_string(),
            self.options.bundle.to_string_lossy().into_owned(),
            self.staged.to_string_lossy().into_owned(),
        ];
        if self.options.test_image.is_some() {
            args.extend(self.options.relaunch.iter().cloned());
        }
        if let Some(start_swap) = &self.options.start_swap {
            start_swap(&script, &args);
        }
        (self.options.quit)();
    }

    fn on(&mut self, event: &str, listener: EventListener) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .listeners
            .entry(event.to_owned())
            .or_default()
            .push(listener);
    }
}

/// The self-updater for this copy, reading `feed`, when it runs from a `Locust.app` it can replace; None otherwise.
/// Only `updates::app_updater` asks for it, and only once the self-update switch is on with a feed set. It reads the
/// releases list and nothing more: download, disk-image and swap seams stay unset, so it never downloads or replaces
/// anything (each refuses with `NO_FEED_MESSAGE`).
pub fn this_mac(feed: &str, current_version: &str) -> Option<MacUpdater> {
    let exec = std::env::current_exe().ok()?;
    let bundle = mac_self_update_target(&exec.to_string_lossy(), |folder| {
        std::fs::metadata(folder).is_ok_and(|facts| !facts.permissions().readonly())
    })?;
    let cache_folder = crate::store::Store::local().ok()?.root().join("mac-updates");
    let feed = feed.to_owned();
    let fetch: FetchReleasesFn = Arc::new(move || {
        let agent = ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(30)).build();
        agent.get(&feed).call().map_err(|e| e.to_string())?.into_json::<serde_json::Value>().map_err(|e| e.to_string())
    });
    Some(create_mac_updater(MacUpdaterOptions {
        current_version: current_version.to_owned(),
        arch: if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" }.to_owned(),
        bundle: PathBuf::from(bundle),
        cache_folder,
        quit: Arc::new(|| {}),
        pid: Some(std::process::id()),
        fetch_releases: Some(fetch),
        download: None,
        run: None,
        start_swap: None,
        test_image: None,
        relaunch: Vec::new(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::AtomicUsize;

    const BASE: &str = "https://dl.locust.lol";
    const IMAGE: &[u8] = b"a disk image, for the test";

    fn image_sha() -> String {
        format!("{:x}", Sha256::digest(IMAGE))
    }

    fn releases(sha256: &str) -> serde_json::Value {
        json!([{
            "tag_name": "0.517.0",
            "assets": [{
                "name": "Locust-0.517.0-mac-arm64.dmg",
                "browser_download_url": format!("{BASE}/Locust-0.517.0-mac-arm64.dmg"),
                "size": IMAGE.len(),
                "digest": format!("sha256:{sha256}"),
            }]
        }])
    }

    struct Harness {
        root: PathBuf,
        updater: MacUpdater,
        commands: Arc<Mutex<Vec<String>>>,
        swaps: Arc<Mutex<Vec<(PathBuf, Vec<String>)>>>,
        events: Arc<Mutex<Vec<String>>>,
        quits: Arc<AtomicUsize>,
    }

    impl Drop for Harness {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn setup(digest: &str, test_image: Option<TestImage>, relaunch: Vec<String>) -> Harness {
        let root = unique_temp_path("locust-mac-update-", "");
        std::fs::create_dir_all(&root).unwrap();
        let commands: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let swaps: Arc<Mutex<Vec<(PathBuf, Vec<String>)>>> = Arc::new(Mutex::new(Vec::new()));
        let events: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let quits = Arc::new(AtomicUsize::new(0));

        let commands_run = Arc::clone(&commands);
        let swaps_cb = Arc::clone(&swaps);
        let quits_cb = Arc::clone(&quits);
        let digest_owned = digest.to_owned();

        let mut updater = create_mac_updater(MacUpdaterOptions {
            current_version: "0.516.0".into(),
            arch: "arm64".into(),
            bundle: root.join("Locust.app"),
            cache_folder: root.join("cache"),
            quit: Arc::new(move || {
                quits_cb.fetch_add(1, Ordering::SeqCst);
            }),
            pid: Some(4242),
            fetch_releases: Some(Arc::new(move || Ok(releases(&digest_owned)))),
            download: Some(Arc::new(|_url, to, on_percent| {
                on_percent(100);
                std::fs::write(to, IMAGE).map_err(|e| e.to_string())
            })),
            run: Some(Arc::new(move |command, args| {
                let mut line = command.to_owned();
                for a in args {
                    line.push(' ');
                    line.push_str(a);
                }
                commands_run.lock().unwrap().push(line);
                if command == "ditto"
                    && let Some(dest) = args.get(1)
                {
                    let _ = std::fs::create_dir_all(dest);
                }
                if command == "hdiutil"
                    && args.first() == Some(&"attach")
                    && let Some(mount) = args.last()
                {
                    let _ = std::fs::create_dir_all(Path::new(mount).join("Locust.app"));
                }
                CommandOutput {
                    code: Some(0),
                    output: String::new(),
                }
            })),
            start_swap: Some(Arc::new(move |script, args| {
                swaps_cb
                    .lock()
                    .unwrap()
                    .push((script.to_path_buf(), args.to_vec()));
            })),
            test_image,
            relaunch,
        });

        for event in ["download-progress", "update-downloaded", "error"] {
            let events_cb = Arc::clone(&events);
            let name = event.to_owned();
            updater.on(
                event,
                Arc::new(move |_| {
                    events_cb.lock().unwrap().push(name.clone());
                }),
            );
        }
        updater.set_auto_download(true);

        Harness {
            root,
            updater,
            commands,
            swaps,
            events,
            quits,
        }
    }

    // case: a Locust.app in a folder it can write
    #[test]
    fn a_locust_app_in_a_folder_it_can_write() {
        assert_eq!(
            mac_self_update_target("/Applications/Locust.app/Contents/MacOS/Locust", |_| true),
            Some("/Applications/Locust.app".into())
        );
        assert_eq!(
            mac_self_update_target("/Applications/Locust.app/Contents/MacOS/Locust", |_| false),
            None
        );
    }

    // case: not from the disk image, nor macOS's translocated copy of a download, nor a bare binary
    #[test]
    fn not_from_the_disk_image_nor_macoss_translocated_copy_of_a_download_nor_a_bare_binary() {
        assert_eq!(
            mac_self_update_target(
                "/Volumes/Locust 0.516.0/Locust.app/Contents/MacOS/Locust",
                |_| true
            ),
            None
        );
        assert_eq!(
            mac_self_update_target(
                "/private/var/folders/x/AppTranslocation/ABC/d/Locust.app/Contents/MacOS/Locust",
                |_| true
            ),
            None
        );
        assert_eq!(
            mac_self_update_target("/usr/local/bin/locust", |_| true),
            None
        );
    }

    // case: finds the newer release, downloads it, checks it, and stages its app beside the running one
    #[test]
    fn finds_the_newer_release_downloads_it_checks_it_and_stages_its_app_beside_the_running_one() {
        let mut h = setup(&image_sha(), None, Vec::new());
        let result = h.updater.check_for_updates().unwrap().unwrap();
        assert_eq!(result.update_info.version, "0.517.0");
        assert_eq!(result.is_update_available, Some(true));
        assert_eq!(result.download_result, Some(Ok(())));
        assert!(
            h.events
                .lock()
                .unwrap()
                .contains(&"update-downloaded".to_owned())
        );
        let cmds = h.commands.lock().unwrap().clone();
        assert!(cmds.iter().any(|l| {
            l.starts_with("hdiutil attach") && l.contains("-nobrowse") && l.contains("-readonly")
        }));
        assert!(
            cmds.iter()
                .any(|l| l.starts_with("ditto ") && l.ends_with(".Locust-update.app"))
        );
        assert!(
            cmds.iter()
                .any(|l| l.starts_with("xattr -dr com.apple.quarantine"))
        );
        assert!(cmds.iter().any(|l| l.starts_with("hdiutil detach")));
    }

    // case: refuses a download that does not match the release, and installs nothing
    #[test]
    fn refuses_a_download_that_does_not_match_the_release_and_installs_nothing() {
        let mut h = setup(&"0".repeat(64), None, Vec::new());
        let result = h.updater.check_for_updates().unwrap().unwrap();
        let err = result
            .download_result
            .expect("download attempted")
            .expect_err("digest mismatch must fail");
        assert!(err.contains("does not match"), "{err}");
        let evs = h.events.lock().unwrap().clone();
        assert!(evs.contains(&"error".to_owned()));
        assert!(!evs.contains(&"update-downloaded".to_owned()));
        assert!(
            !h.commands
                .lock()
                .unwrap()
                .iter()
                .any(|l| l.starts_with("ditto"))
        );
    }

    // case: on Restart and install, hands the swap to the shell and quits
    #[test]
    fn on_restart_and_install_hands_the_swap_to_the_shell_and_quits() {
        let mut h = setup(&image_sha(), None, Vec::new());
        let result = h.updater.check_for_updates().unwrap().unwrap();
        assert_eq!(result.download_result, Some(Ok(())));
        h.updater.quit_and_install(Some(true), Some(true));
        assert_eq!(h.quits.load(Ordering::SeqCst), 1);
        let swaps = h.swaps.lock().unwrap().clone();
        assert_eq!(swaps.len(), 1);
        assert_eq!(
            swaps[0].1,
            vec![
                "4242".to_owned(),
                h.root.join("Locust.app").to_string_lossy().into_owned(),
                h.root
                    .join(".Locust-update.app")
                    .to_string_lossy()
                    .into_owned(),
            ]
        );
        assert_eq!(std::fs::read_to_string(&swaps[0].0).unwrap(), SWAP_SCRIPT);
        let _ = std::fs::remove_file(&swaps[0].0);
    }

    // case: a person's update reopens Locust as it always has: the check's relaunch arguments are never passed
    #[test]
    fn a_persons_update_reopens_locust_as_it_always_has_the_checks_relaunch_arguments_are_never_passed()
    {
        let mut h = setup(
            &image_sha(),
            None,
            vec!["--user-data-dir=/elsewhere".into()],
        );
        let result = h.updater.check_for_updates().unwrap().unwrap();
        assert_eq!(result.download_result, Some(Ok(())));
        h.updater.quit_and_install(Some(true), Some(true));
        assert_eq!(h.quits.load(Ordering::SeqCst), 1);
        let swaps = h.swaps.lock().unwrap().clone();
        assert_eq!(swaps[0].1.len(), 3);
        let _ = std::fs::remove_file(&swaps[0].0);
    }

    // case: the Mac CI's check reopens its copy on its own profile, so on Colin's Mac it never opens his
    #[test]
    fn the_mac_cis_check_reopens_its_copy_on_its_own_profile_so_on_colins_mac_it_never_opens_his() {
        let mut h = setup(
            &image_sha(),
            Some(TestImage {
                path: PathBuf::from("/tmp/Locust-9.9.9-mac-arm64.dmg"),
                version: "9.9.9".into(),
            }),
            vec![
                "--user-data-dir=/tmp/check-profile".into(),
                "--use-mock-keychain".into(),
            ],
        );
        let result = h.updater.check_for_updates().unwrap().unwrap();
        assert_eq!(result.download_result, Some(Ok(())));
        h.updater.quit_and_install(Some(true), Some(true));
        assert_eq!(h.quits.load(Ordering::SeqCst), 1);
        let swaps = h.swaps.lock().unwrap().clone();
        assert_eq!(
            &swaps[0].1[3..],
            &[
                "--user-data-dir=/tmp/check-profile".to_owned(),
                "--use-mock-keychain".to_owned()
            ]
        );
        let _ = std::fs::remove_file(&swaps[0].0);
    }

    // case: offers nothing when this is already the newest
    #[test]
    fn offers_nothing_when_this_is_already_the_newest() {
        let root = unique_temp_path("locust-mac-update-", "");
        std::fs::create_dir_all(&root).unwrap();
        let sha = image_sha();
        let mut updater = create_mac_updater(MacUpdaterOptions {
            current_version: "0.517.0".into(),
            arch: "arm64".into(),
            bundle: root.join("Locust.app"),
            cache_folder: root.clone(),
            quit: Arc::new(|| {}),
            pid: None,
            fetch_releases: Some(Arc::new(move || Ok(releases(&sha)))),
            download: None,
            run: None,
            start_swap: None,
            test_image: None,
            relaunch: Vec::new(),
        });
        let result = updater.check_for_updates().unwrap().unwrap();
        assert_eq!(result.is_update_available, Some(false));
        let _ = std::fs::remove_dir_all(&root);
    }

    // case: puts the new app in the old one's place and leaves nothing behind
    #[test]
    fn puts_the_new_app_in_the_old_ones_place_and_leaves_nothing_behind() {
        let root = unique_temp_path("locust-swap-", "");
        std::fs::create_dir_all(&root).unwrap();
        let bin = root.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let fake_open = bin.join("open");
        std::fs::write(&fake_open, "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&fake_open, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let app = root.join("Locust.app");
        let next = root.join(".Locust-update.app");
        std::fs::create_dir_all(&app).unwrap();
        std::fs::write(app.join("version"), "old").unwrap();
        std::fs::create_dir_all(&next).unwrap();
        std::fs::write(next.join("version"), "new").unwrap();
        let script = root.join("swap.sh");
        std::fs::write(&script, SWAP_SCRIPT).unwrap();
        let path_env = format!(
            "{}:{}",
            bin.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let _ = std::process::Command::new("sh")
            .arg(&script)
            .arg("999999")
            .arg(&app)
            .arg(&next)
            .env("PATH", path_env)
            .status();
        assert_eq!(std::fs::read_to_string(app.join("version")).unwrap(), "new");
        assert!(!next.exists());
        assert!(!root.join("Locust.app.replaced").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    // case: opens the app as a person expects, and only with more arguments, behind the windows in front, with them
    #[test]
    fn opens_the_app_as_a_person_expects_and_only_with_more_arguments_behind_the_windows_in_front_with_them()
    {
        let root = unique_temp_path("locust-swap-", "");
        std::fs::create_dir_all(&root).unwrap();
        let bin = root.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let opened_file = root.join("opened");
        let fake_open = bin.join("open");
        std::fs::write(
            &fake_open,
            format!("#!/bin/sh\necho \"$@\" >> \"{}\"\n", opened_file.display()),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&fake_open, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let script = root.join("swap.sh");
        std::fs::write(&script, SWAP_SCRIPT).unwrap();
        let path_env = format!(
            "{}:{}",
            bin.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        for extra in [
            Vec::<&str>::new(),
            vec!["--user-data-dir=/tmp/check profile", "--use-mock-keychain"],
        ] {
            let app = root.join("Locust.app");
            std::fs::create_dir_all(&app).unwrap();
            let _ = std::process::Command::new("sh")
                .arg(&script)
                .arg("999999")
                .arg(&app)
                .arg(root.join("missing.app"))
                .args(&extra)
                .env("PATH", &path_env)
                .status();
        }
        let opened: Vec<String> = std::fs::read_to_string(&opened_file)
            .unwrap()
            .trim()
            .lines()
            .map(str::to_owned)
            .collect();
        assert_eq!(opened.len(), 2);
        assert!(opened[0].ends_with("Locust.app"), "{}", opened[0]);
        assert!(!opened[0].starts_with("-g "), "{}", opened[0]);
        assert!(
            opened[1].starts_with("-g ")
                && opened[1].ends_with(
                    "Locust.app --args --user-data-dir=/tmp/check profile --use-mock-keychain"
                ),
            "{}",
            opened[1]
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    // case: and when the new one cannot be moved in, puts the old one back
    #[test]
    fn and_when_the_new_one_cannot_be_moved_in_puts_the_old_one_back() {
        let root = unique_temp_path("locust-swap-", "");
        std::fs::create_dir_all(&root).unwrap();
        let bin = root.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let fake_open = bin.join("open");
        std::fs::write(&fake_open, "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&fake_open, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let app = root.join("Locust.app");
        std::fs::create_dir_all(&app).unwrap();
        std::fs::write(app.join("version"), "old").unwrap();
        let script = root.join("swap.sh");
        std::fs::write(&script, SWAP_SCRIPT).unwrap();
        let path_env = format!(
            "{}:{}",
            bin.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let _ = std::process::Command::new("sh")
            .arg(&script)
            .arg("999999")
            .arg(&app)
            .arg(root.join("missing.app"))
            .env("PATH", path_env)
            .status();
        assert_eq!(std::fs::read_to_string(app.join("version")).unwrap(), "old");
        let _ = std::fs::remove_dir_all(&root);
    }
}

#[cfg(test)]
mod discard_installed_images_tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    fn setup_cache() -> (PathBuf, PathBuf) {
        let root = unique_temp_path("locust-image-cleanup-", "");
        let cache = root.join("cache");
        std::fs::create_dir_all(&cache).unwrap();
        (root, cache)
    }

    // case: removes installed images at startup and preserves newer images and everything outside its exact file names
    #[test]
    fn removes_installed_images_at_startup_and_preserves_newer_images_and_everything_outside_its_exact_file_names()
    {
        let (root, cache_folder) = setup_cache();
        let removed = [
            "Locust-0.842.0-mac-arm64.dmg",
            "Locust-0.843.0-mac-arm64.dmg",
            "Locust-0.9.0-mac-x64.dmg",
        ];
        let kept = [
            "Locust-0.844.0-mac-arm64.dmg",
            "Locust-1.0.0-mac-x64.dmg",
            "Other-0.842.0-mac-arm64.dmg",
            "Locust-0.842.0-mac-arm64.dmg.part",
            "Locust-0.842.0-beta-mac-arm64.dmg",
            "Locust-0.842.0-mac-unknown.dmg",
        ];
        for name in removed.iter().chain(kept.iter()) {
            std::fs::write(cache_folder.join(name), "image").unwrap();
        }
        let directory = "Locust-0.840.0-mac-arm64.dmg";
        std::fs::create_dir(cache_folder.join(directory)).unwrap();
        let outside = root.join(removed[0]);
        std::fs::write(&outside, "outside").unwrap();
        let link = "Locust-0.841.0-mac-arm64.dmg";
        let mut linked = false;
        #[cfg(unix)]
        {
            if std::os::unix::fs::symlink(&outside, cache_folder.join(link)).is_ok() {
                linked = true;
            }
        }
        let run_calls = Arc::new(AtomicUsize::new(0));
        let run_calls_in = Arc::clone(&run_calls);
        let mut updater = create_mac_updater(MacUpdaterOptions {
            current_version: "0.843.0".into(),
            arch: "arm64".into(),
            bundle: root.join("Locust.app"),
            cache_folder: cache_folder.clone(),
            quit: Arc::new(|| {}),
            pid: None,
            fetch_releases: Some(Arc::new(|| Ok(serde_json::json!([])))),
            download: None,
            run: Some(Arc::new(move |_, _| {
                run_calls_in.fetch_add(1, Ordering::SeqCst);
                CommandOutput {
                    code: Some(0),
                    output: String::new(),
                }
            })),
            start_swap: None,
            test_image: None,
            relaunch: Vec::new(),
        });

        let mut remaining: Vec<String> = std::fs::read_dir(&cache_folder)
            .unwrap()
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        remaining.sort();
        let mut expected: Vec<String> = kept
            .iter()
            .map(|s| (*s).to_owned())
            .chain(std::iter::once(directory.to_owned()))
            .collect();
        if linked {
            expected.push(link.to_owned());
        }
        expected.sort();
        assert_eq!(remaining, expected);
        assert!(outside.exists());

        std::fs::write(cache_folder.join(removed[0]), "later").unwrap();
        let _ = updater.check_for_updates();
        updater.quit_and_install(None, None);
        assert!(cache_folder.join(removed[0]).exists());
        assert_eq!(run_calls.load(Ordering::SeqCst), 0);
        let _ = std::fs::remove_dir_all(&root);
    }

    // case: leaves ambiguous running versions alone and tolerates a cache that cannot be listed
    #[test]
    fn leaves_ambiguous_running_versions_alone_and_tolerates_a_cache_that_cannot_be_listed() {
        let (root, cache_folder) = setup_cache();
        let image = cache_folder.join("Locust-0.843.0-mac-arm64.dmg");
        std::fs::write(&image, "image").unwrap();
        for current_version in ["0.843.0-beta", "invalid"] {
            let _ = create_mac_updater(MacUpdaterOptions {
                current_version: current_version.into(),
                arch: "arm64".into(),
                bundle: root.join("Locust.app"),
                cache_folder: cache_folder.clone(),
                quit: Arc::new(|| {}),
                pid: None,
                fetch_releases: None,
                download: None,
                run: None,
                start_swap: None,
                test_image: None,
                relaunch: Vec::new(),
            });
            assert!(image.exists());
        }
        let _ = create_mac_updater(MacUpdaterOptions {
            current_version: "0.843.0".into(),
            arch: "arm64".into(),
            bundle: root.join("Locust.app"),
            cache_folder: image.clone(),
            quit: Arc::new(|| {}),
            pid: None,
            fetch_releases: None,
            download: None,
            run: None,
            start_swap: None,
            test_image: None,
            relaunch: Vec::new(),
        });
        let _ = create_mac_updater(MacUpdaterOptions {
            current_version: "0.843.0".into(),
            arch: "arm64".into(),
            bundle: root.join("Locust.app"),
            cache_folder: root.join("missing"),
            quit: Arc::new(|| {}),
            pid: None,
            fetch_releases: None,
            download: None,
            run: None,
            start_swap: None,
            test_image: None,
            relaunch: Vec::new(),
        });
        let _ = std::fs::remove_dir_all(&root);
    }
}
