//! Parity tests for `packages/runtime-adapters/test/runtime-adapters.test.ts` (launch-argument,
//! mode-containment, safety-guard, version-parsing, and model-list cases).

use super::{claude, copilot, cursor, opencode_config, safety, version};
use crate::approval::{Mode, codex_app_server_policy};
use crate::models::{parse_claude_model_hints, parse_cursor_model_list, parse_opencode_model_list};
use serde_json::json;
use std::path::Path;

fn s(slice: &[&str]) -> Vec<String> {
    slice.iter().map(|x| (*x).to_owned()).collect()
}

// case: holds a read-only Claude mission in default mode with a reading tool list plus Task, never plan mode
#[test]
fn holds_read_only_claude_in_default_mode_with_reading_tools() {
    let args = claude::print_args(&claude::PrintOptions { mode: Mode::Ask, ..Default::default() }).unwrap();
    assert!(args.iter().any(|a| a == "--restricted"));
    let joined = args.join(" ");
    assert!(joined.contains("--permission-mode default"), "{joined}");
    assert!(joined.contains("--tools Read,Glob,Grep,Task,Skill,WebSearch,WebFetch,mcp__*"), "{joined}");
    assert!(!joined.contains("Edit"), "{joined}");
    assert!(!joined.contains("Bash"), "{joined}");
}

// case: lets a Claude mission that may edit actually edit, and run what it changed
#[test]
fn lets_claude_edit_mode_edit_and_run_commands() {
    let args = claude::print_args(&claude::PrintOptions { mode: Mode::AcceptEdits, ..Default::default() }).unwrap();
    let joined = args.join(" ");
    assert!(joined.contains("--permission-mode acceptEdits"), "{joined}");
    assert!(joined.contains("Edit,Write,NotebookEdit,Bash"), "{joined}");
    assert!(args.iter().any(|a| a == "--restricted"));
    assert!(!joined.contains("--disallowedTools"), "{joined}");
    assert!(joined.contains("Bash,Task,Skill,WebSearch,WebFetch,mcp__*"), "{joined}");
}

// case: rejects permission bypass arguments
#[test]
fn rejects_permission_bypass_arguments() {
    let err = safety::assert_safe_runtime_command("codex", &s(&["exec", "--dangerously-bypass-approvals-and-sandbox"]), None).unwrap_err();
    assert!(err.contains("Forbidden runtime argument"), "{err}");
    let via_claude = claude::print_args(&claude::PrintOptions {
        mode: Mode::Ask,
        model: Some("--dangerously-skip-permissions"),
        ..Default::default()
    })
    .unwrap_err();
    assert!(via_claude.contains("Forbidden runtime argument"), "{via_claude}");
}

// case: gives Claude Code bypassPermissions and the editing tools
#[test]
fn gives_claude_code_bypass_permissions_and_editing_tools() {
    let auto = claude::print_args(&claude::PrintOptions { mode: Mode::Auto, ..Default::default() }).unwrap();
    let joined = auto.join(" ");
    assert!(joined.contains("--permission-mode bypassPermissions"), "{joined}");
    assert!(joined.contains("Edit,Write,NotebookEdit,Bash"), "{joined}");
    assert!(!auto.iter().any(|a| a == "--restricted"));
    assert!(joined.contains("Bash,Task,Skill,WebSearch,WebFetch,mcp__*"), "{joined}");
    assert!(!auto.iter().any(|a| a == "--dangerously-skip-permissions"));

    for mode in [Mode::Ask, Mode::AcceptEdits] {
        let other = claude::print_args(&claude::PrintOptions { mode, account_connectors: true, ..Default::default() }).unwrap();
        assert!(other.iter().any(|a| a == "--restricted"), "{mode:?}");
        assert!(!other.join(" ").contains("--disallowedTools"), "{mode:?}");
        assert!(other.join(" ").contains("Task,Skill,WebSearch,WebFetch,mcp__*"), "{mode:?}");
        assert!(!other.iter().any(|a| a == "--strict-mcp-config"), "{mode:?}");
    }
    let asking = claude::print_args(&claude::PrintOptions { mode: Mode::Ask, ..Default::default() }).unwrap();
    assert!(!asking.join(" ").contains("Bash"));
    assert!(!asking.join(" ").contains("Write"));
}

