use super::*;
use crate::voice_call_rules::{AddOn, instructions, thread_config};
use serde_json::json;
use std::{fs, path::PathBuf};

struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "locust-call-folder-{}",
            crate::invariants::unique_nanos()
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src/login.ts"),
            "export function login() {\n  // TODO: check the password\n  return true\n}\n",
        )
        .unwrap();
        fs::write(
            root.join("README.md"),
            "# Acme\nThe login page is in src/login.ts.\n",
        )
        .unwrap();
        fs::write(root.join(".env"), "SECRET=synthetic-phrase\n").unwrap();
        fs::write(root.join("logo.png"), [0x89, 0x50, 0x4e, 0x47, 0, 0, 0, 0]).unwrap();
        Self(root)
    }
    fn ask(&self, name: &str, args: Value) -> ToolResult {
        call(Some(&self.0), name, &args)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// case: finds files by their path, and never lists a file of secrets
#[test]
fn find_omits_secrets_and_matches_paths() {
    let f = Folder::new();
    let all = f.ask("find_files", json!({}));
    assert_eq!(all.text, "README.md\nlogo.png\nsrc/login.ts");
    assert_eq!(
        f.ask("find_files", json!({"query":"LOGIN"})).text,
        "src/login.ts"
    );
    assert_eq!(
        f.ask("find_files", json!({"query":"nothing-here"})).text,
        "No file's path has \"nothing-here\" in it."
    );
}

// case: reads a text file in the folder, and refuses secrets, binaries and anything outside it
#[test]
fn reads_only_safe_text_inside_the_folder() {
    let f = Folder::new();
    let read = f.ask("read_file", json!({"path":"src/login.ts"}));
    assert!(!read.is_error);
    assert!(read.text.contains("src/login.ts, lines 1-5 of 5."));
    assert!(read.text.contains("// TODO: check the password"));
    assert!(
        f.ask("read_file", json!({"path":"src/login.ts","from_line":3}))
            .text
            .contains("lines 3-5 of 5")
    );
    assert_eq!(
        f.ask("read_file", json!({"path":".env"})),
        ToolResult::refused("That file holds secrets, so a call never reads it.")
    );
    assert_eq!(
        f.ask("read_file", json!({"path":"logo.png"})),
        ToolResult::refused("logo.png is not text, so a call cannot read it out.")
    );
    let outside = Folder::new();
    let absolute = outside.0.join("README.md");
    std::os::unix::fs::symlink(absolute, f.0.join("alias.txt")).unwrap();
    for path in ["../outside.txt", "C:\\Windows\\win.ini", "alias.txt"] {
        assert!(f.ask("read_file", json!({"path":path})).is_error, "{path}");
    }
    assert_eq!(
        f.ask("read_file", json!({"path":"missing.ts"})),
        ToolResult::refused("There is no missing.ts in the folder.")
    );
}

// case: searches the folder’s text, never a file of secrets
#[test]
fn searches_text_and_excludes_secrets() {
    let f = Folder::new();
    assert_eq!(
        f.ask("search_files", json!({"text":"login"})).text,
        "README.md:2: The login page is in src/login.ts.\nsrc/login.ts:1: export function login() {"
    );
    assert!(
        f.ask("search_files", json!({"text":"synthetic-phrase"}))
            .text
            .contains("is in none of the")
    );
}

// case: needs a folder open, and does nothing else
#[test]
fn no_folder_and_no_other_tools() {
    assert_eq!(
        call(None, "find_files", &json!({})),
        ToolResult::refused("No folder is open in Locust.")
    );
    assert!(
        Folder::new()
            .ask("write_file", json!({"path":"a"}))
            .is_error
    );
}

// case: stay on, with plugins, hooks and notify still off, and a clash with the call’s own name still refused
#[test]
fn enables_only_addressed_remote_connectors() {
    let settings = json!({"mcp_servers":{
        "robinhood":{"url":"https://connector.example.test/mcp"},
        "my.service":{"url":"https://example.test/mcp"},
        "node_repl":{"command":"node_repl.exe"}
    }});
    let config = thread_config(&settings, None, true).unwrap();
    assert_eq!(
        config,
        json!({
            "features.plugins":false, "features.hooks":false, "notify":[],
            "features.shell_tool":false, "features.browser_use":false, "features.browser_use_external":false,
            "features.in_app_browser":false,"features.computer_use":false,"model_reasoning_effort":"low","mcp_servers.node_repl.enabled":false
        })
    );
    assert!(
        thread_config(
            &json!({"mcp_servers":{"my.tool":{"command":"tool.exe"}}}),
            None,
            true
        )
        .is_err()
    );
    let addon = AddOn {
        command: "native".into(),
        args: Vec::new(),
        env: Default::default(),
    };
    assert!(
        thread_config(
            &json!({"mcp_servers":{"locust_call":{}}}),
            Some(&addon),
            true
        )
        .is_err()
    );
}

// case: leave Locust's own call tools unasked, and every other server asking as before (0.747)
#[test]
fn only_the_private_tools_get_automatic_approval() {
    let addon = AddOn {
        command: "native".into(),
        args: Vec::new(),
        env: Default::default(),
    };
    let config = thread_config(
        &json!({"mcp_servers":{"remote":{"url":"https://connector.example.test"}}}),
        Some(&addon),
        true,
    )
    .unwrap();
    assert_eq!(
        config["mcp_servers.locust_call.default_tools_approval_mode"],
        "approve"
    );
    let modes: Vec<_> = config
        .as_object()
        .unwrap()
        .keys()
        .filter(|key| key.ends_with("approval_mode"))
        .collect();
    assert_eq!(
        modes,
        ["mcp_servers.locust_call.default_tools_approval_mode"]
    );
    assert_eq!(config["features.shell_tool"], false);
    assert_eq!(config["features.computer_use"], false);
}

// case: are told to the voice only when they are the call’s, and the folder is read with tools, never commands
#[test]
fn names_connectors_only_when_the_call_has_them() {
    let told = instructions("acme", true, true, None);
    assert!(told.contains("find_files, read_file and search_files; never run shell commands"));
    assert!(told.contains("only say it is done once the tool says so"));
    assert!(!instructions("acme", true, false, None).contains("connectors"));
}

// case: lists the hand-off tools and the folder’s, each that only reads marked so, so Codex asks nothing for them
#[test]
fn call_bridge_exposes_only_the_scoped_tools() {
    let list = crate::door::voice_tool_list();
    let tools = list.as_array().unwrap();
    let mut names: Vec<_> = tools
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    names.sort_unstable();
    assert_eq!(names, crate::voice_call_rules::TOOLS);
    let mut read_only: Vec<_> = tools
        .iter()
        .filter(|tool| tool["annotations"]["readOnlyHint"] == true)
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    read_only.sort_unstable();
    assert_eq!(
        read_only,
        [
            "find_files",
            "list_background_runs",
            "list_teammates",
            "read_file",
            "read_reply",
            "search_files"
        ]
    );
}

// case: leaves another app’s list as it was: no folder tools there
#[test]
fn the_regular_bridge_does_not_gain_the_calls_tools() {
    let list = crate::door::tool_list();
    let tools = list.as_array().unwrap();
    assert!(!tools.iter().any(|tool| tool["name"] == "read_file"));
    assert!(tools.iter().any(|tool| tool["name"] == "run_routine"));
}
