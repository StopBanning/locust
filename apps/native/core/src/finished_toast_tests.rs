//! Electron's `finishedToast.test.ts` ("Wren finished": when a finish is worth hearing about), with its values and
//! words. Rust's `attention::finished_toast` takes what Electron's `EndedRun` holds as plain values (the window,
//! `src/ui/attention_watch.rs`, reads them off the turn's record), and Electron's `lastSaid` is
//! `carry_on::latest_final` (the last final message) with its first line taken and bounded in `finished_toast`.
use crate::attention::finished_toast;
use crate::carry_on::latest_final;
use serde_json::{Value, json};

const FOUR_MIN: u64 = 4 * 60_000;
const SAID: &str = "Moved the port to 3001.\nAll 12 tests pass.";

fn said(text: &str, last: bool) -> Value {
    json!({"type": "message.delta", "payload": {"itemId": "m", "operation": if last { "replace" } else { "append" }, "text": text, "final": last}})
}
fn toast(took: u64, last_said: Option<&str>) -> Option<(String, String)> {
    finished_toast(true, false, true, took, Some("Wren"), last_said, None)
}

#[test]
fn says_who_finished_the_first_line_of_what_they_said_and_how_long_it_took() {
    assert_eq!(toast(FOUR_MIN, Some(SAID)), Some(("Wren finished".into(), "Moved the port to 3001. · took 4 min".into())));
}

#[test]
fn stays_quiet_for_a_reply_the_person_was_very_likely_watching_arrive() {
    assert_eq!(toast(40_000, Some(SAID)), None);
}

/// The stop, the room's and the relay's run. A restored run and one with no start time are the window's: it toasts only
/// a turn this process ran (`live_turns`), and a record with no `createdAt` reads as 0 ms, under the minute.
#[test]
fn stays_quiet_for_a_stop_the_person_pressed_and_for_anything_not_theirs() {
    // cancelled, interrupted: neither completed nor failed
    assert_eq!(finished_toast(false, false, true, FOUR_MIN, Some("Wren"), Some(SAID), None), None);
    // a room member's or the relay's run: started by someone else
    assert_eq!(finished_toast(true, false, false, FOUR_MIN, Some("Wren"), Some(SAID), None), None);
}

#[test]
fn says_a_failed_run_stopped_and_why_in_its_own_first_line() {
    assert_eq!(
        finished_toast(false, true, true, FOUR_MIN, Some("Wren"), Some(SAID), Some("Copilot CLI is not signed in.\nRun copilot login.")),
        Some(("Wren stopped".into(), "Copilot CLI is not signed in. · after 4 min".into()))
    );
}

#[test]
fn reads_the_last_thing_said_in_full_never_a_fragment_still_being_written() {
    assert_eq!(latest_final(&[said("Checking.", true), said("half a sent", false)]).as_deref(), Some("Checking."));
    assert_eq!(latest_final(&[]), None);
    assert_eq!(toast(FOUR_MIN, latest_final(&[]).as_deref()).unwrap().1, "Done. · took 4 min");
}

#[test]
fn bounds_a_long_line_and_says_hours_when_it_ran_that_long() {
    let long = "x".repeat(400);
    let body = toast(FOUR_MIN, latest_final(&[said(&long, true)]).as_deref()).unwrap().1;
    let line = body.strip_suffix(" · took 4 min").unwrap();
    assert_eq!(line.chars().count(), 140);
    assert!(toast(125 * 60_000, Some(SAID)).unwrap().1.contains("took 2 h 5 min"));
}
