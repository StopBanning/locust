//! Electron's `packages/runtime-adapters/test/cursor-secrets-in-text.test.ts`, case by case, through the caller: a
//! Cursor turn whose stand-in `cursor-agent` repeats a fake `sk-` key while thinking and in its answer
//! (cursor_events_case_tests.rs `turn`). The key must be scrubbed from the text the ledger keeps, not only from evidence.
use super::cursor_events_case_tests::{lines_of, run};
use super::*;
use serde_json::json;

/// Built at run time so no key-shaped literal sits in the public source (secret scanners flag the `sk-` shape).
fn fake_key() -> String {
    format!("{}-test-FAKESECRET-LOCUST-7731-abcdefghijklmnop", "sk")
}

fn ledger(events: &[Event]) -> String {
    serde_json::to_string(&events.iter().map(|e| (&e.kind, &e.payload)).collect::<Vec<_>>()).unwrap()
}

// case: is scrubbed from the reasoning the step carries
#[test]
fn a_repeated_key_is_scrubbed_from_the_reasoning_step() {
    let key = fake_key();
    let events = run(&lines_of(&[
        json!({"type": "thinking", "subtype": "delta", "text": format!("The key {key} looks like ")}),
        json!({"type": "thinking", "subtype": "delta", "text": "a Stripe test secret."}),
        json!({"type": "thinking", "subtype": "completed"}),
    ]));
    let done = events.iter().find(|e| e.kind == "step.completed").unwrap();
    let message = done.payload["message"].as_str().unwrap_or("");
    assert!(message.contains("[redacted]"), "{message}");
    assert!(message.contains("a Stripe test secret."), "{message}");
    assert!(!ledger(&events).contains(&key));
}

// case: is scrubbed from the answer, fragment and whole alike
#[test]
fn a_repeated_key_is_scrubbed_from_every_piece_of_the_answer() {
    let key = fake_key();
    let say = |text: String, stamp: u64| json!({"type": "assistant", "message": {"content": [{"type": "text", "text": text}]}, "timestamp_ms": stamp});
    let events = run(&lines_of(&[
        say(format!("Here it is: {key}"), 1),
        say(" and that is all.".into(), 2),
        say(format!("Here it is: {key} and that is all."), 3),
    ]));
    let texts: Vec<&str> = events.iter().filter(|e| e.kind == "message.delta").map(|e| e.payload["text"].as_str().unwrap()).collect();
    assert!(!texts.is_empty());
    assert!(texts.concat().contains("[redacted]"));
    assert!(!ledger(&events).contains(&key));
}
