//! apps/desktop/src/main/ask-starts-none-of-your-codex-programs.test.ts (8 cases). Fixed 20 Nov: Rust refused in its
//! own words ("Could not read Codex settings; refusing...") and said a Codex too old for `config/read` was Codex's raw
//! RPC error, so a person was never told that updating Codex is what helps.
use super::*;
use serde_json::json;

fn off() -> Value {
    json!({"features.plugins": false, "features.browser_use": false, "features.browser_use_external": false,
        "features.in_app_browser": false, "features.computer_use": false})
}

fn with(mut config: Value, extra: Value) -> Value {
    for (k, v) in extra.as_object().unwrap() {
        config[k] = v.clone();
    }
    config
}

fn refusal(settings: &Value, failure: Option<&str>, sealed: bool) -> String {
    thread_config(settings, failure, sealed).unwrap_err().to_string()
}

// case: switches off each server that runs as a program, by name, every plugin, and its own browser and computer use
#[test]
fn switches_off_each_program_server_by_name() {
    let made = read_only_config(&json!({"model": "gpt-6-luna", "mcp_servers": {
        "node_repl": {"command": "node", "args": ["repl.mjs"]}, "notes": {"command": "notes-server", "enabled": true},
        "docs": {"url": "https://docs.example.com/mcp"}}}))
    .unwrap();
    assert_eq!(made, with(off(), json!({"mcp_servers.node_repl.enabled": false, "mcp_servers.notes.enabled": false})));
}

// case: leaves a server reached at an address as the person has it: Codex still refuses its tools that do more than read
#[test]
fn leaves_an_address_server_as_it_is() {
    assert_eq!(read_only_config(&json!({"mcp_servers": {"gmail": {"url": "https://mail.example.com/mcp"}}})).unwrap(), off());
}

// case: switches off a server that names both a program and an address, and one it cannot read the shape of
#[test]
fn switches_off_both_and_an_odd_one() {
    let made = read_only_config(&json!({"mcp_servers": {"both": {"url": "https://x.example.com", "command": "x"}, "odd": "not a table"}})).unwrap();
    assert_eq!(made, with(off(), json!({"mcp_servers.both.enabled": false, "mcp_servers.odd.enabled": false})));
}

// case: with no servers of the person’s own, still takes every plugin’s, and the browser and computer use
#[test]
fn with_no_servers_still_takes_plugins_browser_and_computer_use() {
    assert_eq!(read_only_config(&json!({})).unwrap(), off());
    assert_eq!(read_only_config(&json!({"mcp_servers": null})).unwrap(), off());
}

// case: starts nothing, and says why, rather than start with something left on
#[test]
fn starts_nothing_and_says_why() {
    for settings in [Value::Null, json!("settings"), json!([]), json!({"mcp_servers": "x"}), json!({"mcp_servers": []})] {
        assert_eq!(refusal(&settings, None, false), SETTINGS_UNREAD, "{settings}");
    }
    assert!(SETTINGS_UNREAD.contains("could not read your Codex settings, so it did not start this turn"));
    let dotted = refusal(&json!({"mcp_servers": {"a.b": {"command": "x"}}}), None, false);
    assert_eq!(dotted, "Locust could not switch off your Codex MCP server \"a.b\" for this turn, so it did not start it. In Ask and Plan it starts none of the servers your Codex runs as programs on this computer.");
    let long = format!("{}.x", "n".repeat(100));
    assert!(refusal(&json!({"mcp_servers": {long.clone(): {"command": "x"}}}), None, false).contains(&format!("\"{}\"", &long[..60])), "the name is said to 60 characters");
}

// case: tells a person whose Codex is too old for the read to update it, and anyone else to send again
#[test]
fn a_codex_too_old_is_told_to_update_and_anyone_else_to_send_again() {
    let old = "Invalid request: unknown variant `config/read`, expected one of `initialize`, `thread/start`";
    for sealed in [false, true] {
        let said = refusal(&Value::Null, Some(old), sealed);
        assert!(said.starts_with("Your Codex is too old to tell Locust its settings") && said.ends_with("Update Codex, then send it again."), "{said}");
    }
    let other = refusal(&Value::Null, Some("The runtime connection ended."), false);
    assert!(other.contains("could not read your Codex settings") && other.ends_with("Send it again."));
    assert!(thread_config(&json!({}), Some(old), false).is_ok(), "settings that did come back are used, whatever else was said");
}

fn ask_turn(scenario: &str) -> io::Result<()> {
    let folder = std::env::temp_dir();
    let mut command = Command::new("python3");
    command.arg(concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/fixtures/m1-app-server.py")).arg(scenario);
    let mut runtime = Codex::spawn(&mut command).unwrap();
    runtime.start(&Turn {
        prompt: "Synthetic prompt".into(),
        folder,
        model: Some("synthetic-model".into()),
        effort: None,
        resume: None,
        mode: crate::approval::Mode::Ask,
        approvals: None,
        mission_id: "synthetic".into(),
        run_id: "run_synthetic".into(),
    })
}

#[test]
fn an_ask_turn_on_a_codex_that_refuses_the_read_says_which_refusal_it_was() {
    assert_eq!(ask_turn("config-too-old").unwrap_err().to_string(), CODEX_TOO_OLD);
    assert_eq!(ask_turn("config-refused").unwrap_err().to_string(), SETTINGS_UNREAD);
}

// case: adds no shell, no web search and no ChatGPT apps, and switches off every server, connectors too
#[test]
fn a_sealed_run_adds_no_tool_and_switches_off_every_server() {
    let made = sealed_config(&json!({"mcp_servers": {"node_repl": {"command": "node"}, "gmail": {"url": "https://mail.example.com/mcp"}}})).unwrap();
    assert_eq!(made, with(off(), json!({"features.apps": false, "features.shell_tool": false, "web_search": "disabled",
        "mcp_servers.node_repl.enabled": false, "mcp_servers.gmail.enabled": false})));
}

// case: refuses as Ask does when it cannot read the settings
#[test]
fn a_sealed_run_refuses_as_ask_does() {
    assert_eq!(refusal(&Value::Null, None, true), SETTINGS_UNREAD);
}
