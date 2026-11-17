// Carries packages/runtime-adapters/test/a-side-question-forks-the-session.test.ts (Electron 4ce2f37a): a question on
// the side continues a copy of the session and leaves the session itself as it was. Claude Code's cases are here.
// Rust has no `opencode run` command line to give `-s … --fork`: it forks an OpenCode session through the served
// session's own `/session/{id}/fork` (opencode_serve.rs), so the OpenCode cases are not carried.
use super::claude::{PrintOptions, print_args};
use crate::approval::Mode;

#[test]
fn a_claude_code_session_is_forked_with_resume_then_fork_session() {
    let args = print_args(&PrintOptions { mode: Mode::Auto, resume: Some("sess-1"), fork: true, ..Default::default() }).unwrap();
    let at = args.iter().position(|a| a == "--resume").expect("--resume");
    assert_eq!(args[at..at + 3], ["--resume", "sess-1", "--fork-session"]);
}

#[test]
fn a_claude_code_run_with_no_session_to_fork_adds_no_fork_flag() {
    let args = print_args(&PrintOptions { mode: Mode::Auto, fork: true, ..Default::default() }).unwrap();
    assert!(!args.iter().any(|a| a == "--fork-session"), "{args:?}");
}

// Rust's own: an ordinary follow-up resumes the session itself.
#[test]
fn an_ordinary_claude_code_follow_up_is_not_a_fork() {
    let args = print_args(&PrintOptions { mode: Mode::Auto, resume: Some("sess-1"), ..Default::default() }).unwrap();
    assert!(args.iter().any(|a| a == "--resume") && !args.iter().any(|a| a == "--fork-session"), "{args:?}");
}
