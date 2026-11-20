//! Electron `renderer/src/a-busy-provider-offers-continue.test.ts` (0.511): a run the provider dropped is offered
//! Continue. The rule is `failedOnProviderSide`, Electron's inputs word for word; the card and its press are in
//! `src/ui/busy_continue_tests.rs`.
//!
//! Not `carry_on::provider_failed_first`: that is Electron's `providerFailedFirst` (shared/carry-on.ts), which carries
//! a throttled or signed-out turn on to another agent and keeps its own cases in `provider_carry_on_tests.rs`.

use super::{BUSY_CONTINUE_SENTENCE, CONTINUE_PROMPT, failed_on_provider_side, offers_continue};
use serde_json::{Value, json};

fn said(message: &str) -> bool {
    failed_on_provider_side(Some(message), None)
}

// case: knows the provider was busy from what the runtime said
#[test]
fn knows_the_provider_was_busy_from_what_the_runtime_said() {
    assert!(said("Selected model is at capacity. Please try a different model."));
    assert!(said("The server is overloaded, try again later"));
    assert!(said("stream disconnected before completion: error sending request"));
    assert!(failed_on_provider_side(
        Some("Codex invocation did not complete successfully"),
        Some("ERROR: HTTP 503 Service Unavailable\n")
    ));
}

// case: and not a limit of the person's own, a sign-in, or anything else
#[test]
fn and_not_a_limit_of_the_persons_own_a_sign_in_or_anything_else() {
    assert!(!failed_on_provider_side(Some("You've hit your usage limit."), Some("ERROR: insufficient_quota\n")));
    assert!(!said("Too many requests, rate limit exceeded"));
    assert!(!said("Your access token could not be refreshed. Please sign in again."));
    assert!(!said("Codex invocation did not complete successfully"));
    assert!(!failed_on_provider_side(None, None));
    // Rust's own, beside Electron's: a limit said with a busy word is still the person's limit.
    assert!(!said("The server is overloaded: rate limit exceeded"));
}

fn failed(message: &str) -> Value {
    json!({"type": "run.failed", "payload": {"kind": "unknown", "message": message}})
}

// case: the card says it is the provider, and Continue sends into the same conversation
#[test]
fn the_card_says_it_is_the_provider_and_continue_sends_into_the_same_conversation() {
    assert_eq!(
        BUSY_CONTINUE_SENTENCE,
        "The model's servers were busy: not your account, and nothing you did. What it finished is kept; Continue picks up where it stopped."
    );
    assert_eq!(CONTINUE_PROMPT, "Continue from where you stopped.");
    // Offered for a turn that ended failed on the provider's side, after any work; never while it runs, never once a
    // later run finished, and never for a failure that is not the provider's.
    let tool = json!({"type": "tool.completed", "payload": {"toolCallId": "t1"}});
    let busy = failed("Selected model is at capacity. Please try a different model.");
    assert!(offers_continue(&[tool.clone(), busy.clone()], false));
    assert!(!offers_continue(&[tool.clone(), busy.clone()], true));
    assert!(!offers_continue(&[busy.clone(), json!({"type": "run.completed", "payload": {}})], false));
    assert!(!offers_continue(&[failed("You've hit your usage limit.")], false));
    // A turn the person stopped is not a failure, whatever its record says.
    assert!(!offers_continue(&[json!({"type": "run.cancelled", "payload": {"message": "Selected model is at capacity."}})], false));
    assert!(!offers_continue(&[], false));
}

/// The window reads it from the turn's own record: a failed tip on the provider's side, as a ledger file holds it.
#[test]
fn a_failed_tip_on_the_providers_side_offers_continue_from_its_record() {
    let dir = std::env::temp_dir().join(format!("locust-busy-continue-{}-{}", std::process::id(), line!()));
    std::fs::create_dir_all(&dir).unwrap();
    let record = |message: &str| {
        let lines = [
            json!({"schemaVersion": 27, "recordType": "mission.created", "metadata": {"missionId": "m1", "prompt": "Synthetic prompt", "runtime": "codex", "model": "sample", "createdAt": "2026-01-01T00:00:00Z"}}),
            json!({"schemaVersion": 27, "recordType": "mission.event", "event": {"sequence": 1, "occurredAt": "2026-01-01T00:00:01Z", "type": "run.started", "payload": {}}}),
            json!({"schemaVersion": 27, "recordType": "mission.event", "event": {"sequence": 2, "occurredAt": "2026-01-01T00:00:02Z", "type": "run.failed", "payload": {"kind": "unknown", "message": message}}}),
        ];
        std::fs::write(dir.join("m1.jsonl"), lines.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
        crate::ledger::Session::new(dir.clone(), "m1".into()).poll().unwrap()
    };
    let busy = record("Selected model is at capacity. Please try a different model.");
    assert!(busy.failed && busy.busy_continue);
    assert!(!record("You've hit your usage limit.").busy_continue);
    let _ = std::fs::remove_dir_all(&dir);
}
