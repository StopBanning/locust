//! qa-0365-opencode-worktree.test.ts: a run that ended because OpenCode refused a folder says which folder, and that it
//! was outside what the run may use; an ordinary early exit still says it ended without a step.
use super::*;
use serde_json::json;

/// The measured stderr of a run that ended this way, verbatim in shape.
const REJECTED: &str = "! permission requested: external_directory (C:\\Users\\<home>\\AppData\\Local\\Temp\\locust-drive-worktree-ws-KCeRh0\\*); auto-rejecting";

#[test]
fn the_failure_names_the_directory_boundary_not_a_missing_stop() {
    // case: the failure names the directory boundary, not a missing stop
    let mut events = Normalizer::new(None);
    for record in [
        json!({"type":"step_start","part":{"type":"step-start"}}),
        json!({"type":"text","part":{"type":"text","text":"Locust files ahead - pulling your poem draft into place."}}),
        json!({"type":"tool_use","part":{"type":"tool","tool":"list","callID":"call_1","state":{"status":"completed","input":{"path":"C:\\Users\\<home>\\AppData\\Local\\Temp\\locust-drive-worktree-ws-KCeRh0"}}}}),
    ] {
        events.accept(&record);
    }
    let failure = &events.finish(false, Some(0), REJECTED, false)[0];
    assert_eq!(failure.kind, "run.failed");
    let message = failure.payload["message"].as_str().unwrap();
    let lower = message.to_lowercase();
    assert!(lower.contains("outside") || lower.contains("not allowed") || lower.contains("directory"), "{message}");
    assert!(message.contains("locust-drive-worktree-ws-KCeRh0"), "{message}");
}

#[test]
fn still_reports_an_ordinary_early_exit_as_one() {
    // case: still reports an ordinary early exit as one
    let mut events = Normalizer::new(None);
    events.accept(&json!({"type":"step_start","part":{"type":"step-start"}}));
    let failure = &events.finish(false, Some(0), "", false)[0];
    assert_eq!(failure.kind, "run.failed");
    assert!(failure.payload["message"].as_str().unwrap().contains("without a step"), "{}", failure.payload);
}
