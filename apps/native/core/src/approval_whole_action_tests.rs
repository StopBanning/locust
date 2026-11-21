//! AN APPROVAL SHOWS THE WHOLE ACTION (Electron `an-approval-shows-the-whole-action.test.ts`, QA-2026-09-29 round 2,
//! R14). The card is headed "exact action" and once showed 600 characters of a connector call while Approve let all of
//! it through: a mail whose recipient was written after a long body was approved without the recipient on the card.
use super::*;
use serde_json::json;

// case: shows a connector call whole, down to a recipient written last
#[test]
fn shows_a_connector_call_whole_down_to_a_recipient_written_last() {
    let body = "Here is the summary you asked for. ".repeat(26);
    assert!(body.len() > 900);
    let (kind, _, detail, _, _) = describe_claude_tool(
        "mcp__claude_ai_Gmail__send_message",
        &json!({ "subject": "Weekly summary", "body": body, "to": "someone-else@example.com" }),
        "C:/work",
    );
    assert_eq!(kind, Kind::Connector);
    assert!(detail.contains(r#""to": "someone-else@example.com""#), "{detail}");
    assert!(detail.contains(body.trim()));
    // One field to a line, so the fields read as fields, in the order the model wrote them.
    assert!(detail.split('\n').count() >= 5, "{detail}");
    assert!(detail.find("\"subject\"").unwrap() < detail.find("\"body\"").unwrap());
    assert!(detail.find("\"body\"").unwrap() < detail.find("\"to\"").unwrap());
}

// case: keeps a command's line breaks, as the command will run
#[test]
fn keeps_a_commands_line_breaks_as_the_command_will_run() {
    let command = "cat > notes.txt <<'EOF'\nfirst line\nsecond line\nEOF\ncurl -X POST https://example.com/upload -d @notes.txt";
    let (kind, _, detail, _, _) = describe_claude_tool("Bash", &json!({ "command": command }), "C:/work");
    assert_eq!(kind, Kind::Command);
    assert_eq!(detail, command);
}

// case: shows the path of a Codex command on Windows as the shell gets it
#[test]
fn shows_the_path_of_a_codex_command_on_windows_as_the_shell_gets_it() {
    let sent = r#""C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe" -Command 'python -m pytest -q'"#;
    let (_, _, detail, _) =
        describe_codex("item/commandExecution/requestApproval", &json!({ "command": sent, "cwd": "C:/acme" })).unwrap();
    assert_eq!(detail, r#""C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe" -Command 'python -m pytest -q'"#);
}

// case: leaves a command as sent when its path was not escaped as a whole, and leaves the rest alone
#[test]
fn leaves_a_command_as_sent_when_its_path_was_not_escaped_as_a_whole_and_leaves_the_rest_alone() {
    let plain = r#""C:\Program Files\Git\bin\bash.exe" -lc 'ls'"#;
    assert_eq!(program_as_run(plain), plain);
    let mixed = r#""C:\\tools\odd.exe" run"#;
    assert_eq!(program_as_run(mixed), mixed);
    assert_eq!(program_as_run(r#""C:\\bin\\x.exe" 'a\\b'"#), r#""C:\bin\x.exe" 'a\\b'"#);
    assert_eq!(program_as_run("npm test"), "npm test");
    // A quote never closed is not a path.
    assert_eq!(program_as_run(r#""C:\\bin\\x.exe run"#), r#""C:\\bin\\x.exe run"#);
}

// case: counts what it cannot show, and never cuts it silently
#[test]
fn counts_what_it_cannot_show_and_never_cuts_it_silently() {
    let shown = whole_detail(&format!("{}{}", "a".repeat(20_000), "b".repeat(1234)));
    assert!(shown.contains("[1,234 more characters not shown here. Deny if you cannot see enough to decide.]"), "{}", &shown[shown.len() - 120..]);
    assert!(!shown.contains('b'));
    assert_eq!(MAX_DETAIL, 20_000);
    // One past the limit is one character, said so.
    assert!(whole_detail(&"a".repeat(20_001)).ends_with("[1 more character not shown here. Deny if you cannot see enough to decide.]"));
    // At the limit, nothing is cut or counted.
    assert_eq!(whole_detail(&"a".repeat(20_000)), "a".repeat(20_000));
}
