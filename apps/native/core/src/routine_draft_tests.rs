//! routines.test.ts, "saving a conversation as a routine" and draftName, one Rust test per case.
use super::*;

fn turn(id: &str, prompt: &str, follows: Option<&str>, started_by: Option<&str>) -> Metadata {
    Metadata {
        mission_id: id.into(),
        prompt: prompt.into(),
        runtime: "cursor".into(),
        model: "composer-2.5".into(),
        parent: follows.map(str::to_owned),
        continues_reason: follows.map(|_| "follow-up".to_owned()),
        started_by_kind: started_by.map(str::to_owned),
        ..Default::default()
    }
}
fn index(missions: &[Metadata]) -> HashMap<String, Metadata> {
    missions.iter().map(|m| (m.mission_id.clone(), m.clone())).collect()
}
/// Every turn completed, unless named otherwise.
fn phases(of: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    move |id| Some(of.iter().find(|(m, _)| *m == id).map_or("completed", |(_, p)| *p).to_owned())
}

#[test]
fn is_named_after_what_the_person_typed_not_the_attachment_line() {
    let missions = [turn("m1", &crate::attachments::with_attachments("Summarise the notes.", &["notes.md".to_owned()]), None, None)];
    let draft = routine_draft(&missions[0], &index(&missions), phases(&[])).unwrap();
    assert_eq!(draft.name, "Summarise the notes.");
    // The step keeps its file, so a replay reads it again.
    assert!(draft.steps[0].contains("notes.md"));
}

#[test]
fn takes_the_words_a_person_typed_on_each_turn_in_order_with_where_each_came_from() {
    let missions = [
        turn("m1", "Read status.ts and summarise it.", None, None),
        turn("m2", "Now list every file, one per line.", Some("m1"), None),
        turn("m3", "Say done.", Some("m2"), None),
    ];
    let draft = routine_draft(&missions[2], &index(&missions), phases(&[])).unwrap();
    assert_eq!(draft.steps, ["Read status.ts and summarise it.", "Now list every file, one per line.", "Say done."]);
    assert_eq!(draft.learned_from, ["m1", "m2", "m3"]);
    assert_eq!(draft.name, "Read status.ts and summarise it.");
    assert!(!draft.truncated);
}

#[test]
fn drops_turns_the_host_wrote_whatever_started_them() {
    let missions = [
        turn("m1", "Read status.ts.", None, None),
        turn("m2", "Booty (Custom) replied to you; it is quoted below...", Some("m1"), Some("relay")),
        turn("m3", "Pick this back up from its last checkpoint...", Some("m2"), Some("resume")),
        turn("m4", "List every file.", Some("m3"), None),
    ];
    let draft = routine_draft(&missions[3], &index(&missions), phases(&[])).unwrap();
    assert_eq!(draft.steps, ["Read status.ts.", "List every file."]);
    assert_eq!(draft.learned_from, ["m1", "m4"]);
}

#[test]
fn leaves_out_a_turn_the_person_stopped_and_keeps_the_ones_that_worked() {
    let missions = [
        turn("m1", "Read status.ts.", None, None),
        turn("m2", "Delete everything in dist.", Some("m1"), None),
        turn("m3", "List every file.", Some("m2"), None),
    ];
    let draft = routine_draft(&missions[2], &index(&missions), phases(&[("m2", "cancelled")])).unwrap();
    assert_eq!(draft.steps, ["Read status.ts.", "List every file."]);
}

#[test]
fn still_offers_a_conversation_whose_every_turn_failed_less_the_ones_stopped() {
    let missions = [turn("m1", "Summarise the report.", None, None), turn("m2", "Not this one.", Some("m1"), None)];
    let draft = routine_draft(&missions[1], &index(&missions), phases(&[("m1", "failed"), ("m2", "cancelled")])).unwrap();
    assert_eq!(draft.steps, ["Summarise the report."]);
}

#[test]
fn has_nothing_to_save_when_a_person_typed_nothing_in_it() {
    let relayed = turn("m1", "Wren asked you to...", None, Some("relay"));
    assert_eq!(routine_draft(&relayed, &index(std::slice::from_ref(&relayed)), phases(&[])), None);
    let empty = turn("m1", "   ", None, None);
    assert_eq!(routine_draft(&empty, &index(std::slice::from_ref(&empty)), phases(&[])), None);
}

#[test]
fn cuts_a_long_conversation_at_the_limit_and_says_it_was_cut() {
    let missions: Vec<Metadata> = (1..=MAX_ROUTINE_STEPS + 3)
        .map(|i| {
            let follows = (i > 1).then(|| format!("m{}", i - 1));
            turn(&format!("m{i}"), &format!("step {i}"), follows.as_deref(), None)
        })
        .collect();
    let draft = routine_draft(missions.last().unwrap(), &index(&missions), phases(&[])).unwrap();
    assert_eq!(draft.steps.len(), MAX_ROUTINE_STEPS);
    assert_eq!(draft.steps.last().unwrap(), &format!("step {MAX_ROUTINE_STEPS}"));
    assert_eq!(draft.learned_from.len(), MAX_ROUTINE_STEPS);
    assert!(draft.truncated);
}

#[test]
fn names_itself_from_the_first_step_on_a_word_boundary_without_pretending_to_be_short() {
    assert_eq!(draft_name("  Read   status.ts\nand summarise  "), "Read status.ts and summarise");
    let long = draft_name("Read every file in this workspace and write a summary of what the project is for");
    assert!(long.ends_with("..."));
    assert!(long.chars().count() <= 63);
    assert!(!long.contains("  "));
    // Cut on the last space past the 24th character, never mid-word.
    assert_eq!(long, "Read every file in this workspace and write a summary of...");
}

// ---- cases the breaks asked for -----------------------------------------------------------------------------------

#[test]
fn where_some_turns_worked_only_those_are_kept() {
    let missions = [turn("m1", "Try the flaky thing.", None, None), turn("m2", "Summarise the report.", Some("m1"), None)];
    let draft = routine_draft(&missions[1], &index(&missions), phases(&[("m1", "failed")])).unwrap();
    assert_eq!(draft.steps, ["Summarise the report."]);
    assert_eq!(draft.learned_from, ["m2"]);
}

#[test]
fn a_name_with_no_space_late_enough_is_cut_where_it_reaches_the_limit() {
    let long = format!("Read {}", "x".repeat(70));
    assert_eq!(draft_name(&long), format!("Read {}...", "x".repeat(55)));
}

#[test]
fn a_turn_carried_to_another_agent_gives_the_words_the_person_typed_not_the_briefing() {
    let first = turn("m1", "Read status.ts.", None, None);
    let mut carried = turn("m2", &format!("You are continuing a conversation another agent started...{}List every file.", crate::conversation_turns::INSTRUCTION_SECTION), Some("m1"), None);
    carried.continues_reason = Some("route-switch".into());
    let missions = [first, carried];
    let draft = routine_draft(&missions[1], &index(&missions), phases(&[])).unwrap();
    assert_eq!(draft.steps.last().map(String::as_str), Some("List every file."));
    assert!(!draft.steps.iter().any(|s| s.contains("You are continuing")));
}
