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

// ---- --check-history ---------------------------------------------------------------------------------------------

fn conversation(id: &str, prompt: &str) -> String {
    let header = serde_json::json!({"schemaVersion": 27, "recordType": "mission.created", "metadata": {
        "missionId": id, "runId": "run_1", "prompt": prompt, "runtime": "codex", "model": "account-default",
        "requestedRouteId": "codex", "resolvedRouteId": "codex-account:default", "cliVersion": "0.151.0",
        "workspaceId": "ws_test", "sandbox": "read-only", "executionPolicyVersion": 1,
        "createdAt": "2026-11-25T10:00:00.000Z"}});
    format!("{header}\n")
}

#[test]
fn check_history_reports_by_reason_and_writes_nothing() {
    let dir = temp("history");
    let mut ok = conversation("mission_ok", "Hello.");
    ok.push_str(&format!("{}\n", serde_json::json!({"schemaVersion": 27, "recordType": "mission.event", "event": {
        "id": "event_1", "runId": "run_1", "missionId": "mission_ok", "sequence": 1, "type": "message.delta",
        "occurredAt": "2026-11-25T10:00:00.000Z", "sourceAdapter": "cursor", "payload": {"itemId": "msg_0",
        "operation": "append", "text": "Hi.", "final": true, "evidence": {"redacted": true}}}})));
    std::fs::write(dir.join("mission_ok.jsonl"), ok).unwrap();
    std::fs::write(dir.join("mission_bad.jsonl"), "not a header\n").unwrap();
    std::fs::write(dir.join("mission_zero.jsonl"), conversation("mission_zero", "Asked.")).unwrap();
    let before = as_it_is(&dir);
    let mut out = Vec::new();
    let code = cutover_flag(&args(&["--check-history", dir.to_str().unwrap()]), &mut out);
    let said = String::from_utf8(out).unwrap();
    assert_eq!(code, Some(0), "{said}");
    assert!(said.contains("Files read: 3. Read whole with turns: 1."), "{said}");
    assert!(
        said.contains("refused: no conversation could be read (invalid-record): 1\n  mission_bad.jsonl"),
        "{said}"
    );
    assert!(said.contains("read with zero turns: only the opening prompt, no reply, tool or plan: 1\n  mission_zero.jsonl"), "{said}");
    assert_eq!(as_it_is(&dir), before, "the check wrote in the folder it read");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_history_without_a_folder_or_with_a_missing_one_exits_1_and_says_why() {
    let mut out = Vec::new();
    assert_eq!(cutover_flag(&args(&["--check-history"]), &mut out), Some(1));
    assert!(String::from_utf8_lossy(&out).contains("Say which folder"));
    let dir = temp("history-missing");
    let mut out = Vec::new();
    let code = cutover_flag(&args(&["--check-history", dir.join("nope").to_str().unwrap()]), &mut out);
    assert_eq!(code, Some(1));
    assert!(String::from_utf8_lossy(&out).contains("could not be listed"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// By hand, over a COPY of Electron's `mission-ledger` (never the folder itself):
/// `LOCUST_HISTORY_COPY=<copy> cargo nextest run -p locust-native --run-ignored only check_history_over_a_real_copy --no-capture`.
#[test]
#[ignore = "reads the folder LOCUST_HISTORY_COPY names; run by hand over a copy"]
fn check_history_over_a_real_copy() {
    let copy = std::env::var("LOCUST_HISTORY_COPY").expect("LOCUST_HISTORY_COPY names a copy of mission-ledger");
    let mut out = Vec::new();
    let code = cutover_flag(&args(&["--check-history", &copy]), &mut out);
    println!("{}", String::from_utf8_lossy(&out));
    assert_eq!(code, Some(0));
}
