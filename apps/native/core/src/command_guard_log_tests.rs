//! Electron `a-codex-teammate-is-guarded-too.test.ts`: the two cases the guard's own tests did not name -- the line
//! Codex runs on Windows, and the log a drive can ask for (0.720).
use super::*;

/// The event Codex handed the hook in the measurement, with this command in it.
fn codex_event(command: &str) -> String {
    serde_json::json!({"session_id": "s", "turn_id": "t", "transcript_path": null, "cwd": "C:\\work\\pebble", "hook_event_name": "PreToolUse", "model": "gpt-6-luna", "permission_mode": "bypassPermissions", "tool_name": "Bash", "tool_input": {"command": command}, "tool_use_id": "exec-1"}).to_string()
}

// case: names the guard on Windows the way Codex runs a hook there: in PowerShell, on Locust’s own binary run as node
#[test]
fn names_the_guard_on_windows_the_way_codex_runs_a_hook_there_in_powershell_on_locusts_own_binary() {
    // Here Locust's own binary IS the guard (--command-guard), so there is no script and nothing to run as node.
    let exe = std::path::Path::new("C:\\Users\\Jane O'Neil\\AppData\\Local\\Programs\\Locust\\Locust.exe");
    assert_eq!(hook_command_logged(exe, None, true), "& 'C:/Users/Jane O''Neil/AppData/Local/Programs/Locust/Locust.exe' '--command-guard'");
    // And elsewhere, a POSIX shell's quoting, as before.
    let mac = std::path::Path::new("/Applications/Locust \"Beta\".app/Contents/MacOS/Locust");
    assert_eq!(hook_command_logged(mac, None, false), "\"/Applications/Locust \\\"Beta\\\".app/Contents/MacOS/Locust\" --command-guard");
    assert_eq!(hook_command_logged(mac, None, false), hook_command(mac), "no log named: the same line");
}

// case: is written only when LOCUST_GUARD_LOG names a file, one per question, with what was decided
#[test]
fn is_written_only_when_locust_guard_log_names_a_file_one_per_question_with_what_was_decided() {
    let folder = std::env::temp_dir().join(format!("locust-guard-log-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    std::fs::create_dir_all(&folder).unwrap();
    let log = folder.join("asked.jsonl");
    // No log named: nothing written.
    assert_eq!(answer_and_log(&codex_event("npm test"), "/tmp", None), None);
    assert!(!log.exists());
    assert_eq!(answer_and_log(&codex_event("npm test"), "/tmp", Some(&log)), None);
    assert!(answer_and_log(&codex_event("taskkill /F /IM msedge.exe"), "/tmp", Some(&log)).is_some());
    let lines: Vec<serde_json::Value> = std::fs::read_to_string(&log).unwrap().trim().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(lines, vec![
        serde_json::json!({"tool": "Bash", "command": "npm test", "refused": false}),
        serde_json::json!({"tool": "Bash", "command": "taskkill /F /IM msedge.exe", "refused": true}),
    ]);
    // The command is kept to its first 200 characters.
    assert_eq!(serde_json::from_str::<serde_json::Value>(&guard_log_line(&codex_event(&"x".repeat(250)), false)).unwrap()["command"].as_str().unwrap().len(), 200);
    // A drive's log is named in the hook's own line, which is all a runtime's hook inherits.
    let exe = std::path::Path::new("/Applications/Locust.app/Contents/MacOS/Locust");
    assert_eq!(hook_command_logged(exe, Some("/tmp/a \"b\".jsonl"), false), "LOCUST_GUARD_LOG=\"/tmp/a \\\"b\\\".jsonl\" \"/Applications/Locust.app/Contents/MacOS/Locust\" --command-guard");
    assert_eq!(
        hook_command_logged(std::path::Path::new("C:\\L\\Locust.exe"), Some("C:\\logs\\o'k.jsonl"), true),
        "$env:LOCUST_GUARD_LOG='C:/logs/o''k.jsonl'; & 'C:/L/Locust.exe' '--command-guard'"
    );
    let _ = std::fs::remove_dir_all(&folder);
}

// ---- a-copilot-teammate-is-guarded-too.test.ts ----
fn copilot_line(windows: bool) -> String {
    let exe = std::path::Path::new(if windows { "C:\\Users\\Jane O'Neil\\AppData\\Local\\Programs\\Locust\\Locust.exe" } else { "/Applications/Locust.app/Contents/MacOS/Locust" });
    let hooks: serde_json::Value = serde_json::from_str(&copilot_hooks_for(exe, None, windows)).unwrap();
    assert_eq!(hooks["hooks"]["PreToolUse"][0]["matcher"], "Bash|PowerShell");
    assert_eq!(hooks["hooks"]["PreToolUse"][0]["hooks"][0]["timeout"], 15);
    hooks["hooks"]["PreToolUse"][0]["hooks"][0]["command"].as_str().unwrap().to_owned()
}

// case: names the guard on Windows as Codex’s is named, inside a try that always ends 0
#[test]
fn names_the_guard_on_windows_as_codexs_is_named_inside_a_try_that_always_ends_0() {
    let exe = std::path::Path::new("C:\\Users\\Jane O'Neil\\AppData\\Local\\Programs\\Locust\\Locust.exe");
    assert_eq!(copilot_line(true), "try { & 'C:/Users/Jane O''Neil/AppData/Local/Programs/Locust/Locust.exe' '--command-guard' } catch {}; exit 0");
    assert_eq!(copilot_line(true), format!("try {{ {} }} catch {{}}; exit 0", hook_command_logged(exe, None, true)));
}

// case: names it elsewhere as Claude Code’s is named, and never fails
#[test]
fn names_it_elsewhere_as_claude_codes_is_named_and_never_fails() {
    let exe = std::path::Path::new("/Applications/Locust.app/Contents/MacOS/Locust");
    let claude: serde_json::Value = serde_json::from_str(&claude_settings(exe)).unwrap();
    assert_eq!(copilot_line(false), format!("{} || true", claude["hooks"]["PreToolUse"][0]["hooks"][0]["command"].as_str().unwrap()));
}
