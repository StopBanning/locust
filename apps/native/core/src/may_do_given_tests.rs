//! what-it-may-do-is-what-it-was-given.test.ts: what the panel says a run may do is read back out of the command Rust's
//! own builder makes for that runtime and mode (its config, its tool list, its flags), so a change to what a run is
//! given fails here until the panel says it too.

use super::what_it_may_do;
use crate::approval::{Mode, codex_app_server_policy};
use crate::runtime::{codex, copilot, cursor, muse, opencode_config};
use serde_json::{Value, json};
use std::path::Path;

const FOLDER: &str = "/work/pebble";
const SANDBOXES: [&str; 3] = ["read-only", "workspace-write", "full-access"];

fn said(runtime: &str, sandbox: &str, mode: Option<&str>) -> Vec<String> {
    what_it_may_do(runtime, sandbox, mode).rows.iter().map(|r| format!("{} {}", r.verdict, r.text)).collect()
}

fn has(rows: &[String], line: &str) -> bool {
    rows.iter().any(|r| r == line)
}

/// The mode a person picks that runs at this sandbox (Edit for workspace-write).
fn mode_of(sandbox: &str) -> Mode {
    match sandbox {
        "read-only" => Mode::Ask,
        "workspace-write" => Mode::AcceptEdits,
        _ => Mode::Auto,
    }
}

fn opencode_permission(mode: Mode) -> Value {
    opencode_config::legacy_config(mode, Some("file:///guard.mjs"))["permission"].clone()
}

fn claude(mode: Mode, skills: &[&str]) -> Vec<String> {
    crate::native::claude_args(mode, Some("bridge.json"), None, "guard.json", None, None, None, &[], skills).unwrap()
}

fn after<'a>(args: &'a [String], flag: &str) -> &'a str {
    &args[args.iter().position(|a| a == flag).unwrap_or_else(|| panic!("{flag} in {args:?}")) + 1]
}

// case: OpenCode: the web, commands and the folder, from the config the run is given
#[test]
fn opencode_the_web_commands_and_the_folder_from_its_config() {
    for sandbox in SANDBOXES {
        let permission = opencode_permission(mode_of(sandbox));
        let rows = said("opencode", sandbox, None);
        if sandbox == "full-access" {
            assert_eq!(permission["external_directory"], "allow");
            assert!(has(&rows, "allow run any command your account can run, and reach any network"));
            continue;
        }
        // The web: OpenCode's own default where no rule names it, and allowed by name after Ask's catch-all.
        for web in [&permission["webfetch"], &permission["websearch"]] {
            assert!(web.is_null() || web == "allow", "{sandbox}: {web}");
        }
        assert!(has(&rows, "allow search the web and open web pages"), "{sandbox}");
        if sandbox == "read-only" {
            let first = permission.as_object().unwrap().iter().next().map(|(k, v)| (k.clone(), v.clone()));
            assert_eq!(first, Some(("*".into(), json!("ask"))));
            assert!(has(&rows, "deny using tools from your own MCP servers"));
        } else {
            assert!(permission["*"].is_null());
            assert!(!rows.join("\n").contains("MCP"));
        }
        let joined = rows.join("\n");
        assert!(!joined.contains("network access") && !rows.iter().any(|r| r.starts_with("deny") && r.contains("network")), "{sandbox}");
        // A sealed run refuses every "ask"; nothing named means allowed.
        if permission["bash"] == "ask" {
            assert!(has(&rows, "deny running commands"), "{sandbox}");
        }
        if permission["bash"].is_null() {
            assert!(has(&rows, "allow run commands, which can reach anything your account can"), "{sandbox}");
        }
        assert_eq!(permission["external_directory"], "deny", "{sandbox}");
        assert!(has(&rows, "deny opening files outside this folder, other than by running a command"), "{sandbox}");
    }
}

// case: OpenCode, Approve each: what its server asks for is what the panel says it asks for
#[test]
fn opencode_approve_each_says_what_its_server_asks_for() {
    let p = opencode_permission(Mode::ApproveEach);
    assert_eq!([&p["edit"], &p["bash"], &p["webfetch"], &p["external_directory"]], [&json!("ask"); 4]);
    let first = p.as_object().unwrap().iter().next().map(|(k, v)| (k.clone(), v.clone()));
    assert_eq!(first, Some(("*".into(), json!("ask"))));
    assert_eq!(p["websearch"], "allow");
    let rows = said("opencode", "workspace-write", Some("approve-each"));
    for line in [
        "allow change files in this folder, once you approve each change",
        "allow run commands and open web pages, once you approve each one",
        "allow search the web",
        "allow open files outside this folder, once you approve it",
        "allow use tools from your own MCP servers, once you approve each one",
    ] {
        assert!(has(&rows, line), "{line}");
    }
}

