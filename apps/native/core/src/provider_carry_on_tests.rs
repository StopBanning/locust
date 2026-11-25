//! Electron `main/a-provider-that-fails-first-is-carried-on.test.ts` (0.839): a turn whose provider, sign-in or network
//! failed before it ran one tool is carried on through the same card and the same Without asking as a used-up plan.
//! The classification cases, Electron's events and words. The carry itself is in `native::tests::provider_carry_on`.

use crate::carry_on::{carry_on_prompt, carry_on_reason, ended_on_plan_limit, provider_failed_first, stopped_for, stopped_sentence, Reason};
use serde_json::{json, Value};

pub(crate) fn failed(kind: &str, message: &str, stderr: Option<&str>) -> Value {
    let mut payload = json!({"kind": kind, "message": message});
    if let Some(stderr) = stderr {
        payload["process"] = json!({"stderr": stderr});
    }
    json!({"type": "run.failed", "sourceAdapter": "claude", "payload": payload})
}
pub(crate) fn signed_out() -> Value {
    failed("authentication-failed", "Invalid API key · Please run /login", None)
}
pub(crate) fn busy() -> Value {
    failed("unknown", "Claude Code stopped: API Error: 529 Overloaded", None)
}
pub(crate) fn offline() -> Value {
    failed("process-failed", "Codex invocation did not complete successfully", Some("starting\nError: getaddrinfo ENOTFOUND api.openai.com\n"))
}
fn throttled() -> Value {
    failed("temporary-rate-limit", "Rate limited, try again shortly", None)
}
fn refused() -> Value {
    failed("safety-blocked", "The request was refused", None)
}
fn puzzling() -> Value {
    failed("unknown", "Claude Code stopped: something went wrong", None)
}
pub(crate) fn tool() -> Value {
    json!({"type": "tool.started", "sourceAdapter": "claude", "payload": {"toolCallId": "t1", "name": "Read"}})
}
fn quota() -> Value {
    json!({"type": "run.failed", "sourceAdapter": "codex", "payload": {"kind": "quota-exhausted", "message": "You've hit your usage limit."}})
}

#[test]
fn a_provider_failure_first_is_an_expired_sign_in_a_provider_down_or_busy_a_throttle_or_no_network() {
    for events in [[signed_out()], [busy()], [offline()], [throttled()]] {
        assert!(provider_failed_first(&events), "{events:?}");
    }
    assert_eq!(carry_on_reason(&[busy()]), Some(Reason::Provider));
}

// case: is never a refusal, an error nobody can read, a run that finished, or one that had already run a tool
#[test]
fn a_provider_failure_first_is_never_a_refusal_an_unreadable_error_a_finished_run_or_one_that_ran_a_tool() {
    assert!(!provider_failed_first(&[refused()]));
    assert!(!provider_failed_first(&[puzzling()]));
    assert!(!provider_failed_first(&[busy(), json!({"type": "run.completed", "payload": {}})]));
    // Even a read: the next agent would find the turn begun, and could do it twice.
    assert!(!provider_failed_first(&[tool(), busy()]));
    assert!(!provider_failed_first(&[]));
}

// case: leaves a used-up plan to its own reason, which still carries on as it did
#[test]
fn a_used_up_plan_keeps_its_own_reason_and_only_a_stopped_turn_is_carried_on() {
    assert_eq!(carry_on_reason(&[quota()]), Some(Reason::Plan));
    assert!(!provider_failed_first(&[quota()]));
    assert_eq!(stopped_for(Some("failed"), &[busy()]), Some(Reason::Provider));
    assert_eq!(stopped_for(Some("cancelled"), &[busy()]), None);
    assert!(!ended_on_plan_limit(Some("failed"), &[busy()]));
}

#[test]
fn the_carried_turn_is_told_its_provider_failed_and_the_card_says_why() {
    assert_eq!(
        carry_on_prompt("claude", Reason::Provider),
        "Claude Code stopped on an error from its provider before it did anything. Do the task from the start, the way the task asks."
    );
    // Thread.tsx's card sentence (the renderer half of Electron's last case).
    assert!(stopped_sentence("claude", "provider").ends_with("stopped on an error before it did anything, so another agent can do the task from the start."));
}

/// Electron's last case reads the renderer's source for the seam's sentence and Settings' heading and search word; the
/// native window's are read the same way, from the files that draw them.
// case: says why on the card and on the seam, and Settings says it covers both
#[test]
fn the_seam_and_settings_say_a_failed_provider_is_carried_on_in_electrons_words() {
    let window = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src");
    let read = |rel: &str| std::fs::read_to_string(window.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
    // The card (Thread.tsx's sentence, drawn from carry_on::stopped_sentence).
    assert!(stopped_sentence("claude", "provider").ends_with("stopped on an error before it did anything, so another agent can do the task from the start."));
    assert!(read("ui/handoff_divider.rs").contains("’s provider failed before it did anything, so Locust carried the work on with {to_label}."));
    assert!(read("ui/settings_panel.rs").contains(r#"text("When an agent cannot go on").role("heading").level(2)"#));
    let pages = read("settings_pages.rs");
    let at = pages.find("\"When an agent cannot go on\",\n                &[").expect("Settings' search entry for the section");
    let words = &pages[at..at + pages[at..].find("],").expect("the entry's words end")];
    assert!(words.contains("\"provider error\""), "{words}");
}
