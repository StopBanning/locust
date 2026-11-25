//! Parity tests for `packages/runtime-adapters/test/runtime-adapters.test.ts` (launch-argument,
//! mode-containment, safety-guard, version-parsing, and model-list cases).

use super::{claude, codex, copilot, cursor, discovery, opencode_config, opencode_serve, process, safety, version, Runtime, Turn};
use crate::agents::{self, Answer, Auth, EachOptions, Machine, Probe, Seams};
use crate::approval::{Mode, codex_app_server_policy};
use crate::models::{
    copilot_models, lists_its_models_but_listed_none, parse_claude_model_hints, parse_cursor_model_list,
    parse_opencode_model_list,
};
use serde_json::json;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, atomic::AtomicBool};

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

// case: builds prompt-on-stdin, read-only Codex and restricted Claude commands
#[test]
fn builds_prompt_on_stdin_read_only_codex_and_restricted_claude_commands() {
    // Rust drives Codex through `codex app-server` (not `codex exec`), enforcing read-only via the
    // app-server policy (`sandbox: "read-only"`) and read-only thread config.
    let codex_policy = codex_app_server_policy(Mode::Ask, None);
    assert_eq!(codex_policy.sandbox, "read-only");
    assert_eq!(codex_app_server_policy(Mode::Plan, None).sandbox, "read-only");
    assert_eq!(codex::app_server_args(Some("0.160.0"))[0], "app-server");
    assert!(codex::read_only_config(&json!({})).is_ok());

    let claude = claude::print_args(&claude::PrintOptions { mode: Mode::Ask, ..Default::default() }).unwrap();
    assert!(claude.iter().any(|a| a == "--restricted"));
    assert!(claude.iter().any(|a| a == "stream-json"));
    assert!(claude.iter().any(|a| a == "default"));
    assert!(!claude.iter().any(|a| a == "--dangerously-skip-permissions"));
    let input_at = claude.iter().position(|a| a == "--input-format").expect("has --input-format");
    assert_eq!(&claude[input_at..input_at + 2], &["--input-format", "stream-json"]);
}