// case: gives Codex danger-full-access, which is its own name for it
#[test]
fn gives_codex_danger_full_access_in_auto_mode() {
    let policy = codex_app_server_policy(Mode::Auto, None);
    assert_eq!(policy.sandbox, "danger-full-access");
    assert_eq!(Mode::Auto.sandbox(), "full-access");
    assert!(safety::assert_safe_runtime_command("codex", &s(&["exec", "--sandbox", "danger-full-access"]), Some("full-access")).is_ok());
}

// case: gives Cursor --force and never its --yolo alias
#[test]
fn gives_cursor_force_and_never_yolo() {
    let args = cursor::print_args(Path::new("/work/repo"), Mode::Auto, None, None, None).unwrap();
    assert!(args.iter().any(|a| a == "--force"));
    assert!(!args.iter().any(|a| a == "--yolo"));
    assert!(!args.iter().any(|a| a == "--sandbox"));
}

// case: gives Copilot --allow-all-paths and never --allow-all
#[test]
fn gives_copilot_allow_all_paths_and_never_allow_all() {
    let args = copilot::prompt_args("go", Mode::Auto, None, None, None, None, &[], None);
    assert!(args.iter().any(|a| a == "--allow-all-paths"));
    assert!(!args.iter().any(|a| a == "--allow-all"));
    assert!(!args.iter().any(|a| a == "--allow-all-urls"));
    assert!(!args.join(" ").contains("--deny-tool"));
}

// case: gives OpenCode --auto, drops the read-only denials, and says it may leave the folder
#[test]
fn gives_opencode_auto_drops_denials_and_allows_external_directory() {
    let config = opencode_config::legacy_config(Mode::Auto, None);
    assert!(config["permission"].get("edit").is_none());
    assert!(config["permission"].get("write").is_none());
    assert_eq!(config["permission"]["external_directory"], "allow");
    let in_worktree = opencode_config::for_run(config, Mode::Auto, false, Some("C:\\work\\shop"), None, "C:\\work\\repo");
    assert_eq!(
        in_worktree,
        json!({"tools": {"question": false}, "permission": {"external_directory": "allow"}})
    );
}

// case: unlocks those two arguments only for a full-access run
#[test]
fn unlocks_full_access_arguments_only_for_full_access() {
    for arg in ["--force", "--allow-all-paths"] {
        assert!(safety::assert_safe_runtime_command("cursor", &s(&[arg]), None).unwrap_err().contains("Forbidden"));
        assert!(safety::assert_safe_runtime_command("cursor", &s(&[arg]), Some("workspace-write")).unwrap_err().contains("Forbidden"));
        assert!(safety::assert_safe_runtime_command("cursor", &s(&[arg]), Some("full-access")).is_ok());
    }
    for args in [s(&["--sandbox", "danger-full-access"]), s(&["--sandbox=danger-full-access"])] {
        assert!(safety::assert_safe_runtime_command("codex", &args, None).unwrap_err().contains("danger-full-access"));
        assert!(safety::assert_safe_runtime_command("codex", &args, Some("full-access")).is_ok());
    }
    for args in [s(&["--permission-mode", "bypassPermissions"]), s(&["--permission-mode=bypassPermissions"])] {
        assert!(safety::assert_safe_runtime_command("claude", &args, None).unwrap_err().contains("bypassPermissions"));
        assert!(safety::assert_safe_runtime_command("claude", &args, Some("full-access")).is_ok());
    }
}

