//! Electron's `packages/runtime-adapters/test/opencode-commands.test.ts`, its run cases ("one of OpenCode's commands,
//! run"), on opencode-serve-run's stand-in server and caller (the parent module). Electron's cases read the
//! `opencode run` command line; the native app runs OpenCode 1 only through its server, where a command is sent to
//! `/session/{id}/command` (opencode-serve-run's command case) and a message to `prompt_async`.
use super::*;

#[test]
fn refuses_a_name_no_command_could_have() {
    // case: refuses a name no command could have
    // Natively a typed command is known by `command_named` before anything is sent: a name no command could have is
    // never taken for one, so it is never sent as one (Electron's createOpenCodeRunCommand throws "not a command name").
    for typed in ["/init; rm -rf", "/--auto"] {
        assert_eq!(crate::runtime_commands::command_named(typed), None, "{typed}");
    }
    assert_eq!(crate::runtime_commands::command_named("/init Keep it under five lines."), Some("init".into()));
}

#[test]
fn is_an_ordinary_run_when_there_is_no_command() {
    // case: is an ordinary run when there is no command
    let ran = run(&recorded(), Mode::Auto, None, |t, rt| {
        t.prompt = "hi".into();
        rt
    });
    let paths: Vec<String> = ran.server.calls().iter().map(|c| c["path"].as_str().unwrap().to_owned()).collect();
    assert!(!paths.iter().any(|p| p.ends_with("/command")), "{paths:?}");
    assert_eq!(ran.server.sent("/prompt_async").unwrap()["parts"], json!([{"type": "text", "text": "hi"}]));
    assert_eq!(ran.events.last().unwrap().kind, "run.completed");
}
