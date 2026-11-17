// Electron's rules for renaming and retitling an open ACP call (packages/runtime-adapters/src/acp-events.ts at
// 4ce2f37a, the `tool_call_update` branch, ~line 393): a call still named `other`/`tool` is renamed only when the
// update's own kind is absent or `other`, from the update's title or else the earlier one, and never to `other` or
// `tool`; a later title replaces the earlier only while that is still a bare tool word; a title the command repeats
// is left out of the finished row; a shell word in the title makes it a shell, whose `locations` is only its folder.
use super::{Event, acp_events::AcpNormalizer};
use serde_json::{Value, json};

const FOLDER: &str = "C:\\work\\scratch\\oc2-tool-rows-probe";
const FILE: &str = "C:\\work\\scratch\\oc2-tool-rows-probe\\note.txt";

fn normalize(updates: &[Value]) -> Vec<Event> {
    let mut acp = AcpNormalizer::default();
    updates.iter().flat_map(|u| acp.update(u)).collect()
}
fn of<'a>(events: &'a [Event], kind: &str) -> &'a Value {
    &events.iter().find(|e| e.kind == kind).unwrap_or_else(|| panic!("{kind}")).payload
}

#[test]
fn a_call_named_other_keeps_its_name_when_an_update_of_its_own_kind_arrives() {
    let events = normalize(&[
        json!({ "sessionUpdate": "tool_call", "toolCallId": "r", "kind": "other", "title": "Looking at the note", "status": "pending" }),
        json!({ "sessionUpdate": "tool_call_update", "toolCallId": "r", "kind": "read", "title": "read", "status": "completed", "locations": [{ "path": FILE }] }),
    ]);
    let completed = of(&events, "tool.completed");
    assert_eq!((&completed["toolKind"], &completed["name"]), (&json!("other"), &json!("other")), "{completed}");
}

#[test]
fn an_update_with_no_kind_or_title_never_renames_a_call_to_tool() {
    let events = normalize(&[
        json!({ "sessionUpdate": "tool_call", "toolCallId": "p", "kind": "other", "title": "Run requested probe", "status": "pending" }),
        json!({ "sessionUpdate": "tool_call_update", "toolCallId": "p", "status": "completed" }),
    ]);
    let completed = of(&events, "tool.completed");
    assert_eq!((&completed["toolKind"], &completed["name"]), (&json!("other"), &json!("other")), "{completed}");
    assert_eq!(completed["title"], "Run requested probe");
}

#[test]
fn an_update_with_no_title_names_the_call_by_the_tool_word_it_was_first_titled() {
    let events = normalize(&[
        json!({ "sessionUpdate": "tool_call", "toolCallId": "g", "kind": "tool", "title": "grep", "status": "pending" }),
        json!({ "sessionUpdate": "tool_call_update", "toolCallId": "g", "status": "completed" }),
    ]);
    assert_eq!(of(&events, "tool.started")["name"], "tool");
    let completed = of(&events, "tool.completed");
    assert_eq!((&completed["toolKind"], &completed["name"]), (&json!("grep"), &json!("grep")), "{completed}");
}

#[test]
fn a_call_named_tool_is_never_renamed_to_other() {
    let events = normalize(&[
        json!({ "sessionUpdate": "tool_call", "toolCallId": "t", "kind": "tool", "title": "Working on it", "status": "pending" }),
        json!({ "sessionUpdate": "tool_call_update", "toolCallId": "t", "kind": "other", "title": "Still working", "status": "completed" }),
    ]);
    let completed = of(&events, "tool.completed");
    assert_eq!((&completed["toolKind"], &completed["name"]), (&json!("tool"), &json!("tool")), "{completed}");
}

#[test]
fn a_later_title_does_not_replace_one_that_is_already_a_sentence() {
    let events = normalize(&[
        json!({ "sessionUpdate": "tool_call", "toolCallId": "r", "kind": "read", "title": "Reading the note", "status": "pending" }),
        json!({ "sessionUpdate": "tool_call_update", "toolCallId": "r", "kind": "read", "title": "Read note.txt", "status": "completed", "locations": [{ "path": FILE }] }),
    ]);
    let completed = of(&events, "tool.completed");
    assert_eq!((&completed["title"], &completed["command"]), (&json!("Reading the note"), &json!(FILE)), "{completed}");
}