// case: still refuses everything else, Auto or not
#[test]
fn still_refuses_everything_else_auto_or_not() {
    for arg in [
        "--dangerously-bypass-approvals-and-sandbox",
        "--dangerously-skip-permissions",
        "--allow-dangerously-skip-permissions",
        "--dangerously-bypass-hook-trust",
        "--full-auto",
        "--yolo",
        "-y",
        "-f",
        "--ignore-rules",
        "--allow-all",
        "--allow-all-urls",
        "--share",
        "--remote",
        "--remote-export",
    ] {
        let err = safety::assert_safe_runtime_command("codex", &s(&[arg]), Some("full-access")).unwrap_err();
        assert!(err.contains("Forbidden"), "{arg}: {err}");
    }
    assert!(safety::assert_safe_runtime_command("codex", &s(&["--ask-for-approval", "never"]), Some("full-access")).unwrap_err().contains("never"));
    // The one-word spelling is refused the same, in every sandbox.
    assert!(safety::assert_safe_runtime_command("codex", &s(&["--ask-for-approval=never"]), Some("full-access")).unwrap_err().contains("never"));
    assert!(safety::assert_safe_runtime_command("gemini", &s(&["--approval-mode", "yolo"]), Some("full-access")).unwrap_err().contains("yolo"));
}

// case: reads an unstated sandbox as the strictest one
#[test]
fn reads_unstated_sandbox_as_strictest() {
    assert!(safety::assert_safe_runtime_command("cursor", &s(&["--force"]), None).unwrap_err().contains("Forbidden"));
}

// case: carries the sandbox on the spec, so the runner can judge what it spawns
#[test]
fn carries_sandbox_for_safety_judgement() {
    let auto_args = cursor::print_args(Path::new("/work/repo"), Mode::Auto, None, None, None).unwrap();
    assert_eq!(cursor::sandbox_for(Mode::Auto), "full-access");
    assert!(safety::assert_safe_runtime_command("cursor", &auto_args, Some(cursor::sandbox_for(Mode::Auto))).is_ok());

    let mut confined = cursor::print_args(Path::new("/work/repo"), Mode::AcceptEdits, None, None, None).unwrap();
    assert_eq!(cursor::sandbox_for(Mode::AcceptEdits), "workspace-write");
    confined.push("--force".into());
    assert!(safety::assert_safe_runtime_command("cursor", &confined, Some(cursor::sandbox_for(Mode::AcceptEdits))).unwrap_err().contains("Forbidden"));
}

// case: parses prerelease versions returned by installed CLIs
#[test]
fn parses_prerelease_versions_from_installed_clis() {
    assert_eq!(version::parse("codex-cli 0.151.0-alpha.7.2").unwrap().version, "0.151.0-alpha.7.2");
    assert_eq!(version::parse("2.1.251 (Claude Code)").unwrap().version, "2.1.251");
    assert_eq!(version::parse("GitHub Copilot CLI 1.0.82.").unwrap().version, "1.0.82");
    assert_eq!(version::parse("1.18.27").unwrap().version, "1.18.27");
}

// case: runs a read-only Cursor mission in ask mode, with the sandbox where it runs, and never forces commands
#[test]
fn runs_read_only_cursor_in_ask_mode_with_sandbox_and_no_force() {
    let args = cursor::print_args(Path::new("/work/repo"), Mode::Ask, None, None, None).unwrap();
    let mut expected = s(&["--print", "--output-format", "stream-json", "--stream-partial-output", "--trust", "--workspace", "/work/repo", "--mode", "ask"]);
    if cursor::cursor_sandbox_available() {
        expected.extend(s(&["--sandbox", "enabled"]));
    }
    assert_eq!(args, expected);
    assert!(!args.iter().any(|a| a == "plan"));
    assert!(!args.iter().any(|a| a == "--force"));
}

