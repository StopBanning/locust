//! Exact route-switch prompt cases from missionView.test.ts.
use crate::{conversation_turns::typed_prompt, ledger::Metadata};
use std::collections::HashMap;
fn mission(id: &str, prompt: &str, parent: Option<&str>) -> Metadata {
    Metadata { mission_id: id.into(), prompt: prompt.into(), parent: parent.map(str::to_owned),
        continues_reason: parent.map(|_| "route-switch".into()), ..Default::default() }
}
fn shown(first: Metadata, next: Metadata) -> String {
    let by = HashMap::from([(first.mission_id.clone(), first), (next.mission_id.clone(), next.clone())]);
    typed_prompt(&next, &by).to_owned()
}
#[test]
fn keeps_the_new_instruction_a_handoff_was_started_for() {
    assert_eq!(shown(mission("first", "Create a module and test it", None), mission("next", "Another agent started this task.\n\nThe person now asks:\n\nRun its tests without editing.", Some("first"))), "Run its tests without editing.");
}
#[test]
fn leaves_a_persons_own_words_alone_even_when_they_contain_the_marker() {
    let typed = mission("typed", "Rename the string \"The person now asks:\" in handoff.ts and update both readers.", None);
    assert_eq!(typed_prompt(&typed, &HashMap::from([("typed".into(), typed.clone())])), "Rename the string \"The person now asks:\" in handoff.ts and update both readers.");
}
#[test]
fn does_not_read_the_marker_out_of_a_briefing_that_quotes_the_original_task() {
    assert_eq!(shown(mission("first", "Rename the string \"The person now asks:\" in handoff.ts and update both readers.", None), mission("rescue", "Another agent started this task:\n\nRename the string \"The person now asks:\" in handoff.ts and update both readers.\n\nThese actions reported finishing before the stop: none.", Some("first"))), "Rename the string \"The person now asks:\" in handoff.ts and update both readers.");
}
#[test]
fn still_shows_the_original_words_when_the_switch_carried_no_new_instruction() {
    assert_eq!(shown(mission("first", "Create a module and test it", None), mission("next", "Another agent started this task. Continue from its checkpoint.", Some("first"))), "Create a module and test it");
}
