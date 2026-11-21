//! Electron's `renderer/src/a-morning-row-says-what-the-run-came-to.test.tsx`, its rule: what a run that ended while
//! the person was away came to (App.tsx `awaySaid`, finishedToast.ts `lastSaid`). The drawn rows are the window's
//! (`window_drive_tests`).
use super::*;

// case: takes the answer's first line from the run's final reply
#[test]
fn takes_the_answers_first_line_from_the_runs_final_reply() {
    let events = vec![
        json!({"type": "message.delta", "payload": {"itemId": "a", "text": "Looking at the sweep.", "final": true}}),
        json!({"type": "message.delta", "payload": {"itemId": "b", "text": "\nFixed 6 checks that had gone stale.\nThe report is in POLISH-REPORT.md.", "final": true}}),
        json!({"type": "message.delta", "payload": {"itemId": "c", "text": "still typing", "final": false}}),
    ];
    assert_eq!(last_said(&events).as_deref(), Some("Fixed 6 checks that had gone stale."));
    assert_eq!(last_said(&[]), None);
}

/// Why it failed comes before anything it said, and a long line is cut at 140 with an ellipsis.
#[test]
fn a_failure_says_why_and_a_long_line_is_cut() {
    let events = vec![
        json!({"type": "message.delta", "payload": {"text": "Starting.", "final": true}}),
        json!({"type": "run.failed", "payload": {"message": "\nOpenCode's service refused the free model, so OpenCode stopped.\nDetails follow."}}),
    ];
    assert_eq!(said_of(&events).as_deref(), Some("OpenCode's service refused the free model, so OpenCode stopped."));
    let long = vec![json!({"type": "message.delta", "payload": {"text": "x".repeat(300), "final": true}})];
    let cut = said_of(&long).unwrap();
    assert_eq!(cut.chars().count(), SAID_MAX);
    assert!(cut.ends_with('…'));
    // A failure with no words falls back to the reply.
    let quiet = vec![json!({"type": "message.delta", "payload": {"text": "Done.", "final": true}}), json!({"type": "run.failed", "payload": {}})];
    assert_eq!(said_of(&quiet).as_deref(), Some("Done."));
}
