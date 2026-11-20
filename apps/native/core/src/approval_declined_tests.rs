//! Electron's `main/a-declined-call-is-not-a-failure.test.ts` (3 cases, 0.374): Codex 0.157 runs a command inside a
//! script, and a declined approval comes back as the script failing, so the row read "failed" in red. Locust answered
//! that approval itself; the item it declined is said as declined.
use super::*;
use crate::runtime::Event;

fn failed(item_id: &str) -> Event {
    Event {
        kind: "tool.failed".into(),
        payload: json!({"itemId": item_id, "toolKind": "commandExecution", "command": "git status", "status": "failed", "exitCode": 1, "phase": "completed"}),
        thread: None,
    }
}

fn set(items: &[&str]) -> HashSet<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

// case: is said as declined when it fails, and nothing else changes
#[test]
fn is_said_as_declined_when_it_fails_and_nothing_else_changes() {
    let completed = Event { kind: "tool.completed".into(), payload: json!({"itemId": "item_1"}), thread: None };
    let out = with_declines(vec![failed("item_1"), failed("item_2"), completed], &set(&["item_1"]));
    assert_eq!(out[0].payload["status"], "declined");
    assert_eq!(out[1].payload["status"], "failed");
    assert_eq!(out[2].kind, "tool.completed");
    assert_eq!(out[2].payload, json!({"itemId": "item_1"}));
    // Only the status is said differently: the rest of the failed call is as Codex put it.
    assert_eq!(out[0].payload["exitCode"], 1);
    assert_eq!(out[0].payload["command"], "git status");
}

// case: is left alone when there is nothing declined
#[test]
fn is_left_alone_when_there_is_nothing_declined() {
    let out = with_declines(vec![failed("item_1")], &HashSet::new());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].payload, failed("item_1").payload);
}

// case: remembers the items the person declined in a run -- and only those, and only while the run lasts
#[test]
fn remembers_the_items_the_person_declined_in_a_run_and_only_those_and_only_while_the_run_lasts() {
    let channel = Channel::new();
    let ask = |item: &'static str| {
        let ch = channel.clone();
        std::thread::spawn(move || {
            ch.ask_codex(
                "run_1",
                "mission_1",
                "item/commandExecution/requestApproval",
                &json!({"itemId": item, "command": "git status", "cwd": "/work"}),
                json!(1),
                &|| false,
            )
        })
    };
    let denied = ask("item_denied");
    let first = loop {
        if let Some(card) = channel.waiting().first().cloned() {
            break card;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let approved = ask("item_approved");
    let second = loop {
        if let Some(card) = channel.waiting().into_iter().find(|c| c.approval_id != first.approval_id) {
            break card;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    channel.decide(Answer::Decision { approval_id: first.approval_id.clone(), decision: Decision::Deny, reason: None }).unwrap();
    channel.decide(Answer::Decision { approval_id: second.approval_id.clone(), decision: Decision::ApproveOnce, reason: None }).unwrap();
    assert_eq!(denied.join().unwrap().unwrap(), json!({"decision": "reject"}));
    assert_eq!(approved.join().unwrap().unwrap(), json!({"decision": "accept"}));
    assert_eq!(channel.declined("run_1"), set(&["item_denied"]));
    assert_eq!(channel.declined("run_other"), HashSet::new());
    channel.release("mission_1");
    assert_eq!(channel.declined("run_1"), HashSet::new());
}
