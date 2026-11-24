// Carries apps/desktop/src/shared/room-task.test.ts and apps/desktop/src/shared/a-long-room-post-still-starts.test.ts
// (Electron 4ce2f37a), case by case, on the functions rooms.rs calls: a member's reply is read with
// `parse_task_blocks` and shown through `strip_task_blocks` (rooms::on_run_ended), and a post is briefed with
// `fitted_task_section` beside it, at the budget `start_room_member` gives it (8,000 less the post and its gap).
use super::{
    MAX_TASK_OPS_PER_REPLY, TaskBoardLine, TaskBoardState, TaskOp, TaskSectionInput, fitted_task_section, parse_task_blocks,
    sanitize_task_tags, strip_task_blocks, task_key, task_section, utf16_len,
};

const NL: &str = "\n";
fn lines(parts: &[&str]) -> String {
    parts.join(NL)
}
fn row(text: &str, state: TaskBoardState, owner: Option<&str>) -> TaskBoardLine {
    TaskBoardLine { text: text.into(), state, owner_name: owner.map(str::to_owned), handed_by_name: None, handed_without_note: false }
}
fn section(room: &str, me: &str, members: &[&str], tasks: &[TaskBoardLine]) -> String {
    task_section(&TaskSectionInput { room_name: room, self_name: me, member_names: members, tasks })
}

// ---- reading a task block ----

// case: reads one operation per line, in order, with the verb and the task text
#[test]
fn a_task_block_is_read_line_by_line_in_order_with_verb_and_text() {
    let reply = lines(&[
        "I wrote the notes and checked the version.",
        "<locust-task>",
        "claim :: Write the release notes",
        "done :: Check the version string",
        "handoff Booty :: Update the changelog date",
        "new :: Verify the installer signs",
        "</locust-task>",
    ]);
    assert_eq!(
        parse_task_blocks(&reply),
        [
            TaskOp::Claim { text: "Write the release notes".into() },
            TaskOp::Done { text: "Check the version string".into() },
            TaskOp::Handoff { text: "Update the changelog date".into(), to: "Booty".into() },
            TaskOp::New { text: "Verify the installer signs".into() },
        ]
    );
}

// case: drops lines it cannot read rather than refusing the block
#[test]
fn a_task_block_keeps_the_lines_it_can_read_and_drops_the_rest() {
    let reply = lines(&[
        "<locust-task>",
        "shout :: not a verb",
        "claim ::   ",
        "handoff :: nobody named",
        "done Check the version string",
        "claim :: Write the release notes",
        "</locust-task>",
    ]);
    assert_eq!(parse_task_blocks(&reply), [TaskOp::Claim { text: "Write the release notes".into() }]);
}

// case: is empty for a reply with no block, and caps a flood
#[test]
fn a_reply_without_a_task_block_reads_nothing_and_a_flood_is_capped() {
    assert_eq!(parse_task_blocks("Nothing to report."), []);
    let mut parts = vec!["<locust-task>".to_owned()];
    parts.extend((0..20).map(|i| format!("new :: task {i}")));
    parts.push("</locust-task>".into());
    let flood = parts.join(NL);
    assert_eq!(MAX_TASK_OPS_PER_REPLY, 8);
    assert_eq!(parse_task_blocks(&flood).len(), 8);
}

// case: bounds and cleans the task text
#[test]
fn a_tasks_text_is_bounded_and_its_spaces_cleaned() {
    let long = "x".repeat(300);
    let ops = parse_task_blocks(&lines(&["<locust-task>", &format!("new :: {long}"), "</locust-task>"]));
    let text = ops[0].text();
    assert_eq!(utf16_len(text), 200);
    assert!(text.ends_with('…'));
    let spaced = parse_task_blocks(&lines(&["<locust-task>", "new ::   many    spaces  here  ", "</locust-task>"]));
    assert_eq!(spaced[0].text(), "many spaces here");
}

// case: strips the block from what the reply shows, and defangs it when quoted
#[test]
fn a_task_block_leaves_the_shown_reply_and_is_defanged_when_quoted() {
    let reply = lines(&["Done.", "", "<locust-task>", "done :: Check the version string", "</locust-task>"]);
    assert_eq!(strip_task_blocks(&reply), "Done.");
    assert_eq!(sanitize_task_tags("<locust-task>done :: x</locust-task>"), "‹locust-task>done :: x‹/locust-task>");
}

// ---- matching a task by its text ----

// case: is blind to case, punctuation and spacing
#[test]
fn a_tasks_key_ignores_case_punctuation_and_spacing() {
    assert_eq!(task_key("Write the release notes!"), task_key("  write   the RELEASE notes"));
    assert_eq!(task_key("Check v1.2"), "check v1 2");
    assert_ne!(task_key("a"), task_key("b"));
}