// case: approves the connectors the person already configured, because nobody is here to be asked -- outside Ask and Plan
#[test]
fn cursor_approves_mcps_outside_ask_and_plan() {
    for mode in [Mode::Ask, Mode::Plan] {
        let args = cursor::print_args(Path::new("/work/repo"), mode, None, None, None).unwrap();
        assert!(!args.iter().any(|a| a == "--approve-mcps"), "{mode:?}");
    }
    for mode in [Mode::AcceptEdits, Mode::Auto] {
        let args = cursor::print_args(Path::new("/work/repo"), mode, None, None, None).unwrap();
        assert!(args.iter().any(|a| a == "--approve-mcps"), "{mode:?}");
    }
}

// case: lets a workspace-write Cursor mission edit, still without --force
#[test]
fn lets_workspace_write_cursor_edit_without_force() {
    let args = cursor::print_args(Path::new("/work/repo"), Mode::AcceptEdits, Some("composer-1"), Some("chat_1"), None).unwrap();
    assert!(!args.iter().any(|a| a == "--mode"));
    assert!(!args.iter().any(|a| a == "--force"));
    assert_eq!(&args[args.len() - 4..], &["--model", "composer-1", "--resume", "chat_1"]);
}

// case: refuses an effort for a runtime that has no effort flag
#[test]
fn refuses_effort_for_cursor() {
    let why = cursor::refusal(Mode::Ask, Some("high")).expect("refuses effort");
    assert!(why.contains("effort"), "{why}");
}

// case: refuses OpenCode's session sharing and Copilot's ways out of the workspace
#[test]
fn refuses_opencode_share_and_copilot_escape_flags() {
    for arg in ["--share", "--allow-all-paths", "--allow-all-urls", "--allow-all", "--remote", "--remote-export", "--enable-memory"] {
        assert!(safety::assert_safe_runtime_command("opencode", &s(&[arg]), None).unwrap_err().contains("Forbidden"), "{arg}");
    }
    assert!(safety::assert_safe_runtime_command("copilot", &s(&["--allow-all-tools"]), None).is_ok());
}

// case: refuses Gemini's yolo mode and Cursor's force flag in any spelling
#[test]
fn refuses_gemini_yolo_and_cursor_force_in_any_spelling() {
    for args in [
        s(&["--approval-mode", "yolo"]),
        s(&["--approval-mode=yolo"]),
        s(&["-y"]),
        s(&["--force"]),
        s(&["-f"]),
        s(&["--yolo"]),
    ] {
        assert!(safety::assert_safe_runtime_command("gemini", &args, None).unwrap_err().contains("Forbidden"), "{args:?}");
    }
}

// case: holds a read-only OpenCode mission with the permission config that actually enforces it
#[test]
fn holds_read_only_opencode_with_enforcing_permission_config() {
    let config = opencode_config::legacy_config(Mode::Ask, None);
    assert_eq!(
        config,
        json!({
            "permission": {
                "*": "ask",
                "read": { "*": "allow", "*.env": "deny", "*.env.*": "deny", "*.env.example": "allow" },
                "glob": "allow",
                "grep": "allow",
                "list": "allow",
                "lsp": "allow",
                "task": "allow",
                "todowrite": "allow",
                "todoread": "allow",
                "skill": "allow",
                "webfetch": "allow",
                "websearch": "allow",
                "codesearch": "allow",
                "edit": "deny",
                "write": "deny",
                "bash": "ask",
                "patch": "deny",
                "external_directory": "deny",
            },
            "tools": { "question": false },
            "experimental": { "continue_loop_on_deny": true },
        })
    );
    let first_key = config["permission"].as_object().unwrap().keys().next().unwrap();
    assert_eq!(first_key, "*");
}

