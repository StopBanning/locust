//! Electron's `main/a-mac-opens-its-own-terminal.test.ts` (1 case): the `.command` file Terminal runs on a Mac is the
//! text `open_with_env` writes, `mac_script_with_env`, line for line.
use super::*;

// case: goes to the folder, removes itself, and becomes the resume -- every word quoted
#[test]
fn the_mac_command_file_goes_to_the_folder_removes_itself_and_becomes_the_resume() {
    let script = mac_script_with_env(
        Path::new("/opt/homebrew/bin/claude"),
        &["--resume".into(), "abc-123".into()],
        Path::new("/Users/ian/My Project's"),
        &[("ELECTRON_RUN_AS_NODE", "1"), ("bad name", "x")],
    );
    assert_eq!(
        script.split('\n').collect::<Vec<_>>(),
        vec![
            "#!/bin/zsh -l",
            "rm -f -- \"$0\"",
            "cd '/Users/ian/My Project'\"'\"'s' || exit 1",
            "export ELECTRON_RUN_AS_NODE='1'",
            "exec '/opt/homebrew/bin/claude' '--resume' 'abc-123'",
            "",
        ]
    );
}
