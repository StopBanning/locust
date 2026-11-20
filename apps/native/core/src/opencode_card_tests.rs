//! Electron's `an-opencode-permission-is-a-card.test.ts` (6 cases): what OpenCode's server asks becomes the card
//! Codex's requests do, and the person's answer goes back as the server's own reply.
use super::*;
use serde_json::json;
use std::time::Duration;

/// Raise OpenCode's request on a channel of its own; hand back the card, the channel and the waiting run.
fn raise(asked: Value, mission: &str) -> (Request, std::sync::Arc<Channel>, std::thread::JoinHandle<Result<Value, String>>) {
    let ch = Channel::new();
    let ch2 = ch.clone();
    let mission = mission.to_owned();
    let handle = std::thread::spawn(move || {
        ch2.ask_codex_with_changes("run", &mission, "opencode", asked["method"].as_str().unwrap(), &asked["params"], asked["id"].clone(), None, &|| false)
    });
    for _ in 0..400 {
        if ch.pending_count() == 1 {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let card = ch.waiting().into_iter().next().expect("a card");
    (card, ch, handle)
}

// case: is a command card naming the command, from OpenCode, and the answer comes back as its reply
#[test]
fn is_a_command_card_naming_the_command_and_the_answer_comes_back_as_its_reply() {
    let asked = open_code_permission_request(&json!({"permission": "bash", "patterns": ["echo hi"], "metadata": {"command": "echo hi"}}), "/work");
    let (card, ch, handle) = raise(asked, "m_1");
    assert_eq!((card.runtime.as_str(), card.kind.as_str(), card.detail.as_str(), card.summary.as_str()), ("opencode", "command", "echo hi", "Run a command"));
    ch.decide(Answer::Decision { approval_id: card.approval_id.clone(), decision: Decision::ApproveOnce, reason: None }).unwrap();
    assert_eq!(open_code_reply_for(&handle.join().unwrap().unwrap()), json!("once"));
}

// case: shows the edit it proposes, by its file relative to the folder (the 0.345 beta report)
#[test]
fn shows_the_edit_it_proposes_by_its_file_relative_to_the_folder() {
    let diff = ["Index: /work/notes.txt", &"=".repeat(67), "--- /work/notes.txt", "+++ /work/notes.txt", "@@ -1,1 +1,1 @@", "-Status: draft", "+Status: ready", ""].join("\n");
    let asked = open_code_permission_request(&json!({"permission": "edit", "patterns": ["notes.txt"], "metadata": {"filepath": "/work/notes.txt", "diff": diff}}), "/work");
    let (card, ch, handle) = raise(asked, "m_2");
    assert_eq!((card.kind.as_str(), card.summary.as_str(), card.detail.as_str()), ("file-change", "Change 1 file", "notes.txt"));
    let patch = card.patch.clone().expect("the edit is shown");
    assert!(patch.text.contains("+Status: ready") && patch.text.contains("-Status: draft"), "{}", patch.text);
    assert!(!patch.text.contains("Index:"), "{}", patch.text);
    assert_eq!((patch.added, patch.removed), (1, 1));
    ch.release("m_2");
    let _ = handle.join();
}

// case: names an edit by its file, and anything else by what it is
#[test]
fn names_an_edit_by_its_file_and_anything_else_by_what_it_is() {
    let edit = open_code_permission_request(&json!({"permission": "edit", "patterns": ["a.txt"], "metadata": {"filepath": "/work/a.txt"}}), "/work");
    assert_eq!((edit["method"].as_str(), edit["params"]["summary"].as_str()), (Some("item/fileChange/requestApproval"), Some("a.txt")));
    let outside = open_code_permission_request(&json!({"permission": "external_directory", "patterns": ["/other/*"], "metadata": {}}), "/work");
    assert_eq!(outside["method"], "item/commandExecution/requestApproval");
    assert_eq!(outside["params"]["command"], "/other/*");
    assert_eq!(outside["params"]["locustCard"]["summary"], "Reach files outside your project folder");
}

// case: says a fetch is a fetch, a write outside the folder is outside it, and what Always allows
#[test]
fn says_a_fetch_is_a_fetch_a_write_outside_is_outside_and_what_always_allows() {
    let fetch = open_code_permission_request(&json!({"permission": "webfetch", "patterns": ["https://example.com/"], "metadata": {"url": "https://example.com/"}}), "/work");
    assert_eq!(fetch["params"]["command"], "https://example.com/");
    assert_eq!(fetch["params"]["locustCard"]["summary"], "Fetch a web page");
    assert_eq!(fetch["params"]["locustCard"]["reversibleSays"], "Nothing on this machine is changed by fetching.");
    let hosts = open_code_permission_request(&json!({"permission": "edit", "patterns": ["/etc/hosts"], "metadata": {"filepath": "/etc/hosts"}}), "/work");
    assert_eq!(hosts["params"]["locustCard"]["summary"], "Change 1 file outside your project folder");
    let echo = open_code_permission_request(&json!({"permission": "bash", "patterns": ["echo SERVED"], "always": ["echo *"], "metadata": {"command": "echo SERVED"}}), "/work");
    assert_eq!(echo["params"]["command"], "echo SERVED");
    assert_eq!(echo["params"]["locustCard"]["alwaysCovers"], "anything matching “echo *”");
}

// case: names a person's own MCP tool, with no exact line where OpenCode gives no details
#[test]
fn names_a_persons_own_mcp_tool_with_no_exact_line() {
    let asked = json!({"permission": "notes_save_note", "patterns": ["*"], "always": ["*"], "metadata": {}});
    let (card, ch, handle) = raise(open_code_permission_request(&asked, "/work"), "m_3");
    assert_eq!((card.kind.as_str(), card.summary.as_str(), card.detail.as_str()), ("command", "Use notes_save_note", ""));
    ch.release("m_3");
    let _ = handle.join();
    let req = open_code_permission_request(&asked, "/work");
    assert_eq!(req["params"]["command"], "");
    assert_eq!(req["params"]["locustAlwaysKey"], "opencode:notes_save_note:*");
    assert_eq!(req["params"]["locustCard"]["alwaysCovers"], "every “notes_save_note” it asks for");
    let given = open_code_permission_request(&json!({"permission": "notes_save_note", "patterns": ["note.txt"], "metadata": {}}), "/work");
    assert_eq!(given["params"]["command"], "note.txt");
}

// case: reads "for the rest of the run" as once, Locust keeping the Always, and anything unclear as a refusal
#[test]
fn reads_for_the_rest_of_the_run_as_once_and_anything_unclear_as_a_refusal() {
    assert_eq!(open_code_reply_for(&json!({"decision": "acceptForSession"})), json!("once"));
    assert_eq!(open_code_reply_for(&json!({"decision": "reject"})), json!("reject"));
    assert_eq!(open_code_reply_for(&Value::Null), json!("reject"));
    assert_eq!(open_code_reply_for(&json!({"decision": "accept", "extra": 1})), json!("once"));
}
