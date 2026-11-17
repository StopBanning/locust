// Carries apps/desktop/src/shared/claim-what-you-start.test.ts and the-named-teammate-claims.test.ts (Electron 4ce2f37a).
use crate::room_task::{
    ClaimCandidateTask, RosterNameRef, TaskBoardLine, TaskBoardState, TaskSectionInput, member_named_by_post, row_to_claim_at_start, task_section,
};

fn section() -> String {
    let tasks = [
        TaskBoardLine { text: "Review Locust codebase".into(), state: TaskBoardState::Open, owner_name: None, handed_by_name: None, handed_without_note: false },
        TaskBoardLine { text: "Write the release notes".into(), state: TaskBoardState::InHand, owner_name: Some("Jimothy".into()), handed_by_name: None, handed_without_note: false },
    ];
    task_section(&TaskSectionInput { room_name: "test", self_name: "Yurt", member_names: &["Yurt", "Jimothy"], tasks: &tasks })
}

#[test]
fn the_board_tells_a_teammate_to_claim_a_row_it_begins() {
    let text = section();
    assert!(text.contains("claim it in the same reply"), "{text}");
    assert!(text.contains("because the person asked you to"), "{text}");
}

#[test]
fn the_board_says_why_a_row_is_claimed() {
    assert!(section().contains("look the same to everyone else"));
}

#[test]
fn the_board_keeps_the_limit_that_stops_a_claim_on_what_is_not_being_done() {
    let text = section();
    assert!(text.contains("Claim only what you are actually doing"), "{text}");
    assert!(text.contains("A row marked with another teammate's name is already theirs"), "{text}");
}

#[test]
fn the_board_still_says_to_end_with_no_block_when_no_task_was_touched() {
    assert!(section().contains("if you touched no task, end with no block"));
}

const ROWS: [ClaimCandidateTask<'static>; 2] = [
    ClaimCandidateTask { task_id: "t1", text: "Write the release notes", state: "open", owner_id: None },
    ClaimCandidateTask { task_id: "t2", text: "Draft the changelog", state: "open", owner_id: None },
];
const MEMBERS: [RosterNameRef<'static>; 2] = [RosterNameRef { teammate_id: "wren", name: "Wren" }, RosterNameRef { teammate_id: "booty", name: "Booty" }];
const NAMED: &str = "Wren, start Write the release notes. Booty, reply OK and do not touch the board.";

#[test]
fn a_named_row_is_claimed_for_the_teammate_the_post_names() {
    assert_eq!(row_to_claim_at_start(NAMED, &ROWS, &MEMBERS, "wren"), Some("t1"));
}

#[test]
fn a_named_row_is_claimed_for_nobody_else_however_early_they_start() {
    assert_eq!(row_to_claim_at_start(NAMED, &ROWS, &MEMBERS, "booty"), None);
}

#[test]
fn a_post_that_names_no_row_claims_nothing() {
    assert_eq!(row_to_claim_at_start("Wren, start the unassigned board task.", &ROWS, &MEMBERS, "wren"), None);
}

#[test]
fn a_row_named_with_no_teammate_named_claims_nothing() {
    assert_eq!(row_to_claim_at_start("Somebody please Write the release notes today.", &ROWS, &MEMBERS, "wren"), None);
}

#[test]
fn a_room_of_one_needs_no_name_to_claim_a_row() {
    assert_eq!(row_to_claim_at_start("Please Write the release notes.", &ROWS, &[RosterNameRef { teammate_id: "wren", name: "Wren" }], "wren"), Some("t1"));
}

#[test]
fn a_member_is_named_by_whole_words_only() {
    assert_eq!(member_named_by_post("tighten the wrench first", &MEMBERS), None);
    assert_eq!(member_named_by_post("wren, go", &MEMBERS), Some("wren"));
}

#[test]
fn a_post_naming_two_members_names_none() {
    assert_eq!(member_named_by_post("Wren and Booty, both of you", &MEMBERS), None);
}

#[test]
fn a_member_with_a_name_of_several_words_is_named() {
    let members = [RosterNameRef { teammate_id: "vg", name: "Van Gogh" }, RosterNameRef { teammate_id: "booty", name: "Booty" }];
    assert_eq!(member_named_by_post("ask Van Gogh to start", &members), Some("vg"));
}

// Rust's own (room-task.ts has both; no Electron case reaches them).

#[test]
fn a_post_that_names_two_rows_claims_neither() {
    assert_eq!(row_to_claim_at_start("Wren, Write the release notes and Draft the changelog.", &ROWS, &MEMBERS, "wren"), None);
}

#[test]
fn a_row_whose_sentence_names_nobody_is_claimed_for_the_one_teammate_the_post_names() {
    assert_eq!(row_to_claim_at_start("Wren, a job for you. Write the release notes.", &ROWS, &MEMBERS, "wren"), Some("t1"));
}
