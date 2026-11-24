//! Electron's `packages/runtime-adapters/test/a-session-openCode-cannot-continue-is-not-resumed.test.ts`, through the
//! callers: the OpenCode run hands each record to `Normalizer::accept` and asks `finish` for the end, and the next
//! message reads the turn's events back with `native::prior_from_records` (codex-mission's `runtimeThreadIdOf`) to
//! decide which session to resume. The error line is Electron's own, word for word.
use super::*;

fn error_line(message: &str) -> Value {
    json!({"type":"error","timestamp":1,"sessionID":"ses_1","error":{"name":"APIError","data":{"message":message,"statusCode":400,"isRetryable":false}}})
}

/// The turn's events, from a step that started to the error and an exit with code 1.
fn turn(message: &str) -> Vec<Event> {
    let mut opencode = Normalizer::new(None);
    let mut events = opencode.accept(&json!({"type":"step_start","sessionID":"ses_1","part":{"type":"step-start"}}));
    events.extend(opencode.accept(&error_line(message)));
    events.extend(opencode.finish(false, Some(1), "", false));
    events
}

fn failed(events: &[Event]) -> Value {
    events.iter().find(|e| e.kind == "run.failed").expect("run.failed").payload.clone()
}

/// What the next message resumes, read from the ledger this turn wrote (after a turn that named session ses_1).
fn resumed(events: &[Event]) -> Option<String> {
    let mut records = vec![
        json!({"recordType":"mission.created","metadata":{"runtime":"opencode"}}),
        json!({"recordType":"event","event":{"type":"run.started","payload":{"runtimeThreadId":"ses_1"}}}),
    ];
    for e in events {
        records.push(json!({"recordType":"event","event":{"type":e.kind,"payload":e.payload}}));
    }
    crate::native::prior_from_records(&records, false).unwrap().thread
}

#[test]
fn is_reported_as_ended_so_the_next_message_starts_a_fresh_one() {
    // case: is reported as ended, so the next message starts a fresh one
    let events = turn("Upstream request failed: [invalid_request_error] reasoning `encrypted_content` was not issued to this caller");
    let payload = failed(&events);
    assert_eq!(payload["sessionEnded"], true);
    assert!(payload["message"].as_str().unwrap().contains("fresh session"), "{payload}");
    assert_eq!(resumed(&events), None);
}

#[test]
fn is_not_claimed_for_any_other_failure() {
    // case: is not claimed for any other failure
    let events = turn("Something else went wrong upstream.");
    let payload = failed(&events);
    assert!(payload.get("sessionEnded").is_none(), "{payload}");
    assert_eq!(resumed(&events).as_deref(), Some("ses_1"));
}