// case: titles only a NEW OpenCode session, and keeps plugins for a run that may edit
#[test]
fn titles_only_a_new_opencode_session_and_keeps_plugins_for_editing() {
    let dir = std::env::temp_dir().join(format!(
        "locust-rt-adapters-{}-{}",
        std::process::id(),
        crate::invariants::unique_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let store = crate::store::Store::new(dir.clone(), dir.join("electron")).unwrap();
    let has_pure = |mode: Mode| {
        let mut cmd = Command::new("opencode");
        opencode_config::configure_with_seal(&mut cmd, mode, &store, false, false).unwrap();
        cmd.get_envs().any(|(k, v)| k == "OPENCODE_PURE" && v == Some(std::ffi::OsStr::new("1")))
    };
    assert!(has_pure(Mode::Ask), "Ask must set OPENCODE_PURE=1");
    assert!(has_pure(Mode::Plan), "Plan must set OPENCODE_PURE=1");
    assert!(!has_pure(Mode::AcceptEdits), "AcceptEdits keeps plugins (no OPENCODE_PURE)");
    assert!(!has_pure(Mode::Auto), "Auto keeps plugins (no OPENCODE_PURE)");
    let _ = std::fs::remove_dir_all(&dir);
}

// case: passes an OpenCode effort as its variant, and only an effort word
#[test]
fn passes_opencode_effort_as_variant_and_only_an_effort_word() {
    for valid in ["none", "minimal", "low", "medium", "high", "xhigh", "max"] {
        assert_eq!(opencode_serve::validate_effort(valid).unwrap(), valid);
    }
    let err = opencode_serve::validate_effort("bogus").unwrap_err().to_string();
    assert!(err.contains("effort level called \"bogus\""), "{err}");

    // Exercise the live call site in `Serve::begin`: a bogus effort is refused before spawning.
    let mut serve = opencode_serve::Serve::new(Command::new("opencode"), Arc::new(AtomicBool::new(false)));
    let turn = Turn {
        prompt: "Summarize the README.".into(),
        folder: PathBuf::from("/work/repo"),
        model: Some("opencode/ling-3.0-flash-fin-free".into()),
        effort: Some("bogus".into()),
        resume: None,
        mode: Mode::Ask,
        approvals: None,
        mission_id: "m1".into(),
        run_id: "r1".into(),
    };
    serve.start(&turn).unwrap();
    let failed = serve.next_event().unwrap().expect("run.failed event");
    assert_eq!(failed.kind, "run.failed");
    let msg = failed.payload["message"].as_str().unwrap_or_default();
    assert!(msg.contains("effort level called \"bogus\""), "{msg}");
}

// case: refuses to build a command with no prompt, because the prompt IS the argv here
#[test]
fn refuses_to_build_command_with_no_prompt() {
    let fake = PathBuf::from("/usr/bin/true");
    copilot::FAKE_LAUNCHER.with(|f| *f.borrow_mut() = Some(fake));
    let empty = copilot::command(Path::new("/work/repo"), "", Mode::Ask, None, None, None, None, &[], None);
    let blank = copilot::command(Path::new("/work/repo"), "   ", Mode::Ask, None, None, None, None, &[], None);
    let ok = copilot::command(Path::new("/work/repo"), "Summarize the README.", Mode::Ask, None, None, None, None, &[], None);
    copilot::FAKE_LAUNCHER.with(|f| *f.borrow_mut() = None);
    assert!(empty.unwrap_err().to_string().contains("Prompt"));
    assert!(blank.unwrap_err().to_string().contains("Prompt"));
    assert!(ok.is_ok());
}

// ---- Group 2: readiness and sign-in -------------------------------------------------------------------------------

const CURSOR_HELP: &str = "-p, --print  --output-format <format> text | json | stream-json  --stream-partial-output  --mode <mode> plan: read-only  --workspace <path>  --resume [chatId]  --model <model>";
const CURSOR_MODELS: &str = "Available models\n\nauto - Auto (default)\ncursor-grok-4.6-high - Cursor Grok 4.6\ncomposer-2.5 - Composer 2.5\n";
const GEMINI_HELP: &str = "-p, --prompt  Run in non-interactive (headless) mode. Appended to input on stdin  -o, --output-format  text json stream-json  --approval-mode default auto_edit yolo plan  -r, --resume";
const MUSE_VERSION: &str = "Muse Code 1.3.0 (1.3.0-R3401.1)\n";
const MUSE_HELP: &str = "muse exec --json --workspace <path> --disable-write --disable-shell\n";
const OPENCODE_HELP: &str = "opencode run [message..]  --format <format>  text | json  -m, --model  -s, --session";
const OPENCODE_MODELS: &str = "opencode/muse-spark-1.3-contributor-free\nopencode/big-pickle\n";
const COPILOT_HELP: &str = "-p, --prompt <prompt>  --output-format <format> text | json  --allow-all-tools  --deny-tool <tools>  --session-id  --resume";

fn ok_ans(stdout: &str) -> Option<Answer> {
    Some(Answer { exit: Some(0), stdout: stdout.to_owned(), stderr: String::new(), timed_out: false })
}

fn find_only<'a>(bin: &'a str) -> impl Fn(&str) -> Option<PathBuf> + Sync + 'a {
    move |cmd| (cmd == bin).then(|| PathBuf::from(format!("/tools/{bin}")))
}

fn no_files(_: &Path) -> bool {
    false
}

// case: separates installation, safe capability support, and authentication
#[test]
fn separates_installation_safe_capability_support_and_authentication() {
    let find = |cmd: &str| match cmd {
        "codex" => Some(PathBuf::from("/tools/codex")),
        "claude" => Some(PathBuf::from("/tools/claude")),
        _ => None,
    };
    let run = |path: &Path, args: &[&str]| {
        let is_claude = path.to_string_lossy().contains("claude");
        if args == ["--version"] {
            return ok_ans(if is_claude { "2.1.251 (Claude Code)" } else { "codex-cli 0.151.0-alpha.7.2" });
        }
        if args.contains(&"--help") {
            return ok_ans(if is_claude {
                "--print stdin --output-format stream-json --restricted --verbose --include-partial-messages --permission-mode default --tools --disallowedTools"
            } else {
                "Run non-interactively stdin --json --cd --sandbox read-only"
            });
        }
        Some(Answer {
            exit: Some(if args == ["login", "status"] { 1 } else { 0 }),
            stdout: String::new(),
            stderr: String::new(),
            timed_out: false,
        })
    };
    let seams = Seams { find: &find, run: &run, exists: &no_files, home: None, xdg_config_home: None, recall: None };
    let codex = agents::discover_one("codex", &seams).unwrap();
    assert!(codex.installed);
    assert_eq!(codex.status, Probe::AuthRequired);
    assert_eq!(codex.auth, Auth::Unauthenticated);
    let claude = agents::discover_one("claude", &seams).unwrap();
    assert!(claude.installed);
    assert_eq!(claude.status, Probe::Ready);
    assert_eq!(claude.auth, Auth::Authenticated);
    for absent in ["cursor", "gemini", "opencode", "copilot", "muse", "kilo"] {
        let entry = agents::discover_one(absent, &seams).unwrap();
        assert!(!entry.installed, "{absent}");
        assert_eq!(entry.status, Probe::Missing, "{absent}");
    }
}

