//! Electron's `a-queued-message-refused-as-busy-waits.test.ts`, with Electron's values: a queued message the host
//! refuses as busy waits; it is not dropped.
use super::*;

fn row() -> QueuedRow {
    QueuedRow {
        id: "q_0_run_4".into(),
        key: "run_4".into(),
        text: "Create five.txt".into(),
        origin: Origin::Person,
        attachments: Vec::new(),
        issue: None,
        restored: false,
        held: None,
        retry_at: None,
        tries: None,
    }
}

#[test]
fn goes_back_pointed_at_the_conversation_it_was_typed_in_not_before_its_retry_time() {
    let back = retried_after_busy(&row(), "run_4", 1_000);
    assert_eq!(back.key, "run_4");
    assert_eq!(back.text, "Create five.txt");
    assert_eq!(back.tries, Some(1));
    assert_eq!(back.retry_at, Some(1_500));
}

#[test]
fn waits_longer_each_time_and_no_longer_than_eight_seconds() {
    let mut current = row();
    let mut waits = Vec::new();
    for _ in 0..7 {
        current = retried_after_busy(&current, "run_4", 0);
        waits.push(current.retry_at.unwrap());
    }
    assert_eq!(waits, [500, 1_000, 2_000, 4_000, 8_000, 8_000, 8_000]);
}

#[test]
fn keeps_its_retry_time_when_the_queue_folds_what_was_typed_behind_it_into_it() {
    let waiting = retried_after_busy(&row(), "run_4", 1_000);
    let behind = QueuedRow { id: "q_1_run_4".into(), text: "And six.txt".into(), ..row() };
    let folded = combine_queued(&[waiting, behind]);
    assert_eq!(folded[0].retry_at, Some(1_500));
    assert!(folded[0].text.contains("And six.txt"));
}

/// Electron's case 1 passes the same key as the row's; App.tsx passes the conversation it was typed in
/// (`previousKey ?? options.requeue.key`), which can differ from a row queued under a pending id.
#[test]
fn goes_back_under_the_key_it_is_given_not_the_one_it_had() {
    let back = retried_after_busy(&QueuedRow { key: "pending:1".into(), ..row() }, "run_4", 1_000);
    assert_eq!(back.key, "run_4");
}
