//! Electron's `a-handoff-says-what-it-carries.test.ts` (5 cases): a reply on another runtime says what it carries
//! before it is sent.
use super::*;

fn switched(kept: &[&str], omitted: &[&str], unsettled_count: usize, task_by_file: bool, task_clipped: bool) -> HandoffPreview {
    HandoffPreview::Switch { kept: kept.iter().map(|s| (*s).to_owned()).collect(), omitted: omitted.iter().map(|s| (*s).to_owned()).collect(), unsettled_count, task_by_file, task_clipped }
}
fn line(p: HandoffPreview) -> Option<String> {
    handoff_preview_line(Some(&p))
}

// case: names each part carried, in the order it is read
#[test]
fn names_each_part_carried_in_the_order_it_is_read() {
    assert_eq!(line(switched(&["task", "earlier", "settled", "summary"], &[], 0, false, false)).as_deref(), Some("It carries the task, the earlier messages, the steps it finished and its last reply."));
}

// case: says what was left out to fit
#[test]
fn says_what_was_left_out_to_fit() {
    assert_eq!(line(switched(&["task", "settled"], &["summary", "earlier"], 0, false, false)).as_deref(), Some("It carries the task and the steps it finished. Left out to fit: its last reply and the earlier messages."));
}

// case: counts the steps that never reported back, and leaves them out of the words when there are none
#[test]
fn counts_the_steps_that_never_reported_back() {
    assert_eq!(line(switched(&["task", "unsettled"], &[], 2, false, false)).as_deref(), Some("It carries the task and 2 steps that never reported back."));
    assert_eq!(line(switched(&["task", "unsettled"], &[], 0, false, false)).as_deref(), Some("It carries the task."));
}

// case: says when the task goes as a file, or only its start fits
#[test]
fn says_when_the_task_goes_as_a_file_or_only_its_start_fits() {
    assert_eq!(line(switched(&["task"], &[], 0, true, false)).as_deref(), Some("It carries the task, as a file."));
    assert_eq!(line(switched(&["task"], &[], 0, false, true)).as_deref(), Some("It carries the start of the task."));
}

// case: says a refusal before the send, and nothing for a reply on the same runtime
#[test]
fn says_a_refusal_before_the_send_and_nothing_for_the_same_runtime() {
    let refused = HandoffPreview::Refused("That conversation is too long to carry to another runtime with this reply.".into());
    assert_eq!(line(refused).as_deref(), Some("Not sent as it is: That conversation is too long to carry to another runtime with this reply."));
    assert_eq!(line(HandoffPreview::Same), None);
    assert_eq!(handoff_preview_line(None), None);
}
