//! Electron's `packages/runtime-adapters/test/a-free-model-refused-is-never-a-sign-in.test.ts`, through the caller:
//! the OpenCode server run (opencode_serve.rs) hands each record to `Normalizer::accept` and asks `finish` for the end,
//! with the model the turn asked for. The record is OpenCode 1.18.27's, measured 2026-10-12, while every free model
//! answered so for everyone.
use super::*;

fn measured() -> Value {
    json!({
        "type": "error",
        "timestamp": 1791817472158_u64,
        "sessionID": "ses_ecf5c461affe7hz3Cz49cHA0hJ",
        "error": {
            "name": "APIError",
            "data": {
                "message": "Upstream request failed: Invalid credential",
                "statusCode": 401,
                "isRetryable": false,
                "responseBody": json!({"error":{"type":"server_error","message":"Upstream request failed: Invalid credential"}}).to_string(),
                "metadata": { "url": "https://opencode.ai/inference/v1/chat/completions" },
            },
        },
    })
}

/// What the run said while it happened and when it ended, for a turn on `model`.
fn said(model: &str) -> (Option<String>, Option<String>) {
    let mut opencode = Normalizer::new(Some(model));
    let mut events = opencode.accept(&measured());
    events.extend(opencode.finish(false, Some(1), "", false));
    let message = |kind: &str| events.iter().find(|e| e.kind == kind).and_then(|e| e.payload["message"].as_str().map(str::to_owned));
    (message("adapter.diagnostic"), message("run.failed"))
}

#[test]
fn says_so_as_opencodes_side_while_it_happens_and_when_the_run_ends_never_a_sign_in() {
    // case: says so, as OpenCode's side, while it happens and when the run ends -- never a sign-in
    let sentence = "OpenCode's service refused the free model, so OpenCode stopped: Upstream request failed: Invalid credential. Free models need no sign-in, so this is on OpenCode's side: pick another model, or try this one again later.";
    let (diagnostic, failed) = said("opencode/space-bunny-free");
    assert_eq!(diagnostic.as_deref(), Some(sentence));
    assert_eq!(failed.as_deref(), Some(sentence));
}

#[test]
fn while_a_model_of_the_persons_own_refused_the_same_way_is_still_pointed_at_its_sign_in() {
    // case: while a model of the person's own, refused the same way, is still pointed at its sign-in
    let (_, failed) = said("anthropic/claude-sonnet-5-5");
    let failed = failed.unwrap();
    assert!(failed.ends_with("Its sign-in may have expired -- `opencode auth login` in a terminal."), "{failed}");
}