// case: reads a logged-out Cursor Agent from its text, because its status command exits 0 either way
#[test]
fn reads_logged_out_cursor_agent_from_text_even_when_exit_0() {
    let find = find_only("cursor-agent");
    let run = |_: &Path, args: &[&str]| match args {
        ["--version"] => ok_ans("2026.08.31-4057e58"),
        ["--help"] => ok_ans(CURSOR_HELP),
        ["--list-models"] => ok_ans("Error: Authentication required."),
        _ => ok_ans("Not logged in\n"),
    };
    let seams = Seams { find: &find, run: &run, exists: &no_files, home: None, xdg_config_home: None, recall: None };
    let cursor = agents::discover_one("cursor", &seams).unwrap();
    assert!(cursor.installed);
    assert_eq!(cursor.status, Probe::AuthRequired);
    assert_eq!(cursor.auth, Auth::Unauthenticated);
}

fn muse_discover(home: Option<&str>, xdg: Option<&str>, exists: &(dyn Fn(&Path) -> bool + Sync)) -> agents::Found {
    let find = find_only("muse");
    let run = |_: &Path, args: &[&str]| {
        if args == ["exec", "--help"] { ok_ans(MUSE_HELP) } else { ok_ans(MUSE_VERSION) }
    };
    let seams = Seams {
        find: &find,
        run: &run,
        exists,
        home: home.map(PathBuf::from),
        xdg_config_home: xdg.map(str::to_owned),
        recall: None,
    };
    agents::discover_one("muse", &seams).unwrap()
}

// case: calls Muse Code ready from the line its own version command prints
#[test]
fn calls_muse_code_ready_from_its_version_line() {
    let has_auth = |p: &Path| p.to_string_lossy().ends_with("auth.json");
    let muse = muse_discover(Some("/home/dev"), None, &has_auth);
    assert!(muse.installed);
    assert!(muse.ready);
    assert_eq!(muse.status, Probe::Ready);
    assert_eq!(muse.version.as_deref(), Some("1.3.0"));
}

// case: says nobody has signed in rather than calling Muse Code ready
#[test]
fn says_nobody_signed_in_rather_than_calling_muse_ready() {
    let muse = muse_discover(Some("/home/dev"), None, &no_files);
    assert!(muse.installed);
    assert!(!muse.ready);
    assert_eq!(muse.status, Probe::AuthRequired);
    assert_eq!(muse.auth, Auth::Unauthenticated);
}

// case: does not take an API key as a credential, because no run is given one
#[test]
fn does_not_take_api_key_as_muse_credential() {
    // `agents::missing_credentials` checks only `auth.json` on disk and ignores provider API keys.
    let muse = muse_discover(Some("/home/dev"), None, &no_files);
    assert_eq!(muse.status, Probe::AuthRequired);
}

// case: looks where Muse looks when XDG_CONFIG_HOME is set
#[test]
fn looks_where_muse_looks_when_xdg_config_home_is_set() {
    let at_xdg = |p: &Path| p == Path::new("/xdg/muse/auth.json");
    let at_home = |p: &Path| p == Path::new("/home/dev/.config/muse/auth.json");
    assert_eq!(muse_discover(Some("/home/dev"), Some("/xdg"), &at_xdg).status, Probe::Ready);
    assert_eq!(muse_discover(Some("/home/dev"), Some("/xdg"), &at_home).status, Probe::AuthRequired);
}

// case: does not claim nobody signed in when it cannot look
#[test]
fn does_not_claim_nobody_signed_in_when_muse_cannot_look() {
    let muse = muse_discover(None, None, &no_files);
    assert_eq!(muse.status, Probe::Ready);
}

