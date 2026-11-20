//! Electron's `a-brief-between-teammates-arrives-whole.test.ts` (3 cases): a brief one teammate writes another
//! arrives as written up to 6,000 characters, is cut with a mark only past that, and fits the store and the brief.
use super::*;

#[test]
fn of_a_real_briefs_length_is_delivered_as_written() {
    let brief = "Inspect electron-builder.yml and discovery.ts, then prepare an optimization plan. ".repeat(40).trim().to_owned();
    assert!(brief.encode_utf16().count() > 3_000);
    assert_eq!(bounded_share_text(&brief), brief);
}

#[test]
fn is_cut_only_past_6_000_characters_and_says_so_with_a_mark() {
    let cut = bounded_share_text(&"a".repeat(MAX_SHARE_TEXT_LENGTH + 50));
    assert_eq!(cut.encode_utf16().count(), MAX_SHARE_TEXT_LENGTH);
    assert!(cut.ends_with('\u{2026}'));
}

#[test]
fn fits_what_the_store_keeps_and_what_a_teammate_is_sent() {
    assert_eq!(crate::workroom::MAX_WORKROOM_MESSAGE_LENGTH, MAX_SHARE_TEXT_LENGTH);
    // With room beside it for the briefing: a message never waits for a turn with less of the person's text in it.
    assert!(MAX_SHARE_TEXT_LENGTH <= crate::briefing::MAX_RUNTIME_PROMPT_LENGTH / 2);
}
