//! What a turn that did not go through says under it, as Electron's `Thread.tsx` draws it (live hour 1, faults 15-17):
//! a usage limit as a red "Usage limit reached" card with its tag and the reset in a person's words; a notice or an
//! error as a line in its own colour with no "error:" label; and the durable receipt, the card under a turn that failed,
//! was cut off, or whose record could not be read to its end.

use super::*;
use locust_core::inspector::{checkpoint_label, ledger_verification_label, runtime_display_name, short_mission_id};

/// The title of a limit card (`Thread.tsx`, `item.type === 'limit'`).
pub(super) const LIMIT_TITLE: &str = "Usage limit reached";

/// A limit the run ended on: `temporary-rate-limit` is a warning in amber; anything else is the red card, its tag the
/// kind the ledger recorded. The reset is the ledger's instant; a person reads a local time (`usageWindowLabel`).
pub(super) fn limit(kind: &str, said: &str, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let said = locust_core::usage_words::usage_window_label(said, chrono::Utc::now());
    if kind == "temporary-rate-limit" {
        return diagnostic("warning", &said, p);
    }
    col()
        .key("limit-card")
        .w_full()
        .radius(l.t.px("radius-card"))
        .border(1.0, l.c("red-border"))
        .child(
            row().w_full().py(8.0).px(12.0).gap(8.0).center_y()
                .child(text(LIMIT_TITLE).font(l.ty.control).color(l.c("text-primary")).grow(1.0))
                .child(text(kind.to_uppercase()).key("limit-tag").font(type_::LABEL.mono()).color(l.c("red-text"))),
        )
        .child(col().w_full().px(12.0).pb(12.0).child(text(said).key("limit-said").font(l.ty.body).color(l.c("text-secondary")).wrap_anywhere()))
}

/// `DiagnosticLine`: the notice's sentence on a rule in its own colour, the shield before it, nothing like "error:" in
/// front. An error is red, a warning amber, anything else quiet.
pub(super) fn diagnostic(level: &str, said: &str, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let (rule, ink) = match level {
        "error" => (l.c("red-border"), l.c("red-text")),
        "warning" => (l.c("amber-border"), l.c("amber-text")),
        _ => (l.c("border-card"), l.c("text-muted")),
    };
    row()
        .w_full()
        .gap(8.0)
        .pl(8.0)
        .border_sides([0.0, 0.0, 0.0, 2.0], rule)
        .child(crate::look::controls::glyph(&l, &crate::look::icons::SHIELD, 12.0, Some(ink)))
        .child(text(said.to_owned()).font(type_::LABEL).color(ink).grow(1.0).shrink().wrap_anywhere().selectable(true))
}

/// What the receipt of the turn at the end of the thread holds, from the ledger's own projection of it.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Receipt {
    pub tag: &'static str,
    pub tone: &'static str,
    pub runtime: String,
    pub model: String,
    pub cli_version: String,
    pub mission: String,
    pub checkpoints: usize,
    pub last_checkpoint: Option<u64>,
    pub unsettled: usize,
    pub ledger: &'static str,
}

/// The receipt of the newest turn, when it carries news: a run that failed or was cut off, a record that did not read
/// to its end, an action that never reported back. A routine one (Electron's `routine`) draws nothing here, and a
/// stopped run already has its own receipt (the stopped card).
pub(super) fn receipt_of(snap: &locust_core::ledger::Snapshot) -> Option<Receipt> {
    if snap.running {
        return None;
    }
    let meta = snap.turn_metas.last()?;
    let phase = if snap.interrupted { "interrupted" } else if snap.failed { "failed" } else { "completed" };
    let view = crate::missions_screen::mission_phase_view(phase, snap.damaged);
    let last_checkpoint = meta.checkpoint_unsettled.keys().next_back().copied();
    let unsettled = last_checkpoint.and_then(|epoch| meta.checkpoint_unsettled.get(&epoch).copied()).unwrap_or(0);
    let ledger = ledger_verification_label(usize::from(snap.damaged));
    let routine = view.tone == "blue" && ledger == "record readable" && unsettled == 0;
    if routine {
        return None;
    }
    Some(Receipt {
        tag: view.tag,
        tone: view.tone,
        runtime: meta.runtime.clone(),
        model: meta.model.clone(),
        cli_version: meta.cli_version.clone().unwrap_or_default(),
        mission: short_mission_id(&meta.mission_id),
        checkpoints: meta.checkpoint_unsettled.len(),
        last_checkpoint,
        unsettled,
        ledger,
    })
}

