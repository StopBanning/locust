use crate::{activity_entries::helper_description, activity_summary, ledger::{Transcript, Item, Status}};
use serde_json::json;
#[test]
fn is_its_own_row_what_it_was_asked_and_whether_it_reported_back() {
    let mut t = Transcript::default();
    t.apply([
        ("tool.started", "t1", "tool_use", "Task", "Search the tests for flaky cases"),
        ("tool.completed", "t1", "tool_use", "Task", "Search the tests for flaky cases"),
        ("tool.started", "t2", "task", "task", "Summarise README"),
    ].into_iter().enumerate().map(|(i,(ty,id,kind,name,command))| json!({
        "recordType":"mission.event", "event":{"sequence":i+1,"type":ty,
        "occurredAt":"2026-08-31T16:00:00.000Z", "payload":{"itemId":id,"toolKind":kind,"name":name,"command":command}}
    })).collect());
    let rows = t.items.iter().filter_map(|item| match item {
        Item::Tool { name, summary, status, detail } => {
            assert!(detail.helper);
            Some((helper_description(name, summary), *status != Status::Running))
        }, _ => None,
    }).collect::<Vec<_>>();
    assert_eq!(rows, [("Search the tests for flaky cases", true), ("Summarise README", false)]);
    assert_eq!(activity_summary::activity_summary(&t.items), "asked 2 subagents");
}
#[test]
fn a_helper_the_runtime_never_named_is_said_to_be_unnamed_not_drawn_as_a_path() {
    assert_eq!(helper_description("Task", "Task"), "a subagent, unnamed");
}
