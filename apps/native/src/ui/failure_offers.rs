//! The failure card's way on (`an-ended-run-offers-the-way-on`, R17 and N11): a free model that gave up on its limit
//! offers the next free model, and a runtime that is signed out offers its own sign-in. Which failures these are is
//! `locust_core::failure_words::{offers_next_model, offers_sign_in}`, read into the ledger's snapshot.
use super::*;

/// The newest turn's runtime and model, as its record says (else the teammate's own).
fn route(s: &State) -> Option<(String, String)> {
    let pane = s.ledger.as_ref()?;
    let meta = pane.snapshot.turn_metas.last();
    let runtime = meta.map(|m| m.runtime.clone()).filter(|r| !r.is_empty()).or_else(|| pane.header.runtime.clone())?;
    let model = meta.map(|m| m.model.clone()).filter(|m| !m.is_empty()).or_else(|| pane.header.model.clone()).unwrap_or_default();
    Some((runtime, model))
}

pub(super) fn view(s: &State, p: &Palette) -> Option<El> {
    let pane = s.ledger.as_ref()?;
    if !(pane.snapshot.next_model || pane.snapshot.sign_in) || turn_running(s) || s.flight.is_some() || s.sending.is_pending() {
        return None;
    }
    let (runtime, model) = route(s)?;
    let l = crate::look::controls::Look::new(p.dark);
    // `.lc-rerun`: the sentence in muted words beside one press.
    let line = |key: &'static str, sentence: String, press: El| {
        row()
            .key(key)
            .w_full()
            .max_w(720.0)
            .px(24.0)
            .pb(12.0)
            .gap(12.0)
            .center_y()
            .child(text(sentence).font(l.ty.body).color(l.c("text-muted")).grow(1.0).shrink().wrap())
            .child(press)
    };
    if pane.snapshot.next_model {
        let models = s.models.get(&pane.header.id).into_iter().flatten().find(|a| a.runtime == runtime).map(|a| a.models.clone()).unwrap_or_default();
        let next = locust_core::route_rows::next_free_model(&runtime, Some(&model), &models, &[])?;
        let label = format!("Switch to {}", locust_core::teammate_tooltip::model_display_name(&runtime, &next.id));
        let (teammate, next_id) = (pane.header.id.clone(), next.id.clone());
        let prompt = pane.snapshot.turn_metas.last().map(|m| m.prompt.clone()).unwrap_or_default();
        let press = crate::look::controls::button(&l, &label, crate::look::controls::Tone::Neutral, false)
            .key("next-model-action")
            .on_press(move |s: &mut State| switch_model(s, &teammate, &runtime, &next_id, &prompt));
        let sentence = if pane.snapshot.next_model_down {
            locust_core::failure_words::MODEL_DOWN_SENTENCE
        } else {
            locust_core::failure_words::AT_ITS_LIMIT_SENTENCE
        };
        return Some(line("next-model", sentence.to_owned(), press));
    }
    // signInCommand: a runtime with no sign-in of its own offers none.
    locust_core::agents::sign_in_command(&runtime, false)?;
    let press = match s.agents_sign_in_rows.get(&runtime) {
        Some(crate::agents_section::SignInRow::Opened { again: false, .. }) => {
            text(locust_core::agents::sign_in_opened_line(&runtime, false)).font(type_::LABEL).color(p.muted)
        }
        _ => {
            let id = runtime.clone();
            crate::look::controls::button(&l, "Sign in", crate::look::controls::Tone::Neutral, false)
                .key("sign-in-action")
                .on_press(move |s: &mut State| crate::agents_section::sign_in(s, &id, false))
        }
    };
    Some(line("sign-in", locust_core::failure_words::signed_out_sentence(&runtime), press))
}

/// Switch to the next model: the teammate's next turn goes there, and the person's words go back in the chat box, to
/// send there (App.tsx `changeRoute` and `setHandBack`). Nothing is sent without the person.
fn switch_model(s: &mut State, teammate: &str, runtime: &str, model: &str, prompt: &str) {
    s.picks.insert(teammate.to_owned(), TurnPick { runtime: Some(runtime.to_owned()), model: model.to_owned(), effort: None, account: None });
    if !prompt.is_empty() {
        s.draft.replace_text(prompt);
        s.draft.focused = true;
    }
}

#[cfg(test)]
#[path = "model_unavailable_tests.rs"]
mod model_unavailable_tests;
