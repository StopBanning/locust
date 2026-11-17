// Carries packages/runtime-adapters/test/an-opencode-2-step-says-what-it-did.test.ts (Electron 4ce2f37a): an OpenCode
// 2 tool row names its tool and the file or command it acted on, never "other" or the folder it ran in.
use super::{Event, acp_events::AcpNormalizer};
use serde_json::{Value, json};

const FOLDER: &str = "C:\\work\\scratch\\oc2-tool-rows-probe";
const FILE: &str = "C:\\work\\scratch\\oc2-tool-rows-probe\\note.txt";

fn normalize(updates: &[Value]) -> Vec<Event> {
    let mut acp = AcpNormalizer::default();
    let mut events: Vec<Event> = updates.iter().flat_map(|u| acp.update(u)).collect();
    events.extend(acp.finish("end_turn", Value::Null, false, None, false));
    events
}
fn of<'a>(events: &'a [Event], kind: &str) -> &'a Value {
    &events.iter().find(|e| e.kind == kind).unwrap_or_else(|| panic!("{kind}")).payload
}
/// Whether any of the row's words is, or holds, the folder the run stood in.
fn holds_folder(row: &Value) -> bool {
    row.as_object().unwrap().values().filter_map(Value::as_str).any(|s| s.contains(FOLDER))
}
fn all(events: &[Event]) -> String {
    serde_json::to_string(&events.iter().map(|e| &e.payload).collect::<Vec<_>>()).unwrap()
}

