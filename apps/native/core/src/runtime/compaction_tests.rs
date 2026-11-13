//! AN OPENCODE COMPACTION IS NOT WHAT THE TEAMMATE SAID (A6.9): renderer/src/a-compaction-is-a-line-not-a-reply.test.ts
//! (the thread's half) and main/a-compaction-summary-is-not-the-reply.test.ts (the host's half).
//!
//! When a conversation outgrows the model's context, OpenCode summarizes it and adds "Continue if you have next
//! steps..." to carry on, and `run` prints both as ordinary text. The summary is the model's notes to itself about the
//! whole conversation, so it quotes what was said before -- blocks included -- and when the run ends right after it, it
//! was the last thing "said": the reply the host reads blocks from, and the answer a relay brings back.
//!
//! Records shaped as OpenCode writes them, through the real normalizer, the real ledger reader and the reply reader.
use super::opencode_events::{COMPACTED, Normalizer};
use crate::ledger::{Item, Transcript};
use serde_json::{Value, json};

fn record(kind: &str, part: Value) -> Value {
    json!({"type": kind, "sessionID": "ses_1", "part": part})
}
fn start() -> Value {
    record("step_start", json!({"type": "step-start"}))
}
fn finish_step(reason: &str) -> Value {
    record("step_finish", json!({"type": "step-finish", "reason": reason, "tokens": {"input": 10, "output": 2}}))
}
fn said(text: &str) -> Value {
    record("text", json!({"type": "text", "text": text, "time": {"start": 1, "end": 2}}))
}
fn read() -> Value {
    record("tool_use", json!({"type": "tool", "tool": "read", "callID": "call_1", "state": {"status": "completed", "input": {"filePath": "a.txt"}, "output": "hi"}}))
}
fn note() -> Value {
    record(
        "text",
        json!({
            "type": "text",
            "synthetic": true,
            "metadata": {"compaction_continue": true},
            "text": "Continue if you have next steps, or stop and ask for clarification if you are unsure how to proceed.",
            "time": {"start": 3, "end": 3}
        }),
    )
}
// What a summary of an earlier turn holds: the message that turn sent.
const SUMMARY: &str = "## Work State\nSent Booty the numbers: <locust-share to=\"Booty\">The totals are 51.</locust-share>";

/// The ledger's records for these lines: the events the normalizer made, then how the process ended.
fn normalized(lines: &[Value], exit_code: i32) -> Vec<Value> {
    let mut n = Normalizer::new(None);
    let mut events: Vec<_> = lines.iter().flat_map(|line| n.accept(line)).collect();
    events.extend(n.finish(false, Some(exit_code), "", false));
    events
        .into_iter()
        .enumerate()
        .map(|(at, e)| json!({"recordType": "mission.event", "event": {"sequence": at + 1, "type": e.kind, "payload": e.payload}}))
        .collect()
}

fn turn() -> Vec<Value> {
    vec![
        start(), said("Reading the file first."), read(), finish_step("tool-calls"),
        start(), said(SUMMARY), finish_step("stop"),
        note(),
        start(), said("a.txt says hi."), finish_step("stop"),
    ]
}

#[test]
fn an_opencode_compaction_in_the_thread_is_a_line_saying_so_not_the_summary_or_the_note_to_the_model() {
    let mut thread = Transcript::default();
    thread.apply(normalized(&turn(), 0));
    let spoken: Vec<&str> = thread
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Assistant { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(spoken.last().copied(), Some("a.txt says hi."));
    assert!(!spoken.iter().any(|t| t.contains("Work State") || t.contains("Continue if you have next steps")), "{spoken:?}");
    let lines: Vec<&str> = thread
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Note { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(lines, vec![COMPACTED]);
}

fn events_of(records: &[Value]) -> Vec<Value> {
    records.iter().map(|r| r["event"].clone()).collect()
}

#[test]
fn an_opencode_compaction_leaves_the_reply_the_host_reads_blocks_from_as_what_came_after_it() {
    let events = events_of(&normalized(&turn(), 0));
    assert_eq!(crate::carry_on::latest_final(&events).as_deref(), Some("a.txt says hi."));
    let door = crate::door_tools::Turn { events, ..Default::default() };
    assert_eq!(crate::door_tools::latest_final(&door).as_deref(), Some("a.txt says hi."));
}

#[test]
fn an_opencode_compaction_summary_is_never_the_reply_even_when_nothing_came_after_it() {
    // The run was stopped after the note: the summary was the last text.
    let events = events_of(&normalized(&turn()[..8], 1));
    let reply = crate::carry_on::latest_final(&events).unwrap_or_default();
    // Before 0.321 this was OpenCode's note to the model; with the note dropped and the summary kept, it would be the
    // summary.
    assert!(!reply.contains("Continue if you have next steps"), "{reply}");
    assert!(!reply.contains("Work State"), "{reply}");
    // So the message an earlier turn sent is not sent again.
    assert!(crate::peer_share::parse_share_blocks(&reply).is_empty());
    let door = crate::door_tools::Turn { events, ..Default::default() };
    assert!(!crate::door_tools::latest_final(&door).unwrap_or_default().contains("Work State"));
}
