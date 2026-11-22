//! apps/desktop/src/main/an-opencode-2-mode-has-native-permission-rules.test.ts (6 cases). Fixed 20 Nov: V2's config
//! was written once at spawn from the mode alone, so everything a run adds afterwards in V1's shape (a reference
//! folder, a worktree's parent `.git`, a chat-only model of your own) never reached V2: it read no reference, no
//! worktree git, and a chat-only model could call tools. Now V2's is written last, from the run's whole V1 config.
use super::*;
use std::process::Command;

struct Run {
    _root: Scratch,
    plugin: PathBuf,
    command: Command,
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn env(c: &Command, key: &str) -> Option<String> {
    c.get_envs().find(|(k, _)| *k == key).and_then(|(_, v)| v.map(|v| v.to_string_lossy().into_owned()))
}

/// An OpenCode 2 command for `mode`, given what the run adds in V1's shape, then finished as spawn finishes it.
fn launch(mode: Mode, add: impl FnOnce(Value) -> Value) -> Run {
    let root = std::env::temp_dir().join(format!("locust-oc2-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    let store = Store::with_codex(root.join("native"), root.join("electron"), vec![]).unwrap();
    let mut command = Command::new("opencode");
    let plugin = configure_with_seal(&mut command, mode, &store, true, false).unwrap();
    rewrite_config(&mut command, add);
    finish_v2(&mut command);
    Run { _root: Scratch(root), plugin, command }
}

fn config(run: &Run) -> Value {
    serde_json::from_str(&env(&run.command, "OPENCODE_CONFIG_CONTENT").unwrap()).unwrap()
}

fn rules(run: &Run) -> Vec<Value> {
    config(run)["agents"]["locust"]["permissions"].as_array().unwrap().clone()
}

fn rule(action: &str, resource: &str, effect: &str) -> Value {
    json!({"action": action, "resource": resource, "effect": effect})
}

// case: uses an owned ACP mode and a directory plugin, without unsupported V1 flags
#[test]
fn uses_an_owned_mode_and_a_directory_plugin_without_v1_flags() {
    let run = launch(Mode::Ask, |c| c);
    let found = config(&run);
    assert_eq!(found["default_agent"], "locust");
    assert_eq!(found["agents"]["locust"]["mode"], "primary");
    assert_eq!(found["plugins"], json!([file_url(&run.plugin)]));
    for v1 in ["permission", "plugin", "experimental", V2_PENDING] {
        assert!(found.get(v1).is_none(), "{v1} is V1's, or Locust's own");
    }
    assert_eq!(found["policies"], json!([rule("integration.use", "plugin:*", "deny"), rule("integration.use", &format!("plugin:{}", run.plugin.display()), "allow")]));
    assert_eq!(env(&run.command, "OPENCODE_CONFIG_PROJECT_DISABLE").as_deref(), Some("1"));
    assert_eq!(env(&run.command, "OPENCODE_DISABLE_CLAUDE_CODE").as_deref(), Some("1"));
    assert_eq!(env(&run.command, "OPENCODE_CONFIG_DIR"), Some(run.plugin.join("config").to_string_lossy().into_owned()));
    assert_eq!(env(&launch(Mode::AcceptEdits, |c| c).command, "OPENCODE_CONFIG_PROJECT_DISABLE"), None);
}

// case: denies edits and subagents in Ask, and lets no shell command through
#[test]
fn denies_edits_and_subagents_in_ask_and_lets_no_shell_command_through() {
    let rules = rules(&launch(Mode::Ask, |c| c));
    for wanted in [rule("edit", "*", "deny"), rule("subagent", "*", "deny"), rule("external_directory", "*", "deny")] {
        assert!(rules.contains(&wanted), "{wanted}");
    }
    let shell: Vec<&Value> = rules.iter().filter(|r| r["action"] == "shell").collect();
    assert_eq!(shell, vec![&rule("shell", "*", "ask")], "one rule, no allows after it");
    assert!(!rules.iter().any(|r| r["action"] == "bash"));
}

// case: keeps its own catch-all, never V1's (0.821)
#[test]
fn keeps_its_own_catch_all_never_v1s() {
    for mode in [Mode::Ask, Mode::ApproveEach, Mode::AcceptEdits, Mode::Auto] {
        let rules = rules(&launch(mode, |c| c));
        assert_eq!(rules.iter().filter(|r| r["action"] == "*" && r["resource"] == "*").count(), 1, "{mode:?}");
        assert_eq!(rules[0]["action"], "*");
    }
    assert_eq!(rules(&launch(Mode::Ask, |c| c))[0], rule("*", "*", "allow"));
}

// case: keeps Approve each asking and makes Auto explicit without silently widening Accept edits
#[test]
fn keeps_approve_each_asking_and_makes_auto_explicit() {
    let asking = rules(&launch(Mode::ApproveEach, |c| c));
    assert_eq!(asking[0], rule("*", "*", "ask"));
    assert!(asking.contains(&rule("edit", "*", "ask")) && asking.contains(&rule("shell", "*", "ask")));
    assert!(rules(&launch(Mode::AcceptEdits, |c| c)).contains(&rule("external_directory", "*", "deny")));
    let auto = launch(Mode::Auto, |c| c);
    assert!(rules(&auto).contains(&rule("external_directory", "*", "allow")));
    assert_eq!(rules(&auto)[0], rule("*", "*", "allow"));
    assert!(config(&auto).get("policies").is_none());
}

// case: grants references and worktree git metadata after the default denial, without granting the shared root
#[test]
fn grants_references_and_worktree_git_after_the_default_denial() {
    let run = launch(Mode::Ask, |c| with_parent_git(with_references(c, Mode::Ask, &["/reference"]), Mode::Ask, "/shared/repo"));
    let paths: Vec<Value> = rules(&run).into_iter().filter(|r| r["action"] == "external_directory").collect();
    assert_eq!(paths[0], rule("external_directory", "*", "deny"));
    assert!(paths.contains(&rule("external_directory", "/shared/repo/.git/*", "allow")), "{paths:?}");
    assert!(paths.contains(&rule("external_directory", "/reference/**", "allow")), "{paths:?}");
    assert!(!paths.contains(&rule("external_directory", "/shared/repo/*", "allow")));
}

// case: does not enable a chat-only own model to call tools and keeps its provider declaration per-run
#[test]
fn a_chat_only_model_of_your_own_calls_no_tool_and_its_provider_stays_per_run() {
    let local = crate::own_models::Provider { name: "Local".into(), base_url: "http://localhost:1234/v1".into(), models: vec!["model".into()], api_key: None, tool_calls: false, prices: None };
    let run = launch(Mode::Auto, |c| crate::own_models::with_opencode_providers(c, &[("own-local".into(), local)]).unwrap());
    let found = config(&run);
    assert!(found["provider"]["own-local"].is_object());
    assert_eq!(rules(&run).last(), Some(&rule("*", "*", "deny")));
    assert!(found.get("tools").is_none(), "V1's tools switch is not V2's");
}

#[test]
fn the_runs_own_skills_folder_reaches_v2() {
    let run = launch(Mode::AcceptEdits, |c| with_skills(c, Some("/run/skills"), &["/mine".into()]));
    assert_eq!(config(&run)["skills"], json!(["/run/skills"]));
}

#[test]
fn spawn_is_what_writes_v2s_config() {
    let root = std::env::temp_dir().join(format!("locust-oc2s-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    let _scratch = Scratch(root.clone());
    let store = Store::with_codex(root.join("native"), root.join("electron"), vec![]).unwrap();
    let mut command = Command::new("opencode");
    configure_with_seal(&mut command, Mode::Ask, &store, true, false).unwrap();
    command.env("LOCUST_NATIVE_OPENCODE_PROTOCOL", "acp");
    let pending: Value = serde_json::from_str(&env(&command, "OPENCODE_CONFIG_CONTENT").unwrap()).unwrap();
    assert!(pending.get(V2_PENDING).is_some(), "still V1's shape until spawn");
    let _runtime = super::super::opencode::OpenCode::spawn(&mut command).unwrap();
    let written: Value = serde_json::from_str(&env(&command, "OPENCODE_CONFIG_CONTENT").unwrap()).unwrap();
    assert_eq!(written["default_agent"], "locust");
    assert!(written.get(V2_PENDING).is_none() && written.get("permission").is_none());
}
