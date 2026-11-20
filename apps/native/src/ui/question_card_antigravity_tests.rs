//! an-antigravity-question-is-answered-here.test.tsx, drawn headless through the approval card. Native's Antigravity
//! runs through `agy -p`, which answers its own questions with "User Skipped", so no native run raises this card yet:
//! these cases hold the card for the day one does (the Antigravity app route).
use super::*;
use locust_core::approval::{Kind, Question, QuestionOption, Request};
use rune::{Env, Runtime};

fn view(s: &State, _: &mut Cx<State>) -> El {
    col().w_full().child(crate::approval_card::approval_card(s, &Palette::new(true)))
}
fn asked() -> Request {
    Request {
        approval_id: "ap_1".into(),
        mission_id: "mission_1".into(),
        runtime: "antigravity".into(),
        kind: Kind::Question,
        summary: "Asking folder organization preference".into(),
        detail: String::new(),
        cwd: Some("/work/pebble".into()),
        requested_at: "2026-09-23T04:43:04.000Z".into(),
        always_covers: None,
        always_key: None,
        data_sent_says: None,
        reversible_says: None,
        patch: None,
        skippable: false,
        answer_in: None,
        claude_input: None,
        questions: vec![Question {
            id: "0".into(),
            header: None,
            question: "How would you like to organize this folder (currently containing README.md and hello.txt)?".into(),
            options: vec![
                QuestionOption { label: "Standard Project Layout — Move documentation to `docs/` and data/source files to `src/`".into(), description: None },
                QuestionOption { label: "Flat Minimalist Layout — Retain all files at the root directory".into(), description: None },
            ],
            is_other: true,
            is_secret: false,
        }],
    }
}
fn drawn(req: Request) -> (String, Vec<rune::ElementInfo>, Runtime<State>) {
    let mut s = State::default();
    s.pending_approval = Some(req);
    let mut rt = Runtime::new(s, view, (900.0, 1200.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let all: Vec<_> = rt.elements().into_iter().filter(|e| !e.exiting).collect();
    let said = all.iter().flat_map(|e| [e.text.clone(), e.label.clone()]).flatten().collect::<Vec<_>>().join("\n");
    (said, all, rt)
}
fn buttons(all: &[rune::ElementInfo]) -> Vec<String> {
    all.iter().filter(|e| e.role == "button").map(|e| e.label.clone().or_else(|| e.text.clone()).unwrap_or_default()).collect()
}

// ---- a question Locust can answer ----

#[test]
fn names_antigravity_offers_every_option_and_a_written_answer_and_sends_or_skips() {
    let (said, all, mut rt) = drawn(Request { skippable: true, ..asked() });
    assert!(said.contains("Antigravity"), "{said}");
    assert!(said.contains("Asking folder organization preference"));
    assert!(said.contains("Standard Project Layout"));
    assert!(said.contains("Something else"));
    assert!(said.contains("Send answer"));
    assert!(buttons(&all).iter().any(|b| b == "Skip"), "{:?}", buttons(&all));
    assert!(said.contains("Skip tells it you would rather not say."));
    // Skip goes back as every question left empty; with no runtime listening here, Locust says it could not.
    let (x, y) = rt.find("skip-answers").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
    assert!(rt.state.pending_approval.is_none(), "Skip answered the card");
}

#[test]
fn offers_no_skip_where_the_runtime_takes_none() {
    let (said, all, _) = drawn(asked());
    assert!(!buttons(&all).iter().any(|b| b == "Skip"));
    assert!(said.contains("Stop the run below if you would rather not answer."));
}

// ---- a question that has to be answered in Antigravity ----

#[test]
fn shows_the_question_and_its_options_says_where_to_answer_and_has_no_buttons() {
    let (said, all, _) = drawn(Request { answer_in: Some("Antigravity".into()), ..asked() });
    assert!(said.contains("How would you like to organize this folder"));
    assert!(said.contains("Flat Minimalist Layout"));
    assert!(said.contains("Answer it in Antigravity"), "{said}");
    assert!(buttons(&all).is_empty(), "{:?}", buttons(&all));
    // Nor does a number key choose for it.
    let mut s = State::default();
    s.pending_approval = Some(Request { answer_in: Some("Antigravity".into()), ..asked() });
    assert!(!question_card::handle_key(&mut s, &KeyEvent::from(Key::Text("1".into()))));
}
