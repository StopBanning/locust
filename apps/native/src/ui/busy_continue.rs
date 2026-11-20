//! A run the provider dropped is offered Continue (`a-busy-provider-offers-continue`, 0.511).
//!
//! A run that did real work and then hit "Selected model is at capacity" is carried on, not repeated: Continue sends a
//! follow-up into the same conversation, which picks up the runtime's own session where it stopped. The press is the
//! person's send; nothing goes without it. Which failures are the provider's is
//! `locust_core::failure_words::failed_on_provider_side`.
use super::*;

pub(super) fn view(s: &State, p: &Palette) -> Option<El> {
    let pane = s.ledger.as_ref()?;
    if !pane.snapshot.busy_continue || turn_running(s) || s.flight.is_some() || s.sending.is_pending() {
        return None;
    }
    let l = crate::look::controls::Look::new(p.dark);
    // `.lc-rerun`: the sentence in muted words beside one press.
    Some(
        row()
            .key("busy-continue")
            .w_full()
            .max_w(720.0)
            .px(24.0)
            .pb(12.0)
            .gap(12.0)
            .center_y()
            .child(
                text(locust_core::failure_words::BUSY_CONTINUE_SENTENCE)
                    .font(l.ty.body)
                    .color(l.c("text-muted"))
                    .grow(1.0)
                    .shrink()
                    .wrap(),
            )
            .child(
                crate::look::controls::button(&l, "Continue", crate::look::controls::Tone::Neutral, false)
                    .key("busy-continue-action")
                    .on_press_tx(continue_after_busy),
            ),
    )
}

/// Continue: Locust's words sent into the same conversation, the person's draft kept for after.
pub(super) fn continue_after_busy(s: &mut State, tx: &mut Tx) {
    let draft = s.draft.text.clone();
    s.draft.replace_text(locust_core::failure_words::CONTINUE_PROMPT);
    if !can_send(s) {
        s.draft.replace_text(draft);
        return;
    }
    send(s, tx);
    s.draft.replace_text(draft);
}

#[cfg(test)]
#[path = "busy_continue_tests.rs"]
mod busy_continue_tests;
