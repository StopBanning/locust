//! This computer's Claude Code run for a follow-up's receipt (`run_exec`, Electron's `execFile` in index.ts's
//! `createClaudeCloud`): a shell script stands in for Claude Code, never the real one.

use super::{ClaudeCloud, ClaudeLaunch, CloudPlatform, CloudSendResult, ExecOptions, run_exec};
use std::collections::BTreeMap;
use std::path::PathBuf;

struct Temp(PathBuf);

impl Temp {
    fn new(label: &str) -> Self {
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("locust-cloud-exec-{label}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(unix)]
fn script(dir: &std::path::Path, body: &str) -> String {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("claude");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path.to_string_lossy().into_owned()
}

#[cfg(unix)]
#[test]
fn a_run_keeps_stdout_apart_from_what_it_said_in_its_folder_with_its_environment() {
    let dir = Temp::new("run");
    let file = script(&dir.0, r#"echo "{\"ok\":true,\"in\":\"$(pwd)\",\"task\":\"$LOCUST_CLAUDE_CLOUD_TASK\",\"arg\":\"$1\"}"; echo 'a warning' >&2; exit 3"#);
    let env = BTreeMap::from([("LOCUST_CLAUDE_CLOUD_TASK".to_string(), "Fix it".to_string())]);
    let ran = run_exec(&file, &["--cloud".into()], &ExecOptions { cwd: dir.0.to_string_lossy().into(), env, verbatim: false });
    let stdout = ran.stdout.expect("stdout kept apart");
    assert_eq!(ran.code, 3);
    assert_eq!(
        stdout.trim(),
        format!(r#"{{"ok":true,"in":"{}","task":"Fix it","arg":"--cloud"}}"#, dir.0.display())
    );
    assert!(!stdout.contains("a warning"));
    assert_eq!(ran.output, format!("{stdout}a warning\n"));
}

#[test]
fn a_run_that_cannot_start_says_why_with_no_stdout() {
    let ran = run_exec("/nonexistent/claude", &[], &ExecOptions { cwd: String::new(), env: BTreeMap::new(), verbatim: false });
    assert_eq!(ran.code, -1);
    assert!(ran.stdout.is_none());
    assert!(!ran.output.is_empty());
}

#[cfg(unix)]
#[test]
fn a_follow_up_on_this_computer_is_confirmed_by_claude_codes_receipt() {
    let dir = Temp::new("send");
    let file = script(&dir.0, r#"printf '%s\n' "$@" > args.txt; echo '{"ok":true}'"#);
    let store = dir.0.join("claude-cloud.json");
    let sessions = serde_json::json!({ "sessions": [{ "id": "cc_0123456789abcdef0123", "startedAt": "2026-10-02T20:00:00.000Z",
        "prompt": "Fix the cart total", "folder": dir.0.to_string_lossy(), "sessionId": "session_01G8RT9yXr7dbhVM7EUzpvEa" }] });
    std::fs::write(&store, sessions.to_string()).unwrap();
    let launch = file.clone();
    let claude = ClaudeCloud::new(&store, CloudPlatform::Darwin, move || {
        Some(ClaudeLaunch { discovered_path: launch.clone(), executable_path: launch.clone(), prefix_args: Vec::new(), env: None })
    })
    .with_exec(run_exec);
    assert_eq!(claude.send("cc_0123456789abcdef0123", "Also the tax"), CloudSendResult::Ok);
    let args = std::fs::read_to_string(dir.0.join("args.txt")).unwrap();
    assert_eq!(args, "-p\n--cloud\nsession_01G8RT9yXr7dbhVM7EUzpvEa\n--output-format\njson\nAlso the tax\n");
}
