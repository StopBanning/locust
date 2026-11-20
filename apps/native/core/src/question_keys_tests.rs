//! a-question-is-answered-by-number.test.ts, one Rust test per case.
use super::*;

fn questions() -> Vec<NumberKeyQuestion<'static>> {
    vec![
        NumberKeyQuestion { id: "q1", options: vec!["Postgres", "SQLite", "Keep the JSON file"] },
        NumberKeyQuestion { id: "q2", options: vec!["Yes", "No"] },
        NumberKeyQuestion { id: "q3", options: vec![] },
    ]
}
fn chosen(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs.iter().map(|(q, a)| ((*q).to_owned(), (*a).to_owned())).collect()
}
fn at(key: &str, picked: &[(&str, &str)], focused: Option<&str>) -> Option<(String, String)> {
    number_key_answer(key, false, false, &questions(), &chosen(picked), focused)
}
fn pick(q: &str, a: &str) -> Option<(String, String)> {
    Some((q.to_owned(), a.to_owned()))
}

#[test]
fn chooses_that_option_of_the_first_question_still_without_one() {
    assert_eq!(at("2", &[], None), pick("q1", "SQLite"));
    assert_eq!(at("1", &[("q1", "SQLite")], None), pick("q2", "Yes"));
}

#[test]
fn answers_the_question_the_focus_is_in_even_one_already_answered() {
    assert_eq!(at("3", &[("q1", "SQLite")], Some("q1")), pick("q1", "Keep the JSON file"));
}

#[test]
fn does_nothing_while_typing_with_a_modifier_held_past_the_options_or_for_other_keys() {
    assert_eq!(number_key_answer("1", false, true, &questions(), &HashMap::new(), None), None);
    assert_eq!(number_key_answer("1", true, false, &questions(), &HashMap::new(), None), None);
    for key in ["4", "0", "a", "Enter", "12", ""] {
        assert_eq!(at(key, &[], None), None, "{key}");
    }
}

#[test]
fn does_nothing_once_every_question_with_options_has_its_answer_unless_the_focus_says_which() {
    assert_eq!(at("1", &[("q1", "Postgres"), ("q2", "No")], None), None);
    assert_eq!(at("2", &[("q1", "Postgres"), ("q2", "Yes")], Some("q2")), pick("q2", "No"));
}

// A case the breaks asked for: a question that takes only words, coming first, does not swallow the number.
#[test]
fn a_question_with_no_options_is_passed_over_even_when_it_comes_first() {
    let qs = vec![NumberKeyQuestion { id: "q0", options: vec![] }, NumberKeyQuestion { id: "q1", options: vec!["Yes", "No"] }];
    assert_eq!(number_key_answer("2", false, false, &qs, &HashMap::new(), None), pick("q1", "No"));
    // Nor does the focus in it: the number goes to the first question it can answer.
    assert_eq!(number_key_answer("1", false, false, &qs, &HashMap::new(), Some("q0")), pick("q1", "Yes"));
}
