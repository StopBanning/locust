//! what-locust-can-stop-is-what-it-asks.test.ts, held to this engine: each row's reach to the modes `mode_facts`
//! offers, and to the code that builds the run and raises a card here.
use super::*;
use crate::approval::Mode;
use serde_json::json;

const RUNTIMES: [&str; 11] = ["codex", "claude", "cursor", "gemini", "opencode", "copilot", "antigravity", "muse", "kilo", "acp", "apple"];
const MODES: [&str; 5] = ["ask", "plan", "accept-edits", "approve-each", "auto"];

fn runnable(runtime: &str) -> bool {
    MODES.iter().any(|m| crate::mode_facts::runs_on(m, runtime))
}

fn the(runtime: &str) -> &'static StopRow {
    row(runtime).unwrap_or_else(|| panic!("no row for {runtime}"))
}

#[test]
fn lists_each_runtime_with_a_mode_once_and_none_without_one() {
    let mut runnable: Vec<&str> = RUNTIMES.iter().copied().filter(|r| runnable(r)).collect();
    assert!(!runnable.contains(&"gemini"));
    let mut listed: Vec<&str> = WHAT_LOCUST_CAN_STOP.iter().map(|r| r.runtime).collect();
    runnable.sort();
    listed.sort();
    assert_eq!(listed, runnable);
}

#[test]
fn says_each_action_exactly_where_approve_each_action_is_offered_and_locust_sets_what_is_asked() {
    for entry in WHAT_LOCUST_CAN_STOP {
        let offered = crate::mode_facts::runs_on("approve-each", entry.runtime);
        // An added ACP agent decides for itself which actions it asks about.
        assert_eq!(entry.reach == StopReach::EachAction, offered && entry.runtime != "acp", "{}", entry.runtime);
        assert_eq!(entry.asks.contains("Approve each action"), offered, "{}", entry.runtime);
    }
    let mut each: Vec<&str> = RUNTIMES.iter().copied().filter(|r| crate::mode_facts::runs_on("approve-each", r)).collect();
    each.sort();
    assert_eq!(each, ["acp", "claude", "codex", "copilot", "opencode"]);
    assert_eq!(the("acp").reach, StopReach::SomeActions);
    assert!(the("acp").asks.contains("agent\u{2019}s own setting"));
}

#[test]
fn codex_asks_in_approve_each_action_and_never_in_the_other_modes() {
    assert_eq!(crate::approval::codex_app_server_policy(Mode::ApproveEach, None).approval_policy, "untrusted");
    for mode in [Mode::Ask, Mode::Plan, Mode::AcceptEdits, Mode::Auto] {
        assert_eq!(crate::approval::codex_app_server_policy(mode, None).approval_policy, "never", "{mode:?}");
    }
    // Ask and Plan start none of the MCP programs, plugins, browser or computer use the row names.
    let config = crate::runtime::codex::read_only_config(&json!({"mcp_servers": {"local": {"command": "node"}}})).unwrap();
    for key in ["features.plugins", "features.browser_use", "features.computer_use", "mcp_servers.local.enabled"] {
        assert_eq!(config[key], json!(false), "{key}");
    }
    assert!(the("codex").always.is_some_and(|a| a.starts_with("Locust keeps an Always")));
}

