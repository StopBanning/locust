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
    let now = chrono::Utc::now();
    let card = agent_card(locust_core::carry_on::display_name(runtime), &ready_state(version.as_deref()), Some(reading), now);
    Some(draw(l, runtime, &card, reading, now).key("model-chip-card").anchored_fit("model-chip", rune::Side::Top, rune::Align::End, 8.0))
}

/// `.lc-agentcard`: 232 wide, the raised fill, a card edge and the pending shadow; a head (the mark, the name and under
/// it the state), a block for each window, and the reading's source.
fn draw(l: &crate::look::controls::Look, runtime: &str, card: &AgentCard, said: &str, now: chrono::DateTime<chrono::Utc>) -> El {
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
    if !card.windows.is_empty() {
        body = body.child(usage_meters(l, MeterReading::Card(said, card), now));
    }
    body
}

/// shell.css `.lc-agentcard__fill`: a usage bar in the app's own tones, quiet, amber from 80% (`is-pressing`), red when
/// spent (`is-spent`).
pub(crate) fn bar_colour(l: &crate::look::controls::Look, tone: Tone) -> Color {
    match tone {
        Tone::Spent => l.c("red"),
        Tone::Pressing => l.c("amber"),
        Tone::Calm => l.c("text-secondary"),
    }
}

pub(crate) enum MeterReading<'a> {
    Inline(&'a str),
    Card(&'a str, &'a AgentCard),
}

impl<'a> From<&'a str> for MeterReading<'a> {
    fn from(said: &'a str) -> Self {
        Self::Inline(said)
    }
}

/// UsageMeters.tsx: one meter per window and the whole reading as its tooltip. The chip card gives each meter
/// one line across its width and keeps the runtime's reading source and reset time.
pub(crate) fn usage_meters<'a>(
    l: &crate::look::controls::Look,
    reading: impl Into<MeterReading<'a>>,
    now: chrono::DateTime<chrono::Utc>,
) -> El {
    use locust_core::usage_words::{ReadOf, usage_read_of, usage_window_label, usage_window_sentence};

    let inline_card;
    let (said, card, inline) = match reading.into() {
        MeterReading::Inline(said) => {
            inline_card = agent_card("", "", Some(said), now);
            (said, &inline_card, true)
        }
        MeterReading::Card(said, card) => (said, card, false),
    };
    let mut meters = if inline {
        row().flex_wrap().gap(10.0).center_y()
    } else {
        col().w_full().gap(9.0)
    }.key("usage-meters").help(usage_window_sentence(said, now));

    for (i, window) in card.windows.iter().enumerate() {
        let meter_key = format!("usage-meter-{i}");
        let label = window.label();
        let label = if inline { label.strip_suffix(" window").unwrap_or(label) } else { label };
        let name = text(label.to_owned())
            .key(format!("{meter_key}-name")).font(l.ty.body).color(l.c("text-secondary"));
        let mut meter = if inline { row().gap(6.0).center_y() } else { col().w_full().gap(3.0) };
        meter = meter.key(&meter_key);
        match window {
            CardWindow::Reset { .. } => {
                meter = meter.child(name).child(
                    text(window.reset_since().unwrap_or_default()).font(l.ty.body).color(l.c("text-muted"))
                );
            }
            CardWindow::Bar { value, fill, tone, .. } => {
                let colour = bar_colour(l, *tone);
                let figure = if inline && !value.ends_with(" left") {
                    format!("{}% left", 100u32.saturating_sub(*fill))
                } else {
                    value.clone()
                };
                let figure = text(figure).key(format!("{meter_key}-figure")).font(l.ty.body).color(colour);
                let fraction = (*fill).min(100) as f32 / 100.0;
                let bar = el().h(4.0).radius(2.0).fill(l.c("border-hairline")).a11y_hidden()
                    .child(el().w_frac(fraction).h(4.0).radius(2.0).fill(colour));
                meter = if inline {
                    meter.child(name).child(bar.w(48.0)).child(figure)
                } else {
                    meter
                        .child(row().gap(8.0).child(name.grow(1.0).shrink().truncate()).child(figure))
                        .child(bar.w_full())
                        .children(window.resets_line().map(|said| text(said).font(l.ty.body).color(l.c("text-muted"))))
                };
            }
        }
        meters = meters.child(meter);
    }

    let read = if inline {
        usage_read_of(said).map(|read| {
            let at = match read {
                ReadOf::Run(at) | ReadOf::Account(at) => at,
            };
            format!("as of {}", usage_window_label(&at, now))
        })
    } else {
        card.read.clone()
    };
    meters.children(read.map(|read| {
        text(read).key("usage-meters-read").font(l.ty.body).color(l.c("text-muted")).wrap()
    }))
}

#[cfg(test)]
#[path = "agent_card_ui_meters_tests.rs"]
mod meters_tests;
