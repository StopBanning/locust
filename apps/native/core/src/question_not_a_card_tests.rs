// Carries apps/desktop/src/shared/a-question-that-is-not-a-card-stays-in-the-reply.test.ts (Electron 4ce2f37a). The
// it.each's four shapes are a test each.
use crate::protocol_tags::{parse_decision, strip_decision_blocks};

fn ask(body: &str) -> String {
    format!("<locust-ask>\n{body}\n</locust-ask>")
}

#[test]
fn an_ask_block_that_became_the_card_is_taken_out_of_the_reply() {
    let reply = format!("I need a decision.\n\n{}", ask("Which database?\n- Postgres :: what the team knows\n- SQLite :: nothing to run"));
    assert_eq!(parse_decision(&reply).map(|d| d.options.len()), Some(2));
    assert_eq!(strip_decision_blocks(&reply), "I need a decision.");
}

fn stays_as_plain_words(body: &str) {
    let reply = format!("I need a decision.\n\n{}", ask(body));
    assert!(parse_decision(&reply).is_none(), "{body}");
    let shown = strip_decision_blocks(&reply);
    assert!(shown.contains(body.split('\n').next().unwrap()), "{shown}");
    assert!(!shown.contains("locust-ask"), "{shown}");
    assert!(!shown.contains("::"), "{shown}");
    assert_eq!(shown.split('\n').filter(|l| l.starts_with("- ")).count(), body.split('\n').count() - 1, "{shown}");
}

#[test]
fn an_ask_block_the_card_rules_refused_for_one_option_stays_as_plain_words() {
    stays_as_plain_words("Ship it now?\n- Ship :: today");
}

#[test]
fn an_ask_block_the_card_rules_refused_for_five_options_stays_as_plain_words() {
    stays_as_plain_words("Which colour?\n- Red\n- Blue\n- Green\n- Gold\n- Grey");
}

#[test]
fn an_ask_block_the_card_rules_refused_for_yes_and_no_stays_as_plain_words() {
    stays_as_plain_words("Delete the old branch?\n- Yes :: it is merged\n- No :: keep it");
}

#[test]
fn an_ask_block_the_card_rules_refused_for_the_same_option_twice_stays_as_plain_words() {
    stays_as_plain_words("Which one?\n- Postgres\n- postgres");
}

#[test]
fn an_ask_block_after_the_first_stays_too() {
    let reply = ["Two things.".to_owned(), ask("Which database?\n- Postgres\n- SQLite"), ask("Which host?\n- Fly :: cheaper\n- Render :: simpler")].join("\n\n");
    assert_eq!(parse_decision(&reply).map(|d| d.question), Some("Which database?".to_owned()));
    assert_eq!(strip_decision_blocks(&reply), "Two things.\n\nWhich host?\n- Fly — cheaper\n- Render — simpler");
}

#[test]
fn an_ask_block_shown_as_an_example_in_code_is_left_as_written() {
    let reply = format!("Write it like this:\n\n```\n{}\n```", ask("Which?\n- A\n- B"));
    assert_eq!(strip_decision_blocks(&reply), reply);
}
