//! An agent's card, drawn (Electron `AgentMark.tsx`'s `AgentCard`, and the model chip in `Composer.tsx`): over the chat
//! box's model chip on hover, above it and aligned to its right edge, because the chip sits at the window's right. The
//! card's content is `locust_core::agent_card`; this draws it and keeps the readings it draws.
//!
//! The readings are the ledger's, read off the window's thread once the pointer first rests on a chip.

use super::*;
use locust_core::agent_card::{AgentCard, CardWindow, Tone, agent_card, chip_version, ready_state};

/// Read every account's latest usage reading, once per ask.
pub(super) fn request(s: &mut State) {
    let Some(store) = s.data.clone() else { return };
    if s.usage_readings_job.is_pending() {
        return;
    }
    s.usage_readings_job.request(store, |store| locust_core::usage_ledger::usage_windows(&store));
}

pub(super) fn poll(s: &mut State) -> bool {
    let Some(readings) = s.usage_readings_job.take() else { return false };
    s.usage_readings = readings;
    true
}

/// The card over the model chip, or None: no reading for this account, or the picker is open (it opens in the same
/// place) -- Electron draws it for neither.
pub(super) fn chip_card(s: &State, l: &crate::look::controls::Look, runtime: &str, account: Option<&str>) -> Option<El> {
    if !s.chip_hover || s.model_menu.is_some() {
        return None;
    }
    let key = locust_core::agent_accounts::usage_key(runtime, account);
    let reading = s.usage_readings.get(&key)?;
    let version = s.home.boot.probes.iter().find(|p| p.id == runtime).and_then(|p| p.version.as_deref()).map(chip_version);
    let card = agent_card(locust_core::carry_on::display_name(runtime), &ready_state(version.as_deref()), Some(reading), chrono::Utc::now());
    Some(draw(l, runtime, &card).key("model-chip-card").anchored_fit("model-chip", rune::Side::Top, rune::Align::End, 8.0))
}

/// `.lc-agentcard`: 232 wide, the raised fill, a card edge and the pending shadow; a head (the mark, the name and under
/// it the state), a block for each window, and the reading's source.
fn draw(l: &crate::look::controls::Look, runtime: &str, card: &AgentCard) -> El {
    let mut head = row().gap(8.0).center_y();
    if let Some(mark) = crate::look::controls::agent_mark(l, runtime, 16.0, l.c("text-secondary")) {
        head = head.child(mark);
    }
    head = head.child(
        col()
            .grow(1.0)
            .shrink()
            .gap(1.0)
            .child(text(card.name.clone()).font(l.ty.control).color(l.c("text-primary")).truncate())
            .child(text(card.state.clone()).font(l.ty.body).color(l.c("text-muted")).truncate()),
    );
    let mut body = col().w(232.0).px(12.0).pt(10.0).pb(11.0).gap(9.0).radius(l.t.px("radius-card")).fill(l.c("bg-raised")).border(1.0, l.c("border-card")).child(head);
    for window in &card.windows {
        body = body.child(window_block(l, window));
    }
    if !card.windows.is_empty()
        && let Some(read) = &card.read
    {
        body = body.child(text(read.clone()).font(l.ty.body).color(l.c("text-muted")).wrap());
    }
    body
}

fn window_block(l: &crate::look::controls::Look, window: &CardWindow) -> El {
    match window {
        CardWindow::Reset { label, .. } => col()
            .gap(3.0)
            .child(text(label.clone()).font(l.ty.body).color(l.c("text-secondary")))
            .child(text(window.reset_since().unwrap_or_default()).font(l.ty.body).color(l.c("text-muted"))),
        CardWindow::Bar { label, value, fill, tone, .. } => {
            let colour = match tone {
                Tone::Spent => l.c("red"),
                Tone::Pressing => l.c("amber"),
                Tone::Calm => l.c("text-secondary"),
            };
            let mut block = col()
                .gap(3.0)
                .child(row().gap(8.0).child(text(label.clone()).font(l.ty.body).color(l.c("text-secondary")).grow(1.0).shrink().truncate()).child(text(value.clone()).font(l.ty.body).color(colour)))
                .child(
                    el().w_full().h(4.0).radius(2.0).fill(l.c("border-hairline")).child(el().w(208.0 * (*fill as f32 / 100.0)).h(4.0).radius(2.0).fill(colour)),
                );
            if let Some(resets) = window.resets_line() {
                block = block.child(text(resets).font(l.ty.body).color(l.c("text-muted")));
            }
            block
        }
    }
}
