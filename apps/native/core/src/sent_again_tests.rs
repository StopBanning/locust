//! `an-ended-run-offers-the-way-on.test.tsx`, the logic under its cases (the drawn cases are in
//! src/ui/sent_again_tests.rs): which stopped turn gives way to the one sent again, which says it was stopped before
//! it replied, when a fresh session is worth saying, and a free model read as at its limit from its last word.
use crate::failure_words::failed_on_its_limit;
use crate::ledger::{Item, Transcript};
use crate::sent_again::{a_turn_before_is_shown, given_way, sent_again_by, stopped_before_saying};
use serde_json::{json, Value};

fn turn(prompt: &str, events: &[(&str, Value)]) -> Vec<Item> {
    let records: Vec<Value> = events
        .iter()
        .enumerate()
        .map(|(n, (kind, payload))| json!({ "event": { "id": format!("evt_{n}"), "runId": "run_1", "missionId": "m", "sequence": n + 1,
            "occurredAt": "2026-09-29T12:00:00.000Z", "sourceAdapter": "claude", "type": kind, "payload": payload } }))
        .collect();
    let mut transcript = Transcript::default();
    transcript.apply(records);
    std::iter::once(Item::User(prompt.into())).chain(transcript.items).collect()
}
fn stopped_early(prompt: &str) -> Vec<Item> {
    turn(prompt, &[("run.started", json!({})), ("run.cancelled", json!({}))])
}
fn spoke(prompt: &str) -> Vec<Item> {
    turn(prompt, &[("run.started", json!({})), ("message.delta", json!({ "itemId": "m1", "text": "Starting", "final": true })), ("run.cancelled", json!({}))])
}
fn answered(prompt: &str) -> Vec<Item> {
    turn(prompt, &[("run.started", json!({})), ("message.delta", json!({ "itemId": "m1", "text": "Started.", "final": true })), ("run.completed", json!({}))])
}

// case: draws the message once: the stopped copy gives way to the one sent again
#[test]
fn draws_the_message_once_the_stopped_copy_gives_way_to_the_one_sent_again() {
    let items = [stopped_early("Summarize the turns"), answered("Summarize the turns")].concat();
    assert_eq!(given_way(&items), [(0, stopped_early("Summarize the turns").len())]);
    assert!(sent_again_by("Summarize the turns", &stopped_early("x")[1..], Some(" Summarize the turns ")));
    // Words that say nothing are never the same message sent again.
    assert!(!sent_again_by("  ", &stopped_early("x")[1..], Some("  ")));
}

// case: and says no fresh session began when nothing before it is lost
#[test]
fn and_says_no_fresh_session_began_when_nothing_before_it_is_lost() {
    assert!(!a_turn_before_is_shown(&[stopped_early("Summarize the turns"), answered("Summarize the turns")].concat()));
    assert!(a_turn_before_is_shown(&[answered("Start here"), stopped_early("Summarize the turns"), answered("Summarize the turns")].concat()));
    assert!(!a_turn_before_is_shown(&answered("Only one")));
}

// case: keeps a stopped turn that said something, or that was followed by other words
#[test]
fn keeps_a_stopped_turn_that_said_something_or_that_was_followed_by_other_words() {
    assert!(given_way(&[spoke("Summarize the turns"), answered("Summarize the turns")].concat()).is_empty());
    assert!(given_way(&[stopped_early("Summarize the turns"), answered("Something else")].concat()).is_empty());
    // Nothing comes after the newest turn, so it never gives way.
    assert!(given_way(&stopped_early("Summarize the turns")).is_empty());
}

// case: says a kept turn that was stopped before it replied was stopped, and only that one
#[test]
fn says_a_kept_turn_that_was_stopped_before_it_replied_was_stopped_and_only_that_one() {
    assert!(stopped_before_saying(&stopped_early("a")[1..]));
    assert!(!stopped_before_saying(&spoke("a")[1..]));
    assert!(!stopped_before_saying(&answered("a")[1..]));
    let ran = turn("a", &[("run.started", json!({})), ("tool.started", json!({ "itemId": "c1", "toolKind": "command_execution", "name": "Read", "command": "a.txt" })), ("run.cancelled", json!({}))]);
    assert!(!stopped_before_saying(&ran[1..]));
}

// case: is read as its limit from the runtime's last word
#[test]
fn is_read_as_its_limit_from_the_runtimes_last_word() {
    let said = r#"timestamp=2026-09-29T03:40:47.596Z level=ERROR message="stream error" error.error="AI_APICallError: Rate limit exceeded""#;
    assert!(failed_on_its_limit(Some(said)));
    assert!(failed_on_its_limit(Some(&format!("starting\n{said}\n\n"))));
    assert!(!failed_on_its_limit(Some("Error: ENOENT: no such file")));
    assert!(!failed_on_its_limit(None));
    // The LAST word: a limit said earlier and then something else is not this.
    assert!(!failed_on_its_limit(Some(&format!("{said}\nError: ENOENT: no such file"))));
}