#[test]
fn a_call_opened_without_a_title_is_not_given_one_later() {
    let events = normalize(&[
        json!({ "sessionUpdate": "tool_call", "toolCallId": "r", "kind": "read", "status": "pending" }),
        json!({ "sessionUpdate": "tool_call_update", "toolCallId": "r", "kind": "read", "title": "Read note.txt", "status": "completed", "locations": [{ "path": FILE }] }),
    ]);
    let completed = of(&events, "tool.completed");
    assert!(completed.get("title").is_none(), "{completed}");
    assert_eq!(completed["command"], FILE);
}

#[test]
fn a_title_the_later_command_repeats_is_left_out_of_the_finished_row() {
    let events = normalize(&[
        json!({ "sessionUpdate": "tool_call", "toolCallId": "s", "kind": "execute", "title": "echo hi", "status": "pending" }),
        json!({ "sessionUpdate": "tool_call_update", "toolCallId": "s", "status": "completed", "rawInput": { "command": "echo hi" } }),
    ]);
    assert_eq!(of(&events, "tool.started")["title"], "echo hi");
    let completed = of(&events, "tool.completed");
    assert_eq!(completed["command"], "echo hi");
    assert!(completed.get("title").is_none(), "{completed}");
}

#[test]
fn a_shell_word_in_an_updates_title_keeps_the_folder_out_of_the_command() {
    let events = normalize(&[
        json!({ "sessionUpdate": "tool_call", "toolCallId": "x", "kind": "search", "title": "Looking for files", "status": "pending" }),
        json!({ "sessionUpdate": "tool_call_update", "toolCallId": "x", "kind": "other", "title": "bash", "status": "completed", "locations": [{ "path": FOLDER }] }),
    ]);
    let completed = of(&events, "tool.completed");
    assert!(completed.get("command").is_none(), "{completed}");
    assert_eq!((&completed["name"], &completed["title"]), (&json!("search"), &json!("Looking for files")));
}

#[test]
fn a_title_that_only_repeats_the_tools_name_is_left_out() {
    // OpenCode's `bash`: the acp-events.ts comment on the started row.
    let events = normalize(&[
        json!({ "sessionUpdate": "tool_call", "toolCallId": "b", "kind": "execute", "title": "bash", "status": "pending" }),
        json!({ "sessionUpdate": "tool_call_update", "toolCallId": "b", "status": "completed" }),
    ]);
    for kind in ["tool.started", "tool.completed"] {
        let row = of(&events, kind);
        assert_eq!(row["name"], "bash");
        assert!(row.get("title").is_none(), "{row}");
    }
}

#[test]
fn a_call_first_titled_with_a_shell_word_keeps_the_folder_out_when_a_later_update_names_neither() {
    let events = normalize(&[
        json!({ "sessionUpdate": "tool_call", "toolCallId": "x", "kind": "read", "title": "bash", "status": "pending" }),
        json!({ "sessionUpdate": "tool_call_update", "toolCallId": "x", "status": "completed", "locations": [{ "path": FOLDER }] }),
    ]);
    let completed = of(&events, "tool.completed");
    assert!(completed.get("command").is_none(), "{completed}");
}

#[test]
fn the_word_execute_in_an_updates_title_is_a_shell_too() {
    let events = normalize(&[
        json!({ "sessionUpdate": "tool_call", "toolCallId": "x", "kind": "search", "title": "Looking for files", "status": "pending" }),
        json!({ "sessionUpdate": "tool_call_update", "toolCallId": "x", "kind": "other", "title": "execute", "status": "completed", "locations": [{ "path": FOLDER }] }),
    ]);
    let completed = of(&events, "tool.completed");
    assert!(completed.get("command").is_none(), "{completed}");
}

#[test]
fn a_call_named_shell_keeps_the_folder_out_whatever_kind_a_later_update_says() {
    let events = normalize(&[
        json!({ "sessionUpdate": "tool_call", "toolCallId": "s", "kind": "other", "title": "shell", "status": "pending" }),
        json!({ "sessionUpdate": "tool_call_update", "toolCallId": "s", "kind": "read", "title": "Reading", "status": "completed", "locations": [{ "path": FOLDER }] }),
    ]);
    let completed = of(&events, "tool.completed");
    assert_eq!(completed["name"], "shell");
    assert!(completed.get("command").is_none(), "{completed}");
}