// case: Claude Code: asks in every mode but Auto, and has a shell only in Edit
#[test]
fn claude_code_asks_in_every_mode_but_auto_and_has_a_shell_only_in_edit() {
    let args = |mode: Mode| crate::native::claude_args(mode, Some("bridge.json"), None, "guard.json", None, None, None, &[], &[]).unwrap();
    let tools = |mode: Mode| {
        let a = args(mode);
        a[a.iter().position(|x| x == "--tools").unwrap() + 1].split(',').map(str::to_owned).collect::<Vec<_>>()
    };
    for mode in [Mode::Ask, Mode::Plan, Mode::AcceptEdits, Mode::ApproveEach] {
        assert!(args(mode).contains(&"--permission-prompt-tool".into()), "{mode:?}");
    }
    assert!(!args(Mode::Auto).contains(&"--permission-prompt-tool".into()));
    assert!(!tools(Mode::Ask).contains(&"Bash".into()));
    assert!(tools(Mode::AcceptEdits).contains(&"Bash".into()));
    let each = args(Mode::ApproveEach);
    assert_eq!(each[each.iter().position(|x| x == "--permission-mode").unwrap() + 1], "default");
    // No account connectors are loaded, which is why the row says none is called.
    assert!(args(Mode::AcceptEdits).contains(&"--strict-mcp-config".into()));
    assert!(the("claude").without.contains("connectors are not loaded here"));
    // The command guard rides in every mode, Auto included.
    assert!(args(Mode::Auto).contains(&"--settings".into()));
    assert_eq!(the("claude").reach, StopReach::EachAction);
}

#[test]
fn opencode_in_approve_each_action_its_server_asks_about_commands_changes_web_pages_outside_paths_and_the_persons_own_mcp_tools_not_searches() {
    let each = crate::runtime::opencode_config::legacy_config(Mode::ApproveEach, None)["permission"].clone();
    for key in ["bash", "edit", "webfetch", "external_directory", "*"] {
        assert_eq!(each[key], json!("ask"), "{key}");
    }
    assert_eq!([&each["websearch"], &each["codesearch"]], [&json!("allow"), &json!("allow")]);
    assert!(the("opencode").asks.contains("your own MCP servers"));
    assert!(the("opencode").without.starts_with("Web searches, in every mode."));
    // In Ask and Plan the catch-all asks too, and outside Approve each an ask is refused: none of the person's own
    // MCP tools run there, nor any write.
    let ask = crate::runtime::opencode_config::legacy_config(Mode::Ask, None)["permission"].clone();
    assert_eq!(ask["*"], json!("ask"));
    assert_eq!(ask["edit"], json!("deny"));
    assert!(the("opencode").without.contains("In Ask and Plan, no file changes, no commands and none of your own MCP tools"));
}

