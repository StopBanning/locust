//! Electron's `antigravity-is-found-on-a-mac.test.ts` (4 cases): agy, installed by Antigravity's install.sh into
//! ~/.local/bin, read as "not installed" on the first Mac run. Rust finds every agent on PATH, and at launch PATH gains
//! the folders installers use (`shell_path`), ~/.local/bin among them; a relative PATH entry is never searched.
#![cfg(unix)]
use super::*;
use crate::runtime::discovery::find_in;
use std::os::unix::fs::PermissionsExt;

struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A folder with an executable `agy` at `at` inside it.
fn with_agy(at: &[&str]) -> Temp {
    let root = std::env::temp_dir().join(format!("locust-agy-mac-{}", crate::invariants::unique_nanos()));
    let file = at.iter().fold(root.clone(), |p, part| p.join(part));
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
    Temp(root)
}

fn path_on(os: &str, home: &std::path::Path, current: &str) -> String {
    shell_path(os, Some(current), &home.to_string_lossy(), &|_| None).unwrap_or_default()
}

// case: is found where install.sh puts it, ~/.local/bin/agy
#[test]
fn is_found_where_install_sh_puts_it() {
    let home = with_agy(&[".local", "bin", "agy"]);
    for os in ["macos", "linux"] {
        // PATH at launch, kept to the home folder's entries so an agy installed on this machine cannot answer for it.
        let path = path_on(os, &home.0, "/usr/bin");
        let mine = std::env::join_paths(std::env::split_paths(&path).filter(|d| d.starts_with(&home.0))).unwrap();
        assert_eq!(find_in("agy", &mine).unwrap(), home.0.join(".local/bin/agy"), "{os}");
    }
}

// case: is found on PATH when --dir put it elsewhere
#[test]
fn is_found_on_path_when_dir_put_it_elsewhere() {
    let home = with_agy(&["unused"]);
    let tools = with_agy(&["bin", "agy"]);
    let path = path_on("macos", &home.0, &format!("/nowhere:{}", tools.0.join("bin").display()));
    assert_eq!(find_in("agy", path.as_ref()).unwrap(), tools.0.join("bin/agy"));
}

// case: is not found where there is none, and a relative PATH entry is not searched (control)
#[test]
fn is_not_found_where_there_is_none_and_a_relative_entry_is_not_searched() {
    let home = with_agy(&["unused"]);
    // Only the home folder's own install places, so an agy installed on this machine cannot answer for it.
    let home_only = std::env::join_paths(mac_install_folders(&home.0.to_string_lossy()).into_iter().filter(|f| f.starts_with(&*home.0.to_string_lossy()))).unwrap();
    assert!(find_in("agy", &home_only).is_err());
    // A relative entry is never searched, even when the folder it names, read from here, would hold one.
    let here = with_agy(&["bin", "agy"]);
    let cwd = std::env::current_dir().unwrap().canonicalize().unwrap();
    let bin = here.0.join("bin").canonicalize().unwrap();
    let up = "../".repeat(cwd.components().count() - 1);
    let relative = format!("{up}{}", bin.strip_prefix("/").unwrap().display());
    assert!(std::path::Path::new(&relative).join("agy").exists(), "the relative entry does name a real agy: {relative}");
    assert!(find_in("agy", std::ffi::OsStr::new(&format!("bin:.:{relative}"))).is_err());
}

// case: on Windows, still looks only where install.ps1 puts it, not in ~/.local/bin (control)
#[test]
fn on_windows_the_path_does_not_gain_local_bin() {
    let home = with_agy(&[".local", "bin", "agy"]);
    assert_eq!(shell_path("windows", Some("C:\\tools"), &home.0.to_string_lossy(), &|_| None).as_deref(), Some("C:\\tools"));
}
