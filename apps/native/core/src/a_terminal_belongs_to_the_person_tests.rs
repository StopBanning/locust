//! Electron `main/a-terminal-belongs-to-the-person-and-ends-with-the-app.test.ts`, the cases Rust carries so far.
//! Expected words and values are Electron's (`shared/organization-policy.ts`, `main/organization-policy.ts`,
//! `main/terminal-folder.ts`).

use super::*;
use crate::org_policy::{PolicyState, parse_policy, terminal_refusal};
use serde_json::json;
use std::cell::RefCell;

fn state(policy: serde_json::Value) -> PolicyState {
    PolicyState {
        path: "/etc/locust/policy.json".into(),
        present: true,
        policy: policy.as_object().cloned().unwrap_or_default(),
        error: None,
    }
}

// a terminal owned by the person

#[test]
fn an_organization_terminal_restriction_refuses_even_a_direct_host_request() {
    let parsed = parse_policy(&json!({"terminal": false}), &mut |_| {}).expect("a policy");
    let off = PolicyState { policy: parsed, ..state(json!({})) };
    let dir = std::env::temp_dir().join(format!("locust-terminal-refused-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(&dir).unwrap();
    let marker = dir.join("started");
    let script = format!("touch {}", marker.display());
    let refused = Terminal::open_unless(terminal_refusal(Some(&off)), &dir, Path::new("/bin/sh"), &["-c", &script]).err().expect("refused");
    assert!(refused.contains("Set by your organization"), "{refused}");
    // Nothing was spawned.
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert!(!marker.exists());
    let _ = std::fs::remove_dir_all(&dir);
    let wrong = parse_policy(&json!({"terminal": "false"}), &mut |_| {}).expect_err("refused");
    assert!(wrong.contains("terminal"), "{wrong}");
    assert_eq!(terminal_refusal(Some(&state(json!({"terminal": true})))), None);
    // The machine's own policy file, read where every terminal opens (check_policy), refuses the same way.
    let file = std::env::temp_dir().join(format!("locust-terminal-policy-{}.json", crate::invariants::unique_nanos()));
    std::fs::write(&file, r#"{"terminal": false}"#).unwrap();
    let refused = check_policy_at(&file.to_string_lossy()).expect_err("refused");
    std::fs::write(&file, r#"{"terminal": true}"#).unwrap();
    let allowed = check_policy_at(&file.to_string_lossy());
    let _ = std::fs::remove_file(&file);
    assert!(refused.contains("Set by your organization"), "{refused}");
    assert_eq!(allowed, Ok(()));
}

#[test]
fn an_unrestricted_terminal_cannot_bypass_other_organization_restrictions() {
    for policy in [json!({"highestMode": "ask"}), json!({"allowedAgents": []}), json!({"allowedModels": {}}), json!({"monthlyLimitUsd": 0})] {
        let refused = terminal_refusal(Some(&PolicyState { path: "/policy".into(), ..state(policy.clone()) })).unwrap_or_default();
        assert!(refused.contains("Set by your organization"), "{policy}: {refused}");
    }
    let broken = PolicyState { path: "/policy".into(), error: Some("Invalid organization policy".into()), ..state(json!({})) };
    assert_eq!(terminal_refusal(Some(&broken)).as_deref(), Some("Invalid organization policy"));
}

// a terminal's folder comes from Locust's own state

struct Options {
    mission: Option<String>,
    teammates: Vec<TerminalTeammate>,
    trees: Vec<(String, String)>,
    asked: RefCell<Vec<String>>,
}

fn options() -> Options {
    Options {
        mission: Some("ws_other".into()),
        teammates: vec![TerminalTeammate { teammate_id: "alice".into(), folder: None, worktree: true }],
        trees: vec![("alice".into(), "/conversation/.locust/worktrees/alice".into())],
        asked: RefCell::new(Vec::new()),
    }
}

fn folder(context: TerminalContext, o: &Options) -> Option<String> {
    terminal_folder(
        &context,
        &TerminalFolderFacts {
            current: &|| Some("/project".into()),
            mission: &|_| o.mission.clone(),
            folder_of: &|_| Some("/conversation".into()),
            owner_of: &|_| Some("alice".into()),
            teammates: &|| o.teammates.clone(),
            trees: &|root| {
                o.asked.borrow_mut().push(root.to_owned());
                o.trees.clone()
            },
        },
    )
}

fn mission(id: &str) -> TerminalContext {
    TerminalContext { mission_id: Some(id.into()), teammate_id: None }
}

#[test]
fn uses_the_current_project_before_a_first_message_and_a_conversations_own_worktree_afterwards() {
    let o = options();
    assert_eq!(folder(TerminalContext::default(), &o).as_deref(), Some("/project"));
    assert_eq!(folder(mission("mission_1"), &o).as_deref(), Some("/conversation/.locust/worktrees/alice"));
    assert_eq!(o.asked.borrow().last().map(String::as_str), Some("/conversation"));
    let no_worktree = Options { teammates: vec![TerminalTeammate { teammate_id: "alice".into(), folder: None, worktree: false }], ..options() };
    assert_eq!(folder(mission("mission_1"), &no_worktree).as_deref(), Some("/conversation"));
}

#[test]
fn refuses_unknown_conversations_teammates_and_worktrees_instead_of_guessing_a_folder() {
    assert_eq!(folder(mission("unknown"), &Options { mission: None, ..options() }), None);
    assert_eq!(folder(TerminalContext { mission_id: None, teammate_id: Some("unknown".into()) }, &options()), None);
    assert_eq!(folder(TerminalContext { mission_id: None, teammate_id: Some("alice".into()) }, &Options { trees: Vec::new(), ..options() }), None);
}

// Lane check: Electron `policyTerminalRefusal` refuses when `highestMode !== undefined && highestMode !== 'auto'`.
#[test]
fn a_highest_mode_of_auto_leaves_the_terminal_to_the_person_and_any_lower_one_refuses_it() {
    assert_eq!(terminal_refusal(Some(&state(json!({ "highestMode": "auto" })))), None);
    for lower in ["plan", "ask", "accept-edits", "approve-each"] {
        assert!(terminal_refusal(Some(&state(json!({ "highestMode": lower })))).is_some(), "{lower}");
    }
}