#[test]
fn a_read_reported_as_other_is_named_by_the_tool_word_in_its_title_and_the_file() {
    let events = normalize(&[
        json!({ "sessionUpdate": "tool_call", "toolCallId": "read-1", "kind": "other", "title": "read", "status": "pending", "rawInput": {}, "locations": [] }),
        json!({ "sessionUpdate": "tool_call_update", "toolCallId": "read-1", "kind": "other", "title": "read", "status": "in_progress", "rawInput": { "path": FILE }, "locations": [{ "path": FILE }] }),
        json!({ "sessionUpdate": "tool_call_update", "toolCallId": "read-1", "status": "completed", "locations": [{ "path": FILE }] }),
    ]);
    let started = of(&events, "tool.started");
    assert_eq!((&started["toolKind"], &started["name"]), (&json!("read"), &json!("read")));
    assert!(started.get("command").is_none(), "{started}");
    let completed = of(&events, "tool.completed");
    assert_eq!((&completed["toolKind"], &completed["name"], &completed["command"], &completed["status"]), (&json!("read"), &json!("read"), &json!(FILE), &json!("completed")));
    assert!(!all(&events).contains(r#""name":"other""#));
}

#[test]
fn a_read_whose_kind_was_already_read_keeps_the_file_when_the_completion_has_only_a_location() {
    let events = normalize(&[
        json!({ "sessionUpdate": "tool_call", "toolCallId": "read-1", "kind": "read", "title": "read", "status": "pending", "rawInput": {}, "locations": [] }),
        json!({ "sessionUpdate": "tool_call_update", "toolCallId": "read-1", "kind": "read", "title": "read", "status": "in_progress", "rawInput": { "path": FILE }, "locations": [{ "path": FILE }] }),
        json!({ "sessionUpdate": "tool_call_update", "toolCallId": "read-1", "status": "completed", "locations": [{ "path": FILE }] }),
    ]);
    let completed = of(&events, "tool.completed");
    assert_eq!((&completed["name"], &completed["toolKind"], &completed["command"]), (&json!("read"), &json!("read"), &json!(FILE)));
}

#[test]
fn a_shell_keeps_the_command_its_update_gave_and_never_the_folder_its_completion_carries() {
    let events = normalize(&[
        json!({ "sessionUpdate": "tool_call", "toolCallId": "sh-1", "kind": "execute", "title": "shell", "status": "pending", "rawInput": { "cwd": FOLDER }, "locations": [{ "path": FOLDER }] }),
        json!({ "sessionUpdate": "tool_call_update", "toolCallId": "sh-1", "kind": "execute", "title": "echo hi", "status": "in_progress", "rawInput": { "command": "echo hi", "workdir": FOLDER }, "locations": [{ "path": FOLDER }] }),
        json!({ "sessionUpdate": "tool_call_update", "toolCallId": "sh-1", "status": "completed", "locations": [{ "path": FOLDER }] }),
    ]);
    let started = of(&events, "tool.started");
    assert_eq!((&started["toolKind"], &started["name"]), (&json!("command_execution"), &json!("bash")));
    assert!(started.get("command").is_none(), "{started}");
    let completed = of(&events, "tool.completed");
    assert_eq!((&completed["toolKind"], &completed["name"], &completed["command"], &completed["status"]), (&json!("command_execution"), &json!("bash"), &json!("echo hi"), &json!("completed")));
    assert!(!holds_folder(completed), "{completed}");
}

#[test]
fn a_shell_reported_as_other_and_titled_shell_is_its_command_not_the_folder_it_started_in() {
    let events = normalize(&[
        json!({ "sessionUpdate": "tool_call", "toolCallId": "sh-1", "kind": "other", "title": "shell", "status": "pending", "rawInput": {}, "locations": [{ "path": FOLDER }] }),
        json!({ "sessionUpdate": "tool_call_update", "toolCallId": "sh-1", "kind": "other", "title": "echo hi", "status": "completed", "rawInput": { "command": "echo hi" }, "locations": [{ "path": FOLDER }] }),
    ]);
    let started = of(&events, "tool.started");
    assert!(started.get("command").is_none(), "{started}");
    assert_ne!(started["name"], "other");
    let completed = of(&events, "tool.completed");
    assert_eq!(completed["command"], "echo hi");
    assert!(!holds_folder(completed), "{completed}");
}

#[test]
fn copilots_rows_keep_the_command_and_its_sentence_from_the_first_report() {
    // The tool_call update of fixtures/acp/copilot-1.0.88-bash-rejected.jsonl.
    let call = json!({ "sessionUpdate": "tool_call", "toolCallId": "call_GfYFYmhnqXIp2a9V7osocij6", "title": "Run requested probe", "kind": "execute", "status": "pending",
        "rawInput": { "command": "echo ACP-PROBE", "description": "Run requested probe" } });
    let mut acp = AcpNormalizer::default();
    let mut events = acp.update(&call);
    events.extend(acp.update(&json!({ "sessionUpdate": "tool_call_update", "toolCallId": "call_GfYFYmhnqXIp2a9V7osocij6", "kind": "execute", "status": "failed",
        "title": "Run requested probe", "rawInput": { "command": "echo ACP-PROBE" } })));
    events.extend(acp.prompt_boundary("end_turn"));
    events.extend(acp.finish("end_turn", Value::Null, false, None, false));
    let started = of(&events, "tool.started");
    assert_eq!((&started["toolKind"], &started["name"], &started["command"], &started["title"]), (&json!("command_execution"), &json!("bash"), &json!("echo ACP-PROBE"), &json!("Run requested probe")));
    let failed = of(&events, "tool.failed");
    assert_eq!((&failed["name"], &failed["command"], &failed["title"], &failed["status"]), (&json!("bash"), &json!("echo ACP-PROBE"), &json!("Run requested probe"), &json!("failed")));
}

#[test]
fn a_sentence_is_never_read_as_a_tool_when_copilot_says_its_kind_is_other() {
    let events = normalize(&[
        json!({ "sessionUpdate": "tool_call", "toolCallId": "t", "kind": "other", "title": "Run requested probe", "status": "pending", "rawInput": { "command": "echo ACP-PROBE" } }),
        json!({ "sessionUpdate": "tool_call_update", "toolCallId": "t", "kind": "other", "title": "Run requested probe", "status": "completed", "rawInput": { "command": "echo ACP-PROBE" } }),
    ]);
    let started = of(&events, "tool.started");
    assert_eq!((&started["name"], &started["toolKind"], &started["command"], &started["title"]), (&json!("other"), &json!("other"), &json!("echo ACP-PROBE"), &json!("Run requested probe")));
    let completed = of(&events, "tool.completed");
    assert_eq!((&completed["name"], &completed["command"], &completed["title"]), (&json!("other"), &json!("echo ACP-PROBE"), &json!("Run requested probe")));
}

// Rust's own (acp-events.ts: "kind other until the title arrives"): a call first said as a sentence is named by the
// tool word a later update gives.
#[test]
fn a_call_first_said_as_a_sentence_takes_the_tool_word_a_later_update_gives() {
    let events = normalize(&[
        json!({ "sessionUpdate": "tool_call", "toolCallId": "r", "kind": "other", "title": "Looking at the note", "status": "pending", "rawInput": {} }),
        json!({ "sessionUpdate": "tool_call_update", "toolCallId": "r", "kind": "other", "title": "read", "status": "completed", "locations": [{ "path": FILE }] }),
    ]);
    assert_eq!(of(&events, "tool.started")["name"], "other");
    let completed = of(&events, "tool.completed");
    assert_eq!((&completed["toolKind"], &completed["name"], &completed["command"]), (&json!("read"), &json!("read"), &json!(FILE)));
}
