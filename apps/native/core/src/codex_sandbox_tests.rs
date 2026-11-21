//! a-codex-sandbox-that-did-not-start-says-why.test.tsx, the check and the folded line: a command Codex's sandbox
//! never started is known by what Codex said, and the line over the steps says so where it read only "1 failed".
use super::*;
use crate::ledger::{Item, Status, ToolDetail};

const SAID: &str = "Failed to create unified exec process: helper_unknown_error: setup refresh had errors";

// case: is known by what Codex said, and nothing else is
#[test]
fn is_known_by_what_codex_said_and_nothing_else_is() {
    assert!(sandbox_did_not_start(Some(SAID)));
    assert!(!sandbox_did_not_start(Some("v22.11.0")));
    assert!(!sandbox_did_not_start(Some("Get-Content : Cannot find path")));
    assert!(!sandbox_did_not_start(None));
    // Either of Codex's two phrases, in any case.
    assert!(sandbox_did_not_start(Some("exec failed: HELPER_UNKNOWN_ERROR")));
    assert!(sandbox_did_not_start(Some("Setup refresh had errors")));
    // What the row says, in Electron's words (CODEX_SANDBOX_DID_NOT_START).
    assert_eq!(SANDBOX_DID_NOT_START, "Codex\u{2019}s Windows sandbox could not set itself up, so this command never started. That happens when another program holds one of the sandbox\u{2019}s files, usually a Codex app process still running in the background (codex.exe or codex-computer-use-swift.exe in Task Manager). Once it has ended, ask again; or switch this teammate to Auto, which runs without the sandbox.");
}

fn shell(command: &str, output: &str, exit_code: Option<i64>) -> Item {
    ran(command, output, exit_code, Status::Failed)
}
fn ran(command: &str, output: &str, exit_code: Option<i64>, status: Status) -> Item {
    Item::Tool {
        name: "shell".into(),
        summary: String::new(),
        detail: Box::new(ToolDetail { id: command.into(), arguments: command.into(), output: output.into(), exit_code, finished: true, ..Default::default() }),
        status,
    }
}
fn said(items: &[Item]) -> String {
    crate::steps_line::steps_line(items, true, None, false).segments.iter().map(|s| s.text.clone()).collect::<Vec<_>>().join(" · ")
}

// case: says the sandbox did not start, where it read only "1 failed"
#[test]
fn says_the_sandbox_did_not_start_where_it_read_only_1_failed() {
    assert_eq!(said(&[shell("node --version", SAID, None)]), "Ran node --version · 1 failed · Codex\u{2019}s sandbox did not start");
    assert_eq!(said(&[shell("node --version", SAID, None), shell("npm --version", SAID, None)]), "Ran 2 commands · 2 failed · Codex\u{2019}s sandbox did not start");
}

// case: does not, when another failure is among them (control)
#[test]
fn does_not_when_another_failure_is_among_them() {
    let line = said(&[shell("node --version", SAID, None), shell("npm test", "1 test failed", Some(1))]);
    assert!(!line.contains("sandbox"), "{line}");
    assert!(line.contains("2 failed"), "{line}");
    // Codex's words printed by a command that ran are not the sandbox failing: the one failure is another.
    let line = said(&[ran("cat codex.log", SAID, Some(0), Status::Done), shell("npm test", "1 test failed", Some(1))]);
    assert!(!line.contains("sandbox") && line.contains("1 failed"), "{line}");
}