// case: gives a run with no shell Locust's git tool, and only such a run
#[test]
fn gives_read_only_opencode_git_tool_and_only_read_only() {
    let git_cmd = s(&["/Apps/locust-native", "--git-reads"]);
    let ask = opencode_config::for_run(opencode_config::legacy_config(Mode::Ask, None), Mode::Ask, false, None, Some(&git_cmd), "/work/repo");
    assert_eq!(
        ask["mcp"]["locust_git"],
        json!({"type": "local", "command": git_cmd, "environment": {"LOCUST_GIT_FOLDER": "/work/repo"}, "enabled": true})
    );
    assert_eq!(ask["permission"]["locust_git_*"], "allow");
    assert_eq!(ask["permission"]["bash"], "ask");
    for mode in [Mode::AcceptEdits, Mode::Auto] {
        let other = opencode_config::for_run(opencode_config::legacy_config(mode, None), mode, false, None, Some(&git_cmd), "/work/repo");
        assert!(other.get("mcp").is_none(), "{mode:?}");
        assert!(other["permission"].get("locust_git_*").is_none(), "{mode:?}");
    }
}

// case: lets a worktree run reach the repository it belongs to, and only that
#[test]
fn lets_opencode_worktree_reach_parent_git_and_only_that() {
    let wt = opencode_config::for_run(
        opencode_config::legacy_config(Mode::AcceptEdits, None),
        Mode::AcceptEdits,
        false,
        Some(r"C:\work\shop"),
        None,
        r"C:\work\shop\.locust\worktrees\tm_wren",
    );
    assert_eq!(
        wt,
        json!({
            "tools": { "question": false },
            "permission": { "external_directory": { r"C:\work\shop\.git\*": "allow", "*": "deny" } }
        })
    );
    let ro = opencode_config::for_run(
        opencode_config::legacy_config(Mode::Ask, None),
        Mode::Ask,
        false,
        Some(r"C:\work\shop"),
        None,
        r"C:\work\shop\.locust\worktrees\tm_wren",
    );
    assert_eq!(ro["permission"]["edit"], "deny");
    assert_eq!(ro["permission"]["write"], "deny");
    assert_eq!(ro["permission"]["bash"], "ask");
    assert_eq!(ro["permission"]["patch"], "deny");
    assert_eq!(ro["permission"]["external_directory"], json!({ r"C:\work\shop\.git\*": "allow", "*": "deny" }));
    assert_eq!(ro["experimental"]["continue_loop_on_deny"], true);

    let in_place = opencode_config::for_run(opencode_config::legacy_config(Mode::AcceptEdits, None), Mode::AcceptEdits, false, None, None, r"C:\work\shop");
    assert_eq!(in_place, json!({"tools": {"question": false}, "permission": {"external_directory": "deny"}}));
    assert!(!in_place.to_string().contains("allow"));
}

// case: lets a workspace-write OpenCode mission edit, and states its confinement rather than assuming it
#[test]
fn lets_workspace_write_opencode_edit_with_stated_confinement() {
    let config = opencode_config::legacy_config(Mode::AcceptEdits, None);
    assert_eq!(config, json!({"tools": {"question": false}, "permission": {"external_directory": "deny"}}));
}

// case: holds a read-only Copilot mission with the tool denylist that was measured refusing writes
#[test]
fn holds_read_only_copilot_with_tool_denylist() {
    let args = copilot::prompt_args("Summarize the README in one sentence.", Mode::Ask, None, None, None, Some("uuid-1"), &[], None);
    assert_eq!(
        args,
        s(&[
            "-p", "Summarize the README in one sentence.", "--output-format", "json", "--allow-all-tools", "--no-color",
            "--excluded-tools=session_store_sql", "--deny-tool=write,shell", "--session-id", "uuid-1"
        ])
    );
}

// case: starts none of the person's own MCP servers in a read-only Copilot mission, and only there (0.824)
#[test]
fn copilot_disables_persons_mcp_servers_only_in_read_only() {
    let servers = s(&["notes", "robinhood-trading"]);
    assert_eq!(copilot::mcp_off_args(Mode::Ask, &servers), s(&["--disable-mcp-server", "notes", "--disable-mcp-server", "robinhood-trading"]));
    for mode in [Mode::AcceptEdits, Mode::Auto] {
        assert!(copilot::mcp_off_args(mode, &servers).is_empty(), "{mode:?}");
    }
}

