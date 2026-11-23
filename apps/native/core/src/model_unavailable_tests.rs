//! `a-free-model-that-is-down-offers-the-next.test.ts` (3 cases): OpenCode's words for a model or endpoint that is
//! unavailable, or a free model refused outright by OpenCode's service, offer the next free model; nothing else does.
use serde_json::json;

use crate::failure_words::{model_unavailable, offers_next_model, offers_next_model_down};

fn failed(message: Option<&str>) -> serde_json::Value {
    match message {
        Some(m) => json!({ "type": "run.failed", "payload": { "message": m } }),
        None => json!({ "type": "run.failed", "payload": {} }),
    }
}

// case: reads OpenCode's words for a model or endpoint that is unavailable
#[test]
fn reads_opencodes_words_for_a_model_or_endpoint_that_is_unavailable() {
    let model = "OpenCode stopped: Upstream request failed: Model is unavailable.";
    let endpoint = "OpenCode stopped: Endpoint is unavailable.";
    assert!(model_unavailable(Some(model)));
    assert!(model_unavailable(Some(endpoint)));
    let e1 = failed(Some(model));
    let e2 = failed(Some(endpoint));
    assert!(offers_next_model([&e1], false));
    assert!(offers_next_model_down([&e1], false));
    assert!(offers_next_model([&e2], false));
    assert!(offers_next_model_down([&e2], false));
}

// case: and a free model OpenCode’s service refused outright (2026-10-12, "Invalid credential" on every free model)
#[test]
fn and_a_free_model_opencodes_service_refused_outright() {
    let refused = "OpenCode's service refused the free model, so OpenCode stopped: Upstream request failed: Invalid credential. Free models need no sign-in, so this is on OpenCode's side: pick another model, or try this one again later.";
    assert!(model_unavailable(Some(refused)));
    let e_refused = failed(Some(refused));
    assert!(offers_next_model([&e_refused], false));
    assert!(offers_next_model_down([&e_refused], false));
    // A paid route's refusal is the person's sign-in, not a model that is down.
    let paid = "OpenCode was refused by the provider and stopped: Unauthorized.. Its sign-in may have expired -- `opencode auth login` in a terminal.";
    assert!(!model_unavailable(Some(paid)));
    let e_paid = failed(Some(paid));
    assert!(!offers_next_model([&e_paid], false));
    assert!(!offers_next_model_down([&e_paid], false));
}

// case: and nothing else: a retirement, a busy server or a plain failure is not it
#[test]
fn and_nothing_else_a_retirement_a_busy_server_or_a_plain_failure_is_not_it() {
    for msg in [
        Some("OpenCode stopped: Model exo-free has been deprecated."),
        Some("Selected model is at capacity. Please try a different model."),
        Some("The file is unavailable offline"),
        None,
    ] {
        assert!(!model_unavailable(msg), "{msg:?}");
        let ev = failed(msg);
        assert!(!offers_next_model([&ev], false), "{msg:?}");
        assert!(!offers_next_model_down([&ev], false), "{msg:?}");
    }
}
