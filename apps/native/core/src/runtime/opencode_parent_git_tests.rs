//! Electron packages/runtime-adapters/test/the-parent-git-is-reachable-on-every-platform.test.ts: an OpenCode run in a
//! worktree may reach the parent repository's `.git`, and nothing else outside its folder.
use crate::approval::Mode;
use crate::runtime::opencode_config::{for_run, git_dir_pattern, legacy_config, legacy_sealed_config, with_git_reads, with_parent_git, with_references};
use serde_json::json;

// case: is spelled as the root is: backslashes on Windows, slashes elsewhere
#[test]
fn is_spelled_as_the_root_is_backslashes_on_windows_slashes_elsewhere() {
    assert_eq!(git_dir_pattern("C:\\work\\repo"), "C:\\work\\repo\\.git\\*");
    assert_eq!(git_dir_pattern("/home/u/repo"), "/home/u/repo/.git/*");
    assert_eq!(git_dir_pattern("/home/u/repo/"), "/home/u/repo/.git/*");
    // A Windows share has no drive letter; its backslashes alone say how it is spelled.
    assert_eq!(git_dir_pattern("\\\\server\\repo"), "\\\\server\\repo\\.git\\*");
}

// case: is the one directory allowed outside the worktree
#[test]
fn is_the_one_directory_allowed_outside_the_worktree() {
    for mode in [Mode::AcceptEdits, Mode::Ask, Mode::Plan, Mode::ApproveEach] {
        let config = with_parent_git(legacy_config(mode, None), mode, "/home/u/repo");
        assert_eq!(
            config["permission"]["external_directory"],
            json!({ "/home/u/repo/.git/*": "allow", "*": "deny" }),
            "{mode:?}"
        );
    }
    // A read-only worktree run keeps its denials, as opencodeWorktreeConfig(root, true) spreads them in.
    let ask = with_parent_git(legacy_config(Mode::Ask, None), Mode::Ask, "/home/u/repo");
    assert_eq!(ask["permission"]["edit"], "deny");
    assert_eq!(ask["experimental"]["continue_loop_on_deny"], true);
    // Auto reaches past both.
    let auto = with_parent_git(legacy_config(Mode::Auto, None), Mode::Auto, "/home/u/repo");
    assert_eq!(auto["permission"]["external_directory"], "allow");
    // Reference folders a read-only run may read stay readable beside it.
    let refs = with_parent_git(with_references(legacy_config(Mode::Ask, None), Mode::Ask, &["/refs/a"]), Mode::Ask, "/home/u/repo");
    assert_eq!(refs["permission"]["external_directory"]["/refs/a"], "allow");
    assert_eq!(refs["permission"]["external_directory"]["/home/u/repo/.git/*"], "allow");
    assert_eq!(refs["permission"]["external_directory"]["*"], "deny");
}

#[test]
fn the_git_tool_is_the_runs_own_server_in_its_folder_and_its_tools_are_allowed_last() {
    let command = vec!["/Applications/Locust.app/Contents/MacOS/locust-native".to_owned(), "--git-reads".to_owned()];
    let config = with_git_reads(legacy_config(Mode::Ask, None), &command, "/work/project");
    assert_eq!(
        config["mcp"]["locust_git"],
        json!({ "type": "local", "command": command, "environment": { "LOCUST_GIT_FOLDER": "/work/project" }, "enabled": true })
    );
    let rules: Vec<&String> = config["permission"].as_object().unwrap().keys().collect();
    assert_eq!(rules.first().map(|k| k.as_str()), Some("*"));
    assert_eq!(rules.last().map(|k| k.as_str()), Some("locust_git_*"));
    assert_eq!(config["permission"]["locust_git_*"], "allow");
    assert_eq!(config["permission"]["bash"], "ask");
}

#[test]
fn a_run_gets_the_parent_git_in_a_worktree_and_the_git_tool_only_with_no_shell_and_a_sealed_run_neither() {
    let tool = vec!["locust-native".to_owned(), "--git-reads".to_owned()];
    let ask = for_run(legacy_config(Mode::Ask, None), Mode::Ask, false, Some("/repo"), Some(&tool), "/repo/.locust/worktrees/tm_wren");
    assert_eq!(ask["permission"]["external_directory"], json!({ "/repo/.git/*": "allow", "*": "deny" }));
    assert_eq!(ask["mcp"]["locust_git"]["environment"]["LOCUST_GIT_FOLDER"], "/repo/.locust/worktrees/tm_wren");
    let plan = for_run(legacy_config(Mode::Plan, None), Mode::Plan, false, None, Some(&tool), "/repo");
    assert_eq!(plan["permission"]["external_directory"], "deny");
    assert!(plan["mcp"]["locust_git"].is_object());
    // Any other mode's shell runs git itself.
    let edit = for_run(legacy_config(Mode::AcceptEdits, None), Mode::AcceptEdits, false, None, Some(&tool), "/repo");
    assert!(edit.get("mcp").is_none());
    let sealed = for_run(legacy_sealed_config(None), Mode::Ask, true, Some("/repo"), Some(&tool), "/repo");
    assert_eq!(sealed, legacy_sealed_config(None));
}