fn failed(payload: Value) -> Value {
    json!({ "type": "run.failed", "payload": payload })
}

// case: offers the next model on the failure card
// (the reading under it: a turn that ended failed on its limit; the card and its press are in src/ui/sent_again_tests.rs)
#[test]
fn offers_the_next_model_on_the_failure_card_when_the_turn_ended_on_its_limit() {
    use crate::failure_words::{offers_continue, offers_next_model, AT_ITS_LIMIT_SENTENCE};
    let said = r#"timestamp=2026-09-29T03:40:47.596Z level=ERROR message="stream error" error.error="AI_APICallError: Rate limit exceeded""#;
    let limit = failed(json!({ "message": "OpenCode ended. The runtime reported that it is out of capacity right now.", "process": { "stderr": said } }));
    assert!(offers_next_model(&[json!({ "type": "run.started", "payload": {} }), limit.clone()], false));
    // Never while it runs, never once a later run ended otherwise, never for another failure.
    assert!(!offers_next_model(&[limit.clone()], true));
    assert!(!offers_next_model(&[limit.clone(), json!({ "type": "run.started", "payload": {} })], false));
    assert!(!offers_next_model(&[failed(json!({ "message": "x", "process": { "stderr": "Error: ENOENT: no such file" } }))], false));
    assert!(!offers_next_model(&[failed(json!({ "message": "x" }))], false));
    // A limit is not the provider being busy: Continue is not offered beside it.
    assert!(!offers_continue(&[limit], false));
    assert!(AT_ITS_LIMIT_SENTENCE.contains("puts your message back in the chat box"));
}

// case: offers that runtime's sign-in on the failure card
// (the reading under it: a turn that ended failed signed out; the card and its press are in src/ui/sent_again_tests.rs)
#[test]
fn offers_that_runtimes_sign_in_when_the_turn_ended_signed_out() {
    use crate::failure_words::{offers_sign_in, signed_out_sentence};
    let out = failed(json!({ "kind": "authentication-failed", "message": "Your access token could not be refreshed. Please log out and sign in again." }));
    assert!(offers_sign_in(&[out.clone()], false));
    assert!(!offers_sign_in(&[out.clone()], true));
    assert!(!offers_sign_in(&[out, json!({ "type": "run.completed", "payload": {} })], false));
    assert_eq!(signed_out_sentence("codex"), "Codex CLI is signed out. Sign in again, then send your message.");
}

// case: offers nothing of the kind otherwise
#[test]
fn offers_no_sign_in_for_any_other_failure() {
    use crate::failure_words::offers_sign_in;
    assert!(!offers_sign_in(&[failed(json!({ "kind": "unknown", "message": "The model returned an unexpected response." }))], false));
    assert!(!offers_sign_in(&[json!({ "type": "run.cancelled", "payload": { "kind": "authentication-failed" } })], false));
    assert!(!offers_sign_in(&[], false));
}

/// The window reads both from the turn's own record, as a ledger file holds it.
#[test]
fn a_failed_tip_reads_its_way_on_from_its_record() {
    let dir = std::env::temp_dir().join(format!("locust-way-on-{}-{}", std::process::id(), line!()));
    std::fs::create_dir_all(&dir).unwrap();
    let record = |payload: Value| {
        let lines = [
            json!({"schemaVersion": 27, "recordType": "mission.created", "metadata": {"missionId": "m1", "prompt": "Synthetic prompt", "runtime": "opencode", "model": "opencode/beam-free", "createdAt": "2026-01-01T00:00:00Z"}}),
            json!({"schemaVersion": 27, "recordType": "mission.event", "event": {"sequence": 1, "occurredAt": "2026-01-01T00:00:01Z", "type": "run.started", "payload": {}}}),
            json!({"schemaVersion": 27, "recordType": "mission.event", "event": {"sequence": 2, "occurredAt": "2026-01-01T00:00:02Z", "type": "run.failed", "payload": payload}}),
        ];
        std::fs::write(dir.join("m1.jsonl"), lines.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
        crate::ledger::Session::new(dir.clone(), "m1".into()).poll().unwrap()
    };
    let limit = record(json!({"kind": "unknown", "message": "OpenCode ended.", "process": {"stderr": "Rate limit exceeded"}}));
    assert!(limit.next_model && !limit.sign_in);
    let out = record(json!({"kind": "authentication-failed", "message": "Please sign in again."}));
    assert!(out.sign_in && !out.next_model);
    let other = record(json!({"kind": "unknown", "message": "The model returned an unexpected response."}));
    assert!(!other.sign_in && !other.next_model);
    std::fs::remove_dir_all(&dir).ok();
}
