//! Carries apps/desktop/src/main/a-question-card-grows-with-the-reply.test.ts, first case: the sizes a person's own
//! message and a teammate's question card are set in grow at each reply size (tokens.css `--lc-text-said`,
//! `--lc-text-ask`, `--lc-text-ask-foot` and the `data-replysize` rules). The second case is drawn, in
//! src/ui/decision_card_reply_size_tests.rs.
use super::*;

// case: grows the card and the person's message at each step
#[test]
fn grows_the_card_and_the_persons_message_at_each_step() {
    let standard = ask_sizes("standard");
    let large = ask_sizes("large");
    let largest = ask_sizes("largest");
    assert_eq!(standard, AskSizes { said: 14.5, ask: 12.0, ask_foot: 10.5 });
    assert_eq!(large, AskSizes { said: 15.0, ask: 13.5, ask_foot: 11.5 });
    assert_eq!(largest, AskSizes { said: 17.0, ask: 15.0, ask_foot: 12.5 });
    let picks: [(&str, fn(AskSizes) -> f32); 3] = [("said", |a| a.said), ("ask", |a| a.ask), ("ask-foot", |a| a.ask_foot)];
    for (token, pick) in picks {
        assert!(pick(standard) > 0.0, "{token}");
        assert!(pick(large) > pick(standard), "{token}");
        assert!(pick(largest) > pick(large), "{token}");
    }
    // Anything else, as a setting that has not loaded or was written by hand, is the standard size.
    assert_eq!(ask_sizes(""), standard);
    assert_eq!(ask_sizes("huge"), standard);
}

#[test]
fn the_reply_size_chosen_is_kept_and_an_unknown_one_reads_as_standard() {
    assert_eq!(parse_settings(&serde_json::json!({})).reply_size, "standard");
    assert_eq!(parse_settings(&serde_json::json!({"replySize": "largest"})).reply_size, "largest");
    assert_eq!(parse_settings(&serde_json::json!({"replySize": "giant"})).reply_size, "standard");
}
