//! Settings > AI agents, "What Locust can stop" (WhatLocustCanStop.tsx, 0.617): the lede, then one folded row per AI
//! agent, named, with how much a card can stop (green only where each action can), opening to what it asks first,
//! what it does without asking, and who keeps an Always. The sentences are core's `what_locust_can_stop`, read off
//! this engine's own run code.
use super::*;
use locust_core::what_locust_can_stop::{StopReach, WHAT_LOCUST_CAN_STOP, WHAT_LOCUST_CAN_STOP_LEDE};

pub(super) const HEADING: &str = "What Locust can stop";

fn open_key(runtime: &str) -> String {
    format!("stops/{runtime}")
}

pub(super) fn section(s: &State, p: &Palette) -> El {
    let mut list = col().w_full().gap(0.0);
    for r in WHAT_LOCUST_CAN_STOP {
        let key = open_key(r.runtime);
        let open = s.activity_open.get(&key).copied().unwrap_or(false);
        let green = r.reach == StopReach::EachAction;
        let tag = text(r.reach.label().to_owned())
            .key(format!("stops-{}-reach", r.runtime))
            .font(type_::LABEL)
            .color(if green { p.green } else { p.secondary })
            .padding(4.0)
            .radius(4.0)
            .fill(if green { p.green.alpha(0.14) } else { p.raised });
        let chevron = if open { &crate::look::icons::CHEVRON_DOWN } else { &crate::look::icons::CHEVRON_RIGHT };
        let l = crate::look::controls::Look::new(p.dark);
        let head = row()
            .w_full()
            .gap(8.0)
            .py(8.0)
            .center_y()
            .key(format!("stops-{}", r.runtime))
            .role("button")
            .label(format!("{}: {}", locust_core::carry_on::display_name(r.runtime), r.reach.label()))
            .child(crate::look::controls::glyph(&l, chevron, 12.0, Some(p.muted)))
            .child(text(locust_core::carry_on::display_name(r.runtime).to_owned()).font(type_::LABEL).color(p.text).grow(1.0))
            .child(tag)
            .on_press(move |s: &mut State| {
                let now = s.activity_open.get(&key).copied().unwrap_or(false);
                s.activity_open.insert(key.clone(), !now);
            });
        let mut entry = col().w_full().border_sides([0.0, 0.0, 1.0, 0.0], p.hairline).child(head);
        if open {
            let term = |label: &str, said: &str| {
                col()
                    .w_full()
                    .gap(2.0)
                    .child(text(label.to_owned()).font(type_::LABEL).color(p.muted))
                    .child(text(said.to_owned()).font(type_::LABEL).color(p.secondary).w_full().wrap())
            };
            let mut body = col().w_full().gap(8.0).pl(20.0).pb(10.0).key(format!("stops-{}-open", r.runtime));
            body = body.child(term("Asks first", r.asks)).child(term("Without asking", r.without));
            if let Some(always) = r.always {
                body = body.child(term("Always", always));
            }
            entry = entry.child(body);
        }
        list = list.child(entry);
    }
    col()
        .w_full()
        .gap(6.0)
        .padding(10.0)
        .key("what-locust-can-stop")
        .child(text(HEADING.to_owned()).role("heading").level(2).font(type_::TITLE).color(p.text))
        .child(text(WHAT_LOCUST_CAN_STOP_LEDE.to_owned()).font(type_::LABEL).color(p.secondary).w_full().wrap())
        .child(list)
}

#[cfg(test)]
#[path = "what_locust_can_stop_ui_tests.rs"]
mod tests;
