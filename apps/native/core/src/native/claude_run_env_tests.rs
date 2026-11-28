//! A Claude run starts with Claude Code's background tasks switched off, as Electron's has since 15 Nov 2026
//! (packages/runtime-adapters/test/a-claude-run-has-no-background-tasks.test.ts; commands.ts `CLAUDE_RUN_ENV`).
//! The answer key found it on 28 Nov: Electron 0.882 sets the switch on every Claude case of the command grid.
use super::*;

/// A stand-in Claude Code that writes the switch, its own arguments and whether a Locust key reached it, then answers
/// one turn the way Claude Code does (the stream `commands_claude` in tests.rs sends), so a follow-up can resume it.
fn logging_claude(t: &Temp, log: &Path) -> Option<(PathBuf, Vec<String>)> {
    let fake = t.0.join("claude_run_env_fake.py");
    let py = r#"import json, os, sys
log_path = sys.argv[1]
with open(log_path, 'a', encoding='utf-8') as f:
    f.write(json.dumps({'switch': os.environ.get('CLAUDE_CODE_DISABLE_BACKGROUND_TASKS'), 'key': os.environ.get('ANTHROPIC_API_KEY'), 'args': sys.argv[2:]}) + '\n')
def send(v):
    print(json.dumps(v), flush=True)
sys.stdin.readline()
send({"type": "system", "subtype": "init", "session_id": "33b21513-a4fb-478d-ad84-21cb7ef30571", "cwd": "/tmp", "tools": ["Read"], "mcp_servers": [], "model": "synthetic-model", "permissionMode": "default", "slash_commands": [], "claude_code_version": "2.1.252", "agents": [], "skills": []})
send({"type": "assistant", "message": {"id": "msg_env", "content": [{"type": "text", "text": "OK"}]}})
send({"type": "result", "subtype": "success", "is_error": False, "result": "OK"})
try:
    sys.stdin.read()
except Exception:
    pass
"#;
    fs::write(&fake, py).unwrap();
    Some((PathBuf::from("python3"), vec![fake.to_string_lossy().into_owned(), log.to_string_lossy().into_owned()]))
}

fn logged(log: &Path) -> Vec<Value> {
    fs::read_to_string(log).unwrap_or_default().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

fn args_of(run: &Value) -> Vec<String> {
    run["args"].as_array().unwrap().iter().filter_map(Value::as_str).map(str::to_owned).collect()
}

// case: starts with Claude Code's own switch for background tasks turned off
#[test]
fn a_claude_turn_starts_with_background_tasks_switched_off() {
    let t = Temp::new();
    let log = t.0.join("claude_run_env.jsonl");
    let _g = hooks::install(t.store(), logging_claude(&t, &log));
    let run = send(claude_job(&t, None, "Synthetic prompt")).unwrap();
    finished(&run.conversation_id);
    let runs = logged(&log);
    assert_eq!(runs.len(), 1, "{runs:?}");
    assert_eq!(runs[0]["switch"].as_str(), Some("1"), "{runs:?}");
}

// case: has it off when the session is resumed too, and keeps the launch's own variables beside it
#[test]
fn a_resumed_claude_turn_has_it_off_too_and_keeps_the_launchs_own_settings() {
    let t = Temp::new();
    let log = t.0.join("claude_run_env.jsonl");
    let _g = hooks::install(t.store(), logging_claude(&t, &log));
    let first = send(claude_job(&t, None, "Synthetic prompt")).unwrap();
    finished(&first.conversation_id);
    let second = send(claude_job(&t, Some(&first.conversation_id), "Synthetic prompt")).unwrap();
    finished(&second.conversation_id);
    let runs = logged(&log);
    assert_eq!(runs.len(), 2, "{runs:?}");
    let resumed = &runs[1];
    let args = args_of(resumed);
    assert!(args.iter().any(|a| a == "--resume"), "the follow-up resumes the session: {args:?}");
    assert_eq!(resumed["switch"].as_str(), Some("1"), "{resumed}");
    // The launch's own settings stay beside it: the turn's arguments, and no Locust key handed on.
    assert!(args.iter().any(|a| a == "--print"), "{args:?}");
    assert!(resumed["key"].is_null(), "{resumed}");
}

/// The one-shot run (`run_cli.rs`) gets the same switch.
#[test]
fn a_one_shot_claude_command_has_background_tasks_switched_off() {
    let folder = std::env::temp_dir();
    let command = crate::runtime::claude::command_at(PathBuf::from("/usr/bin/true"), &folder, Some("haiku"));
    let set: Vec<(String, Option<String>)> = command
        .get_envs()
        .map(|(k, v)| (k.to_string_lossy().into_owned(), v.map(|v| v.to_string_lossy().into_owned())))
        .collect();
    assert!(set.contains(&("CLAUDE_CODE_DISABLE_BACKGROUND_TASKS".into(), Some("1".into()))), "{set:?}");
}