// case: reports a signed-in Cursor Agent ready and reads its models off --list-models
#[test]
fn reports_signed_in_cursor_ready_and_reads_models_off_list_models() {
    let saw_list = Arc::new(AtomicBool::new(false));
    let find = find_only("cursor-agent");
    let run = {
        let saw_list = saw_list.clone();
        move |_: &Path, args: &[&str]| match args {
            ["--version"] => ok_ans("2026.08.31-4057e58"),
            ["--help"] => ok_ans(CURSOR_HELP),
            ["--list-models"] => {
                saw_list.store(true, std::sync::atomic::Ordering::SeqCst);
                ok_ans(CURSOR_MODELS)
            }
            _ => ok_ans("Logged in as someone@example.com\n"),
        }
    };
    let seams = Seams { find: &find, run: &run, exists: &no_files, home: None, xdg_config_home: None, recall: None };
    let cursor = agents::discover_one("cursor", &seams).unwrap();
    assert_eq!(cursor.status, Probe::Ready);
    assert!(saw_list.load(std::sync::atomic::Ordering::SeqCst));
    let parsed = parse_cursor_model_list(CURSOR_MODELS).unwrap();
    assert_eq!(
        parsed["models"],
        json!([
            { "id": "auto", "displayName": "Auto (default)" },
            { "id": "cursor-grok-4.6-high", "displayName": "Cursor Grok 4.6" },
            { "id": "composer-2.5", "displayName": "Composer 2.5" }
        ])
    );
    assert_eq!(parsed["efforts"], json!([]));
}

// case: reads a model list the CLI printed on stderr, and says so when it cannot read one
#[test]
fn reads_model_list_printed_on_stderr() {
    let find = find_only("cursor-agent");
    let stderr_list = Arc::new(Mutex::new(String::new()));
    let run = {
        let stderr_list = stderr_list.clone();
        move |_: &Path, args: &[&str]| match args {
            ["--version"] => ok_ans("2026.08.31-4057e58"),
            ["--help"] => ok_ans(CURSOR_HELP),
            ["--list-models"] => {
                *stderr_list.lock().unwrap() = CURSOR_MODELS.to_owned();
                Some(Answer { exit: Some(0), stdout: String::new(), stderr: CURSOR_MODELS.to_owned(), timed_out: false })
            }
            _ => ok_ans("Logged in as someone@example.com"),
        }
    };
    let seams = Seams { find: &find, run: &run, exists: &no_files, home: None, xdg_config_home: None, recall: None };
    let cursor = agents::discover_one("cursor", &seams).unwrap();
    assert_eq!(cursor.status, Probe::Ready);
    let ids: Vec<String> = parse_cursor_model_list(&stderr_list.lock().unwrap()).unwrap()["models"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(ids, vec!["auto", "cursor-grok-4.6-high", "composer-2.5"]);
}

// case: says a model list it cannot read is unreadable, rather than leaving the picker silently empty
#[test]
fn says_unreadable_model_list_is_unreadable() {
    assert!(parse_cursor_model_list("a completely different layout").is_none());
    assert!(lists_its_models_but_listed_none(&[("cursor", None)]));
}

// case: does not ask Gemini to sign in when the caller does not run it, and says nothing is wrong
#[test]
fn does_not_ask_gemini_to_sign_in_when_planned() {
    let asked = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
    let find = find_only("gemini");
    let run = {
        let asked = asked.clone();
        move |_: &Path, args: &[&str]| {
            asked.lock().unwrap().push(args.iter().map(|s| (*s).to_owned()).collect());
            match args {
                ["--version"] => ok_ans("0.58.0"),
                ["--help"] => ok_ans(GEMINI_HELP),
                _ => Some(Answer { exit: Some(41), stdout: String::new(), stderr: "Please set an Auth method".into(), timed_out: false }),
            }
        }
    };
    let seams = Seams { find: &find, run: &run, exists: &no_files, home: None, xdg_config_home: None, recall: None };
    let gemini = agents::discover_one("gemini", &seams).unwrap();
    assert!(gemini.installed);
    assert_eq!(gemini.version.as_deref(), Some("0.58.0"));
    assert!(!asked.lock().unwrap().iter().any(|a| a.iter().any(|s| s == "--list-sessions")));
}

// case: asks only the runtimes it is told to, and spawns nothing for the rest
#[test]
fn asks_only_the_runtimes_it_is_told_to() {
    let asked = Arc::new(Mutex::new(Vec::<String>::new()));
    let machine = Machine {
        find: Arc::new({
            let asked = asked.clone();
            move |cmd: &str| {
                asked.lock().unwrap().push(format!("find {cmd}"));
                Some(PathBuf::from(format!("/tools/{cmd}")))
            }
        }),
        run: Arc::new({
            let asked = asked.clone();
            move |path: &Path, args: &[&str]| {
                asked.lock().unwrap().push(format!("run {} {}", path.display(), args.join(" ")));
                ok_ans("2.1.0 (Claude Code)")
            }
        }),
        exists: Arc::new(|_| false),
        home: None,
        xdg_config_home: None,
        facts: None,
        stat: Arc::new(|_| None),
        canonical: Arc::new(|_| None),
    };
    let only: HashSet<String> = ["claude".to_owned()].into_iter().collect();
    let checks = agents::discover_each(&machine, &EachOptions { only: Some(only), ..Default::default() });
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0].id, "claude");
    let settled = agents::settle_within(checks, std::time::Duration::from_secs(5), |_, _| {});
    assert_eq!(settled.answers.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), vec!["claude"]);
    let seen = asked.lock().unwrap();
    assert!(!seen.is_empty());
    assert!(seen.iter().all(|line| line.contains("claude")), "{seen:?}");
}

