//! Electron's `a-refusal-is-not-a-crash.test.ts` (4 cases), with Electron's values and words: a run stopped by
//! Locust's own mode reads as the mode's refusal, not as a broken runtime. Electron finishes with exit code 1, not
//! cancelled, no output limit.
use crate::runtime::opencode_events::Normalizer;

/// The run's `run.failed` message, for this stderr.
fn said(stderr: &str) -> String {
    let mut n = Normalizer::new(Some("opencode/beam-free"));
    n.finish(false, Some(1), stderr, false)
        .into_iter()
        .find(|e| e.kind == "run.failed")
        .and_then(|e| e.payload["message"].as_str().map(str::to_owned))
        .unwrap_or_default()
}

#[test]
fn names_the_tool_the_mode_refused_not_the_missing_stop_step() {
    let message = said("! permission requested: bash (Test-Path forbidden-ask.txt); auto-rejecting\n");
    assert!(message.contains("does not allow bash"), "{message}");
    // The whole phrase: the window reads it back to offer the mode switch instead of a rerun.
    assert!(message.contains("The mode this run is in does not allow"), "{message}");
    assert!(message.contains("Nothing was changed"), "{message}");
    assert!(!message.contains("ended without a step"), "{message}");
}

#[test]
fn covers_the_other_three_tools_a_mode_can_deny() {
    for tool in ["edit", "write", "patch"] {
        let message = said(&format!("permission requested: {tool} (a.txt); auto-rejecting"));
        assert!(message.contains(&format!("does not allow {tool}")), "{tool}: {message}");
    }
}

#[test]
fn leaves_the_folder_refusal_saying_which_folder() {
    let message = said("permission requested: external_directory (C:/other/*); auto-rejecting");
    assert!(message.contains("outside the folder this run may use"), "{message}");
}

#[test]
fn still_says_the_vague_thing_when_there_is_nothing_better_to_say() {
    assert!(said("").contains("ended without a step that reported it had stopped"));
}
