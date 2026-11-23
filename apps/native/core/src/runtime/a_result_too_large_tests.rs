//! a-result-too-large-to-keep-still-answered.test.ts: the host drops a record over its cap, and a runtime that asked gets
//! a stand-in in its place (size, type, the calls it answered, none of its content), which the Claude reader closes
//! the call with.
use super::*;
use crate::runtime::claude_events::ClaudeNormalizer;
use serde_json::{Value, json};

const SECRET: &str = "the-content-of-the-big-result";

fn big_result() -> String {
    json!({"type":"user","message":{"content":[{"tool_use_id":"toolu_big","type":"tool_result","content":format!("{SECRET} {}", "x".repeat(2_000))}]}}).to_string()
}

/// Every line through a 256-byte cap; a runtime that asked gets the stand-in where a record was dropped.
fn records_for(stand_ins: bool, lines: &[&str]) -> Vec<String> {
    let mut split = Lines::new(&Limits { max_record_bytes: 256, ..Limits::default() });
    let mut pieces = split.feed(lines.iter().map(|l| format!("{l}\n")).collect::<String>().as_bytes());
    pieces.extend(split.end());
    pieces
        .into_iter()
        .filter_map(|p| match p {
            Piece::Record(raw) => Some(raw),
            Piece::Dropped { head, bytes, .. } => stand_ins.then(|| stand_in(&head, bytes).to_string()),
        })
        .collect()
}

#[test]
fn hands_nothing_on_from_an_oversized_line_the_process_ended_in_the_middle_of_2026_10_10_sweep() {
    // case: hands nothing on from an oversized line the process ended in the middle of (2026-10-10 sweep)
    let mut split = Lines::new(&Limits { max_record_bytes: 256, ..Limits::default() });
    let mut pieces = split.feed(format!(r#"{{"type":"user","content":"{}"#, "x".repeat(2_000)).as_bytes());
    pieces.extend(split.feed(br#"tail of the same line"}"#));
    pieces.extend(split.end());
    assert!(!pieces.iter().any(|p| matches!(p, Piece::Record(_))), "{pieces:?}");
}

#[test]
fn leaves_its_size_its_type_and_the_call_it_answered_and_none_of_its_content() {
    // case: leaves its size, its type and the call it answered -- and none of its content
    let raws = records_for(true, &[&big_result(), r#"{"type":"next"}"#]);
    assert_eq!(raws.len(), 2);
    let stand_in: Value = serde_json::from_str(&raws[0]).unwrap();
    assert_eq!(stand_in["type"], "locust.oversized");
    assert_eq!(stand_in["recordType"], "user");
    assert_eq!(stand_in["callIds"], json!(["toolu_big"]));
    assert!(stand_in["bytes"].as_u64().unwrap() > 2_000);
    assert!(!raws.join(" ").contains(SECRET));
}

#[test]
fn leaves_nothing_for_a_runtime_that_did_not_ask() {
    // case: leaves nothing for a runtime that did not ask
    assert_eq!(records_for(false, &[&big_result(), r#"{"type":"next"}"#]), [r#"{"type":"next"}"#]);
}

#[test]
fn names_a_cursor_edit_by_the_id_its_adapter_will_use_and_keeps_none_of_the_file() {
    // case: names a Cursor edit by the id its adapter will use, and keeps none of the file
    let id = "call-scrub-a0-0\nfc_scrub_a0_0";
    let line = json!({
        "type": "tool_call",
        "subtype": "completed",
        "call_id": id,
        "tool_call": {
            "editToolCall": {
                "args": {"path": "src/app.ts", "streamContent": "x"},
                "result": {"success": {"beforeFullFileContent": format!("{SECRET} {}", "y".repeat(2_000)), "afterFullFileContent": SECRET}}
            },
            "toolCallId": id
        }
    })
    .to_string();
    let raws = records_for(true, &[&line, r#"{"type":"next"}"#]);
    assert_eq!(raws.len(), 2);
    let stand_in: Value = serde_json::from_str(&raws[0]).unwrap();
    assert_eq!(stand_in["type"], "locust.oversized");
    assert_eq!(stand_in["recordType"], "tool_call");
    assert_eq!(stand_in["callIds"], json!(["call-scrub-a0-0|fc_scrub_a0_0"]));
    assert!(!raws.join(" ").contains(SECRET));
}

#[test]
fn closes_the_call_as_answered_too_large_to_keep_instead_of_leaving_it_to_read_did_not_report() {
    // case: closes the call as answered, too large to keep, instead of leaving it to read 'did not report'
    let mut claude = ClaudeNormalizer::default();
    claude.accept(&json!({"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu_big","name":"Read"}}}));
    // Any other record's stand-in closes nothing, even while the call is still open.
    assert!(
        claude
            .accept(&json!({"type":"locust.oversized","bytes":300_000,"recordType":"assistant","callIds":["toolu_big"]}))
            .is_empty()
    );
    let closed = claude.accept(&json!({"type":"locust.oversized","bytes":412_000,"recordType":"user","callIds":["toolu_big","toolu_unknown"]}));
    assert_eq!(closed.len(), 1);
    assert_eq!(closed[0].kind, "tool.completed");
    assert_eq!(closed[0].payload["itemId"], "toolu_big");
    assert_eq!(closed[0].payload["status"], "result too large to keep");
    assert!(closed[0].payload["output"].as_str().unwrap().contains("402 KB"));
}
