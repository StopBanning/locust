//! Electron's `combine-queued.test.ts`: three thoughts typed during one run are one instruction, not three turns.

use super::*;

fn row(id: &str) -> QueuedRow {
    QueuedRow {
        id: id.into(),
        key: "run_1".into(),
        text: format!("text {id}"),
        origin: Origin::Person,
        attachments: Vec::new(),
        issue: None,
        restored: false,
        held: None,
    }
}
fn host(id: &str) -> QueuedRow {
    QueuedRow { origin: Origin::Host, ..row(id) }
}
fn with_text(id: &str, text: &str) -> QueuedRow {
    QueuedRow { text: text.into(), ..row(id) }
}
fn with_attachment(id: &str, file: &str) -> QueuedRow {
    QueuedRow { attachments: vec![file.into()], ..row(id) }
}

#[test]
fn merges_a_run_of_plain_follow_ups_in_order_separated() {
    let folded = combine_queued(&[row("a"), row("b"), row("c")]);
    assert_eq!(folded.len(), 1);
    assert_eq!(folded[0].text, ["text a", "text b", "text c"].join(QUEUE_SEPARATOR));
    // The merged row keeps the first row's identity.
    assert_eq!(folded[0].id, "a");
}

#[test]
fn leaves_a_single_row_exactly_as_it_was() {
    let one = vec![row("a")];
    assert_eq!(combine_queued(&one), one);
    assert_eq!(combine_queued(&[]), Vec::<QueuedRow>::new());
}

#[test]
fn never_merges_anything_the_host_queued() {
    let rows = vec![row("a"), host("step"), row("c")];
    assert_eq!(combine_queued(&rows), rows);
}

#[test]
fn stops_at_a_host_row_and_keeps_everything_after_it_in_place() {
    let folded = combine_queued(&[row("a"), row("b"), host("step")]);
    assert_eq!(folded.len(), 2);
    assert_eq!(folded[0].text, ["text a", "text b"].join(QUEUE_SEPARATOR));
    assert_eq!(folded[1].id, "step");
}

#[test]
fn a_host_row_at_the_front_blocks_the_merge_without_eating_the_rest() {
    let rows = vec![host("step"), row("b"), row("c")];
    assert_eq!(combine_queued(&rows), rows);
}

#[test]
fn lets_the_front_carry_attachments_and_refuses_them_on_a_follower() {
    assert!(can_merge_front(&with_attachment("a", "a.png")));
    assert!(!can_merge_follower(&with_attachment("b", "b.png")));
    let folded = combine_queued(&[with_attachment("a", "a.png"), with_attachment("b", "b.png"), row("c")]);
    assert_eq!(folded.len(), 3);
    assert_eq!(folded[0].attachments, vec!["a.png".to_owned()]);
}

#[test]
fn an_empty_or_blank_row_merges_nothing() {
    assert!(!can_merge_front(&with_text("a", "   ")));
    let rows = vec![with_text("a", ""), row("b")];
    assert_eq!(combine_queued(&rows), rows);
}

#[test]
fn trims_each_part_but_keeps_the_words() {
    let folded = combine_queued(&[with_text("a", "  first  "), with_text("b", "\nsecond\n")]);
    assert_eq!(folded[0].text, format!("first{QUEUE_SEPARATOR}second"));
}