// case: never offers a Copilot teammate the machine's other Copilot conversations, on either route (0.378)
#[test]
fn copilot_excludes_session_store_sql_on_both_routes() {
    for args in [
        copilot::prompt_args("go", Mode::Ask, None, None, None, Some("uuid-1"), &[], None),
        copilot::prompt_args("go", Mode::Auto, None, None, Some("uuid-1"), None, &[], None),
        copilot::acp_args(None, None, None),
    ] {
        assert!(args.iter().any(|a| a == "--excluded-tools=session_store_sql"), "{args:?}");
    }
}

// case: lets a workspace-write Copilot mission edit, and resumes by the id the host minted
#[test]
fn lets_workspace_write_copilot_edit_and_resume() {
    let args = copilot::prompt_args("go", Mode::AcceptEdits, Some("gpt-5.6-luna"), None, Some("uuid-1"), None, &[], None);
    assert!(!args.iter().any(|a| a == "--deny-tool=write,shell"));
    assert_eq!(&args[args.len() - 3..], &["--model", "gpt-5.6-luna", "--resume=uuid-1"]);
    assert!(!args.iter().any(|a| a == "--session-id"));
}

// case: PASSES an effort to Copilot, which does take one
#[test]
fn passes_effort_to_copilot() {
    let args = copilot::prompt_args("go", Mode::Ask, None, Some("high"), None, None, &[], None);
    let at = args.iter().position(|a| a == "--effort").expect("has --effort");
    assert_eq!(args[at + 1], "high");
}

// case: reads each OpenCode model's variants as its efforts, in the scale's order
#[test]
fn reads_opencode_model_variants_as_efforts_in_scale_order() {
    let verbose = [
        "opencode/big-pickle",
        "{",
        r#"  "id": "big-pickle","#,
        r#"  "variants": {}"#,
        "}",
        "opencode/space-bunny-free",
        "{",
        r#"  "id": "space-bunny-free","#,
        r#"  "variants": {"#,
        r#"    "max": { "reasoningEffort": "max" },"#,
        r#"    "low": { "reasoningEffort": "low" },"#,
        r#"    "turbo": { "speed": "fast" }"#,
        "  }",
        "}",
    ]
    .join("\n");
    let parsed = parse_opencode_model_list(&verbose).unwrap();
    assert_eq!(
        parsed["models"],
        json!([
            { "id": "opencode/big-pickle", "displayName": "big-pickle" },
            { "id": "opencode/space-bunny-free", "displayName": "space-bunny-free", "description": "free", "efforts": ["low", "max"] }
        ])
    );
    let plain = parse_opencode_model_list("opencode/big-pickle\nopencode/space-bunny-free\n").unwrap();
    assert_eq!(
        plain["models"],
        json!([
            { "id": "opencode/big-pickle", "displayName": "big-pickle" },
            { "id": "opencode/space-bunny-free", "displayName": "space-bunny-free", "description": "free" }
        ])
    );
}

// case: reads OpenCode's model list as it prints it, and marks the free models free
#[test]
fn reads_opencode_model_list_and_marks_free_models() {
    let parsed = parse_opencode_model_list(
        "opencode/muse-spark-1.3-contributor-free\nopencode/nemotron-3.5-lightning-free\nopencode/big-pickle\n",
    )
    .unwrap();
    assert_eq!(
        parsed["models"],
        json!([
            { "id": "opencode/muse-spark-1.3-contributor-free", "displayName": "muse-spark-1.3-contributor-free", "description": "free" },
            { "id": "opencode/nemotron-3.5-lightning-free", "displayName": "nemotron-3.5-lightning-free", "description": "free" },
            { "id": "opencode/big-pickle", "displayName": "big-pickle" }
        ])
    );
    assert_eq!(parsed["efforts"], json!([]));
}

