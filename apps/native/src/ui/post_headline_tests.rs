//! Electron's `post-headline.test.ts`: the line above a post. Electron's states map to native's: answered = Completed,
//! replied = Running { replied: true }, running = Running { replied: false }, failed, waiting = Queued, absent.

use super::{post_headline, MemberPostState, PostMemberStatus};

fn member(name: &str, state: &str) -> PostMemberStatus {
    let state = match state {
        "answered" => MemberPostState::Completed,
        "replied" => MemberPostState::Running { replied: true },
        "running" => MemberPostState::Running { replied: false },
        "failed" => MemberPostState::Failed,
        "waiting" => MemberPostState::Queued,
        "absent" => MemberPostState::Absent,
        other => panic!("{other}"),
    };
    PostMemberStatus { teammate_id: format!("tm_{name}"), name: name.into(), state }
}
fn many(prefix: &str, n: usize, state: &str) -> Vec<PostMemberStatus> {
    (0..n).map(|i| member(&format!("{prefix}{i}"), state)).collect()
}

#[test]
fn says_so_when_everyone_answered() {
    assert_eq!(post_headline(&many("M", 8, "answered")), "8 asked · all answered");
}

#[test]
fn names_one_or_two_and_counts_more() {
    let mut some = many("M", 6, "answered");
    some.extend([member("Otto", "running"), member("Sable", "failed")]);
    assert_eq!(post_headline(&some), "8 asked · 6 answered · Otto running · Sable failed");
    let mut lots = many("M", 5, "answered");
    lots.extend(many("F", 3, "failed"));
    assert_eq!(post_headline(&lots), "8 asked · 5 answered · 3 failed");
}

#[test]
fn counts_a_queue_separately_from_what_was_asked() {
    let mut mixed = many("A", 2, "answered");
    mixed.extend(many("R", 4, "running"));
    mixed.extend(many("W", 2, "waiting"));
    assert_eq!(post_headline(&mixed), "6 asked · 2 answered · 4 running · 2 waiting for a slot");
}

#[test]
fn writes_no_zero() {
    let said = post_headline(&[member("A", "answered"), member("B", "running")]);
    assert!(!said.contains("0 "));
    assert_eq!(said, "2 asked · 1 answered · B running");
}

#[test]
fn says_something_even_for_a_post_nothing_came_back_from() {
    assert_eq!(post_headline(&[member("A", "running"), member("B", "running")]), "2 asked · A and B running");
}

#[test]
fn never_says_all_answered_while_someone_is_still_working() {
    let both = [member("A", "replied"), member("B", "replied")];
    assert_eq!(post_headline(&both), "2 asked · A and B replied, still working");
    assert!(!post_headline(&both).contains("all answered"));
}

#[test]
fn counts_the_repliers_past_two_like_every_other_state() {
    let three = [member("A", "replied"), member("B", "replied"), member("C", "replied")];
    assert_eq!(post_headline(&three), "3 asked · 3 replied, still working");
}

#[test]
fn keeps_a_finished_one_separate_from_one_that_only_spoke() {
    assert_eq!(post_headline(&[member("A", "answered"), member("B", "replied")]), "2 asked · 1 answered · B replied, still working");
}

#[test]
fn never_calls_someone_the_post_did_not_reach_a_failure() {
    let one = [member("A", "answered"), member("B", "absent")];
    assert_eq!(post_headline(&one), "1 asked · all answered");
    assert!(!post_headline(&one).contains("failed"));
}

#[test]
fn still_counts_a_real_failure() {
    assert_eq!(post_headline(&[member("A", "answered"), member("B", "failed")]), "2 asked · 1 answered · B failed");
}
