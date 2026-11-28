//! The transcript and the row formatters the window uses, without a live runtime.
use crate::{ledger::{Item, ToolDetail, Transcript}, path_words::display_path};
use serde_json::{Value, json};

fn command(output: Option<&str>) -> Box<ToolDetail> {
    let record = |n: u64, kind: &str, data: Value| json!({
        "recordType": "mission.event", "event": {
            "sequence": n, "type": kind, "runId":"run_1", "missionId":"mission_1",
            "occurredAt":"2026-10-04T05:00:00.000Z", "sourceAdapter":"codex", "payload": data
        }
    });
    let payload = json!({"name":"shell", "toolKind":"command_execution",
        "itemId":"call", "command":"seq 1 3"});
    let mut done = payload.clone();
    if let Some(output) = output {
        done["output"] = json!(output);
    }
    let mut t = Transcript::default();
    t.apply(vec![
        record(1, "tool.started", payload),
        record(2, "tool.completed", done),
    ]);
    t.items.into_iter().find_map(|item| match item {
        Item::Tool { detail, .. } if detail.id == "call" => Some(detail),
        _ => None,
    }).expect("the command reaches the transcript")
}

// case: carries the output the runtime reported
#[test]
fn command_completion_carries_the_runtime_output_into_the_drawers_detail() {
    let detail = command(Some("1\n2\n3"));
    assert_eq!(detail.output, "1\n2\n3");
    assert!(detail.output_reported);
}

// case: carries nothing where the runtime reported nothing
#[test]
fn a_command_without_output_is_not_marked_as_having_reported_it() {
    let detail = command(None);
    assert!(detail.output.is_empty());
    assert!(!detail.output_reported);
}

// case: keeps an empty output distinct from an absent one
#[test]
fn a_command_that_printed_nothing_still_reported_its_output() {
    let empty = command(Some(""));
    let absent = command(None);
    assert_eq!(empty.output, absent.output);
    assert!(empty.output_reported);
    assert!(!absent.output_reported);
}

const WORKSPACE: &str = r"C:\Users\dev\.claude";
const SCRATCH: &str = r"C:\Users\dev\.cursor\projects\x\agent-tools\3a5a3562.txt";

// case: are named for what they are, not by their UUID
#[test]
fn a_cursor_scratch_row_names_the_saved_result_without_its_random_id() {
    assert_eq!(display_path(SCRATCH, Some(WORKSPACE)), "Cursor's saved tool result");
}

// case: is relative inside the folder, as before
#[test]
fn a_row_in_the_workspace_uses_the_relative_path() {
    assert_eq!(display_path(r"C:\Users\dev\.claude\report.md", Some(WORKSPACE)), "report.md");
    assert_eq!(display_path("pebble/hello.txt", Some(WORKSPACE)), "pebble/hello.txt");
}

// case: is the last two segments of a path elsewhere, behind an ellipsis
#[test]
fn a_row_for_an_outside_long_path_keeps_its_last_two_parts() {
    assert_eq!(
        display_path(r"C:\Users\dev\Documents\Codex\checkout\apps\desktop\src\components\Thread.tsx", Some(WORKSPACE)),
        "…/components/Thread.tsx"
    );
    assert_eq!(display_path("/home/dev/project/src/main.rs", Some(WORKSPACE)), "…/src/main.rs");
}

// case: keeps a short absolute path whole, and names a scratch file for what it is
#[test]
fn a_short_external_path_stays_absolute_but_a_scratch_file_gets_words() {
    let short = r"C:\notes\todo.txt";
    assert_eq!(display_path(short, Some(WORKSPACE)), short);
    assert_eq!(display_path(SCRATCH, Some(WORKSPACE)), "Cursor's saved tool result");
}