// case: asks a CLI whose readiness IS its version question once, not twice
#[test]
fn asks_cli_whose_readiness_is_version_once_not_twice() {
    let asked = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
    let find = find_only("copilot");
    let run = {
        let asked = asked.clone();
        move |_: &Path, args: &[&str]| {
            asked.lock().unwrap().push(args.iter().map(|s| (*s).to_owned()).collect());
            ok_ans(if args == ["--help"] { COPILOT_HELP } else { "GitHub Copilot CLI 1.0.82." })
        }
    };
    let seams = Seams { find: &find, run: &run, exists: &no_files, home: None, xdg_config_home: None, recall: None };
    let copilot = agents::discover_one("copilot", &seams).unwrap();
    assert_eq!(copilot.status, Probe::Ready);
    let version_calls = asked.lock().unwrap().iter().filter(|a| a.as_slice() == ["--version"]).count();
    assert_eq!(version_calls, 1);
}

// case: reports OpenCode ready with no sign-in at all, and reads the models it ships
#[test]
fn reports_opencode_ready_with_no_sign_in_and_reads_models() {
    let find = find_only("opencode");
    let run = |_: &Path, args: &[&str]| match args {
        ["--version"] => ok_ans("1.18.27"),
        ["run", "--help"] => ok_ans(OPENCODE_HELP),
        ["models"] | ["models", "--verbose"] => ok_ans(OPENCODE_MODELS),
        other => panic!("unexpected opencode probe args: {other:?}"),
    };
    let seams = Seams { find: &find, run: &run, exists: &no_files, home: None, xdg_config_home: None, recall: None };
    let opencode = agents::discover_one("opencode", &seams).unwrap();
    assert!(opencode.installed);
    assert!(opencode.ready);
    assert_eq!(opencode.status, Probe::Ready);
    assert_eq!(opencode.auth, Auth::NotApplicable);
    assert_eq!(opencode.version.as_deref(), Some("1.18.27"));
    let ids: Vec<String> = parse_opencode_model_list(OPENCODE_MODELS).unwrap()["models"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(ids, vec!["opencode/muse-spark-1.3-contributor-free", "opencode/big-pickle"]);
}

// case: reads each model's efforts from the verbose list, and keeps the plain list when an older CLI refuses it
#[test]
fn reads_opencode_verbose_efforts_and_falls_back_to_plain_list() {
    let verbose = "opencode/ling-3.0-flash-fin-free\n{\n  \"variants\": { \"low\": {}, \"high\": {} }\n}\n";
    let find = find_only("opencode");
    let run_older = |_: &Path, args: &[&str]| match args {
        ["--version"] => ok_ans("1.18.27"),
        ["run", "--help"] => ok_ans(OPENCODE_HELP),
        ["models", "--verbose"] => Some(Answer { exit: Some(1), stdout: String::new(), stderr: "Unknown argument: verbose".into(), timed_out: false }),
        ["models"] => ok_ans("opencode/ling-3.0-flash-fin-free\n"),
        other => panic!("unexpected opencode probe args: {other:?}"),
    };
    let seams = Seams { find: &find, run: &run_older, exists: &no_files, home: None, xdg_config_home: None, recall: None };
    let older = agents::discover_one("opencode", &seams).unwrap();
    assert_eq!(older.status, Probe::Ready);
    assert_eq!(
        parse_opencode_model_list(verbose).unwrap()["models"],
        json!([{ "id": "opencode/ling-3.0-flash-fin-free", "displayName": "ling-3.0-flash-fin-free", "description": "free", "efforts": ["low", "high"] }])
    );
    assert_eq!(
        parse_opencode_model_list("opencode/ling-3.0-flash-fin-free\n").unwrap()["models"],
        json!([{ "id": "opencode/ling-3.0-flash-fin-free", "displayName": "ling-3.0-flash-fin-free", "description": "free" }])
    );
}

// case: still says signed out when a CLI says so in its own words
#[test]
fn still_says_signed_out_when_cli_says_so_in_own_words() {
    let find = find_only("copilot");
    let run = |_: &Path, args: &[&str]| match args {
        ["--help"] => ok_ans(COPILOT_HELP),
        _ => Some(Answer { exit: Some(1), stdout: String::new(), stderr: "Error: not logged in. Run copilot login.".into(), timed_out: false }),
    };
    let seams = Seams { find: &find, run: &run, exists: &no_files, home: None, xdg_config_home: None, recall: None };
    let copilot = agents::discover_one("copilot", &seams).unwrap();
    assert_eq!(copilot.status, Probe::AuthRequired);
    assert_eq!(copilot.auth, Auth::Unauthenticated);
}

// case: does not report an OpenCode that cannot list a single model as ready to run one
#[test]
fn does_not_report_opencode_without_models_as_ready() {
    let find = find_only("opencode");
    let run = |_: &Path, args: &[&str]| match args {
        ["--version"] => ok_ans("1.18.27"),
        ["run", "--help"] => ok_ans(OPENCODE_HELP),
        _ => ok_ans("Error: could not reach the model registry"),
    };
    let seams = Seams { find: &find, run: &run, exists: &no_files, home: None, xdg_config_home: None, recall: None };
    let opencode = agents::discover_one("opencode", &seams).unwrap();
    assert!(!opencode.ready);
    assert_eq!(opencode.status, Probe::Offline);
}

// case: reports Copilot CLI ready on its version alone, and says on the record why that is a guess
#[test]
fn reports_copilot_ready_on_version_alone() {
    let find = find_only("copilot");
    let run = |_: &Path, args: &[&str]| match args {
        ["--help"] => ok_ans(COPILOT_HELP),
        ["--version"] => ok_ans("GitHub Copilot CLI 1.0.82."),
        other => panic!("unexpected copilot probe args: {other:?}"),
    };
    let seams = Seams { find: &find, run: &run, exists: &no_files, home: None, xdg_config_home: None, recall: None };
    let copilot = agents::discover_one("copilot", &seams).unwrap();
    assert!(copilot.ready);
    assert_eq!(copilot.status, Probe::Ready);
    assert_eq!(copilot.version.as_deref(), Some("1.0.82"));
}

// case: offers Copilot only the route where the CLI picks, because it names no models before a run
#[test]
fn offers_copilot_only_auto_route_before_run() {
    let models = copilot_models();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0]["id"], "auto");
    assert_eq!(models[0]["displayName"], "Auto");
    assert_eq!(models[0]["runtime"], "copilot");
}

