//! What a turn that did not go through says under it, as Electron's `Thread.tsx` draws it (live hour 1, faults 15-17):
//! a usage limit as a red "Usage limit reached" card with its tag and the reset in a person's words; a notice or an
//! error as a line in its own colour with no "error:" label; and the durable receipt, the card under a turn that failed,
//! was cut off, or whose record could not be read to its end.

use super::*;
use locust_core::ledger::Item;
use locust_core::inspector::{checkpoint_label, ledger_verification_label, runtime_display_name, short_mission_id};

/// The turn an item belongs to: from the person's words before it to the next person's words.
fn turn_of(items: &[Item], index: usize) -> std::ops::Range<usize> {
    let from = items[..index].iter().rposition(|i| matches!(i, Item::User(_))).map_or(0, |at| at + 1);
    let to = items[index..].iter().position(|i| matches!(i, Item::User(_))).map_or(items.len(), |n| index + n);
    from..to
}

fn is_the_limit(item: &Item) -> bool {
    matches!(item, Item::Limit { kind, .. } if kind != "temporary-rate-limit")
}

/// Whether the turn this item is in ended on a usage limit (`quota-exhausted`): the run could not go on, and says so once.
pub(super) fn ended_on_limit(items: &[Item], index: usize) -> bool {
    items[turn_of(items, index)].iter().any(is_the_limit)
}

/// An item the limit card already says: the error line repeating the limit, and the carry-on card that now sits inside
/// the limit card (review change 2: "no second line repeating the limit").
pub(super) fn said_by_the_limit_card(items: &[Item], index: usize) -> bool {
    match &items[index] {
        Item::Note { level, .. } if level == "error" => ended_on_limit(items, index),
        Item::CarryOn { why, .. } if why == "plan" => ended_on_limit(items, index),
        _ => false,
    }
}

/// The agent the turn this item is in ran on, as Locust names it in a sentence.
fn agent_of(s: &State, items: &[Item], index: usize) -> String {
    let metas = s.ledger.as_ref().map(|pane| pane.snapshot.turn_metas.as_slice()).unwrap_or(&[]);
    let person_turns = items[..index].iter().filter(|i| matches!(i, Item::User(_))).count();
    let meta = person_turns
        .checked_sub(1)
        .and_then(|ui| metas.iter().filter(|m| locust_core::conversation_turns::fix_of_turn(m).is_none()).nth(ui))
        .or_else(|| metas.last());
    match meta.map(|m| m.runtime.as_str()).filter(|r| !r.is_empty()) {
        Some(runtime) => locust_core::carry_on::display_name(runtime).to_owned(),
        None => "The agent".to_owned(),
    }
}

/// "Claude Code reached its 5-hour limit · resets Wed 8:30 PM": the window that ran out, from the ledger's own
/// reading, and its reset said once in a person's words (`usageWindowLabel`).
pub(super) fn limit_sentence(agent: &str, said: &str, now: chrono::DateTime<chrono::Utc>) -> String {
    let windows = locust_core::usage_words::usage_windows_of(said, now);
    let fullest = windows.iter().max_by_key(|w| w.percent);
    let which = fullest.map(|w| w.name.trim_end_matches(" window").to_lowercase()).filter(|w| !w.is_empty());
    let mut line = match which {
        Some(which) => format!("{agent} reached its {which} limit"),
        None => format!("{agent} reached its usage limit"),
    };
    if let Some(resets) = fullest.and_then(|w| w.resets.as_deref()) {
        line.push_str(&format!(" \u{b7} resets {resets}"));
    }
    line
}

/// Whether the carry-on offer is held back: a turn is under way, or about to be.
pub(super) fn carry_on_held_back(s: &State) -> bool {
    s.flight.is_some() || s.sending.is_pending() || s.ledger.as_ref().is_some_and(|pane| pane.snapshot.running || pane.after_tip.is_some())
}

