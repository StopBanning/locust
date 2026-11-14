//! Electron's missionView.test.ts: 'the command a shell row shows', 'rail labels', 'which tool names mean a file was
//! touched (0.35.2 QA)', and the shellCommandText cases of 'a Codex run, said as words' (0.610).
use super::*;
use crate::mission_export::edit_tool_name;

const PS: &str = r#""C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe""#;

#[test]
fn a_shell_row_shows_what_ran_not_the_host_that_ran_it() {
    assert_eq!(shell_command_text(&format!(r#"{PS} -Command "npm test""#)), "npm test");
    assert_eq!(shell_command_text(&format!("{PS} -NoProfile -NonInteractive -Command 'node --test'")), "node --test");
}

#[test]
fn a_shell_row_keeps_a_command_that_was_never_wrapped() {
    assert_eq!(shell_command_text("pnpm build"), "pnpm build");
    assert_eq!(shell_command_text("git diff -- src/streak.js"), "git diff -- src/streak.js");
}

#[test]
fn a_shell_row_unwraps_a_cmd_exe_host_too() {
    assert_eq!(shell_command_text(r#"C:\Windows\System32\cmd.exe /d /s /c "npm run build""#), "npm run build");
}

#[test]
fn a_shell_row_undoes_the_quote_doubling_the_host_introduced() {
    assert_eq!(shell_command_text(&format!(r#"{PS} -Command "rg -n ""streak"" src""#)), r#"rg -n "streak" src"#);
}

#[test]
fn a_codex_command_shows_as_the_shell_got_it_when_every_backslash_was_escaped() {
    let escaped = r#""C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe" -Command "Get-Content 'C:\\Users\\me\\.codex\\skills\\imagegen\\SKILL.md'; Get-Content notes.md""#;
    assert_eq!(shell_command_text(escaped), r"Get-Content 'C:\Users\me\.codex\skills\imagegen\SKILL.md'; Get-Content notes.md");
}

#[test]
fn a_codex_command_is_left_alone_when_one_backslash_is_not_half_of_a_pair() {
    let plain = r#""C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe" -Command "Get-Content C:\Users\me\notes.md""#;
    assert_eq!(shell_command_text(plain), r"Get-Content C:\Users\me\notes.md");
}

#[test]
fn a_codex_command_still_undoes_an_escaped_quote_inside_an_escaped_payload() {
    let quoted = r#""C:\\Windows\\powershell.exe" -Command "rg \"streak\" C:\\work""#;
    assert_eq!(shell_command_text(quoted), r#"rg "streak" C:\work"#);
}

#[test]
fn a_shell_row_undoes_an_escaped_quote_even_where_the_payload_was_not_escaped_whole() {
    assert_eq!(shell_command_text(r#"powershell.exe -Command "echo \"hi\" C:\x""#), r#"echo "hi" C:\x"#);
}

#[test]
fn a_shell_row_undoes_powershells_doubled_single_quote_inside_either_quoting() {
    assert_eq!(shell_command_text(r#"powershell.exe -Command "Write-Output 'it''s' C:\\x""#), r"Write-Output 'it's' C:\x");
    assert_eq!(shell_command_text("powershell.exe -Command 'Write-Output ''hi'''"), "Write-Output 'hi'");
}

#[test]
fn a_shell_row_whose_host_wrapped_only_a_lone_quote_shows_nothing_and_never_panics() {
    assert_eq!(shell_command_text("powershell.exe -Command \""), "");
    assert_eq!(shell_command_text("cmd.exe /c '"), "");
}

#[test]
fn a_rail_label_collapses_whitespace_and_bounds_a_long_command() {
    let long = r#"tool.shell · "C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -Command "Get-Content -Raw -LiteralPath .\package.json""#;
    let label = rail_label(long);
    assert_eq!(label.chars().count(), RAIL_LABEL_LIMIT);
    assert!(label.ends_with('\u{2026}'));
    assert!(label.starts_with("tool.shell · \"C:\\Windows"));
}

#[test]
fn a_rail_label_leaves_a_short_label_exactly_as_it_is() {
    assert_eq!(rail_label("tool.shell · pnpm test"), "tool.shell · pnpm test");
    let exactly = "x".repeat(RAIL_LABEL_LIMIT);
    assert_eq!(rail_label(&exactly), exactly, "at the limit is not past it");
}

#[test]
fn a_rail_label_never_lets_a_multi_line_command_become_multiple_lines() {
    assert_eq!(rail_label("a\nb\n  c"), "a b c");
}

#[test]
fn a_to_do_list_or_a_sub_agent_writer_is_not_a_file_touched() {
    assert!(!edit_tool_name("todowrite"));
    assert!(!edit_tool_name("write_agent"));
    assert!(!edit_tool_name("todoread"));
}

#[test]
fn a_removal_which_cursor_calls_delete_is_a_file_touched() {
    assert!(edit_tool_name("delete"));
    assert!(edit_tool_name("deleteFile"));
}

#[test]
fn the_ordinary_edit_tools_still_count_and_the_readers_do_not() {
    for name in ["write", "Write", "edit", "apply_patch", "file_change", "create_file", "rename"] {
        assert!(edit_tool_name(name), "{name}");
    }
    for name in ["read", "grep", "glob", "shell", "web_search"] {
        assert!(!edit_tool_name(name), "{name}");
    }
}
