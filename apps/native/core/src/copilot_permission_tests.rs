//! Electron's `main/a-copilot-permission-is-a-card.test.ts` (0.377): what Copilot asks over the Agent Client Protocol
//! becomes the card Codex's and OpenCode's requests do, and the person's answer goes back as the kind of option to
//! choose. The first request is Copilot 1.0.88's own, as Electron measured it on 2026-09-26.

use crate::approval::{acp_answer_for, acp_permission_request, Answer, Channel, Decision, Kind, Request};
use crate::store::Store;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn measured() -> Value {
    json!({
        "sessionId": "3abe5d9d-b422-4bf5-8c95-97d605d3f819",
        "toolCall": {
            "toolCallId": "call_utkWEwr3SiwPun0Iym7cESBP",
            "title": "Print the required value from the shell",
            "kind": "execute",
            "status": "pending",
            "rawInput": {"command": "echo ACP-OK-7", "commands": ["echo ACP-OK-7"]}
        },
        "options": [
            {"optionId": "allow_once", "kind": "allow_once", "name": "Allow once"},
            {"optionId": "allow_always", "kind": "allow_always", "name": "Always allow"},
            {"optionId": "reject_once", "kind": "reject_once", "name": "Deny"}
        ]
    })
}

fn with_call(call: Value) -> Value {
    let mut asked = measured();
    asked["toolCall"] = call;
    asked
}