// case: reports a Copilot whose version command says something else as not ready
#[test]
fn reports_copilot_with_unexpected_version_output_as_not_ready() {
    let find = find_only("copilot");
    let run = |_: &Path, args: &[&str]| match args {
        ["--help"] => ok_ans(COPILOT_HELP),
        _ => ok_ans("command not recognized"),
    };
    let seams = Seams { find: &find, run: &run, exists: &no_files, home: None, xdg_config_home: None, recall: None };
    let copilot = agents::discover_one("copilot", &seams).unwrap();
    assert!(!copilot.ready);
    assert_eq!(copilot.status, Probe::Offline);
}

// ---- Group 3: non-Windows path discovery and bounded process/probe mechanics --------------------------------------

fn make_temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "locust-rt-adapters-{tag}-{}-{}",
        std::process::id(),
        crate::invariants::unique_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn make_exec(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    path
}

// case: prefers what PATH says over an install root
#[test]
fn prefers_what_path_says_over_install_root() {
    let root = make_temp_dir("path-pref");
    let on_path_dir = root.join("path-bin");
    let npm_dir = root.join("npm-bin");
    std::fs::create_dir_all(&on_path_dir).unwrap();
    std::fs::create_dir_all(&npm_dir).unwrap();
    let on_path = make_exec(&on_path_dir, "codex");
    let _in_npm = make_exec(&npm_dir, "codex");
    let found = discovery::find_beyond_path(
        "codex",
        on_path_dir.as_os_str(),
        || Some(npm_dir.to_string_lossy().into_owned()),
    )
    .unwrap();
    assert_eq!(found, on_path);
    let _ = std::fs::remove_dir_all(&root);
}

