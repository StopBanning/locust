//! How this Linux copy was installed (Electron `main/linux-install.ts`), which decides how it updates.
//!
//! An AppImage is one file the person keeps where they like (`$APPIMAGE`). A `.deb` or `.rpm` belongs to the system's
//! package manager (`package-type` beside the app's archive): Locust never downloads or installs its update itself, and
//! hands over the package URL from the feed's file list. Anything else is not an installed copy and changes nothing.

use crate::updates::UpdateFile;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinuxInstall {
    AppImage,
    Deb,
    Rpm,
}

impl LinuxInstall {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AppImage => "appimage",
            Self::Deb => "deb",
            Self::Rpm => "rpm",
        }
    }
}

pub fn linux_install_of<F>(
    env: &HashMap<&str, &str>,
    resources_path: &str,
    read: F,
) -> Option<LinuxInstall>
where
    F: Fn(&str) -> Result<String, String>,
{
    if env.get("APPIMAGE").is_some_and(|v| !v.is_empty()) {
        return Some(LinuxInstall::AppImage);
    }
    let path = format!("{}/package-type", resources_path.trim_end_matches('/'));
    let kind = read(&path).ok()?;
    match kind.trim() {
        "deb" => Some(LinuxInstall::Deb),
        "rpm" => Some(LinuxInstall::Rpm),
        _ => None,
    }
}