// case: reads no models from output that is not a model list
#[test]
fn reads_no_models_from_non_list_output() {
    assert!(parse_opencode_model_list("").is_none());
    assert!(parse_opencode_model_list("Error: something went wrong").is_none());
    assert_eq!(parse_opencode_model_list("a/b\na/b\njust words").unwrap()["models"], json!([{ "id": "a/b", "displayName": "b" }]));
}

// case: reads no models from a Cursor that is signed out, and none from a list it cannot read
#[test]
fn reads_no_models_from_signed_out_or_unreadable_cursor() {
    assert!(parse_cursor_model_list("Error: Authentication required.").is_none());
    assert!(parse_cursor_model_list("").is_none());
    assert_eq!(parse_cursor_model_list("x - X\nx - X again\njust words").unwrap()["models"], json!([{ "id": "x", "displayName": "X" }]));
}

// case: passes the chosen model and effort to Claude Code
#[test]
fn passes_chosen_model_and_effort_to_claude_code() {
    let args = claude::print_args(&claude::PrintOptions { mode: Mode::Ask, model: Some("fable"), effort: Some("high"), ..Default::default() }).unwrap();
    let m = args.iter().position(|a| a == "--model").unwrap();
    assert_eq!(args[m + 1], "fable");
    let e = args.iter().position(|a| a == "--effort").unwrap();
    assert_eq!(args[e + 1], "high");
}

// case: refuses an effort that is not one plain word
#[test]
fn refuses_effort_that_is_not_one_plain_word() {
    assert!(claude::print_args(&claude::PrintOptions { mode: Mode::Ask, effort: Some("high; rm -rf"), ..Default::default() }).unwrap_err().contains("Effort is invalid"));
    assert!(claude::print_args(&claude::PrintOptions { mode: Mode::Ask, effort: Some(""), ..Default::default() }).unwrap_err().contains("Effort is invalid"));
}

// case: reads the aliases and effort levels the Claude CLI advertises in its own help
#[test]
fn reads_claude_aliases_and_efforts_from_help() {
    let help = [
        "  --model <model>                       Model for the current session. Provide",
        "                                        an alias for the latest model (e.g.",
        "                                        'fable', 'opus', or 'sonnet') or a",
        "                                        model's full name (e.g.",
        "                                        'claude-fable-5').",
        "  --effort <level>                      Effort level for the current session",
        "                                        (low, medium, high, xhigh, max)",
    ]
    .join("\n");
    assert_eq!(
        parse_claude_model_hints(&help).unwrap(),
        json!({ "aliases": ["fable", "opus", "sonnet"], "efforts": ["low", "medium", "high", "xhigh", "max"] })
    );
}

// case: names nothing when the help does not
#[test]
fn names_nothing_when_claude_help_does_not() {
    assert!(parse_claude_model_hints("Usage: claude [options]").is_none());
}

// case: resumes a Claude session by id
#[test]
fn resumes_claude_session_by_id() {
    let args = claude::print_args(&claude::PrintOptions { mode: Mode::Ask, resume: Some("sess-42"), ..Default::default() }).unwrap();
    let at = args.iter().position(|a| a == "--resume").unwrap();
    assert_eq!(args[at + 1], "sess-42");
}

// case: starts a fresh conversation when no session is given
#[test]
fn starts_fresh_conversation_when_no_session_given() {
    let args = claude::print_args(&claude::PrintOptions { mode: Mode::Ask, ..Default::default() }).unwrap();
    assert!(!args.iter().any(|a| a == "--resume"));
}

// case: refuses a session id that is not plain text
#[test]
fn refuses_session_id_not_plain_text() {
    assert!(claude::print_args(&claude::PrintOptions { mode: Mode::Ask, resume: Some(""), ..Default::default() }).is_err());
    assert!(claude::print_args(&claude::PrintOptions { mode: Mode::Ask, resume: Some("bad\0id"), ..Default::default() }).is_err());
}
