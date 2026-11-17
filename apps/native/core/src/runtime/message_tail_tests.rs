// Carries packages/runtime-adapters/test/message-tail.test.ts (Electron 4ce2f37a): the cap keeps the END, where the
// protocol blocks live. `text(value, 16384)` is boundedMessageText; `text(value, 131072)` is boundedAnswerText.
use super::events::text;

const SHARE: &str = r#"<locust-share to="Gem">Watch the to= attribute.</locust-share>"#;
fn bounded_message(value: &str) -> String {
    text(value, 16_384)
}
fn bounded_answer(value: &str) -> String {
    text(value, 131_072)
}

#[test]
fn a_message_under_the_cap_is_left_exactly_as_it_is() {
    assert_eq!(bounded_message("short"), "short");
    let exact = "x".repeat(16_384);
    assert_eq!(bounded_message(&exact), exact);
}

#[test]
fn a_trailing_protocol_block_is_kept_on_an_answer_far_over_the_cap() {
    let long = format!("{}\n{SHARE}", "x".repeat(80_000));
    assert!(bounded_message(&long).contains(SHARE));
}

#[test]
fn a_bounded_message_keeps_its_beginning_too() {
    let long = format!("The short version is yes.{}\n{SHARE}", "x".repeat(80_000));
    let bounded = bounded_message(&long);
    assert!(bounded.starts_with("The short version is yes."));
    assert!(bounded.contains(SHARE));
}

#[test]
fn a_bounded_message_says_where_it_cut() {
    assert!(bounded_message(&"x".repeat(80_000)).contains("truncated"));
}

#[test]
fn a_bounded_message_still_respects_the_ledger_cap() {
    for size in [16_385, 40_000, 500_000] {
        assert!(bounded_message(&"x".repeat(size)).encode_utf16().count() <= 16_384, "{size}");
    }
}

#[test]
fn a_long_answer_of_60000_characters_is_kept_whole() {
    let report = (0..600).map(|i| format!("## Section {i}\n{}", "word ".repeat(18))).collect::<Vec<_>>().join("\n");
    assert!(report.len() > 60_000);
    assert_eq!(bounded_answer(&report), report);
    assert!(bounded_message(&report).encode_utf16().count() <= 16_384);
}

#[test]
fn an_answer_past_its_own_limit_is_bounded_keeping_the_trailing_block() {
    let huge = format!("The short version is yes.{}\n{SHARE}", "x".repeat(300_000));
    let bounded = bounded_answer(&huge);
    assert!(bounded.encode_utf16().count() <= 131_072);
    assert!(bounded.starts_with("The short version is yes."));
    assert!(bounded.contains(SHARE));
}
