//! `scripts/bundle-mac.sh`, run as Colin runs it (Cutover 2 row 3). A stand-in `cargo` first on PATH stands for the
//! release build: it notes what it was asked and what LOCUST_BUILD_COMMIT it saw, and leaves a small signed Mach-O
//! (a copy of /usr/bin/true) where the build's binary goes. Everything after it is the script's own: the icon, the
//! Info.plist, the ad-hoc signature, and in release mode the disk image Electron's Mac updater installs
//! (apps/desktop/src/main/mac-self-update.ts: the newest `Locust-<x.y.z>-mac-<arm64|x64>.dmg`, a `Locust.app` at its
//! root). Nothing is uploaded, and nothing here goes near /Applications.

use std::path::{Path, PathBuf};
use std::process::Command;

fn script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/bundle-mac.sh")
}

/// The window crate's own version, read from its manifest the plain way (the first `version = ` after `[package]`).
fn crate_version() -> String {
    let manifest = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../Cargo.toml")).unwrap();
    let package = manifest.split("[package]").nth(1).expect("a [package] table");
    package
        .lines()
        .find_map(|line| line.trim().strip_prefix("version = ").map(|v| v.trim_matches('"').to_owned()))
        .expect("a package version")
}

fn arch() -> &'static str {
    if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" }
}

