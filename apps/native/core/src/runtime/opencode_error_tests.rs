//! Electron's `packages/runtime-adapters/test/opencode-error.test.ts`, case by case, through the caller: the OpenCode
//! serve runtime (runtime/opencode_serve.rs) hands each record to `opencode_events::Normalizer::accept`, which reads an
//! `error` record with `ErrorFacts::of` (Electron's `openCodeErrorFacts`) and says it with `ErrorFacts::sentence`
//! (`openCodeErrorSentence`) on the run's error row.
use super::*;

/// What OpenCode emitted on 2026-09-07 (`mission_dd66e48a.jsonl`), trimmed only of headers that carry nothing.
fn measured() -> Value {
    json!({
        "type": "error",
        "timestamp": 1788821984477u64,
        "sessionID": "ses_f81e7dca6ffeAO77Dba1KsKE0g",
        "error": {
            "name": "APIError",
            "data": {
                "message": "Error from provider (Console): Rate limit exceeded. Please try again later.",
                "statusCode": 429,
                "isRetryable": true,
                "responseBody": json!({"type": "error", "error": {"type": "FreeUsageLimitError", "message": "Error from provider (Console): Rate limit exceeded."}}).to_string(),
            }
        }
    })
}

/// What the error row says for this record, on a model that is free or not.
fn row(record: Value, model: &str) -> String {
    let mut n = Normalizer::new(Some(model));
    let events = n.accept(&record);
    let said: Vec<&Event> = events.iter().filter(|e| e.kind == "adapter.diagnostic" && e.payload["code"] == "opencode.provider_error").collect();
    assert_eq!(said.len(), 1, "{:?}", events.iter().map(|e| &e.payload).collect::<Vec<_>>());
    said[0].payload["message"].as_str().unwrap().to_owned()
}

/// An `error` record carrying these facts.
fn error(message: Option<&str>, status: Option<i64>, kind: Option<&str>) -> Value {
    let mut data = json!({});
    if let Some(m) = message {
        data["message"] = json!(m);
    }
    if let Some(s) = status {
        data["statusCode"] = json!(s);
    }
    if let Some(k) = kind {
        data["type"] = json!(k);
    }
    json!({"type": "error", "error": {"name": "APIError", "data": data}})
}

const PAID: &str = "anthropic/claude-sonnet-4";
const FREE: &str = "opencode/beam-free";

fn facts(record: &Value) -> ErrorFacts {
    ErrorFacts::of(record).expect("facts")
}

#[test]
fn pulls_the_providers_own_error_type_out_of_the_response_body() {
    let facts = facts(&measured());
    assert_eq!(facts.kind.as_deref(), Some("FreeUsageLimitError"));
    assert_eq!(facts.status, Some(429));
    assert_eq!(facts.retryable, Some(true));
    assert!(facts.message.unwrap().contains("Rate limit exceeded"));
}

#[test]
fn falls_back_to_the_errors_name_when_there_is_no_body_to_read() {
    assert_eq!(facts(&json!({"type": "error", "error": {"name": "APIError", "data": {}}})).kind.as_deref(), Some("APIError"));
}

#[test]
fn survives_a_response_body_that_is_not_json() {
    let record = json!({"type": "error", "error": {"name": "APIError", "data": {"message": "boom", "statusCode": 500, "responseBody": "{\"type\":\"err"}}});
    let facts = facts(&record);
    assert_eq!(facts.kind.as_deref(), Some("APIError"));
    assert_eq!(facts.message.as_deref(), Some("boom"));
    assert_eq!(row(record, PAID), "OpenCode stopped: boom");
}

#[test]
fn answers_nothing_for_a_record_carrying_no_error_at_all() {
    assert!(ErrorFacts::of(&json!({"type": "error"})).is_none());
    let mut n = Normalizer::new(Some(PAID));
    let events = n.accept(&json!({"type": "error"}));
    assert!(events.iter().all(|e| e.payload["code"] != "opencode.provider_error"));
}

#[test]
fn names_the_free_usage_limit_in_plain_words_and_what_to_do() {
    let said = row(measured(), FREE).to_lowercase();
    assert!(said.contains("free model"), "{said}");
    assert!(said.contains("no usage left"), "{said}");
    assert!(said.contains("another model"), "{said}");
    assert!(!said.contains("without a step"), "{said}");
}

#[test]
fn says_rate_limited_and_quotes_the_provider_for_a_429_that_is_not_the_free_tier() {
    let said = row(error(Some("Too many requests in 1 minute."), Some(429), Some("RateLimitError")), PAID);
    assert!(said.to_lowercase().contains("rate limited"), "{said}");
    assert!(said.contains("Too many requests in 1 minute."), "{said}");
}

#[test]
fn points_at_sign_in_when_the_provider_refused_the_credentials() {
    for status in [401, 403] {
        let said = row(error(Some("Unauthorized"), Some(status), None), PAID);
        assert!(said.to_lowercase().contains("refused"), "{said}");
        assert!(said.contains("auth login"), "{said}");
    }
}

#[test]
fn on_a_free_model_says_the_refusal_is_opencodes_side_there_is_no_sign_in_to_renew() {
    for status in [401, 403] {
        let said = row(error(Some("Upstream request failed: Invalid credential"), Some(status), None), FREE);
        assert_eq!(said, "OpenCode's service refused the free model, so OpenCode stopped: Upstream request failed: Invalid credential. Free models need no sign-in, so this is on OpenCode's side: pick another model, or try this one again later.");
        assert!(!said.contains("auth login") && !said.contains("sign-in may have expired"), "{said}");
    }
    assert!(row(error(Some("Unauthorized."), Some(401), None), PAID).contains("auth login"));
}

#[test]
fn quotes_the_runtime_rather_than_paraphrasing_it_for_anything_else() {
    let said = row(error(Some("The model gpt-9 does not exist."), Some(404), Some("NotFoundError")), PAID);
    assert!(said.contains("The model gpt-9 does not exist."), "{said}");
}

#[test]
fn admits_it_does_not_know_rather_than_inventing_a_cause() {
    let said = row(json!({"type": "error", "error": {"data": {}}}), PAID);
    assert!(said.to_lowercase().contains("without saying what it was"), "{said}");
}

#[test]
fn never_tells_anyone_a_run_simply_stopped_for_no_reason() {
    for (record, model) in [
        (measured(), FREE),
        (error(Some("Too many requests."), Some(429), None), PAID),
        (error(Some("Unauthorized."), Some(401), None), PAID),
        (error(Some("Internal server error."), Some(500), None), PAID),
        (json!({"type": "error", "error": {"data": {}}}), PAID),
    ] {
        let said = row(record, model);
        assert!(said.chars().count() > 20, "{said}");
        assert!(!said.to_lowercase().contains("without a step that reported"), "{said}");
    }
}

#[test]
fn says_what_happened_and_what_to_do_not_what_the_provider_said() {
    let said = row(
        error(Some("Upstream request failed: [invalid_request_error] reasoning `encrypted_content` was not issued to this caller"), None, None),
        FREE,
    );
    assert!(!said.contains("encrypted_content"), "{said}");
    assert!(!said.contains("invalid_request_error"), "{said}");
    assert!(said.contains("could not continue this session"), "{said}");
    assert!(said.contains("Nothing was lost"), "{said}");
    assert!(said.contains("fresh session"), "{said}");
}

#[test]
fn leaves_every_other_provider_message_alone() {
    let said = row(error(Some("something else entirely"), None, None), PAID);
    assert!(said.contains("something else entirely"), "{said}");
}
