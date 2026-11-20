//! steering.test.ts (10 cases): a message queued while a teammate works waits, goes as the next turn only after a
//! completed run that is on screen, and is held with a note otherwise; and it follows its conversation when the run is
//! re-keyed (`requeued_rows`, the list form of Electron's `requeuedTo`).
use super::*;

fn verdict(running: bool, phase: Option<&str>, on_screen: bool) -> QueuedVerdict {
    queued_verdict(VerdictInput { running, phase, on_screen, restored: false, held: None })
}

fn held(note: &str) -> QueuedVerdict {
    QueuedVerdict::Held { note: note.into() }
}

fn row(key: &str) -> QueuedRow {
    QueuedRow { id: "q1".into(), key: key.into(), text: "and then deploy it".into(), origin: Origin::Person, attachments: vec![], issue: None, restored: false, held: None, retry_at: None, tries: None }
}

// case: waits while the run is still going
#[test]
fn waits_while_the_run_is_still_going() {
    assert_eq!(verdict(true, Some("running"), true), QueuedVerdict::Waiting);
}

// case: goes as the next turn once that run completed
#[test]
fn goes_as_the_next_turn_once_the_run_completed() {
    assert_eq!(verdict(false, Some("completed"), true), QueuedVerdict::Send);
}

// case: is HELD when the run did not complete, and says which way it ended
#[test]
fn is_held_when_the_run_did_not_complete_and_says_how_it_ended() {
    assert_eq!(verdict(false, Some("failed"), true), held("held — the run failed"));
    assert_eq!(verdict(false, Some("cancelled"), true), held("held — the run was stopped"));
    assert_eq!(verdict(false, Some("interrupted"), true), held("held — the run did not finish"));
}

// case: is held when the person has moved to another conversation
#[test]
fn is_held_when_the_person_moved_to_another_conversation() {
    assert_eq!(verdict(false, Some("completed"), false), held("typed in another conversation — sending puts it here instead"));
}

// case: is held rather than sent when the run it belonged to is gone
#[test]
fn is_held_when_its_run_is_gone() {
    assert_eq!(verdict(false, None, true), held("held — that conversation is no longer open"));
}

// case: never sends on anything but a completed run that is on screen
#[test]
fn never_sends_on_anything_but_a_completed_run_on_screen() {
    let mut sends = 0;
    for running in [true, false] {
        for phase in [None, Some("starting"), Some("running"), Some("completed"), Some("failed"), Some("cancelled"), Some("interrupted")] {
            for on_screen in [true, false] {
                if verdict(running, phase, on_screen) == QueuedVerdict::Send {
                    sends += 1;
                }
            }
        }
    }
    assert_eq!(sends, 1);
}

// case: moves to the real runId when the run is re-keyed
#[test]
fn moves_to_the_real_run_id_when_re_keyed() {
    let moved = requeued_rows(&[row("start_1")], "start_1", "run_9");
    assert_eq!(moved.len(), 1);
    assert_eq!(moved[0].key, "run_9");
    assert_eq!(moved[0].text, "and then deploy it");
}

// case: leaves a message queued against another conversation alone
#[test]
fn leaves_a_message_queued_elsewhere_alone() {
    let elsewhere = row("run_other");
    assert_eq!(requeued_rows(&[elsewhere.clone()], "start_1", "run_9"), vec![elsewhere]);
}

// case: has nothing to move when nothing is queued
#[test]
fn has_nothing_to_move_when_nothing_is_queued() {
    assert!(requeued_rows(&[], "start_1", "run_9").is_empty());
}

// case: no longer reports the conversation as gone once it has followed
#[test]
fn once_followed_it_no_longer_reads_as_gone() {
    let moved = requeued_rows(&[row("start_1")], "start_1", "run_9");
    assert_eq!(verdict(true, Some("running"), moved[0].key == "run_9"), QueuedVerdict::Waiting);
}