fn temp_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("bundle-mac-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

struct Bundled {
    out: PathBuf,
    cargo_log: String,
    said: String,
}

/// Runs the script with `args`, with LOCUST_BUILD_COMMIT set to `commit` or unset, the build standing in.
fn bundle(root: &Path, args: &[&str], commit: Option<&str>) -> Bundled {
    let bin = root.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let log = root.join("cargo.log");
    let cargo = bin.join("cargo");
    std::fs::write(
        &cargo,
        format!(
            "#!/bin/sh\nprintf 'args=%s\\ncommit=%s\\n' \"$*\" \"${{LOCUST_BUILD_COMMIT-(unset)}}\" > '{}'\n\
             mkdir -p \"$CARGO_TARGET_DIR/release\" && cp /usr/bin/true \"$CARGO_TARGET_DIR/release/locust-native\"\n",
            log.display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
    let out = root.join("out");
    let mut command = Command::new("/bin/sh");
    command
        .arg(script())
        .args(args)
        .env("PATH", format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", bin.display()))
        .env("CARGO_TARGET_DIR", root.join("target"))
        .env_remove("LOCUST_BUILD_COMMIT")
        .env_remove("LOCUST_NATIVE_LINKS");
    if let Some(commit) = commit {
        command.env("LOCUST_BUILD_COMMIT", commit);
    }
    let done = command.output().unwrap();
    let said = format!("{}{}", String::from_utf8_lossy(&done.stdout), String::from_utf8_lossy(&done.stderr));
    assert!(done.status.success(), "bundle-mac.sh {args:?} failed: {said}");
    Bundled { out, cargo_log: std::fs::read_to_string(&log).unwrap_or_default(), said }
}

fn plist_value(plist: &Path, key: &str) -> String {
    let read = Command::new("/usr/bin/plutil").args(["-extract", key, "raw", "-o", "-"]).arg(plist).output().unwrap();
    assert!(read.status.success(), "{key} in {}: {}", plist.display(), String::from_utf8_lossy(&read.stderr));
    String::from_utf8_lossy(&read.stdout).trim().to_owned()
}

/// The disk image mounted read-only where only this test looks, and detached again whatever happens.
struct Mounted(PathBuf);
impl Drop for Mounted {
    fn drop(&mut self) {
        let _ = Command::new("/usr/bin/hdiutil").args(["detach", "-force"]).arg(&self.0).output();
    }
}

fn mount(image: &Path, at: &Path) -> Mounted {
    std::fs::create_dir_all(at).unwrap();
    let attached = Command::new("/usr/bin/hdiutil")
        .args(["attach", "-nobrowse", "-readonly", "-noautoopen", "-mountpoint"])
        .arg(at)
        .arg(image)
        .output()
        .unwrap();
    assert!(attached.status.success(), "{}", String::from_utf8_lossy(&attached.stderr));
    Mounted(at.to_owned())
}

fn visible_names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !n.starts_with('.'))
        .collect();
    names.sort();
    names
}

#[test]
fn release_mode_makes_a_locust_app_with_electrons_id_and_this_crates_version_in_the_disk_image_electrons_updater_takes() {
    let root = temp_root("release");
    let version = crate_version();
    let made = bundle(&root, &["--release", "--out", root.join("out").to_str().unwrap()], Some("abc1234def"));

    // The build was asked for a release, and was handed the commit.
    assert!(made.cargo_log.contains("args=build --release"), "{}", made.cargo_log);
    assert!(made.cargo_log.contains("commit=abc1234def"), "{}", made.cargo_log);

    // The app: Locust.app, Electron's bundle id (so macOS's permissions stay with the same app), this crate's version.
    let app = made.out.join("Locust.app");
    let plist = app.join("Contents/Info.plist");
    assert_eq!(plist_value(&plist, "CFBundleIdentifier"), "com.automatedworkflow.locust");
    assert_eq!(plist_value(&plist, "CFBundleName"), "Locust");
    assert_eq!(plist_value(&plist, "CFBundleShortVersionString"), version);
    let executable = plist_value(&plist, "CFBundleExecutable");
    assert!(app.join("Contents/MacOS").join(&executable).is_file(), "{executable}");
    assert!(app.join("Contents/Resources/icon.icns").is_file());
    // Electron's Locust claims locust:// links; the copy that takes its place does too.
    assert_eq!(plist_value(&plist, "CFBundleURLTypes.0.CFBundleURLSchemes.0"), "locust");
    assert!(!made.out.join("Locust Native.app").exists(), "{:?}", visible_names(&made.out));

    // The disk image: the exact name Electron's updater looks for, and a Locust.app at its root.
    let image_name = format!("Locust-{version}-mac-{}.dmg", arch());
    let image = made.out.join(&image_name);
    assert!(image.is_file(), "no {image_name} in {:?}: {}", visible_names(&made.out), made.said);
    let mounted = mount(&image, &root.join("mnt"));
    let names = visible_names(&mounted.0);
    assert!(names.contains(&"Locust.app".to_owned()), "{names:?}");
    assert!(names.iter().all(|n| n == "Locust.app" || n == "Applications"), "{names:?}");
    let packed = mounted.0.join("Locust.app/Contents/Info.plist");
    assert_eq!(plist_value(&packed, "CFBundleIdentifier"), "com.automatedworkflow.locust");
    assert_eq!(plist_value(&packed, "CFBundleShortVersionString"), version);
    assert!(mounted.0.join("Locust.app/Contents/MacOS").join(&executable).is_file());
    drop(mounted);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_preview_keeps_its_own_name_and_id_takes_the_crate_version_too_and_makes_no_disk_image() {
    let root = temp_root("preview");
    let made = bundle(&root, &["--out", root.join("out").to_str().unwrap()], None);

    // No commit set: none is made up for the build.
    assert!(made.cargo_log.contains("args=build --release"), "{}", made.cargo_log);
    assert!(made.cargo_log.contains("commit=(unset)"), "{}", made.cargo_log);

    let plist = made.out.join("Locust Native.app/Contents/Info.plist");
    assert_eq!(plist_value(&plist, "CFBundleIdentifier"), "lol.locust.native.preview");
    assert_eq!(plist_value(&plist, "CFBundleName"), "Locust Native");
    assert_eq!(plist_value(&plist, "CFBundleShortVersionString"), crate_version());
    // Beside Electron's Locust, the preview leaves locust:// links alone unless asked.
    let links = Command::new("/usr/bin/plutil").args(["-extract", "CFBundleURLTypes", "raw", "-o", "-"]).arg(&plist).output().unwrap();
    assert!(!links.status.success());
    assert_eq!(visible_names(&made.out), vec!["Locust Native.app".to_owned()]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn an_argument_it_does_not_know_stops_it_before_anything_is_built() {
    let root = temp_root("unknown");
    let done = Command::new("/bin/sh")
        .arg(script())
        .arg("--relase")
        .env("PATH", format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", root.join("bin").display()))
        .env("CARGO_TARGET_DIR", root.join("target"))
        .output()
        .unwrap();
    assert!(!done.status.success());
    assert!(String::from_utf8_lossy(&done.stderr).contains("--relase"));
    assert!(!root.join("target").exists());
    let _ = std::fs::remove_dir_all(&root);
}
