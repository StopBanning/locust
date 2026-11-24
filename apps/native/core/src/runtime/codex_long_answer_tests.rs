//! Electron's `packages/runtime-adapters/test/codex-long-answer-keeps-its-length.test.ts`, case by case. Electron reads
//! Codex's `exec` lines; Rust runs Codex through its app server, whose reader is this `Normalizer` (codex.rs hands it
//! each notification: `self.normal.accept(method, params)`), so the answer comes as the app server says it, an
//! `item/completed` agentMessage, where Electron's comes as an `item.completed` agent_message.
use super::*;

fn answered(text: &str) -> Vec<Event> {
    Normalizer::default().accept("item/completed", &json!({"threadId":"t1","item":{"id":"a","type":"agentMessage","text":text}}))
}

#[test]
fn is_bounded_once_at_the_message_limit_and_never_at_the_evidence_limit() {
    // case: is bounded once, at the message limit, and never at the evidence limit
    let events = answered(&"x".repeat(12_000));
    let kept = events.iter().find(|e| e.kind == "message.delta").unwrap().payload["text"].as_str().unwrap().to_owned();
    assert_eq!(kept.chars().count(), 12_000);
    assert!(!kept.contains("[truncated]"));
}

#[test]
fn is_still_scrubbed_of_a_key_it_repeats() {
    // case: is still scrubbed of a key it repeats
    let token = &format!("{}-test-FAKESECRET-LOCUST-7731-abcdefghijklmnop", "sk"); // built at run time: no key-shaped literal
    let events = answered(&format!("The key is {token}."));
    let all = serde_json::to_string(&events.iter().map(|e| &e.payload).collect::<Vec<_>>()).unwrap();
    assert!(!all.contains(token));
    assert!(all.contains("[redacted]"));
}

#[test]
fn keeps_the_two_jobs_apart_secrets_have_no_size_evidence_has_one() {
    // case: keeps the two jobs apart: secrets have no size, evidence has one
    let long = "y".repeat(10_000);
    assert_eq!(redact_secrets(&long).chars().count(), 10_000);
    assert!(redact_text(&long).contains("[truncated]"));
}
