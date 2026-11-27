//! Electron's `main/the-host-refuses-always-for-a-reaching-command.test.ts` (2 cases). Electron checks that its two IPC
//! handlers call the shared decision; here the same two paths are driven: an answer from the window goes through
//! `Channel::decide`, which enforces the decision before the runtime is answered, and a rule from a card goes through
//! `save_rule_and_decide`, which refuses before any rule is built or saved.
use super::*;
use crate::store::Store;
use serde_json::json;
use std::time::{Duration, Instant};

const COMMAND: &str = "item/commandExecution/requestApproval";
/// A command that reaches other programs (it ends one by name).
const REACHING: &str = "taskkill /IM python.exe";

struct Host {
    root: std::path::PathBuf,
    store: Store,
    _hook: crate::native::hooks::Guard,
}
impl Drop for Host {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn host() -> Host {
    let root = std::env::temp_dir().join(format!("locust-reach-always-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("electron")).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    let hook = crate::native::hooks::install(store.clone(), None);
    Host { root, store, _hook: hook }
}

/// Ask on `channel` for `command`, on a thread of its own, and wait for its card.
fn ask(channel: &std::sync::Arc<Channel>, mission: &str, command: &str) -> (std::thread::JoinHandle<Result<Value, String>>, Request) {
    let (c, m, params) = (channel.clone(), mission.to_owned(), json!({"command": command, "cwd": "C:\\work"}));
    let waiting = std::thread::spawn(move || c.ask_codex_with_changes(&format!("run_{m}"), &m, "codex", COMMAND, &params, json!(1), None, &|| false));
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    loop {
        if let Some(card) = channel.waiting_for_mission(mission).into_iter().next() {
            return (waiting, card);
        }
        assert!(Instant::now() < deadline, "no card was raised");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn always(channel: &Channel, card: &Request) -> Result<Recorded, String> {
    channel.decide(Answer::Decision { approval_id: card.approval_id.clone(), decision: Decision::ApproveAlways, reason: None })
}

// case: passes every answer from the window through enforcedAnswer before it reaches a holder
#[test]
fn an_always_from_the_window_for_a_reaching_command_reaches_the_runtime_as_once() {
    let _host = host();
    let channel = Channel::new();
    let (waiting, card) = ask(&channel, "mission_reach", REACHING);
    always(&channel, &card).unwrap();
    assert_eq!(waiting.join().unwrap(), Ok(json!({"decision": "accept"})));
}

// case: refuses a rule from a card before any rule is built or saved
#[test]
fn a_rule_from_a_card_for_a_reaching_command_is_refused_and_nothing_is_saved() {
    let host = host();
    let channel = crate::native::approvals_channel();
    let mission = format!("mission_rule_{}", crate::invariants::unique_nanos());
    channel.register_teammate(&mission, "tm_wren");
    let (waiting, card) = ask(&channel, &mission, REACHING);
    let refused = save_rule_and_decide(&card, crate::approval_rules::RuleEffect::Allow).unwrap_err();
    assert!(refused.starts_with("Not saved as a rule: this command "), "{refused}");
    assert!(refused.ends_with(" Locust asks each time."), "{refused}");
    assert!(crate::approval_rule_store::list(&host.store).unwrap_or_default().is_empty());
    // The card still waits for the person's own answer.
    assert_eq!(channel.waiting_for_mission(&mission).len(), 1);
    channel.decide(Answer::Decision { approval_id: card.approval_id.clone(), decision: Decision::Deny, reason: None }).unwrap();
    assert_eq!(waiting.join().unwrap(), Ok(json!({"decision": "decline"})));
}