struct Host {
    channel: Arc<Channel>,
    root: PathBuf,
    _hook: crate::native::hooks::Guard,
}
impl Drop for Host {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn host() -> Host {
    let root = std::env::temp_dir().join(format!("locust-copilot-permission-{}-{}", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("electron")).unwrap();
    let hook = crate::native::hooks::install(Store::new(root.join("native"), root.join("electron")).unwrap(), None);
    Host { channel: Channel::new(), root, _hook: hook }
}

/// Raise Copilot's request as a card, answer it with `decision`, and hand back the card and whether it was allowed.
fn raised(asked: Value, decision: Decision) -> (Request, bool) {
    let host = host();
    let channel = host.channel.clone();
    let asking = std::thread::spawn(move || channel.ask_acp_runtime("copilot", "run_1", "m_1", &asked, "C:\\work", &|| false));
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    let card = loop {
        if let Some(card) = host.channel.waiting().into_iter().next() {
            break card;
        }
        assert!(Instant::now() < deadline, "no card was raised");
        std::thread::sleep(Duration::from_millis(5));
    };
    host.channel.decide(Answer::Decision { approval_id: card.approval_id.clone(), decision, reason: None }).unwrap();
    (card, asking.join().unwrap().unwrap())
}

#[test]
fn is_a_command_card_naming_the_command_copilot_asked_to_run_from_copilot_and_the_answer_is_the_kind_of_option() {
    let (card, allowed) = raised(measured(), Decision::ApproveOnce);
    assert_eq!((card.runtime.as_str(), &card.kind, card.summary.as_str(), card.detail.as_str()), ("copilot", &Kind::Command, "Run a command", "echo ACP-OK-7"));
    assert!(allowed);
    let (_, denied) = raised(measured(), Decision::Deny);
    assert!(!denied);
}

#[test]
fn a_shell_that_only_names_its_folder_is_a_card_with_no_command_the_folder_is_not_what_would_run() {
    let asked = with_call(json!({"toolCallId": "t2b", "title": "shell", "kind": "other", "rawInput": {}, "locations": [{"path": "C:\\work\\scratch\\oc2-tool-rows-probe"}]}));
    let (card, _) = raised(asked, Decision::Deny);
    assert_eq!((&card.kind, card.detail.as_str()), (&Kind::Command, ""));
    assert!(!card.detail.contains("oc2-tool-rows-probe"));
}

#[test]
fn a_shell_call_that_did_not_say_its_command_is_a_card_that_says_so() {
    let asked = with_call(json!({"toolCallId": "t2", "title": "Tidy the build folder", "kind": "execute"}));
    let (card, _) = raised(asked, Decision::Deny);
    assert_eq!((&card.kind, card.summary.as_str(), card.detail.as_str()), (&Kind::Command, "Run a command it did not describe", ""));
}

#[test]
fn an_edit_shows_its_change_by_its_file_relative_to_the_folder() {
    let asked = with_call(json!({
        "toolCallId": "t3", "title": "Edit notes.txt", "kind": "edit",
        "locations": [{"path": "C:\\work\\notes.txt"}],
        "content": [{"type": "diff", "path": "C:\\work\\notes.txt", "oldText": "Status: draft\n", "newText": "Status: ready\n"}]
    }));
    let (card, _) = raised(asked, Decision::Deny);
    assert_eq!((&card.kind, card.summary.as_str(), card.detail.as_str()), (&Kind::FileChange, "Change 1 file", "notes.txt"));
    let patch = card.patch.expect("the card shows the change");
    assert_eq!((patch.added, patch.removed), (1, 1));
    assert!(patch.text.contains("+Status: ready"), "{}", patch.text);
    // Named from the folder, not the drive.
    assert!(!patch.text.contains("C:"), "{}", patch.text);
}

#[test]
fn anything_else_is_said_in_the_agents_own_words_with_what_it_names() {
    let outside = with_call(json!({"toolCallId": "t4", "title": "Read a file outside the folder", "kind": "read", "locations": [{"path": "C:\\other\\keys.txt"}]}));
    let (method, params) = acp_permission_request(&outside, "C:\\work");
    assert_eq!(method, "item/commandExecution/requestApproval");
    assert_eq!((params["command"].as_str(), params["itemId"].as_str()), (Some("Read a file outside the folder: C:\\other\\keys.txt"), Some("t4")));
    let (_, fetch) = acp_permission_request(&with_call(json!({"toolCallId": "t5", "kind": "fetch"})), "C:\\work");
    assert_eq!(fetch["command"], "Use fetch");
}

#[test]
fn reads_for_the_rest_of_the_run_as_always_and_anything_unclear_as_a_refusal() {
    assert_eq!(acp_answer_for(&json!({"decision": "accept"})), "allow_once");
    assert_eq!(acp_answer_for(&json!({"decision": "acceptForSession"})), "allow_always");
    assert_eq!(acp_answer_for(&json!({"decision": "reject"})), "reject_once");
    assert_eq!(acp_answer_for(&json!({"decision": "approve"})), "reject_once");
    assert_eq!(acp_answer_for(&Value::Null), "reject_once");
}

// case: a denial's reason goes on as the next thing the run is told -- ACP's answer has no room for it
#[test]
fn a_denials_reason_goes_on_as_the_next_thing_the_run_is_told_acps_answer_has_no_room_for_it() {
    let host = host();
    let channel = host.channel.clone();
    let asking = std::thread::spawn(move || channel.ask_acp_runtime("copilot", "6", "m_6", &measured(), "C:\\work", &|| false));
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    let card = loop {
        if let Some(card) = host.channel.waiting().into_iter().next() {
            break card;
        }
        assert!(Instant::now() < deadline, "no card was raised");
        std::thread::sleep(Duration::from_millis(5));
    };
    host.channel
        .decide(Answer::Decision { approval_id: card.approval_id.clone(), decision: Decision::Deny, reason: Some("Use npm test instead.".into()) })
        .unwrap();
    // The answer itself is only the option's kind: reject_once.
    assert!(!asking.join().unwrap().unwrap());
    // The reason waits for the run, to be shown it as its next input (native.rs pass_on_denials: "I declined that. ...").
    let said: Vec<(String, String, String)> = host.channel.take_denied_saying("6").into_iter().map(|d| (d.run_id, d.mission_id, d.reason)).collect();
    assert_eq!(said, vec![("6".to_owned(), "m_6".to_owned(), "Use npm test instead.".to_owned())]);
    // And the tool call it was about is remembered as declined, so its failure reads as the person's no.
    assert!(host.channel.declined("6").contains("call_utkWEwr3SiwPun0Iym7cESBP"));
}
