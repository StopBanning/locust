//! Electron's `packages/runtime-adapters/test/antigravity-gaps.test.ts` (0.389), case by case, through the caller:
//! the Antigravity mission service hands the events of each transcript read to `restore_gaps`, which asks the
//! language server (`CascadeApi::planner_texts`) for the whole `PLANNER_RESPONSE` texts from the gapped message's step
//! and fills each `<truncated N bytes>` gap only from the step that holds it (`with_restored_gaps`, `restore_truncated`).
use super::*;
use crate::runtime::Event;
use crate::runtime::antigravity_events::restore_truncated;
use serde_json::json;

const BEFORE: &str = "3. Accessibility & Engine Core: Implements Windows High Contrast mode (`forced-colors: acti";
const MISSING: &str = "ve`) across the shell, and keeps the kill-switch ready for the fall review. The team also owns the";
const AFTER: &str = "/kill-switch recommendations due 2026-11-16 and municipal AI hearing developments.";

fn whole() -> String {
    format!("{BEFORE}{MISSING}{AFTER}")
}
fn gapped() -> String {
    format!("{BEFORE}\n<truncated 1066 bytes>\n{AFTER}")
}

/// The language server: answers the planner texts it was given, and records the step it was asked from.
struct Server {
    texts: Vec<String>,
    asked: Mutex<Vec<usize>>,
}

impl CascadeApi for Server {
    fn pending_question(&self, _: &str, _: usize) -> Result<Option<AntigravityPendingQuestion>, String> {
        Ok(None)
    }
    fn answer_question(&self, _: &str, _: &AntigravityPendingQuestion, _: &[AntigravityQuestionResponse]) -> Result<(), String> {
        Ok(())
    }
    fn planner_texts(&self, _: &str, from_step: usize) -> Result<Vec<String>, String> {
        crate::invariants::lock(&self.asked).push(from_step);
        Ok(self.texts.clone())
    }
}

fn server(texts: &[String]) -> Arc<Server> {
    Arc::new(Server { texts: texts.to_vec(), asked: Mutex::new(Vec::new()) })
}

/// Electron's `delta`: a whole message as the transcript reader records it.
fn delta(text: &str, item_id: &str) -> Event {
    Event {
        kind: "message.delta".into(),
        payload: json!({"itemId": item_id, "operation": "replace", "text": text, "final": true, "evidence": {"recordType": "PLANNER_RESPONSE"}}),
        thread: None,
    }
}

/// What the service records for one gapped message, the server holding `texts`.
fn restored(message: &str, texts: &[String]) -> String {
    let api: Arc<dyn CascadeApi> = server(texts);
    let events = restore_gaps(Some(&api), "conv_1", vec![delta(message, "msg_12")]);
    events[0].payload["text"].as_str().unwrap().to_owned()
}

#[test]
fn comes_back_whole_from_the_step_that_holds_it() {
    assert_eq!(restored(&gapped(), &["An unrelated answer.".into(), whole()]), whole());
}

#[test]
fn is_never_filled_from_a_different_step_the_words_on_both_sides_must_be_that_steps() {
    let gap = gapped();
    for candidate in [
        format!("{BEFORE} something else entirely."),
        format!("Something else. {AFTER}"),
        format!("{BEFORE}{MISSING}{MISSING}{MISSING}{MISSING}"),
        format!("{BEFORE}\n<truncated 12 bytes>\n{AFTER}"),
    ] {
        assert_eq!(restored(&gap, &[candidate.clone()]), gap, "{candidate}");
        assert_eq!(restore_truncated(&gap, &[candidate]), None);
    }
}

#[test]
fn holds_every_piece_between_two_gaps_in_order() {
    let twice = format!("start {MISSING} middle {MISSING} end");
    let gaps = "start \n<truncated 90 bytes>\n middle \n<truncated 90 bytes>\n end";
    assert_eq!(restored(gaps, &[twice.clone()]), twice);
    let elsewhere = "start \n<truncated 90 bytes>\n elsewhere \n<truncated 90 bytes>\n end";
    assert_eq!(restored(elsewhere, &[twice]), elsewhere);
}

#[test]
fn leaves_a_text_with_no_gap_alone() {
    let api = server(&[whole()]);
    let dyn_api: Arc<dyn CascadeApi> = api.clone();
    let events = restore_gaps(Some(&dyn_api), "conv_1", vec![delta("no gap here", "msg_12")]);
    assert_eq!(events[0].payload["text"], "no gap here");
    assert!(crate::invariants::lock(&api.asked).is_empty(), "nothing to restore asks the server nothing");
    assert_eq!(restore_truncated("no gap here", &[whole()]), None);
}

#[test]
fn carry_the_whole_answer_where_the_server_had_it_and_everything_else_as_it_came() {
    let api: Arc<dyn CascadeApi> = server(&[whole()]);
    let other = delta("A short, whole answer.", "msg_3");
    let other_payload = other.payload.clone();
    let events = restore_gaps(Some(&api), "conv_1", vec![delta(&gapped(), "msg_12"), other]);
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].kind, "message.delta");
    assert_eq!(events[0].payload["text"], whole().as_str());
    assert_eq!(events[0].payload["itemId"], "msg_12", "only the text changes: its place is the same");
    assert_eq!(events[1].kind, "message.delta");
    assert_eq!(events[1].payload, other_payload);
}

#[test]
fn keep_the_gap_when_the_server_did_not_have_the_step() {
    assert_eq!(restored(&gapped(), &["Nothing like it.".into()]), gapped());
    // No server to ask: the gap stays as the transcript had it.
    let events = restore_gaps(None, "conv_1", vec![delta(&gapped(), "msg_12")]);
    assert_eq!(events[0].payload["text"], gapped().as_str());
}

#[test]
fn scrub_the_restored_text_as_every_recorded_text_is_scrubbed_matching_the_scrubbed_transcript() {
    let key = format!("{}-ant-api03-{}1234", "sk", "abcdefgh".repeat(2));
    let whole = format!("Set the key: {key}. {MISSING}{AFTER}");
    let gapped = format!("Set the key: [redacted]. \n<truncated 40 bytes>\n{AFTER}");
    let text = restored(&gapped, &[whole]);
    assert!(!text.contains(&key), "{text}");
    assert!(text.contains(MISSING), "{text}");
}

#[test]
fn know_a_gap_when_they_see_one_and_the_step_a_message_came_from() {
    assert!(has_antigravity_gap(&delta(&gapped(), "msg_12")));
    assert!(!has_antigravity_gap(&delta("whole", "msg_12")));
    assert_eq!(antigravity_message_step("msg_12"), Some(12));
    assert_eq!(antigravity_message_step("m1"), None);
    // The server is asked from the gapped message's own step.
    let api = server(&[whole()]);
    let dyn_api: Arc<dyn CascadeApi> = api.clone();
    restore_gaps(Some(&dyn_api), "conv_1", vec![delta("whole", "msg_3"), delta(&gapped(), "msg_12")]);
    assert_eq!(*crate::invariants::lock(&api.asked), vec![12]);
}