/// What carrying a stopped turn on says and offers: the sentence, one press for the agent Locust suggests, "Choose
/// another agent", and that menu when it is open. Under a limit it sits inside the limit card; after any other stop it
/// is the card of its own (messages.rs).
pub(super) fn carry_on_body(s: &State, conversation_id: &str, from: &str, sentence: &str, key: &str, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let mut line = row().w_full().gap(12.0).center_y().child(col().grow(1.0).child(text(sentence.to_owned()).font(l.ty.body).color(l.c("text-muted")).wrap()));
    if let Some((runtime, model)) = &s.carry_offer {
        let label = format!("Carry on with {}", model.clone().unwrap_or_else(|| locust_core::carry_on::display_name(runtime).to_owned()));
        let (id, runtime, model) = (conversation_id.to_owned(), runtime.clone(), model.clone().unwrap_or_default());
        line = line.child(
            crate::look::controls::button(&l, &label, crate::look::controls::Tone::Neutral, false)
                .key("carry-on-button")
                .on_press(move |s: &mut State| crate::carry(s, id.clone(), runtime.clone(), model.clone())),
        );
    }
    let mut body = col().w_full().gap(6.0).child(line).child(
        text("Choose another agent")
            .font(l.ty.body)
            .color(l.c("text-muted"))
            .hover(l.c("hover-surface"))
            .py(2.0)
            .role("button")
            .on_press(|s: &mut State| crate::open_models(s, crate::CARRY_TARGET.into())),
    );
    if s.model_menu.as_deref() == Some(crate::CARRY_TARGET) {
        if let Some(agents) = s.models.get(crate::CARRY_TARGET).filter(|a| !a.is_empty()) {
            let mut list = col().key(format!("{key}-carry-menu")).w_full().max_h(180.0).scroll(ScrollSpec { stick_bottom: false }).gap(4.0);
            for agent in agents.iter().filter(|a| a.runtime != from) {
                for model in &agent.models {
                    let (id, runtime, model_id) = (conversation_id.to_owned(), agent.runtime.clone(), model.id.clone());
                    list = list.child(
                        el().padding(8.0)
                            .disabled(s.flight.is_some())
                            .child(text(format!("{} \u{b7} {}", crate::engine::runtime_name(&agent.runtime), model.name)).color(p.text))
                            .on_press(move |s: &mut State| crate::carry(s, id.clone(), runtime.clone(), model_id.clone())),
                    );
                }
            }
            body = body.child(list);
        } else {
            body = body.child(text("Loading models\u{2026}").color(p.muted));
        }
    }
    body
}

/// A limit the run ended on: `temporary-rate-limit` is only a warning line; anything else is the ONE amber card, with
/// the carry-on offer inside it when the turn can be carried on. A limit is a wait, not a fault: red stays for errors.
pub(super) fn limit(s: &State, items: &[Item], index: usize, key: &str, p: &Palette) -> El {
    let Item::Limit { kind, text: said } = &items[index] else { return el() };
    let l = crate::look::controls::Look::new(p.dark);
    let now = chrono::Utc::now();
    if kind == "temporary-rate-limit" {
        return diagnostic("warning", &locust_core::usage_words::usage_window_label(said, now), p);
    }
    let mut card = col()
        .key("limit-card")
        .w_full()
        .gap(8.0)
        .padding(12.0)
        .radius(l.t.px("radius-card"))
        .fill(l.c("amber-tint"))
        .border(1.0, l.c("amber-border"))
        .child(text(limit_sentence(&agent_of(s, items, index), said, now)).key("limit-said").font(l.ty.control).color(l.c("text-primary")).wrap_anywhere());
    // The carry-on offer for this turn, when its card was about to follow: inside, not beside, this card.
    let span = turn_of(items, index);
    let offer = items[span.start..span.end].iter().find_map(|i| match i {
        Item::CarryOn { conversation_id, from, why } if why == "plan" => Some((conversation_id.as_str(), from.as_str(), why.as_str())),
        _ => None,
    });
    if let Some((id, from, why)) = offer.filter(|_| !carry_on_held_back(s)) {
        let sentence = locust_core::carry_on::stopped_sentence(from, why);
        // The first half of the sentence is the limit, said above: what is left to say is what is kept.
        let kept = sentence.split_once(". ").map_or(sentence.as_str(), |(_, rest)| rest);
        card = card.child(carry_on_body(s, id, from, kept, key, p));
    }
    card
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

/// The durable receipt under the newest turn. Under a usage limit it is folded to "Details" (review change 2: a limit is
/// a wait, and the receipt folds as a finished turn's does); the press brings the card up under it.
pub(super) fn receipt(s: &State, items: &[Item], p: &Palette) -> Option<El> {
    let card = receipt_card(s, p)?;
    let newest = items.iter().rposition(|i| matches!(i, Item::User(_))).map_or(0, |at| at + 1);
    if !items[newest..].iter().any(is_the_limit) {
        return Some(card);
    }
    let l = crate::look::controls::Look::new(p.dark);
    let key = format!("receipt-shown:{}", s.ledger.as_ref()?.snapshot.tip);
    let shown = s.activity_open.get(&key).copied().unwrap_or(false);
    let toggle = key.clone();
    let chevron = if shown { &crate::look::icons::CHEVRON_DOWN } else { &crate::look::icons::CHEVRON_RIGHT };
    let details = row()
        .key("receipt-details")
        .role("button")
        .gap(4.0)
        .center_y()
        .on_press(move |s: &mut State| {
            let now = s.activity_open.get(&toggle).copied().unwrap_or(false);
            s.activity_open.insert(toggle.clone(), !now);
        })
        .child(text("Details").font(type_::LABEL).color(l.c("text-muted")))
        .child(crate::look::controls::glyph(&l, chevron, 11.0, Some(l.c("text-muted"))));
    Some(if shown { col().w_full().gap(8.0).child(details).child(card) } else { col().w_full().child(details) })
}

/// The durable receipt card (`ReceiptCard`), at rest: its head and its line; the line opens the rest.
fn receipt_card(s: &State, p: &Palette) -> Option<El> {
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
