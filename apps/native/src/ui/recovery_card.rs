//! The interrupted-run card. It never offers a Resume control: there is no reconciled checkpoint execution yet.
use super::*;
use crate::recovery::Offer;

pub(super) fn card(s: &State, p: &Palette) -> Option<El> {
    if s.flight.is_some() {
        return None;
    }
    let pane = s.ledger.as_ref()?;
    let offer = pane.snapshot.recovery.as_ref()?;
    let mut card = col()
        .w_full()
        .gap(8.0)
        .padding(12.0)
        .radius(9.0)
        .fill(p.raised);
    match offer {
        Offer::Refused(note) => {
            card = card
                .child(text("Cannot be resumed from here").color(p.text))
                .child(text(note.clone()).color(p.secondary).wrap_anywhere())
        }
        Offer::Resume {
            epoch,
            note,
            unverified,
        } => {
            card = card
                .child(text("Interrupted — the last checkpoint was kept").color(p.text))
                .child(
                    text(format!("checkpoint {epoch}"))
                        .font(type_::LABEL)
                        .color(p.muted),
                )
                .child(text(note.clone()).color(p.secondary).wrap_anywhere());
            for name in unverified {
                card = card.child(text(name.clone()).color(p.amber).wrap_anywhere());
            }
            if !unverified.is_empty() {
                // Electron's caution beside "Resume anyway" — kept as words only.
                // Native never mounts a Resume control: no reconciled checkpoint execution.
                card = card.child(
                    text("Check those first — the next run is told to verify them before building on them.")
                        .font(type_::LABEL)
                        .color(p.muted)
                        .wrap_anywhere(),
                );
            }
            // No Resume / Resume anyway button: irreversible work must not be re-run here.
            card = card
                .child(text("Cannot be resumed from here").color(p.secondary))
                .child(
                    text("Native checkpoint resume is not available yet.")
                        .font(type_::LABEL)
                        .color(p.muted),
                );
        }
    }
    Some(card)
}
