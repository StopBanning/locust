//! Electron's packages/runtime-adapters/test/provider-error.test.ts, 'provider error display sentences'.
use super::*;

const MODEL_ERROR: &str = r#"{"type":"error","error":{"message":"model 'gpt-6.1-sol' is not enabled in rustponsesapi","type":"invalid_request_error","param":null,"code":null},"status":400}"#;
const RATE: &str = r#"{"error":{"message":"Too many requests"},"status":429}"#;
const RATE_SAID: &str = "OpenAI rate limited this request. Wait a while, or pick another model and send again.";
const ACCESS_SAID: &str = "OpenAI refused this request because of sign-in or access permissions. Check the agent's sign-in and model access, then send again.";

fn codex(raw: &str) -> String {
    provider_error_sentence(raw, Some("codex"))
}

// case: reads the recorded error %s as a sentence
#[test]
fn reads_each_recorded_error_as_a_sentence() {
    assert_eq!(codex(MODEL_ERROR), "OpenAI refused the model gpt-6.1-sol: it is not enabled here right now. Pick another model and send again.");
    assert_eq!(codex(RATE), RATE_SAID);
    assert_eq!(codex(r#"{"message":"Service unavailable","status":503}"#), "OpenAI is overloaded or unavailable right now. Try again later, or pick another model.");
    assert_eq!(codex(r#"{"error":{"message":"Unauthorized"},"status":401}"#), ACCESS_SAID);
    assert_eq!(codex("The connection closed before the answer finished."), "The connection closed before the answer finished.");
}

#[test]
fn separates_missing_models_quota_exhaustion_and_access_refusal() {
    assert_eq!(codex(r#"{"error":{"message":"Model not found","code":"model_not_found"}}"#), "OpenAI refused the model: it could not be found or is not available here. Pick another model and send again.");
    assert_eq!(codex(r#"{"error":{"message":"No credits left","code":"insufficient_quota"},"status":429}"#), "OpenAI reported that its usage limit has been reached. Pick another model, or try again when usage is available.");
    assert_eq!(codex(r#"{"message":"Access denied","status":403}"#), ACCESS_SAID);
}

#[test]
fn quotes_unknown_json_messages_without_dumping_their_fields() {
    assert_eq!(
        provider_error_sentence(r#"{"message":"Request was rejected","debug":{"internal":"detail"}}"#, Some("cursor")),
        "Cursor's provider refused this request: Request was rejected. Check the model and account settings before sending again."
    );
}

#[test]
fn leaves_malformed_json_and_json_without_an_error_message_alone() {
    for raw in [r#"{"error":"#, "[]", r#"{"status":503}"#, r#""plain text""#] {
        assert_eq!(codex(raw), raw);
    }
}

#[test]
fn unwraps_an_encoded_provider_body_without_losing_its_status() {
    assert_eq!(
        provider_error_sentence(&format!("Antigravity could not run it: {RATE}"), Some("antigravity")),
        "Antigravity's provider rate limited this request. Wait a while, or pick another model and send again."
    );
    assert_eq!(codex(&serde_json::json!({ "message": RATE }).to_string()), RATE_SAID);
    assert_eq!(codex(r#"{"error":{"message":"Please wait"},"statusCode":429}"#), RATE_SAID);
}

// Beyond Electron's file: the corners its regexes and fallbacks decide, each pinned.

#[test]
fn names_each_runtimes_provider_and_a_plain_one_for_the_rest() {
    let raw = r#"{"message":"Nope"}"#;
    for (runtime, who) in [
        (Some("claude"), "Anthropic"),
        (Some("copilot"), "Copilot's provider"),
        (Some("opencode"), "OpenCode's provider"),
        (Some("antigravity"), "Antigravity's provider"),
        (Some("kilo"), "The provider"),
        (None, "The provider"),
    ] {
        assert_eq!(provider_error_sentence(raw, runtime), format!("{who} refused this request: Nope. Check the model and account settings before sending again."));
    }
}

#[test]
fn never_takes_a_verb_for_the_models_name() {
    assert_eq!(codex(r#"{"message":"model is not enabled"}"#), "OpenAI refused the model: it is not enabled here right now. Pick another model and send again.");
    assert_eq!(codex(r#"{"message":"That model does not exist"}"#), "OpenAI refused the model: it could not be found or is not available here. Pick another model and send again.");
}

#[test]
fn reads_the_kind_from_type_when_there_is_no_code_and_a_rate_limit_from_its_words() {
    assert_eq!(codex(r#"{"error":{"message":"Slow down","type":"rate_limit_error"}}"#), RATE_SAID);
    assert_eq!(codex(r#"{"message":"Too many requests, sorry"}"#), RATE_SAID);
    assert_eq!(codex(r#"{"message":"Upstream is temporarily unavailable"}"#), "OpenAI is overloaded or unavailable right now. Try again later, or pick another model.");
    assert_eq!(codex(r#"{"message":"bad","code":"unauthorized"}"#), ACCESS_SAID);
    assert_eq!(codex(r#"{"message":"Quota exceeded for today"}"#), "OpenAI reported that its usage limit has been reached. Pick another model, or try again when usage is available.");
}

#[test]
fn a_status_that_is_not_a_number_leaves_the_outer_one_standing() {
    assert_eq!(codex(r#"{"status":429,"error":{"status":"failed","message":"Please wait"}}"#), RATE_SAID);
    // A null status falls through to statusCode.
    assert_eq!(codex(r#"{"status":null,"statusCode":503,"message":"Hmm"}"#), "OpenAI is overloaded or unavailable right now. Try again later, or pick another model.");
}

#[test]
fn reads_the_message_inside_data_and_strips_the_trailing_stop() {
    assert_eq!(codex(r#"{"data":{"message":"Bad request!!"}}"#), "OpenAI refused this request: Bad request. Check the model and account settings before sending again.");
    assert_eq!(codex(r#"OpenCode stopped: {"message":"Gone."}"#), "OpenAI refused this request: Gone. Check the model and account settings before sending again.");
    assert_eq!(codex(r#"Error: {"message":"Gone"}"#), "OpenAI refused this request: Gone. Check the model and account settings before sending again.");
}

#[test]
fn a_blank_message_is_no_message_and_nesting_past_five_is_left_alone() {
    assert_eq!(codex(r#"{"message":"   "}"#), r#"{"message":"   "}"#);
    let mut deep = serde_json::json!({ "message": "Deep" });
    for _ in 0..6 {
        deep = serde_json::json!({ "error": deep });
    }
    let raw = deep.to_string();
    assert_eq!(codex(&raw), raw);
}

#[test]
fn an_inner_error_keeps_the_code_and_status_said_around_it() {
    assert_eq!(codex(r#"{"code":"insufficient_quota","error":{"message":"No credits"}}"#), "OpenAI reported that its usage limit has been reached. Pick another model, or try again when usage is available.");
    assert_eq!(codex(r#"{"error":{"message":"Nothing here","code":"model_not_found"}}"#), "OpenAI refused the model: it could not be found or is not available here. Pick another model and send again.");
}

#[test]
fn an_error_written_as_a_string_of_json_is_read_through() {
    assert_eq!(codex(&serde_json::json!({ "error": RATE }).to_string()), RATE_SAID);
}

#[test]
fn the_message_is_said_trimmed() {
    assert_eq!(codex(r#"{"message":"  Nope  "}"#), "OpenAI refused this request: Nope. Check the model and account settings before sending again.");
}

#[test]
fn an_unknown_model_is_named_and_unavailable_is_never_a_models_name() {
    assert_eq!(codex(r#"{"message":"unknown model sol-9"}"#), "OpenAI refused the model sol-9: it could not be found or is not available here. Pick another model and send again.");
    assert_eq!(codex(r#"{"message":"model unavailable: not found"}"#), "OpenAI refused the model: it could not be found or is not available here. Pick another model and send again.");
}

#[test]
fn an_invalid_api_key_is_an_access_refusal() {
    assert_eq!(codex(r#"{"message":"Invalid API key provided"}"#), ACCESS_SAID);
}

// ---- 'Codex preserves provider evidence and emits readable error messages', through each runtime's own reader -------
// Rust drives Codex over its app server only, so the exec `error` record is carried as the app server's `error`
// notification, and an exec item error that is not the end as an `error` that will be retried (as in
// codex_events_case_tests.rs).

use crate::runtime::{Event, events::Normalizer};
use serde_json::json;

const CASES: [(&str, &str); 5] = [
    (MODEL_ERROR, "OpenAI refused the model gpt-6.1-sol: it is not enabled here right now. Pick another model and send again."),
    (RATE, RATE_SAID),
    (r#"{"message":"Service unavailable","status":503}"#, "OpenAI is overloaded or unavailable right now. Try again later, or pick another model."),
    (r#"{"error":{"message":"Unauthorized"},"status":401}"#, ACCESS_SAID),
    ("The connection closed before the answer finished.", "The connection closed before the answer finished."),
];

fn app_server_error(message: &str, will_retry: bool) -> Vec<Event> {
    Normalizer::default().accept("error", &json!({ "error": { "message": message }, "willRetry": will_retry }))
}

fn of<'a>(events: &'a [Event], kind: &str) -> &'a Event {
    events.iter().find(|e| e.kind == kind).unwrap_or_else(|| panic!("no {kind} in {:?}", events.iter().map(|e| &e.kind).collect::<Vec<_>>()))
}

// case: keeps the evidence for %s
#[test]
fn keeps_the_evidence_for_each_recorded_error() {
    for (raw, expected) in CASES {
        let events = app_server_error(raw, false);
        let failed = of(&events, "run.failed");
        assert_eq!(failed.payload["message"], expected, "{raw}");
        assert_eq!(failed.payload["evidence"]["raw"]["message"], raw, "{raw}");
    }
}

#[test]
fn gives_a_direct_provider_rate_limit_warning_the_same_sentence_as_its_final_failure() {
    let direct = RATE.replace(r#"{"error""#, r#"{"type":"error","error""#);
    let events = app_server_error(&direct, false);
    assert_eq!(of(&events, "route.limit_detected").payload["message"], RATE_SAID);
    assert_eq!(of(&events, "run.failed").payload["message"], RATE_SAID);
}

#[test]
fn formats_item_errors_without_ending_a_turn_that_can_still_recover() {
    let events = app_server_error(MODEL_ERROR, true);
    let said = of(&events, "adapter.diagnostic");
    assert_eq!(said.payload["message"], CASES[0].1);
    assert_eq!(said.payload["terminal"], false);
    assert!(!events.iter().any(|e| e.kind == "run.failed"));
}

#[test]
fn formats_the_app_server_error_without_losing_the_original_notification() {
    let events = app_server_error(MODEL_ERROR, false);
    assert_eq!(of(&events, "run.failed").payload["message"], CASES[0].1);
    let all = serde_json::to_string(&events.iter().map(|e| &e.payload).collect::<Vec<_>>()).unwrap();
    assert!(all.contains("rustponsesapi"), "{all}");
}

// ---- 'other failed-result strings use the same provider sentences' -------------------------------------------------

#[test]
fn formats_claude_and_cursor_result_errors() {
    let record = json!({ "type": "result", "is_error": true, "subtype": "error", "result": RATE });
    let mut claude = crate::runtime::claude_events::ClaudeNormalizer::default();
    claude.accept(&record);
    let failed = claude.finish(false, "");
    assert_eq!(failed.kind, "run.failed");
    assert_eq!(failed.payload["message"], "Anthropic rate limited this request. Wait a while, or pick another model and send again.");
    assert_eq!(failed.payload["message"], provider_error_sentence(RATE, Some("claude")));
    assert_eq!(failed.payload["evidence"]["raw"]["result"], RATE);

    let mut cursor = crate::runtime::cursor_events::CursorNormalizer::default();
    cursor.accept_line(&record.to_string());
    let failed = cursor.finish(false, Some(1));
    assert_eq!(failed.kind, "run.failed");
    assert_eq!(failed.payload["message"], "Cursor's provider rate limited this request. Wait a while, or pick another model and send again.");
    assert_eq!(failed.payload["message"], provider_error_sentence(RATE, Some("cursor")));
    assert_eq!(failed.payload["evidence"]["raw"]["result"], RATE);
}

#[test]
fn formats_antigravity_clis_result_error_before_adding_a_runtime_prefix() {
    let mut agy = crate::runtime::agy_events::AgyNormalizer::default();
    agy.accept_line(&json!({ "event": "result", "result": { "status": "FAILED", "error": RATE } }).to_string());
    let ending = agy.finish(false, Some(1));
    let failed = of(&ending, "run.failed");
    assert_eq!(failed.payload["message"], "Antigravity's provider rate limited this request. Wait a while, or pick another model and send again.");
    assert_eq!(failed.payload["message"], provider_error_sentence(RATE, Some("antigravity")));
    assert_eq!(failed.payload["evidence"]["raw"]["result"]["error"], RATE);
}
