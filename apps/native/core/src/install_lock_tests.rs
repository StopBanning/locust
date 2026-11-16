//! Electron `main/a-locked-file-is-not-a-permission-problem.test.ts` (code review B4): on Windows an npm EPERM on a
//! rename or unlink is nearly always a file lock, not an unwritable global folder. Electron's values and words.

use crate::agent_install::{classify, Failure};

const LOCKED: &str = concat!(
    "npm error code EPERM\n",
    "npm error syscall rename\n",
    "npm error path C:\\Users\\x\\AppData\\Roaming\\npm\\node_modules\\opencode-ai\n",
    "npm error errno -4048\n",
    "npm error Error: EPERM: operation not permitted, rename 'C:\\Users\\x\\AppData\\Roaming\\npm\\node_modules\\opencode-ai'"
);

fn classified(output: &str, platform: &str) -> Failure {
    classify("opencode-ai", "OpenCode", output, 3, platform)
}

#[test]
fn on_windows_a_locked_file_is_named_as_a_lock_with_no_prefix_to_move() {
    let said = classified(LOCKED, "windows");
    assert!(said.what.contains("running program is holding OpenCode"), "{}", said.what);
    assert_eq!(said.command, None);
}

#[test]
fn on_windows_an_unwritable_folder_is_still_named_as_one() {
    let said = classified("npm error code EACCES\nnpm error Error: EACCES: permission denied, mkdir", "windows");
    assert!(said.what.contains("could not write to its global folder"), "{}", said.what);
}

#[test]
fn off_windows_the_same_eperm_reads_as_a_folder_problem() {
    assert!(classified(LOCKED, "linux").what.contains("could not write to its global folder"));
}
