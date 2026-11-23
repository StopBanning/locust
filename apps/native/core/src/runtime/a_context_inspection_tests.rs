//! a-context-inspection-keeps-the-turns-tools.test.ts: `/context` runs on the turn's own command line, and Claude's
//! reference rules use the absolute-path Edit deny.
use super::*;

const SESSION: &str = "12345678-1234-1234-1234-123456789012";

fn after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).map(String::as_str)
}

#[test]
fn keeps_the_turns_connectors_off_flag_settings_plugins_references_tools_model_and_approval_bridge() {
    // case: keeps the turn's connectors-off flag, settings, plugins, references, tools, model and approval bridge
    let turn = print_args(&PrintOptions {
        mode: Mode::AcceptEdits,
        account_connectors: false,
        references: &["C:/design kit"],
        skills: &["C:/plugin/project"],
        model: Some("sonnet"),
        effort: Some("high"),
        guard_settings: Some("C:/guard/settings.json"),
        bridge: Some("C:/bridge/mcp.json"),
        resume: Some("previous_session"),
        fork: true,
        ..Default::default()
    })
    .unwrap();
    let context = context_args("claude", &turn, SESSION).unwrap();
    for flag in [
        "--restricted",
        "--strict-mcp-config",
        "--settings",
        "--plugin-dir",
        "--add-dir",
        "--tools",
        "--permission-mode",
        "--mcp-config",
        "--permission-prompt-tool",
        "--disallowedTools",
        "--model",
        "--effort",
    ] {
        assert!(context.iter().any(|a| a == flag), "{flag}");
        assert_eq!(after(&context, flag), after(&turn, flag), "{flag}");
    }
    assert_eq!(
        context[context.len() - 7..],
        ["--output-format", "json", "--no-session-persistence", "--resume", SESSION, "--", "/context"]
    );
    assert!(!context.iter().any(|a| a == "--input-format"));
    assert!(!context.iter().any(|a| a == "--fork-session"));
    assert!(!context.iter().any(|a| a == "previous_session"));
    // As a command: the turn's folder, and nothing on stdin.
    let mut launch = Command::new("/usr/local/bin/claude");
    launch.args(&turn).current_dir("/work/project").env("CLAUDE_CONFIG_DIR", "/accounts/a");
    let command = context_command(&launch, SESSION).unwrap();
    assert_eq!(command.get_current_dir(), Some(Path::new("/work/project")));
    assert_eq!(command.get_args().map(|a| a.to_string_lossy().into_owned()).collect::<Vec<_>>(), context);
    assert!(command.get_envs().any(|(k, v)| k == "CLAUDE_CONFIG_DIR" && v == Some("/accounts/a".as_ref())));
}

#[test]
fn does_not_turn_on_strict_mcp_or_add_reference_denials_to_an_auto_invocation_that_did_not_have_them() {
    // case: does not turn on strict MCP or add reference denials to an Auto invocation that did not have them
    let turn = print_args(&PrintOptions {
        mode: Mode::Auto,
        account_connectors: true,
        references: &["C:/reference"],
        ..Default::default()
    })
    .unwrap();
    let context = context_args("claude", &turn, SESSION).unwrap();
    assert!(!context.iter().any(|a| a == "--strict-mcp-config"));
    assert!(!context.iter().any(|a| a == "--disallowedTools"));
    assert!(context.iter().any(|a| a == "bypassPermissions"));
}

#[test]
fn refuses_a_lost_or_truncated_launch_a_non_claude_command_or_disabled_slash_commands_instead_of_guessing() {
    // case: refuses a lost or truncated launch, a non-Claude command, or disabled slash commands instead of guessing
    let turn = print_args(&PrintOptions { bridge: Some("/b"), ..Default::default() }).unwrap();
    assert!(context_args("codex", &turn, SESSION).is_err());
    let with = |extra: &str| [turn.clone(), vec![extra.to_owned()]].concat();
    assert!(context_args("claude", &with("--disable-slash-commands"), SESSION).is_err());
    assert!(context_args("claude", &with("truncated…"), SESSION).is_err());
    assert!(context_args("claude", &[], SESSION).is_err());
    assert!(context_args("claude", &turn, "not a session").is_err());
    assert!(context_command(&Command::new("/usr/local/bin/codex"), SESSION).is_err());
}

#[test]
fn uses_the_documented_windows_posix_drive_spelling_and_the_edit_rule_that_also_governs_write_and_notebookedit() {
    // case: uses the documented Windows POSIX drive spelling and the Edit rule that also governs Write and NotebookEdit
    assert_eq!(reference_deny_rule("C:\\References\\Design kit\\").unwrap(), "Edit(//c/References/Design kit/**)");
    assert_eq!(reference_deny_rule("/opt/design kit").unwrap(), "Edit(//opt/design kit/**)");
    let args = print_args(&PrintOptions {
        mode: Mode::AcceptEdits,
        bridge: Some("/b"),
        references: &["C:/reference", "D:/other"],
        ..Default::default()
    })
    .unwrap();
    let at = args.iter().position(|a| a == "--disallowedTools").unwrap();
    assert_eq!(args[at + 1..], ["Edit(//c/reference/**)", "Edit(//d/other/**)"]);
    assert!(!args.iter().any(|a| a == "Write(//c/reference/**)"));
    assert!(after(&args, "--tools").unwrap().contains("Bash"));
    assert!(!args.iter().any(|a| a == "Bash"));
}

#[test]
fn adds_no_path_denials_in_ask_or_auto_and_refuses_ambiguous_patterns_before_granting_access() {
    // case: adds no path denials in Ask or Auto and refuses ambiguous patterns before granting access
    for mode in [Mode::Ask, Mode::Auto] {
        let args = print_args(&PrintOptions { mode, references: &["C:/reference"], ..Default::default() }).unwrap();
        assert!(!args.iter().any(|a| a == "--disallowedTools"), "{mode:?}");
    }
    for folder in ["C:/foo*", "C:/foo[old]", "C:/foo,bar", "//server/share", "relative"] {
        assert!(reference_deny_rule(folder).unwrap_err().contains("cannot be protected"), "{folder}");
    }
}
