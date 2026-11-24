// Carries packages/runtime-adapters/test/a-sealed-run-uses-no-tool.test.ts (Electron 4ce2f37a): a sealed OpenCode run
// reads only what its prompt gives it: the sealed permission, the pure flag, no git tool, and that a run that may edit
// is never sealed, on the served command a turn starts and on what native.rs adds to it for one run (`for_run`).
use super::opencode_config::{configure_with_seal, legacy_config, legacy_sealed_config};
use crate::{approval::Mode, store::Store};
use serde_json::{Value, json};
use std::{path::PathBuf, process::Command};

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// What an OpenCode command was given in its environment, by name.
fn env_of(mode: Mode, sealed: bool) -> (Option<String>, Value) {
    let temp = Temp(std::env::temp_dir().join(format!("locust-sealed-{}-{}", std::process::id(), crate::invariants::unique_nanos())));
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    let mut command = Command::new("opencode");
    configure_with_seal(&mut command, mode, &store, false, sealed).unwrap();
    let said = |key: &str| {
        command.get_envs().find(|(k, _)| *k == key).and_then(|(_, v)| v).map(|v| v.to_string_lossy().into_owned())
    };
    (said("OPENCODE_PURE"), serde_json::from_str(&said("OPENCODE_CONFIG_CONTENT").unwrap()).unwrap())
}

// case: asks for every tool, so a run with nobody to ask refuses each one, and reaches no folder outside its own
#[test]
fn a_sealed_run_asks_for_every_tool_and_reaches_no_folder_outside_its_own() {
    let (pure, config) = env_of(Mode::Ask, true);
    assert_eq!(config["permission"], json!({ "*": "ask", "external_directory": "deny" }));
    for key in ["mcp", "skills", "instructions"] {
        assert!(config.get(key).is_none(), "{key}: {config}");
    }
    assert_eq!(pure.as_deref(), Some("1"));
}

// case: keeps every tool listed, the shell among them, because the free tier refuses a run whose tools are gone
#[test]
fn a_sealed_run_keeps_every_tool_listed_and_denies_only_other_folders() {
    let config = legacy_sealed_config(None);
    let permission = config["permission"].as_object().unwrap();
    assert_eq!(permission.values().filter(|rule| *rule == "deny").collect::<Vec<_>>(), [&json!("deny")]);
    assert_eq!(permission["external_directory"], "deny");
}

#[test]
fn only_a_read_only_run_is_sealed_and_an_ordinary_ask_keeps_its_reading_tools() {
    let ask = legacy_config(Mode::Ask, None);
    assert_eq!(ask["permission"]["webfetch"], "allow");
    let (_, edit) = env_of(Mode::AcceptEdits, true);
    assert_ne!(edit["permission"], json!({ "*": "ask", "external_directory": "deny" }));
}

// case: is what a served run is told, and seals only a read-only run
#[test]
fn a_served_run_is_sealed_only_when_read_only_and_an_ordinary_ask_keeps_its_tools() {
    let temp = Temp(std::env::temp_dir().join(format!("locust-sealed-served-{}-{}", std::process::id(), crate::invariants::unique_nanos())));
    let store = Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    std::fs::create_dir_all(temp.0.join("work")).unwrap();
    let folder = temp.0.join("work");
    let git_reads = vec!["locust".to_owned(), "--git-reads".to_owned()];
    // The served OpenCode 1 command a turn starts (command_for_turn), with what native.rs adds for one run after it.
    let served = |mode: Mode, sealed: bool| -> Value {
        let mut command = super::opencode::command_for_version(PathBuf::from("opencode"), "1.18.27", &folder, mode, &store, sealed).unwrap();
        let at = folder.to_string_lossy().into_owned();
        super::opencode_config::rewrite_config(&mut command, |config| {
            super::opencode_config::for_run(config, mode, sealed, None, Some(&git_reads), &at)
        });
        let text = command.get_envs().find(|(k, _)| *k == "OPENCODE_CONFIG_CONTENT").and_then(|(_, v)| v).unwrap();
        serde_json::from_str(&text.to_string_lossy()).unwrap()
    };
    let sealed = served(Mode::Ask, true);
    assert_eq!(sealed["permission"], json!({ "*": "ask", "external_directory": "deny" }));
    assert!(sealed.get("mcp").is_none(), "{sealed}");
    // An ordinary Ask run keeps its reading tools and Locust's git tool, as before.
    let ask = served(Mode::Ask, false);
    assert_eq!(ask["permission"]["webfetch"], "allow");
    assert!(ask["mcp"].get("locust_git").is_some(), "{ask}");
    // A mode that may edit is never sealed.
    let edit = served(Mode::AcceptEdits, true);
    assert_ne!(edit["permission"], json!({ "*": "ask", "external_directory": "deny" }));
}