// case: Claude Code: commands only when its tool list has Bash; the web in every mode, asked about outside Auto
#[test]
fn claude_code_commands_only_with_bash_and_the_web_asked_about_outside_auto() {
    for sandbox in ["read-only", "workspace-write"] {
        let args = claude(mode_of(sandbox), &[]);
        let tools: Vec<&str> = after(&args, "--tools").split(',').collect();
        let rows = said("claude", sandbox, None);
        assert!(tools.contains(&"WebFetch") && tools.contains(&"WebSearch"), "{sandbox}: {tools:?}");
        // Given, and asked about: no allow rule names them, so each goes to the bridge's card.
        let list = after(&args, "--tools");
        assert!(!args.iter().any(|a| a != list && (a.starts_with("WebSearch") || a.starts_with("WebFetch"))), "{args:?}");
        assert!(has(&rows, "allow search the web and open web pages, once you approve each one"));
        let commands = if tools.contains(&"Bash") { "allow run commands, which can reach anything your account can" } else { "deny running commands" };
        assert!(has(&rows, commands), "{sandbox}");
        assert!(tools.contains(&"Skill"));
        assert!(has(&rows, "allow use this folder's skills, yours when Settings lends them, and the ones you kept from GitHub, with only the tools listed here"));
        let handed = claude(mode_of(sandbox), &["/data/claude-skills/run/project"]);
        let at = handed.iter().position(|a| a == "--plugin-dir").expect("--plugin-dir");
        assert_eq!(handed[at..at + 4], ["--plugin-dir", "/data/claude-skills/run/project", "--add-dir", "/data/claude-skills/run/project"], "{sandbox}");
    }
    // Auto runs as the person and finds the folder's skills and theirs itself; it needs no --add-dir for the kept ones.
    let auto = claude(Mode::Auto, &["/data/x/library"]);
    let at = auto.iter().position(|a| a == "--plugin-dir").expect("--plugin-dir");
    assert_eq!(auto[at..at + 2], ["--plugin-dir", "/data/x/library"]);
    assert!(!auto.contains(&"--add-dir".to_string()));
    assert!(after(&auto, "--tools").split(',').any(|t| t == "Skill"));
}

fn copilot_args(sandbox: &str) -> Vec<String> {
    copilot::prompt_args("go", mode_of(sandbox), None, None, None, None, &[], None)
}

// case: Copilot CLI: commands refused only where its deny list says so, and paths held unless Auto
#[test]
fn copilot_commands_refused_only_where_its_deny_list_says() {
    assert!(copilot_args("read-only").contains(&"--deny-tool=write,shell".to_string()));
    assert!(has(&said("copilot", "read-only", None), "deny running commands"));
    assert!(copilot::mcp_off_args(Mode::Ask, &["notes".into()]).contains(&"--disable-mcp-server".to_string()));
    assert!(has(&said("copilot", "read-only", None), "deny starting the MCP servers in your Copilot settings"));
    let edit = copilot_args("workspace-write").join(" ");
    assert!(!edit.contains("--deny-tool") && !edit.contains("--allow-all-paths"), "{edit}");
    let rows = said("copilot", "workspace-write", None);
    assert!(has(&rows, "allow run commands, which can reach anything your account can"));
    assert!(has(&rows, "deny opening files outside this folder, other than by running a command"));
    assert!(copilot_args("full-access").contains(&"--allow-all-paths".to_string()));
}

fn muse_args(sandbox: &str) -> Vec<String> {
    muse::exec_args(Path::new(FOLDER), mode_of(sandbox), None, None, None).unwrap()
}

// case: Muse Code: its sandbox stays on, its web tools stay on, its shell is off when read-only
#[test]
fn muse_keeps_its_sandbox_and_web_and_turns_its_shell_off_when_read_only() {
    let read_only = muse_args("read-only");
    assert!(read_only.contains(&"--disable-write".to_string()) && read_only.contains(&"--disable-shell".to_string()), "{read_only:?}");
    assert!(has(&said("muse", "read-only", None), "deny running commands"));
    let edit = muse_args("workspace-write").join(" ");
    assert!(!edit.contains("--disable-sandbox") && !edit.contains("--yolo") && !edit.contains("--disable-web-tools"), "{edit}");
    assert!(edit.contains("--approval-mode never"), "{edit}");
    let rows = said("muse", "workspace-write", None);
    assert!(has(&rows, "allow run commands inside Muse's sandbox") && has(&rows, "allow search the web and open web pages"), "{rows:?}");
    let auto = muse_args("full-access");
    assert!(auto.contains(&"--disable-sandbox".to_string()) && !auto.contains(&"--yolo".to_string()), "{auto:?}");
    assert!(has(&said("muse", "full-access", None), "allow run any command your account can run, and reach any network"));
}

