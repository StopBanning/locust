use super::*;

fn row(id: &str, state: &str, updated: &str) -> crate::engine::ConversationRow {
    crate::engine::ConversationRow { id: id.into(), state: state.into(), updated: updated.into(), title: id.into(), teammate: "Wren".into(), started_by: "the person".into() }
}

#[test]
fn are_the_finished_ones_newest_first() {
    // case: are the finished ones, newest first
    let rows = [row("older", "completed", "2026-09-08T10:00:00Z"), row("newest", "completed", "2026-09-09T10:00:00Z"), row("middle", "completed", "2026-09-09T09:00:00Z")];
    assert_eq!(savable_conversations(&rows, SAVABLE_SHOWN).iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["newest", "middle", "older"]);
}

#[test]
fn are_never_a_run_that_failed_was_cancelled_was_interrupted_or_is_still_going() {
    // case: are never a run that failed, was cancelled, was interrupted or is still going
    let rows: Vec<_> = ["completed", "failed", "cancelled", "interrupted", "running"].iter().map(|p| row(p, p, "2026-09-09T10:00:00Z")).collect();
    assert_eq!(savable_conversations(&rows, SAVABLE_SHOWN).iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["completed"]);
    for phase in ["failed", "cancelled", "interrupted", "running"] {
        assert_eq!(savable_mission_id(Some("m1"), Some(phase), false, true), None);
    }
    assert_eq!(savable_mission_id(Some("m1"), None, false, true), None);
}

#[test]
fn are_capped_so_the_empty_state_does_not_become_a_mission_list() {
    // case: are capped, so the empty state does not become a mission list
    let rows: Vec<_> = (0..20).map(|i| row(&format!("m{i}"), "completed", "2026-09-09T10:00:00Z")).collect();
    assert_eq!(SAVABLE_SHOWN, 4);
    assert_eq!(savable_conversations(&rows, SAVABLE_SHOWN).len(), 4);
}

#[test]
fn keep_a_conversation_whose_time_is_missing_or_unreadable_at_the_end() {
    // case: keep a conversation whose time is missing or unreadable, at the end
    let rows = [row("undated", "completed", ""), row("nonsense", "completed", "not a date"), row("dated", "completed", "2026-09-09T10:00:00Z")];
    assert_eq!(savable_conversations(&rows, SAVABLE_SHOWN).iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["dated", "undated", "nonsense"]);
}

#[test]
fn are_empty_when_nothing_has_finished_so_the_screen_can_say_so_honestly() {
    // case: are empty when nothing has finished, so the screen can say so honestly
    assert!(savable_conversations(&[row("running", "running", "2026-09-09T10:00:00Z")], SAVABLE_SHOWN).is_empty());
    assert!(savable_conversations(&[], SAVABLE_SHOWN).is_empty());
}

#[test]
fn count_their_turns_in_words_that_agree_with_themselves() {
    // case: count their turns in words that agree with themselves
    assert_eq!(turns_label(4), "4 turns");
    assert_eq!(turns_label(1), "1 turn");
    // Rust's absent count defaults to zero at the shelf boundary.
    assert_eq!(turns_label(0), "1 turn");
}

#[test]
fn the_header_requires_a_receipt_a_completed_phase_no_live_turn_and_a_draft() {
    assert_eq!(savable_mission_id(Some("m1"), Some("completed"), false, true), Some("m1"));
    assert_eq!(savable_mission_id(None, Some("completed"), false, true), None);
    assert_eq!(savable_mission_id(Some("pending:3"), Some("completed"), false, true), None);
    assert_eq!(savable_mission_id(Some("m1"), Some("completed"), true, true), None);
    assert_eq!(savable_mission_id(Some("m1"), Some("completed"), false, false), None);
}
