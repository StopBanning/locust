//! WHICH OPTION A NUMBER KEY CHOOSES (Electron `renderer/src/questionKeys.ts`, 0.801), on a runtime's question card, as
//! Claude Code's own question does: 1 to 9 choose the option of that number in the question being answered -- the one
//! the focus is in, else the first one still without a choice. Nothing while typing in a field, with a modifier held,
//! or for a number past the question's options.
use std::collections::HashMap;

/// A question as the card draws it: its id and its options' labels, in order.
pub struct NumberKeyQuestion<'a> {
    pub id: &'a str,
    pub options: Vec<&'a str>,
}

/// The question and the option label a key chooses, or None.
pub fn number_key_answer(
    key: &str,
    modified: bool,
    typing: bool,
    questions: &[NumberKeyQuestion<'_>],
    chosen: &HashMap<String, String>,
    focused: Option<&str>,
) -> Option<(String, String)> {
    if typing || modified {
        return None;
    }
    let mut chars = key.chars();
    let digit = chars.next().filter(|c| ('1'..='9').contains(c) && chars.next().is_none())?;
    let with_options: Vec<_> = questions.iter().filter(|q| !q.options.is_empty()).collect();
    let question = with_options
        .iter()
        .find(|q| Some(q.id) == focused)
        .or_else(|| with_options.iter().find(|q| !chosen.contains_key(q.id)))?;
    let label = question.options.get(digit as usize - '1' as usize)?;
    Some((question.id.to_owned(), (*label).to_owned()))
}

#[cfg(test)]
#[path = "question_keys_tests.rs"]
mod tests;