#[test]
fn copilot_asks_over_its_own_acp_in_approve_each_action_and_its_mcp_servers_are_turned_off_in_ask() {
    let folder = std::env::temp_dir();
    let each = crate::runtime::copilot::command(&folder, "hi", Mode::ApproveEach, None, None, None, None, &[], None);
    if let Ok(c) = each {
        assert!(c.get_args().any(|a| a == "--acp"));
    }
    let ask = crate::runtime::copilot::prompt_args("hi", Mode::Ask, None, None, None, None, &[], None);
    assert!(ask.contains(&"--deny-tool=write,shell".into()));
    let home_dir = std::env::temp_dir().join(format!("locust-copilot-mcp-{}", std::process::id()));
    std::fs::create_dir_all(home_dir.join(".copilot")).unwrap();
    struct Home(std::path::PathBuf);
    impl Home { fn path(&self) -> &std::path::Path { &self.0 } }
    impl Drop for Home { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
    let home = Home(home_dir);
    std::fs::write(home.path().join(".copilot/mcp-config.json"), r#"{"mcpServers":{"notes":{},"github":{}}}"#).unwrap();
    let names = crate::runtime::copilot::mcp_server_names(home.path());
    assert_eq!(names, ["notes", "github"]);
    for mode in [Mode::Ask, Mode::Plan] {
        assert_eq!(crate::runtime::copilot::mcp_off_args(mode, &names), ["--disable-mcp-server", "notes", "--disable-mcp-server", "github"]);
    }
    for mode in [Mode::AcceptEdits, Mode::Auto, Mode::ApproveEach] {
        assert!(crate::runtime::copilot::mcp_off_args(mode, &names).is_empty(), "{mode:?}");
    }
    // No settings file, or one that does not read as settings: nothing to turn off.
    assert!(crate::runtime::copilot::mcp_server_names(&home.path().join("nowhere")).is_empty());
    std::fs::write(home.path().join(".copilot/mcp-config.json"), "not json").unwrap();
    assert!(crate::runtime::copilot::mcp_server_names(home.path()).is_empty());
    assert!(the("copilot").without.contains("none of the MCP servers in your Copilot settings"));
    assert!(!the("copilot").without.contains("still start"));
    // The print run asks for them (the ACP run is Approve each action, where nothing is turned off).
    assert!(include_str!("runtime/copilot.rs").contains("c.args(mcp_off_args(mode, &mcp_server_names(Path::new(&home))));"));
}

#[test]
fn antigravity_only_its_questions_reach_a_card() {
    let folder = std::env::temp_dir();
    assert!(crate::runtime::antigravity::command(&folder, Mode::ApproveEach, None, None, None, &[], None).is_err());
    assert_eq!(the("antigravity").reach, StopReach::QuestionsOnly);
}

#[test]
fn cursor_and_muse_no_path_raises_a_card_for_them() {
    let folder = std::env::temp_dir();
    // Muse is told never to ask; Cursor's print mode has no permission tool at all.
    let muse = crate::runtime::muse::exec_args(&folder, Mode::AcceptEdits, None, None, None).unwrap();
    assert!(muse.windows(2).any(|w| w == ["--approval-mode", "never"]));
    for mode in [Mode::Ask, Mode::AcceptEdits, Mode::Auto] {
        if let Ok(cursor) = crate::runtime::cursor::print_args(&folder, mode, None, None, None) {
            assert!(!cursor.iter().any(|a| a.contains("permission")), "{mode:?}: {cursor:?}");
        }
    }
    for runtime in ["cursor", "muse"] {
        assert_eq!(the(runtime).reach, StopReach::Nothing, "{runtime}");
    }
}

#[test]
fn an_always_is_kept_by_locust_exactly_where_the_request_carries_a_key_for_it() {
    let open = crate::approval::open_code_permission_request(&json!({"permission": "bash", "patterns": ["echo hi"], "always": ["echo *"], "metadata": {}}), "/work");
    assert!(open["params"]["locustAlwaysKey"].is_string(), "{open}");
    let (_, acp) = crate::approval::acp_permission_request(&json!({"toolCall": {"toolCallId": "t", "title": "x", "kind": "execute", "rawInput": {"command": "npm test"}}, "options": []}), "/work");
    assert!(acp["locustAlwaysKey"].is_string(), "{acp}");
    for runtime in ["codex", "claude", "opencode", "copilot", "acp"] {
        assert!(the(runtime).always.is_some_and(|a| a.starts_with("Locust keeps an Always")), "{runtime}");
    }
    for runtime in ["cursor", "antigravity", "muse", "kilo", "apple"] {
        assert!(the(runtime).always.is_none(), "{runtime}");
    }
}

#[test]
fn kilo_runs_without_asking_locust() {
    // Kilo's ACP profile takes no permission requests from the host, so no card can stop it.
    let run = include_str!("runtime/kilo.rs");
    assert!(run.contains("host_permissions: false"));
    assert_eq!(the("kilo").reach, StopReach::Nothing);
    assert!(the("kilo").always.is_none());
}

/// Electron's "the document is the list": the page beside this app is written from this table, so it cannot say what
/// this engine does not do. `LOCUST_WRITE_DOCS=1` rewrites it (vitest's `-u`).
#[test]
fn docs_what_locust_can_stop_md_is_written_from_it_rewrite_with_u() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(WHAT_LOCUST_CAN_STOP_FILE);
    let written = what_locust_can_stop_document();
    if std::env::var_os("LOCUST_WRITE_DOCS").is_some() {
        std::fs::write(&path, &written).expect("the document is writable");
    }
    let kept = std::fs::read_to_string(&path).unwrap_or_default();
    assert_eq!(kept, written, "{} is not what the table says: rewrite it with LOCUST_WRITE_DOCS=1", path.display());
    // Every agent in the table has its section.
    for row in WHAT_LOCUST_CAN_STOP {
        assert!(written.contains(&format!("\n## {}\n", super::document_name(row.runtime))), "{}", row.runtime);
    }
}
