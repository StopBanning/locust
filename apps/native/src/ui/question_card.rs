//! A question the runtime asked, answered (Electron `ApprovalCard.tsx` QuestionCard): each question with its options
//! as choices, a field where it takes words of the person's own (no options, or `isOther`; masked when secret), and
//! Send answer once every question has an answer. It goes back as answers keyed by question id, a chosen option by
//! its label, never as an approval: a question has no session grant, so there is no Always.

use super::*;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct Answers {
    pub chosen: HashMap<String, String>,
    pub typed: HashMap<String, TextEdit>,
}

fn answers_for(s: &State, question: &approval::Question) -> Vec<String> {
    let mut values = Vec::new();
    if let Some(pick) = s.question_answers.chosen.get(&question.id) {
        values.push(pick.clone());
    }
    let free = s.question_answers.typed.get(&question.id).map(|t| t.text.trim().to_owned()).unwrap_or_default();
    if !free.is_empty() {
        values.push(free);
    }
    values
}

fn send(s: &mut State) {
    let Some(card) = s.pending_approval.clone() else { return };
    if card.questions.iter().any(|q| answers_for(s, q).is_empty()) {
        return;
    }
    let answers: Vec<(String, Vec<String>)> = card.questions.iter().map(|q| (q.id.clone(), answers_for(s, q))).collect();
    match locust_core::engine_api::api().answer(&card.approval_id, answers) {
        Ok(()) => {
            s.pending_approval = None;
            s.approval_notice = None;
            s.question_answers = Answers::default();
            s.status = "Working…".into();
            s.status_tone = StatusTone::Green;
        }
        Err(words) => {
            s.pending_approval = None;
            s.approval_notice = Some(words.clone());
            s.status = words;
            s.status_tone = StatusTone::Amber;
        }
    }
}

/// The card's questions and Send, in place of Approve and Deny.
pub(super) fn questions(s: &State, card: &approval::Request, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let mut out = col().w_full().gap(10.0).key("questions");
    for question in &card.questions {
        let mut block = col().w_full().gap(6.0).key(format!("question-{}", question.id));
        if let Some(header) = question.header.as_ref().filter(|h| !h.is_empty()) {
            block = block.child(text(header.clone()).font(type_::LABEL.mono()).color(p.muted));
        }
        block = block.child(text(question.question.clone()).font(type_::BODY).color(p.text).w_full().wrap());
        if !question.options.is_empty() {
            let mut options = col().w_full().gap(4.0).role("radiogroup").label(question.question.clone());
            for (index, option) in question.options.iter().enumerate() {
                let on = s.question_answers.chosen.get(&question.id) == Some(&option.label);
                let (qid, label) = (question.id.clone(), option.label.clone());
                let mut words = col().grow(1.0).gap(2.0).child(text(option.label.clone()).font(type_::LABEL).color(p.text).wrap());
                if let Some(desc) = option.description.as_ref().filter(|d| !d.is_empty()) {
                    words = words.child(text(desc.clone()).font(type_::LABEL).color(p.secondary).wrap());
                }
                let mut line = row()
                    .w_full()
                    .gap(8.0)
                    .px(8.0)
                    .py(6.0)
                    .radius(6.0)
                    .border(1.0, if on { p.amber } else { p.hairline })
                    .key(format!("question-{}-option-{index}", question.id))
                    .role("radio")
                    .label(option.label.clone())
                    .on_press(move |s: &mut State| {
                        s.question_answers.chosen.insert(qid.clone(), label.clone());
                    });
                if index < 9 {
                    line = line.child(text(format!("{}", index + 1)).font(type_::LABEL.mono()).color(p.muted));
                }
                line = line.child(words);
                if on {
                    line = line.fill(p.selected);
                }
                options = options.child(line);
            }
            block = block.child(options);
        }
        // Words of the person's own where the question takes them; a secret answer is never shown.
        if question.options.is_empty() || question.is_other {
            let empty = TextEdit::default();
            let typed = s.question_answers.typed.get(&question.id).unwrap_or(&empty);
            let qid = question.id.clone();
            let mut field = text_input(typed, type_::BODY)
                .key(format!("question-{}-free", question.id))
                .single_line()
                .label(if question.options.is_empty() { question.question.clone() } else { "Something else".into() })
                .placeholder(if question.options.is_empty() { "Your answer" } else { "Something else\u{2026}" }, p.muted)
                .w_full()
                .h(34.0)
                .on_edit(move |s: &mut State| s.question_answers.typed.entry(qid.clone()).or_default());
            if question.is_secret {
                field = field.secure();
            }
            block = block.child(field);
        }
        out = out.child(block);
    }
    let ready = card.questions.iter().all(|q| !answers_for(s, q).is_empty());
    let label = if card.questions.len() > 1 { "Send answers" } else { "Send answer" };
    out = out.child(row().gap(8.0).child(crate::look::controls::primary_button(&l, label, !ready).key("send-answers").on_press(send)));
    out.child(
        text("This goes back to the runtime as your answer. Stop the run below if you would rather not answer.")
            .font(type_::LABEL)
            .color(p.muted)
            .w_full()
            .wrap(),
    )
}