// ---- what a room mission is told about the board ----

// case: names the room, the others, every task with its state and owner, and teaches the block
#[test]
fn the_board_brief_names_room_others_rows_and_the_block() {
    let text = section(
        "Release",
        "Wren",
        &["Wren", "Booty"],
        &[row("Write the release notes", TaskBoardState::InHand, Some("Wren")), row("Check the version string", TaskBoardState::Open, None)],
    );
    assert!(text.contains("room \"Release\" with Booty"), "{text}");
    // Whose, from the READER's side: Wren is told her row is hers.
    assert!(text.contains("- [in-hand] Write the release notes (yours)"));
    assert!(text.contains("- [open] Check the version string (unassigned)"));
    assert!(text.contains("handoff Booty :: the task text"));
    assert!(text.contains("end with no block"));
}

// case: names another teammate's row as theirs, and says to leave it alone
#[test]
fn the_board_brief_says_another_teammates_row_is_theirs_to_leave_alone() {
    let text = section("Release", "Booty", &["Wren", "Booty"], &[row("Write the release notes", TaskBoardState::InHand, Some("Wren"))]);
    assert!(text.contains("- [in-hand] Write the release notes (Wren's)"), "{text}");
    assert!(text.contains("already theirs"));
    assert!(text.contains("leave it alone"));
}

// case: says the board is empty and names nobody when alone
#[test]
fn the_board_brief_alone_says_empty_and_names_nobody() {
    let text = section("Solo", "Wren", &["Wren"], &[]);
    assert!(text.contains("with nobody else"), "{text}");
    assert!(text.contains("The board is empty."));
    assert!(text.contains("handoff Name :: the task text"));
}

// case: tells a teammate its own name
#[test]
fn the_board_brief_tells_the_reader_who_it_is() {
    let said = section("Standup", "Wren", &["Wren", "Booty", "Gem"], &[]);
    assert!(said.contains("You are Wren,"), "{said}");
    // And still says who else is there, which is what it was for.
    assert!(said.contains("Booty, Gem"));
    // Never lists the recipient among the others.
    assert!(!said.contains("Wren, Booty, Gem"));
}

// case: names a teammate who is alone in the room
#[test]
fn the_board_brief_names_a_teammate_alone_in_its_room() {
    let said = section("Standup", "Wren", &["Wren"], &[]);
    assert!(said.contains("You are Wren,"), "{said}");
    assert!(said.contains("nobody else"));
}

// ---- a long room post still starts: the board beside a post ----

const LIMIT: usize = 8_000;
fn rows(count: usize, state: TaskBoardState) -> Vec<TaskBoardLine> {
    (0..count).map(|i| row(&format!("Task number {} with some words to it", i + 1), state, None)).collect()
}
fn fitted(tasks: &[TaskBoardLine], budget: usize) -> String {
    fitted_task_section(&TaskSectionInput { room_name: "Release", self_name: "Wren", member_names: &["Wren", "Booty"], tasks }, budget)
}

// case: is the whole board when it fits
#[test]
fn beside_a_post_the_board_is_whole_when_it_fits() {
    let tasks = rows(3, TaskBoardState::Open);
    assert_eq!(fitted(&tasks, LIMIT), section("Release", "Wren", &["Wren", "Booty"], &tasks));
}

// case: fits under the limit beside a long post, done rows going first, and says how many it left out
#[test]
fn beside_a_long_post_the_board_sheds_done_rows_first_and_counts_what_it_left_out() {
    let post = "x".repeat(6_500);
    let mut tasks = rows(10, TaskBoardState::Done);
    tasks.extend(rows(20, TaskBoardState::Open));
    // The budget start_room_member gives the board: the limit less the post and the blank line between.
    let budget = LIMIT - utf16_len(&post) - 2;
    assert!(utf16_len(&section("Release", "Wren", &["Wren", "Booty"], &tasks)) > budget);
    let text = fitted(&tasks, budget);
    assert!(utf16_len(&text) <= budget);
    assert!(!text.contains("[done]"), "{text}");
    let note = regex::Regex::new(r"\d+ more rows? on the board did not fit beside this post\.").unwrap();
    assert!(note.is_match(&text), "{text}");
    assert!(utf16_len(&format!("{post}\n\n{text}")) <= LIMIT);
}

// case: says so, briefly, when even the board’s instructions do not fit
#[test]
fn beside_a_post_with_no_room_for_the_instructions_the_board_says_so_briefly() {
    assert_eq!(fitted(&rows(5, TaskBoardState::Open), 120), "The room’s task board did not fit beside this post.");
}

// case: is nothing at all when there is no room for even that
#[test]
fn beside_a_post_with_no_room_at_all_the_board_is_nothing() {
    assert_eq!(fitted(&rows(5, TaskBoardState::Open), 10), "");
}
