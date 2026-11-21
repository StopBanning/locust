//! packages/runtime-adapters/test/opencode-asks-with-the-question-card.test.ts (10 cases; 7 carried). OpenCode's own
//! `question` tool waits on a reply Locust never sends, so every run Locust starts has it off. Fixed 20 Nov: a
//! chat-only model of your own replaced `tools` with `{"*": false}`, dropping `question: false`. Not carried: the three
//! "on run" cases (Rust starts OpenCode 1 only as `serve`, never `opencode run`) and "starts a config when there was
//! none" (every Rust run starts from its mode's config, so there is never none).
use super::*;

fn tools(config: &Value) -> Value {
    config["tools"].clone()
}

// case: on the server, read-only
#[test]
fn on_the_server_read_only() {
    assert_eq!(tools(&legacy_config(Mode::Ask, None)), json!({"question": false}));
    assert_eq!(tools(&legacy_config(Mode::Plan, None)), json!({"question": false}));
}

// case: on the server, workspace-write
#[test]
fn on_the_server_workspace_write() {
    assert_eq!(tools(&legacy_config(Mode::AcceptEdits, None)), json!({"question": false}));
}

// case: on the server, full-access
#[test]
fn on_the_server_full_access() {
    assert_eq!(tools(&legacy_config(Mode::Auto, None)), json!({"question": false}));
}

// case: in Approve each, which asks for everything else
#[test]
fn in_approve_each() {
    assert_eq!(tools(&legacy_config(Mode::ApproveEach, None)), json!({"question": false}));
}

// case: in a worktree, beside the repository it may reach
#[test]
fn in_a_worktree_beside_the_repository() {
    let config = with_parent_git(legacy_config(Mode::AcceptEdits, None), Mode::AcceptEdits, "C:/repo");
    assert_eq!(tools(&config), json!({"question": false}));
    assert!(config["permission"].is_object());
}

// case: keeps a chat-only model's tools off, every one of them
#[test]
fn keeps_a_chat_only_models_tools_off_every_one() {
    let chat = crate::own_models::Provider { name: "Local".into(), base_url: "http://localhost:1234/v1".into(), models: vec!["m".into()], api_key: None, tool_calls: false };
    let config = crate::own_models::with_opencode_providers(legacy_config(Mode::Auto, None), &[("own-local".into(), chat)]).unwrap();
    assert_eq!(tools(&config), json!({"*": false, "question": false}));
}

#[test]
fn a_sealed_run_and_opencode_2_keep_the_question_off_too() {
    assert_eq!(tools(&legacy_config_with_seal(Mode::Ask, None, true))["question"], false);
    let v2 = acp_config(Mode::Auto, std::path::Path::new("/guard"));
    assert!(v2["agents"]["locust"]["permissions"].as_array().unwrap().contains(&json!({"action": "question", "resource": "*", "effect": "deny"})));
}