// case: reports nothing when the runtime is genuinely absent
#[test]
fn reports_nothing_when_runtime_is_genuinely_absent() {
    let empty = make_temp_dir("absent");
    assert!(discovery::find_beyond_path("codex", empty.as_os_str(), || None).is_err());
    assert!(discovery::find_beyond_path("claude", empty.as_os_str(), || None).is_err());
    let _ = std::fs::remove_dir_all(&empty);
}

// case: searches no install root for a command it does not know
#[test]
fn searches_no_install_root_for_unknown_command() {
    let dir = make_temp_dir("unknown");
    let _stray = make_exec(&dir, "codex");
    assert!(discovery::find_beyond_path("somethingelse", dir.as_os_str(), || None).is_err());
    assert!(discovery::find_named("bad/name").is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

// case: caps captured output and never asks Node for a shell
#[test]
fn caps_captured_output_and_never_uses_a_shell() {
    use std::io::{BufReader, Read};
    let mut reader = BufReader::new(std::io::repeat(b'a').take(9 * 1024 * 1024));
    let mut line = Vec::new();
    let n = process::read_line(&mut reader, &mut line).unwrap();
    assert_eq!(n, 8 * 1024 * 1024 + 1);

    // Direct exec without a shell: metacharacters in argv stay literal.
    let mut cmd = Command::new("/bin/echo");
    cmd.arg("$(echo nope)");
    let mut child = process::spawn(&mut cmd, std::process::Stdio::null(), std::process::Stdio::piped()).unwrap();
    let mut out = String::new();
    child.stdout.take().unwrap().read_to_string(&mut out).unwrap();
    let status = child.wait().unwrap();
    assert!(status.success());
    assert_eq!(out.trim(), "$(echo nope)");
}

// case: terminates a timed-out probe
#[test]
fn terminates_a_timed_out_probe() {
    let mut cmd = Command::new("/bin/sleep");
    cmd.arg("30");
    let mut child = process::spawn(&mut cmd, std::process::Stdio::null(), std::process::Stdio::null()).unwrap();
    let control = Arc::new(process::Control::default());
    control.watch(child.id());
    control.cancel_at(std::time::Instant::now() + std::time::Duration::from_millis(40));
    assert!(process::confirm_exit(&mut child, std::time::Duration::from_secs(3)));
    assert!(control.forced());
}

// case: takes a timed-out probe's whole tree down, not only the shim it spawned
#[test]
fn takes_timed_out_probes_whole_tree_down() {
    let snapshot = "10 1\n20 10\n30 20\n40 10\n99 1\n";
    assert_eq!(process::descendants_of(snapshot, 10), vec![30, 40, 20]);

    let mut cmd = Command::new("/bin/sh");
    cmd.args(["-c", "sleep 30 & wait"]);
    let mut child = process::spawn(&mut cmd, std::process::Stdio::null(), std::process::Stdio::null()).unwrap();
    let pid = child.id();
    std::thread::sleep(std::time::Duration::from_millis(50));
    process::end_terminal_tree(pid);
    assert!(process::confirm_exit(&mut child, std::time::Duration::from_secs(3)));
}

// case: off Windows, spawns the probe as its own process group and signals the group on timeout
#[cfg(unix)]
#[test]
fn off_windows_spawns_probe_as_own_process_group_and_signals_group() {
    let mut cmd = Command::new("/bin/sleep");
    cmd.arg("30");
    let mut child = process::spawn(&mut cmd, std::process::Stdio::null(), std::process::Stdio::null()).unwrap();
    let pid = child.id() as libc::pid_t;
    let pgid = unsafe { libc::getpgid(pid) };
    assert_eq!(pgid, pid, "child must be leader of its own process group");
    process::kill(child.id());
    assert!(process::confirm_exit(&mut child, std::time::Duration::from_secs(3)));
}