/// The line under the head: "claude 2.1.322 / opus · no checkpoints · record readable" (`lc-receipt__line`).
pub(super) fn receipt_line(r: &Receipt) -> String {
    let who = format!("{} {} / {}", r.runtime, r.cli_version, r.model).replace("  ", " ");
    let checkpoints = if r.checkpoints == 0 { "no checkpoints".to_owned() } else { format!("{} checkpoints", r.checkpoints) };
    let mut line = format!("{who} \u{b7} {checkpoints} \u{b7} {}", r.ledger);
    match r.unsettled {
        0 => {}
        1 => line.push_str(" \u{b7} 1 unverified"),
        n => line.push_str(&format!(" \u{b7} {n} unverified")),
    }
    line
}

/// The durable receipt card (`ReceiptCard`), at rest: its head and its line; the line opens the rest.
pub(super) fn receipt(s: &State, p: &Palette) -> Option<El> {
    let snap = &s.ledger.as_ref()?.snapshot;
    let r = receipt_of(snap)?;
    let l = crate::look::controls::Look::new(p.dark);
    let (border, tag_ink) = match r.tone {
        "red" => (l.c("red-border"), l.c("red-text")),
        "amber" => (l.c("amber-border"), l.c("amber-text")),
        _ => (l.c("blue-border"), l.c("text-muted")),
    };
    let open_key = format!("receipt:{}", snap.tip);
    let open = s.activity_open.get(&open_key).copied().unwrap_or(false);
    let toggle = open_key.clone();
    let chevron = if open { &crate::look::icons::CHEVRON_DOWN } else { &crate::look::icons::CHEVRON_RIGHT };
    let mut card = col()
        .key("durable-receipt")
        .w_full()
        .radius(l.t.px("radius-card"))
        .border(1.0, border)
        .child(
            row().w_full().py(8.0).px(12.0).gap(8.0).center_y()
                .child(
                    row().gap(8.0).center_y().grow(1.0)
                        .child(text("DURABLE RECEIPT").key("receipt-title").font(type_::LABEL.mono()).color(l.c("text-muted")))
                        .child(text(r.tag).key("receipt-tag").font(type_::LABEL.mono()).color(tag_ink)),
                )
                .child(text("restored from local ledger").font(type_::LABEL).color(l.c("text-muted"))),
        )
        .child(
            row().key("receipt-summary").role("button").w_full().px(12.0).pb(8.0).gap(10.0).center_y()
                .on_press(move |s: &mut State| {
                    let now = s.activity_open.get(&toggle).copied().unwrap_or(false);
                    s.activity_open.insert(toggle.clone(), !now);
                })
                .child(text(receipt_line(&r)).key("receipt-line").font(type_::LABEL.mono()).color(l.c("text-muted")).grow(1.0).shrink().wrap_anywhere())
                .child(crate::look::controls::glyph(&l, chevron, 12.0, Some(l.c("text-muted")))),
        );
    if open {
        let line = |label: &str, value: String| {
            row().w_full().px(12.0).gap(10.0)
                .child(text(label.to_owned()).font(type_::LABEL).color(l.c("text-muted")).w(88.0))
                .child(text(value).font(type_::LABEL.mono()).color(l.c("text-primary")).grow(1.0).shrink().wrap_anywhere())
        };
        let ran_on = {
            let platform = match std::env::consts::OS { "macos" => "darwin", "windows" => "win32", other => other };
            let folder = snap.turn_metas.last().and_then(|m| locust_core::ran_on::folder_name(m.cwd.as_deref()));
            locust_core::ran_on::ran_on_line(platform, folder.as_deref())
        };
        let checkpoints = match (r.checkpoints, r.last_checkpoint) {
            (0, _) => "none written".to_owned(),
            (n, Some(epoch)) => format!("{n} written \u{b7} last {}", checkpoint_label(epoch)),
            (n, None) => format!("{n} written"),
        };
        card = card
            .child(line("Runtime", format!("{} {} \u{b7} {}", runtime_display_name(&r.runtime), r.cli_version, r.model).replace("  ", " ")))
            .child(line("Ran on", ran_on))
            .child(line("Run", r.mission.clone()))
            .child(line("Checkpoints", checkpoints))
            .child(line("Ledger", r.ledger.to_owned()))
            .child(el().h(8.0));
    }
    Some(card)
}

#[cfg(test)]
#[path = "failed_turn_cards_tests.rs"]
mod failed_turn_cards_tests;