/// The package a `.deb` or `.rpm` copy is handed for `version`, from the feed's own file list (`linuxPackageUrl` in
/// `main/linux-install.ts`).
pub fn linux_package_url(
    install: LinuxInstall,
    files: Option<&[UpdateFile]>,
    downloads: &str,
) -> Option<String> {
    let ext = match install {
        LinuxInstall::Deb => ".deb",
        LinuxInstall::Rpm => ".rpm",
        LinuxInstall::AppImage => return None,
    };
    let name = files?
        .iter()
        .filter_map(|f| f.url.as_deref())
        .find(|u| u.ends_with(ext))?;
    let base = url::Url::parse(downloads).ok()?;
    base.join(name).ok().map(|u| u.to_string())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinuxUpdateTest {
    pub feed_url: Option<String>,
    pub version: Option<String>,
}

fn on_this_machine(address: &str) -> bool {
    let Ok(parsed) = url::Url::parse(address) else {
        return false;
    };
    parsed.scheme() == "http"
        && matches!(parsed.host_str(), Some("127.0.0.1" | "localhost"))
        && parsed.username().is_empty()
        && parsed.password().is_none()
}

pub fn linux_update_test_of(
    install: Option<LinuxInstall>,
    env: &HashMap<&str, &str>,
) -> Option<LinuxUpdateTest> {
    if install != Some(LinuxInstall::AppImage) {
        return None;
    }
    let feed_url = env
        .get("LOCUST_LINUX_UPDATE_TEST_FEED")
        .copied()
        .filter(|addr| on_this_machine(addr))
        .map(str::to_owned);
    let version = if env.get("APPIMAGE_SILENT_INSTALL") == Some(&"true") {
        env.get("LOCUST_LINUX_UPDATE_TEST_VERSION")
            .copied()
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
    } else {
        None
    };
    if feed_url.is_none() && version.is_none() {
        return None;
    }
    Some(LinuxUpdateTest { feed_url, version })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::updates::test_support::{FakeUpdater, default_options};
    use crate::updates::{
        AppUpdateErrorCode, AppUpdatePhase, AppUpdateState, CheckForUpdatesResult, UpdateInfo,
        UpdaterLike, create_update_service,
    };
    use std::sync::Arc;

    fn none(_file: &str) -> Result<String, String> {
        Err("ENOENT".into())
    }

    // case: is an AppImage when the AppImage runtime names its file
    #[test]
    fn is_an_appimage_when_the_appimage_runtime_names_its_file() {
        let env = HashMap::from([("APPIMAGE", "/home/ian/Apps/Locust.AppImage")]);
        assert_eq!(
            linux_install_of(&env, "/tmp/.mount_Locust/resources", none),
            Some(LinuxInstall::AppImage)
        );
    }

    // case: is a .deb or an .rpm by the package-type file electron-builder leaves beside the app's archive
    #[test]
    fn is_a_deb_or_an_rpm_by_the_package_type_file_electron_builder_leaves_beside_the_apps_archive()
    {
        let env = HashMap::new();
        assert_eq!(
            linux_install_of(&env, "/opt/Locust/resources", |file| {
                if file.replace('\\', "/") == "/opt/Locust/resources/package-type" {
                    Ok("deb\n".into())
                } else {
                    none(file)
                }
            }),
            Some(LinuxInstall::Deb)
        );
        assert_eq!(
            linux_install_of(&env, "/opt/Locust/resources", |_| Ok("rpm".into())),
            Some(LinuxInstall::Rpm)
        );
    }

    // case: is not installed at all when neither says so, or the file names something else
    #[test]
    fn is_not_installed_at_all_when_neither_says_so_or_the_file_names_something_else() {
        let env = HashMap::new();
        assert_eq!(
            linux_install_of(&env, "/work/release/linux-unpacked/resources", none),
            None
        );
        let empty_appimage = HashMap::from([("APPIMAGE", "")]);
        assert_eq!(
            linux_install_of(&empty_appimage, "/r", |_| Ok("pacman".into())),
            None
        );
    }

    // case: hands a .deb copy the feed's .deb, and an .rpm copy its .rpm, as full addresses
    #[test]
    fn hands_a_deb_copy_the_feeds_deb_and_an_rpm_copy_its_rpm_as_full_addresses() {
        let files = vec![
            UpdateFile {
                url: Some("Locust-0.790.0-x86_64.AppImage".into()),
            },
            UpdateFile {
                url: Some("Locust-0.790.0-linux-amd64.deb".into()),
            },
            UpdateFile {
                url: Some("Locust-0.790.0-linux-x86_64.rpm".into()),
            },
        ];
        assert_eq!(
            linux_package_url(LinuxInstall::Deb, Some(&files), "https://dl.locust.lol/"),
            Some("https://dl.locust.lol/Locust-0.790.0-linux-amd64.deb".into())
        );
        assert_eq!(
            linux_package_url(LinuxInstall::Rpm, Some(&files), "https://dl.locust.lol/"),
            Some("https://dl.locust.lol/Locust-0.790.0-linux-x86_64.rpm".into())
        );
        assert_eq!(
            linux_package_url(
                LinuxInstall::Deb,
                Some(&[UpdateFile { url: None }]),
                "https://dl.locust.lol/"
            ),
            None
        );
        assert_eq!(
            linux_package_url(LinuxInstall::Deb, None, "https://dl.locust.lol/"),
            None
        );
    }

    // case: leaves the Linux update test seam off by default, and activates it only on an AppImage copy
    #[test]
    fn leaves_the_linux_update_test_seam_off_by_default_and_activates_it_only_on_an_appimage_copy()
    {
        assert_eq!(
            linux_update_test_of(Some(LinuxInstall::AppImage), &HashMap::new()),
            None
        );
        assert_eq!(
            linux_update_test_of(
                Some(LinuxInstall::AppImage),
                &HashMap::from([
                    ("LOCUST_LINUX_UPDATE_TEST_FEED", ""),
                    ("LOCUST_LINUX_UPDATE_TEST_VERSION", "")
                ])
            ),
            None
        );
        assert_eq!(
            linux_update_test_of(
                Some(LinuxInstall::AppImage),
                &HashMap::from([("LOCUST_LINUX_UPDATE_TEST_VERSION", "9.9.9")])
            ),
            None
        );
        let env = HashMap::from([
            ("LOCUST_LINUX_UPDATE_TEST_FEED", "http://127.0.0.1:41999"),
            ("LOCUST_LINUX_UPDATE_TEST_VERSION", "9.9.9"),
            ("APPIMAGE_SILENT_INSTALL", "true"),
        ]);
        assert_eq!(linux_update_test_of(None, &env), None);
        assert_eq!(linux_update_test_of(Some(LinuxInstall::Deb), &env), None);
        assert_eq!(linux_update_test_of(Some(LinuxInstall::Rpm), &env), None);
        assert_eq!(
            linux_update_test_of(
                Some(LinuxInstall::AppImage),
                &HashMap::from([
                    ("LOCUST_LINUX_UPDATE_TEST_FEED", "http://127.0.0.1:41999"),
                    ("LOCUST_LINUX_UPDATE_TEST_VERSION", "9.9.9"),
                ])
            ),
            Some(LinuxUpdateTest {
                feed_url: Some("http://127.0.0.1:41999".into()),
                version: None,
            })
        );
        assert_eq!(
            linux_update_test_of(Some(LinuxInstall::AppImage), &env),
            Some(LinuxUpdateTest {
                feed_url: Some("http://127.0.0.1:41999".into()),
                version: Some("9.9.9".into()),
            })
        );
    }

    // case: reads a test feed only from this machine, never from anywhere else
    #[test]
    fn reads_a_test_feed_only_from_this_machine_never_from_anywhere_else() {
        for feed in [
            "https://dl.example.test/",
            "http://192.168.1.20:8080/",
            "http://user:pass@127.0.0.1:41999/",
            "file:///tmp/feed",
            "not a url",
        ] {
            assert_eq!(
                linux_update_test_of(
                    Some(LinuxInstall::AppImage),
                    &HashMap::from([("LOCUST_LINUX_UPDATE_TEST_FEED", feed)])
                ),
                None,
                "{feed}"
            );
        }
        assert_eq!(
            linux_update_test_of(
                Some(LinuxInstall::AppImage),
                &HashMap::from([("LOCUST_LINUX_UPDATE_TEST_FEED", "http://localhost:41999/")])
            ),
            Some(LinuxUpdateTest {
                feed_url: Some("http://localhost:41999/".into()),
                version: None,
            })
        );
    }

    fn package_feed(version: &str) -> FakeUpdater {
        let ver = version.to_owned();
        let mut updater = FakeUpdater::with_check(move || {
            Ok(Some(CheckForUpdatesResult {
                update_info: UpdateInfo {
                    version: ver.clone(),
                    files: Some(vec![UpdateFile {
                        url: Some(format!("Locust-{ver}-linux-amd64.deb")),
                    }]),
                },
                is_update_available: None,
                download_result: None,
            }))
        });
        updater.set_auto_download(true);
        updater.set_auto_install_on_app_quit(true);
        updater
    }

    // case: never downloads or installs, and says the new version is out with its package
    #[test]
    fn never_downloads_or_installs_and_says_the_new_version_is_out_with_its_package() {
        let updater = package_feed("0.790.0");
        let mut opts = default_options(updater.clone(), "0.789.0", true);
        opts.package_url = Some(Arc::new(|files| {
            Some(
                linux_package_url(LinuxInstall::Deb, files, "https://dl.locust.lol/")
                    .unwrap_or_else(|| "https://locust.lol/".into()),
            )
        }));
        let service = create_update_service(opts);
        assert!(!updater.auto_download());
        assert!(!updater.auto_install_on_app_quit());
        let checked = service.check();
        assert_eq!(
            checked.data(),
            Some(&AppUpdateState {
                phase: AppUpdatePhase::Available,
                current_version: "0.789.0".into(),
                every_build: false,
                available_version: Some("0.790.0".into()),
                download_url: Some(
                    "https://dl.locust.lol/Locust-0.790.0-linux-amd64.deb".into()
                ),
                percent: None,
                message: None,
                held_back: None,
            })
        );
        assert_eq!(
            service.install().error().map(|e| e.code),
            Some(AppUpdateErrorCode::UpdateNotReady)
        );
    }

    // case: says nothing to download when it is up to date
    #[test]
    fn says_nothing_to_download_when_it_is_up_to_date() {
        let updater = package_feed("0.789.0");
        let mut opts = default_options(updater, "0.789.0", true);
        opts.package_url = Some(Arc::new(|_| Some("https://locust.lol/".into())));
        let service = create_update_service(opts);
        let checked = service.check();
        assert_eq!(
            checked.data(),
            Some(&AppUpdateState {
                phase: AppUpdatePhase::Current,
                current_version: "0.789.0".into(),
                every_build: false,
                available_version: None,
                download_url: None,
                percent: None,
                message: None,
                held_back: None,
            })
        );
    }
}
