//! Cutover 1: the app's `--seed-from-electron`, through `cutover_flag`, the one function `main` hands it to.
//! Made-up folders under the test's temp dir only.

use super::*;
use std::path::{Path, PathBuf};

fn temp(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "locust-cutover-{tag}-{}-{}",
        std::process::id(),
        locust_core::native::uuid_v4().unwrap()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn args(rest: &[&str]) -> Vec<String> {
    std::iter::once("locust-native").chain(rest.iter().copied()).map(String::from).collect()
}

/// Every file under `dir`, with its bytes.
fn as_it_is(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path.clone());
                out.push((path, Vec::new()));
            } else {
                out.push((path.clone(), std::fs::read(&path).unwrap()));
            }
        }
    }
    out.sort();
    out
}

// ---- --seed-from-electron --------------------------------------------------------------------------------------
// It works on the folders the environment names (HOME's Electron folder, LOCUST_NATIVE_DATA), so it runs in a child
// copy of this test binary with those set on the child alone: no other test sees these folders.

const STALE: &str = r#"{"schemaVersion":1,"teammates":[{"teammateId":"old","name":"Old"}],"missionOwners":{},"settings":{}}"#;
const FRESH: &str = r#"{"schemaVersion":1,"teammates":[{"teammateId":"new","name":"New"}],"missionOwners":{},"settings":{}}"#;

#[test]
fn seed_from_electron_copies_electrons_files_into_the_rust_folder_and_keeps_the_old_ones() {
    let dir = temp("seed");
    let (home, config, native) = (dir.join("home"), dir.join("config"), dir.join("native"));
    let electron = if cfg!(target_os = "macos") {
        home.join("Library/Application Support/@teammate/desktop")
    } else {
        config.join("@teammate/desktop")
    };
    std::fs::create_dir_all(&electron).unwrap();
    std::fs::create_dir_all(&native).unwrap();
    std::fs::write(electron.join("teammates.json"), FRESH).unwrap();
    std::fs::write(electron.join("memories.json"), "{ cut off").unwrap();
    std::fs::write(native.join("teammates.json"), STALE).unwrap();
    std::fs::write(native.join("memories.json"), r#"{"schemaVersion":1,"memories":[]}"#).unwrap();
    let electron_before = as_it_is(&electron);

    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["cutover_flags_tests::seed_from_electron_in_a_child", "--exact", "--ignored", "--nocapture", "--test-threads=1"])
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", &config)
        .env("LOCUST_NATIVE_DATA", &native)
        .output()
        .unwrap();
    let words = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));

    assert!(out.status.success(), "{words}");
    assert!(words.contains("exit code 0"), "{words}");
    assert_eq!(std::fs::read_to_string(native.join("teammates.json")).unwrap(), FRESH);
    let aside: Vec<PathBuf> = std::fs::read_dir(&native)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with("seeded-before-"))
        .collect();
    assert_eq!(aside.len(), 1, "{words}");
    assert_eq!(std::fs::read_to_string(aside[0].join("teammates.json")).unwrap(), STALE);
    assert!(words.contains("Copied: teammates.json"), "{words}");
    assert!(words.contains(&format!("Moved aside into {}: teammates.json", aside[0].display())), "{words}");
    assert!(words.contains("Not copied: memories.json: Electron's copy is not JSON"), "{words}");
    assert_eq!(
        std::fs::read_to_string(native.join("memories.json")).unwrap(),
        r#"{"schemaVersion":1,"memories":[]}"#,
        "a bad Electron file leaves the Rust copy as it was"
    );
    assert_eq!(as_it_is(&electron), electron_before, "Electron's folder is only read");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Only ever run by the test above, in its own process.
#[test]
#[ignore = "run by seed_from_electron_copies_electrons_files_into_the_rust_folder_and_keeps_the_old_ones in a child process"]
fn seed_from_electron_in_a_child() {
    let mut out = Vec::new();
    let code = cutover_flag(&args(&["--seed-from-electron"]), &mut out);
    println!("{}", String::from_utf8_lossy(&out));
    println!("exit code {}", code.map(|c| c.to_string()).unwrap_or_else(|| "none".into()));
}

/// No test can run `main`, so this reads it: the arguments go to `cutover_flag` inside `main`, ahead of the login-shell
/// PATH step and the window, and its code ends the process. (Run by hand: `locust-native --seed-from-electron`.)
#[test]
fn main_hands_its_arguments_to_cutover_flag_before_anything_slow() {
    let source = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main.rs")).unwrap();
    let main = &source[source.find("\nfn main() {\n").expect("fn main")..];
    let handed = main
        .find("if let Some(code) = cutover_flag(&std::env::args().collect::<Vec<_>>(), &mut std::io::stdout()) {\n        std::process::exit(code);")
        .expect("main hands its arguments to cutover_flag and exits with its code");
    assert!(handed < main.find("shell_path::adopt_terminal_path();").unwrap());
}
