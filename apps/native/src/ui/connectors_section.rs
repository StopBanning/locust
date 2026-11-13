//! Settings › Connectors › "Connectors" (Electron `ConnectorHealth.tsx`, W8, 0.567): the connectors this machine has,
//! read from the person's Claude Code (`claude mcp list`), each one's state in words and what to do, when it last
//! worked, and Copy report for someone helping. Check again asks again; the reading is held for five minutes.

use super::*;
use locust_core::connector_reader::{self, Connector};

#[derive(Default)]
pub(super) struct Card {
    pub connectors: Option<Vec<Connector>>,
    pub reading: bool,
    pub notice: Option<String>,
}

#[cfg(test)]
static FAKE_READER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Tests answer for Claude Code themselves.
#[cfg(test)]
pub(super) fn use_reader(reader: std::sync::Arc<connector_reader::Reader>) {
    connector_reader::use_reader(reader);
    FAKE_READER.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Tests never run the real Claude Code: without a reader of their own, the card reads nothing.
fn may_read() -> bool {
    #[cfg(test)]
    return FAKE_READER.load(std::sync::atomic::Ordering::SeqCst);
    #[cfg(not(test))]
    true
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Read the connectors off the window's thread. `fresh` counts the held reading as stale (Check again).
pub(super) fn request(s: &mut State, fresh: bool) {
    if s.connectors_job.is_pending() || !may_read() {
        return;
    }
    s.connectors.reading = true;
    s.connectors_job.request(fresh, |fresh| {
        let reader = connector_reader::shared();
        if fresh {
            reader.forget();
        }
        reader.refresh()
    });
}

pub(super) fn poll(s: &mut State) -> bool {
    match s.connectors_job.take() {
        Some(found) => {
            s.connectors.connectors = Some(found);
            s.connectors.reading = false;
            true
        }
        None => false,
    }
}

fn copy_report(s: &mut State, tx: &mut Tx) {
    let all = s.connectors.connectors.clone().unwrap_or_default();
    let text = connector_reader::report(&all, &chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true), now_ms());
    crate::reply_actions::request_copy(tx, text);
    s.connectors.notice = Some("Report copied: names, states, timings and hosts only.".into());
}

pub(super) fn section(s: &State, p: &Palette) -> El {
    let card = &s.connectors;
    let mut body = col().w_full().gap(8.0).key("connectors-health").label("Connectors on this machine");
    match &card.connectors {
        None => {
            body = body.child(
                text(if card.reading {
                    "Reading your connectors…"
                } else {
                    "Your connectors could not be read just now; your teammates keep the ones that last worked. Check again in a moment."
                })
                .font(type_::BODY)
                .color(p.secondary)
                .wrap(),
            )
        }
        Some(all) if all.is_empty() => body = body.child(text("Your Claude Code has no connectors.").font(type_::BODY).color(p.secondary)),
        Some(all) => {
            let now = now_ms();
            let mut list = col().w_full().gap(0.0).radius(8.0).border(1.0, p.hairline);
            for c in all {
                let (state, _, tone) = connector_reader::state_words(c);
                let color = match tone {
                    "good" => p.green,
                    "red" => p.red,
                    _ => p.amber,
                };
                let mut words = col()
                    .gap(2.0)
                    .grow(1.0)
                    .shrink()
                    .child(
                        row()
                            .gap(8.0)
                            .center_y()
                            .child(text(c.name.clone()).font(type_::LABEL).color(p.text))
                            .child(text(connector_reader::where_it_lives(&c.location)).font(type_::LABEL.mono()).color(p.muted)),
                    );
                if let Some(detail) = connector_reader::detail(c, now) {
                    words = words.child(text(detail).font(type_::BODY).color(p.secondary).wrap());
                }
                list = list.child(
                    row()
                        .w_full()
                        .gap(10.0)
                        .padding(10.0)
                        .key(format!("connector-{}", c.name))
                        .child(text(state.to_uppercase()).font(type_::LABEL).color(color).px(8.0).py(2.0).radius(999.0).border(1.0, p.hairline).self_start())
                        .child(words),
                );
            }
            body = body.child(list);
        }
    }
    let check = text(if card.reading { "Checking…" } else { "Check again" }).font(type_::LABEL).color(p.secondary).px(12.0).py(6.0).radius(6.0).border(1.0, p.hairline).key("connectors-check-again").role("button");
    let copy = text("Copy report").font(type_::LABEL).color(p.secondary).px(12.0).py(6.0).radius(6.0).border(1.0, p.hairline).key("connectors-copy-report").role("button");
    body = body.child(
        row()
            .gap(8.0)
            .child(if card.reading { check } else { check.on_press(|s: &mut State| request(s, true)) })
            .child(if card.connectors.is_none() { copy } else { copy.on_press_tx(copy_report) }),
    );
    if let Some(notice) = &card.notice {
        body = body.child(text(notice.clone()).font(type_::BODY).color(p.secondary).wrap());
    }
    body
}
