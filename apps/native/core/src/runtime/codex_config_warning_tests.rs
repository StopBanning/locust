// Carries packages/runtime-adapters/test/a-config-warning-says-its-summary.test.ts (Electron 4ce2f37a).
use super::{Event, events::Normalizer};
use serde_json::json;

fn said(method: &str, params: serde_json::Value) -> Vec<Event> {
    Normalizer::default().accept(method, &params)
}
fn one_warning(events: &[Event], message: &str) {
    assert_eq!(events.len(), 1, "{:?}", events.iter().map(|e| &e.payload).collect::<Vec<_>>());
    assert_eq!(events[0].kind, "adapter.diagnostic");
    let p = &events[0].payload;
    assert_eq!((p["level"].as_str(), p["code"].as_str(), p["message"].as_str(), p["terminal"].as_bool()),
        (Some("warning"), Some("app.warning"), Some(message), Some(false)));
}

#[test]
fn a_config_warning_with_only_a_summary_says_the_summary() {
    one_warning(&said("configWarning", json!({ "summary": "Model 'gpt-4' is deprecated." })), "Model 'gpt-4' is deprecated.");
}

#[test]
fn a_config_warning_with_details_says_them_on_the_line_after_the_summary() {
    one_warning(
        &said("configWarning", json!({ "summary": "Model 'gpt-4' is deprecated.", "details": "Please switch to gpt-5 in config.toml." })),
        "Model 'gpt-4' is deprecated.\nPlease switch to gpt-5 in config.toml.",
    );
}

#[test]
fn a_warning_with_a_message_still_says_it() {
    one_warning(&said("warning", json!({ "message": "Disk space is low." })), "Disk space is low.");
}

#[test]
fn a_guardian_warning_with_a_message_still_says_it() {
    one_warning(&said("guardianWarning", json!({ "message": "Guardian policy warning." })), "Guardian policy warning.");
}

#[test]
fn a_config_warning_with_a_legacy_message_still_says_it() {
    one_warning(&said("configWarning", json!({ "message": "Legacy config warning." })), "Legacy config warning.");
}

#[test]
fn a_config_warning_without_a_summary_or_message_is_dropped() {
    assert!(said("configWarning", json!({ "details": "Details without summary" })).is_empty());
    assert!(said("configWarning", json!({})).is_empty());
}

// Electron reads `summary` only on a configWarning; a warning or guardianWarning says its message.
#[test]
fn a_warning_says_its_message_not_a_summary_it_carries() {
    one_warning(&said("warning", json!({ "summary": "Not this.", "message": "Disk space is low." })), "Disk space is low.");
}
