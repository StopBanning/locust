//! Electron's `tool-vocabulary.test.ts` (every tool name five runtimes produced, and what each one IS) and
//! `command-rows.test.ts` (what counts as a command): `mission_export::is_shell_tool` and `edit_tool_name`.

use crate::mission_export::{edit_tool_name, is_shell_tool};

macro_rules! vocabulary {
    ($($test:ident: $name:literal, $kind:literal, shell $shell:literal, edit $edit:literal;)*) => {$(
        #[test]
        fn $test() {
            assert_eq!(is_shell_tool($name, Some($kind)), $shell, "shell");
            assert_eq!(edit_tool_name($name), $edit, "edit");
        }
    )*};
}

vocabulary! {
    antigravity_run_command_is_a_command: "run_command", "run_command", shell true, edit false;
    antigravity_view_file_is_neither: "view_file", "view_file", shell false, edit false;
    antigravity_write_to_file_is_a_file_change: "write_to_file", "write_to_file", shell false, edit true;
    antigravity_list_dir_is_neither: "list_dir", "list_dir", shell false, edit false;
    antigravity_grep_search_is_neither: "grep_search", "grep_search", shell false, edit false;
    antigravity_ask_question_is_neither: "ask_question", "ask_question", shell false, edit false;
    codex_shell_is_a_command: "shell", "command_execution", shell true, edit false;
    codex_web_search_is_neither: "web_search", "web_search", shell false, edit false;
    cursor_shell_is_a_command: "shell", "shell", shell true, edit false;
    cursor_read_is_neither: "read", "read", shell false, edit false;
    cursor_edit_is_a_file_change: "edit", "edit", shell false, edit true;
    cursor_delete_is_a_file_change: "delete", "delete", shell false, edit true;
    cursor_glob_is_neither: "glob", "glob", shell false, edit false;
    cursor_grep_is_neither: "grep", "grep", shell false, edit false;
    cursor_task_is_neither: "task", "task", shell false, edit false;
    cursor_await_is_neither: "await", "await", shell false, edit false;
    cursor_mcp_is_neither: "mcp", "mcp", shell false, edit false;
    opencode_bash_is_a_command: "bash", "bash", shell true, edit false;
    opencode_read_is_neither: "read", "read", shell false, edit false;
    opencode_grep_is_neither: "grep", "grep", shell false, edit false;
    opencode_skill_is_neither: "skill", "skill", shell false, edit false;
    opencode_write_is_a_file_change: "write", "write", shell false, edit true;
    opencode_todowrite_is_neither: "todowrite", "todowrite", shell false, edit false;
    claude_todowrite_is_neither: "TodoWrite", "tool_use", shell false, edit false;
    copilot_write_agent_is_neither: "write_agent", "tool_use", shell false, edit false;
    copilot_powershell_is_a_command: "powershell", "powershell", shell true, edit false;
    copilot_view_is_neither: "view", "view", shell false, edit false;
}

#[test]
fn a_name_that_only_looks_at_a_file_is_never_a_change_however_it_is_spelled() {
    for name in ["view_file", "read_file", "readFile", "open_file", "show_file", "list_files", "searchFiles"] {
        assert!(!edit_tool_name(name), "{name}");
    }
    for name in ["write_to_file", "writeFile", "edit_file", "delete_file", "create_file", "apply_patch"] {
        assert!(edit_tool_name(name), "{name}");
    }
}

#[test]
fn never_calls_a_connector_tool_a_file_change_whatever_its_verb() {
    for name in [
        "mcp__github__create_issue", "mcp__github__create_pull_request", "mcp__github__delete_file", "mcp__github__push_files",
        "mcp__claude_ai_Gmail__create_draft", "mcp__claude_ai_Google_Drive__create_file", "Google_Drive__create_file", "mcp__linear__create_issue",
    ] {
        assert!(!edit_tool_name(name), "{name}");
    }
}

// command-rows.test.ts
#[test]
fn a_command_is_every_runtimes_shell_however_it_spells_it() {
    assert!(is_shell_tool("Bash", Some("tool_use")), "Claude Code");
    assert!(is_shell_tool("bash", Some("bash")), "OpenCode");
    assert!(is_shell_tool("shell", None), "the older name");
    assert!(is_shell_tool("anything", Some("command_execution")), "Codex");
}

#[test]
fn a_command_is_not_everything() {
    assert!(!is_shell_tool("Read", Some("tool_use")));
    assert!(!is_shell_tool("bashful", Some("tool_use")));
    assert!(!is_shell_tool("todowrite", Some("todowrite")));
    assert!(!is_shell_tool("task", Some("task")));
}