// case: Codex: its own sandbox in every mode, and Approve each asks
#[test]
fn codex_its_own_sandbox_in_every_mode_and_approve_each_asks() {
    assert_eq!(codex_app_server_policy(Mode::Ask, None).sandbox, "read-only");
    assert!(has(&said("codex", "read-only", None), "allow run commands in Codex's sandbox, which lets them read and change nothing"));
    assert_eq!(codex_app_server_policy(Mode::AcceptEdits, None).sandbox, "workspace-write");
    assert!(has(&said("codex", "workspace-write", None), "allow run commands in Codex's sandbox, which keeps their changes in this folder"));
    assert_eq!(codex_app_server_policy(Mode::ApproveEach, None).approval_policy, "untrusted");
    assert!(has(&said("codex", "workspace-write", Some("approve-each")), "allow run commands in Codex's sandbox, asking you before any that does more than read"));
    assert_eq!(codex_app_server_policy(Mode::Auto, None).sandbox, "danger-full-access");
    assert!(has(&said("codex", "full-access", None), "allow run any command your account can run, and reach any network"));
    // Read-only starts none of the person's Codex programs; no other mode says anything about them.
    let config = codex::read_only_config(&json!({"mcp_servers": {"notes": {"command": "notes"}}})).unwrap();
    assert_eq!(config["features.plugins"], false);
    assert_eq!(config["features.computer_use"], false);
    assert_eq!(config["mcp_servers.notes.enabled"], false);
    assert!(has(&said("codex", "read-only", None), "deny starting the MCP servers and plugins your Codex runs as programs on this computer, or its own browser and computer use"));
    for sandbox in ["workspace-write", "full-access"] {
        assert!(!said("codex", sandbox, None).join("\n").contains("MCP"), "{sandbox}");
    }
}

// case: Codex: its own web search is left on in every mode, and Approve each says it does not ask
#[test]
fn codex_leaves_its_web_search_on_and_approve_each_says_it_does_not_ask() {
    assert!(!codex::app_server_args(Some("0.162.0")).join(" ").contains("web_search"));
    // Only a sealed run turns it off; the read-only config a person's Ask gets leaves it alone.
    assert!(codex::read_only_config(&json!({})).unwrap()["web_search"].is_null());
    for sandbox in SANDBOXES {
        assert!(has(&said("codex", sandbox, None), "allow search the web and open web pages"), "{sandbox}");
    }
    assert!(has(&said("codex", "workspace-write", Some("approve-each")), "allow search the web and open web pages, without asking"));
}

fn cursor_args(sandbox: &str) -> Vec<String> {
    cursor::print_args(Path::new(FOLDER), mode_of(sandbox), None, None, None).unwrap()
}

// case: Cursor Agent: says nothing about commands it was not given a rule for
#[test]
fn cursor_says_nothing_about_commands_it_was_not_given_a_rule_for() {
    assert!(!cursor_args("workspace-write").contains(&"--force".to_string()));
    assert_eq!(
        said("cursor", "workspace-write", None),
        [
            "allow read the files in this folder",
            "allow change files in this folder",
            "deny searching the web or opening web pages, which Cursor Agent allows here only in Auto",
        ]
    );
    assert!(cursor_args("full-access").contains(&"--force".to_string()));
    let auto = said("cursor", "full-access", None);
    assert!(has(&auto, "allow run any command your account can run, and reach any network") && has(&auto, "allow search the web and open web pages"));
    // Ask and Plan leave out --approve-mcps, so a folder's own .cursor/mcp.json server is never started.
    assert!(!cursor_args("read-only").contains(&"--approve-mcps".to_string()));
    assert!(has(&said("cursor", "read-only", None), "deny starting the MCP servers this folder names"));
    assert!(cursor_args("workspace-write").contains(&"--approve-mcps".to_string()));
    assert!(!said("cursor", "workspace-write", None).join("\n").contains("MCP"));
}

// case: the web, runtime by runtime, as measured on 2026-10-09 (probe-web-search-each-runtime)
#[test]
fn the_web_runtime_by_runtime_as_measured() {
    assert!(copilot_args("read-only").contains(&"--deny-tool=write,shell".to_string()));
    for sandbox in SANDBOXES {
        assert!(has(&said("copilot", sandbox, None), "allow search the web and open web pages"), "{sandbox}");
    }
    for runtime in ["cursor", "antigravity"] {
        assert!(has(&said(runtime, "full-access", None), "allow search the web and open web pages"), "{runtime}");
        for sandbox in ["read-only", "workspace-write"] {
            assert!(said(runtime, sandbox, None).iter().any(|r| r.starts_with("deny searching the web")), "{runtime} {sandbox}");
        }
    }
}

// case: leaves what it was not given to the runtime, by name
#[test]
fn leaves_what_it_was_not_given_to_the_runtime_by_name() {
    assert_eq!(what_it_may_do("cursor", "workspace-write", None).rest, "Anything not listed is left to Cursor Agent's own settings.");
    assert_eq!(what_it_may_do("opencode", "read-only", None).rest, "Anything not listed is left to OpenCode's own settings.");
}

// case: never denies anything in Auto, on any runtime that offers it
#[test]
fn never_denies_anything_in_auto() {
    for runtime in ["codex", "claude", "cursor", "opencode", "copilot"] {
        assert!(what_it_may_do(runtime, "full-access", None).rows.iter().all(|r| r.verdict == "allow"), "{runtime}");
    }
}
