// Carries packages/runtime-adapters/test/a-sealed-run-uses-no-tool.test.ts (Electron 4ce2f37a): a sealed OpenCode run
// reads only what its prompt gives it. Rust gives an OpenCode run no git tool, browser, skills or instructions in its
// config at all, so what is checked is the permission, the pure flag and that a run that may edit is never sealed.
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

#[test]
fn a_sealed_run_asks_for_every_tool_and_reaches_no_folder_outside_its_own() {
    let (pure, config) = env_of(Mode::Ask, true);
    assert_eq!(config["permission"], json!({ "*": "ask", "external_directory": "deny" }));
    for key in ["mcp", "skills", "instructions"] {
        assert!(config.get(key).is_none(), "{key}: {config}");
    }
    assert_eq!(pure.as_deref(), Some("1"));
}

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
