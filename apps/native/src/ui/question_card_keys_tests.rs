//! a-question-is-answered-by-number.test.ts in the window: a number key on a runtime's question card chooses that
//! option, pressed through the window's own key handler (the rule's cases are in core `question_keys_tests.rs`).
use super::*;
use locust_core::approval::{Kind, Question, QuestionOption, Request};
use rune::{Env, Runtime};

fn question(id: &str, options: &[&str], is_other: bool) -> Question {
    Question {
        id: id.into(),
        header: None,
        question: format!("Question {id}?"),
        options: options.iter().map(|o| QuestionOption { label: (*o).into(), description: None }).collect(),
        is_other,
        is_secret: false,
    }
}
fn card() -> Request {
    Request {
        approval_id: "ap_q".into(),
        mission_id: "mission_q".into(),
        runtime: "codex".into(),
        kind: Kind::Question,
        summary: "A question".into(),
        detail: String::new(),
        cwd: None,
        requested_at: "2026-11-19T22:00:00.000Z".into(),
        always_covers: None,
        always_key: None,
        data_sent_says: None,
        reversible_says: None,
        patch: None,
        skippable: false, answer_in: None, claude_input: None,
        questions: vec![question("q1", &["Postgres", "SQLite", "Keep the JSON file"], true), question("q2", &["Yes", "No"], false)],
    }
}
fn window() -> Runtime<State> {
    let mut s = State::default();
    crate::scenes::team_scene(&mut s);
    s.pending_approval = Some(card());
    let mut rt = Runtime::new(s, crate::view, (1200.0, 2000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}
fn press(rt: &mut Runtime<State>, key: &str) -> bool {
    let mut took = false;
    rt.update(|s, tx| took = crate::on_key(s, &KeyEvent::from(Key::Text(key.into())), tx));
    rt.frame(0.016);
    took
}
fn chosen(rt: &Runtime<State>, q: &str) -> Option<String> {
    rt.state.question_answers.chosen.get(q).cloned()
}

#[test]
fn a_number_chooses_that_option_of_the_first_question_still_without_one_then_the_next() {
    let mut rt = window();
    assert!(press(&mut rt, "2"));
    assert_eq!(chosen(&rt, "q1").as_deref(), Some("SQLite"));
    // The focus is now on q1, so a number answers it again rather than moving on.
    assert!(press(&mut rt, "3"));
    assert_eq!(chosen(&rt, "q1").as_deref(), Some("Keep the JSON file"));
    assert_eq!(chosen(&rt, "q2"), None);
}

#[test]
fn with_no_focus_on_the_card_a_number_goes_to_the_first_question_still_unanswered() {
    let mut rt = window();
    rt.update(|s, _| {
        s.question_answers.chosen.insert("q1".into(), "Postgres".into());
    });
    assert!(press(&mut rt, "1"));
    assert_eq!(chosen(&rt, "q2").as_deref(), Some("Yes"));
}

#[test]
fn pressing_an_option_puts_the_focus_in_its_question() {
    let mut rt = window();
    let (x, y) = rt.find("question-q2-option-1").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
    assert_eq!(chosen(&rt, "q2").as_deref(), Some("No"));
    assert!(press(&mut rt, "1"));
    assert_eq!(chosen(&rt, "q2").as_deref(), Some("Yes"));
    assert_eq!(chosen(&rt, "q1"), None);
}

#[test]
fn a_number_typed_into_the_cards_own_field_or_the_message_box_is_typing_not_a_choice() {
    let mut rt = window();
    rt.update(|s, _| {
        let mut field = TextEdit::default();
        field.focused = true;
        s.question_answers.typed.insert("q1".into(), field);
    });
    assert!(!question_card::handle_key(&mut rt.state, &KeyEvent::from(Key::Text("1".into()))));
    assert_eq!(chosen(&rt, "q1"), None);
    let mut rt = window();
    rt.state.draft.focused = true;
    assert!(!question_card::handle_key(&mut rt.state, &KeyEvent::from(Key::Text("1".into()))));
}

#[test]
fn a_number_with_a_modifier_or_past_the_options_or_with_no_question_card_is_left_alone() {
    let mut rt = window();
    let mut held = KeyEvent::from(Key::Text("1".into()));
    held.mods.cmd = true;
    assert!(!question_card::handle_key(&mut rt.state, &held));
    assert!(!question_card::handle_key(&mut rt.state, &KeyEvent::from(Key::Text("4".into()))));
    assert!(rt.state.question_answers.chosen.is_empty());
    // An approval with no questions leaves numbers to the decision card.
    let mut plain = State::default();
    let mut asked = card();
    asked.questions.clear();
    plain.pending_approval = Some(asked);
    assert!(!question_card::handle_key(&mut plain, &KeyEvent::from(Key::Text("1".into()))));
}
