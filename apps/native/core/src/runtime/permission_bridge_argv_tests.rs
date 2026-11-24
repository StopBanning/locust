//! Electron's `packages/runtime-adapters/test/permission-bridge-argv.test.ts`, case by case, on `print_args`: the
//! command line the Claude Code runtime (runtime/claude.rs) launches every turn with. Electron's sandboxes are this
//! app's modes: read-only is Ask, workspace-write is Accept edits (Approve each when it asks before each), full access
//! is Auto. Codex's side is `codex::app_server_args`, the only command line a native Codex turn has.
use super::*;

const BRIDGE: &str = "C:\\Users\\<home>\\AppData\\Local\\Temp\\locust-permission-abc\\mcp.json";

fn claude(mode: Mode, bridge: Option<&str>) -> Vec<String> {
    print_args(&PrintOptions { mode, bridge, ..Default::default() }).expect("a command line")
}

fn after(args: &[String], flag: &str) -> Option<String> {
    let at = args.iter().position(|a| a == flag)?;
    args.get(at + 1).cloned()
}

#[test]
fn is_named_config_and_tool_both_in_every_mode_that_asks() {
    for mode in [Mode::Ask, Mode::Plan, Mode::AcceptEdits, Mode::ApproveEach] {
        let args = claude(mode, Some(BRIDGE));
        assert_eq!(after(&args, "--mcp-config").as_deref(), Some(BRIDGE), "{mode:?}");
        assert_eq!(after(&args, "--permission-prompt-tool").as_deref(), Some("mcp__locust__approve"), "{mode:?}");
    }
}

#[test]
fn is_absent_in_auto_which_asks_nothing() {
    let args = claude(Mode::Auto, Some(BRIDGE)).join(" ");
    assert!(!args.contains("--mcp-config"), "{args}");
    assert!(!args.contains("--permission-prompt-tool"), "{args}");
}

#[test]
fn is_absent_when_no_bridge_was_registered_rather_than_naming_a_file_that_does_not_exist() {
    for mode in [Mode::Ask, Mode::AcceptEdits] {
        let args = claude(mode, None).join(" ");
        assert!(!args.contains("--mcp-config"), "{mode:?}: {args}");
        assert!(!args.contains("--permission-prompt-tool"), "{mode:?}: {args}");
    }
}

#[test]
fn does_not_replace_the_connector_allow_rules_it_comes_after_them() {
    let args = print_args(&PrintOptions {
        mode: Mode::AcceptEdits,
        connectors: &["claude.ai Robinhood"],
        bridge: Some(BRIDGE),
        ..Default::default()
    })
    .unwrap();
    let rules = args.iter().position(|a| a == "--allowedTools").expect("the allow rules");
    let bridge = args.iter().position(|a| a == "--mcp-config").expect("the bridge");
    assert_eq!(args[rules + 1], "mcp__claude_ai_Robinhood__*");
    assert!(bridge > rules, "{args:?}");
}

#[test]
fn is_ignored_by_a_runtime_that_has_no_such_flags() {
    for version in [None, Some("0.130.0")] {
        let codex = super::super::codex::app_server_args(version).join(" ");
        assert!(!codex.contains("mcp-config"), "{codex}");
        assert!(!codex.contains("permission-prompt-tool"), "{codex}");
    }
}

#[test]
fn refuses_an_empty_config_path_rather_than_sending_a_flag_with_nothing_after_it() {
    for empty in ["", "   "] {
        assert!(print_args(&PrintOptions { mode: Mode::AcceptEdits, bridge: Some(empty), ..Default::default() }).is_err(), "{empty:?}");
    }
}

#[test]
fn asks_before_every_edit_and_command_in_approve_each_claude_codes_default_mode_the_editing_tools_the_bridge() {
    // case: asks before every edit and command in Approve each: Claude Code's default mode, the editing tools, the bridge
    let args = claude(Mode::ApproveEach, Some(BRIDGE));
    assert_eq!(after(&args, "--permission-mode").as_deref(), Some("default"));
    let tools = after(&args, "--tools").unwrap();
    assert!(tools.contains("Write"), "{tools}");
    assert!(tools.contains("Bash"), "{tools}");
    assert_eq!(after(&args, "--permission-prompt-tool").as_deref(), Some("mcp__locust__approve"));
    assert_eq!(after(&claude(Mode::AcceptEdits, Some(BRIDGE)), "--permission-mode").as_deref(), Some("acceptEdits"));
}

#[test]
fn never_runs_approve_each_without_the_bridge_where_every_action_would_be_refused_unseen() {
    let refused = print_args(&PrintOptions { mode: Mode::ApproveEach, ..Default::default() }).unwrap_err();
    assert!(refused.contains("permission host"), "{refused}");
    let ask = claude(Mode::Ask, Some(BRIDGE));
    assert!(!after(&ask, "--tools").unwrap().contains("Write"));
    let auto = claude(Mode::Auto, None);
    assert_eq!(after(&auto, "--permission-mode").as_deref(), Some("bypassPermissions"));
}
