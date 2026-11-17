//! Electron's `a-command-reads-as-a-command.test.ts`: a running step names the command, not the shell that launched it
//! (`missionView.ts` `withoutShellWrapper`).

use crate::shell_words::without_shell_wrapper;
use crate::thread_view::live_line;
use serde_json::json;

#[test]
fn unwraps_the_shape_that_prompted_this() {
    assert_eq!(
        without_shell_wrapper(r#"cmd /c "dir /s /b /o-d .locust sessions backups""#),
        "dir /s /b /o-d .locust sessions backups"
    );
}

#[test]
fn handles_the_spellings_this_app_actually_spawns() {
    assert_eq!(without_shell_wrapper("cmd.exe /c npm test"), "npm test");
    assert_eq!(without_shell_wrapper(r#"powershell -NoProfile -Command "Get-Item x""#), "Get-Item x");
    assert_eq!(without_shell_wrapper(r#"bash -lc "ls -la""#), "ls -la");
    assert_eq!(without_shell_wrapper(r#"/bin/sh -c "make""#), "make");
}

#[test]
fn leaves_an_ordinary_command_alone() {
    assert_eq!(without_shell_wrapper("git status"), "git status");
    assert_eq!(without_shell_wrapper("npm run build"), "npm run build");
}

#[test]
fn does_not_strip_quotes_that_are_not_a_wrapper() {
    assert_eq!(without_shell_wrapper(r#"cmd /c "a" && "b""#), r#""a" && "b""#);
}

#[test]
fn only_matches_at_the_start_so_a_command_that_mentions_one_is_safe() {
    assert_eq!(without_shell_wrapper("echo run cmd /c something"), "echo run cmd /c something");
}

#[test]
fn keeps_the_whole_thing_when_there_is_nothing_after_the_wrapper() {
    assert_eq!(without_shell_wrapper("cmd /c "), "cmd /c");
}

#[test]
fn trims_so_a_row_never_starts_with_whitespace() {
    assert_eq!(without_shell_wrapper("   git log   "), "git log");
}

#[test]
fn the_live_row_names_the_command_without_its_shell() {
    let at = "2026-09-15T10:00:00.000Z";
    let started = json!({"type":"tool.started","occurredAt":at,"payload":{
        "itemId":"t1","name":"shell","command":"cmd /c \"dir /s /b /o-d .locust sessions backups\""}});
    let line = live_line(&[started], true, at, false).unwrap();
    assert_eq!(line.label, "dir /s /b /o-d .locust sessions backups");
}
