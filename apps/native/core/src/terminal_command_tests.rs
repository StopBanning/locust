//! Lane check (26 Nov): the window's Open in terminal reads these two, so a break in either is seen.
use super::*;

// The executable each runtime is discovered as (a-conversation-opens-in-its-own-terminal.test.ts names Cursor's as
// `cursor-agent`; agents.rs finds Antigravity's CLI as `agy`).
#[test]
fn each_runtime_opens_with_the_command_it_is_discovered_as() {
    assert_eq!(terminal_command("cursor"), "cursor-agent");
    assert_eq!(terminal_command("antigravity"), "agy");
    assert_eq!(terminal_command("claude"), "claude");
    assert_eq!(terminal_command("codex"), "codex");
    assert_eq!(terminal_command("opencode"), "opencode");
}

#[test]
fn no_folder_chosen_is_said_and_nothing_is_run() {
    assert_eq!(folder_to_open(None), Err("Choose a folder first. Nothing was run.".to_owned()));
    assert_eq!(folder_to_open(Some("/work/app".into())), Ok("/work/app".to_owned()));
}
