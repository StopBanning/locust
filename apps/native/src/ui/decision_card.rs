//! The ask/decision card (`DecisionCard.tsx` and `missionView.ts` `decisionStanding`).
//! Neutral (never amber) — the run has ended and nothing is pending against the workspace.

use super::*;
use locust_core::protocol_tags::{self, DecisionOption, DecisionRequest};

/// The active decision on the latest turn, if the turn has ended and no user message followed.
pub(super) fn active_decision(
    s: &State,
) -> Option<(Option<String>, &'static str, DecisionRequest)> {
    if turn_running(s) || s.flight.is_some() || s.sending.is_pending() || s.pending_approval.is_some()
    {
        return None;
    }
    let (teammate, conversation) = selected_conversation(s)?;
    let fallback;
    let items: &[ledger::Item] = if let Some(pane) = &s.ledger {
        if pane.loading || pane.snapshot.running {
            return None;
        }
        &pane.items
    } else {
        fallback = conversation
            .turns
            .iter()
            .enumerate()
            .map(|(i, (who, body))| {
                if who == "You" {
                    ledger::Item::User(body.clone())
                } else {
                    ledger::Item::Assistant {
                        item_id: format!("legacy-{i}"),
                        text: body.clone(),
                        r#final: true,
                    }
                }
            })
            .collect::<Vec<_>>();
        &fallback
    };

    let turn_start = items
        .iter()
        .rposition(|item| matches!(item, ledger::Item::User(_)))
        .map(|i| i + 1)
        .unwrap_or(0);
    let latest_items = &items[turn_start..];
    let request = locust_core::thread_view::ended_question(latest_items, false, true)?;
    let teammate_name = if teammate.id == LEDGER_TARGET || teammate.name.trim().is_empty() {
        None
    } else {
        Some(teammate.name.clone())
    };
    let standing = decision_standing(s);
    Some((teammate_name, standing, request))
}

/// What may honestly be said about the workspace (`missionView.ts` `decisionStanding`).
pub(super) fn decision_standing(s: &State) -> &'static str {
    let Some(pane) = &s.ledger else {
        return locust_core::decision_standing::decision_standing(None, &[]);
    };
    let sandbox = pane.snapshot.turn_metas.last().map(|m| m.sandbox.as_str());
    let inputs = pane.worker.as_ref().map(|w| w.inspector_inputs()).unwrap_or_default();
    let events = inputs.last().map(|input| input.events.as_slice()).unwrap_or(&[]);
    locust_core::decision_standing::decision_standing(sandbox, events)
}

pub(super) fn answer_decision(s: &mut State, option: &DecisionOption, tx: &mut Tx) {
    if turn_running(s) || s.flight.is_some() || s.sending.is_pending() || s.pending_approval.is_some()
    {
        return;
    }
    let reply = protocol_tags::decision_reply(option);
    s.draft.replace_text(reply);
    submit_composer(s, tx, false);
}

/// Number keys `1`..`9` choose an option when no text field or modal holds the keyboard.
pub(super) fn handle_key(s: &mut State, event: &KeyEvent, tx: &mut Tx) -> bool {
    if event.mods.cmd || event.mods.ctrl || event.mods.alt || event.mods.shift {
        return false;
    }
    let Key::Text(ref txt) = event.key else {
        return false;
    };
    let mut chars = txt.chars();
    let Some(ch) = chars.next() else {
        return false;
    };
    if chars.next().is_some() {
        return false;
    }
    let digit = ('1'..='9').contains(&ch);
    if s.draft.focused
        || s.find_query.focused
        || s.palette_query.focused
        || s.approval_reason.focused
        || s.sidebar_query.focused
        || s.rename_id.is_some()
        || s.palette_open
        || s.find_open
        || s.settings_panel.open
        || s.people_panel.open
        || s.usage_panel.open
        || s.keyboard_sheet.open
        || s.save_record.open
        || s.import_dialog.open
        || s.home.splash_open
        || s.home.boot_open
        || s.home.board_open
        || s.home.home_open
        || s.pending_approval.is_some()
    {
        return false;
    }
    let Some((_, _, request)) = active_decision(s) else {
        return false;
    };
    if !digit {
        // A typed letter goes to the chat box (DecisionCard.tsx): the person is replying in their own words, which
        // the card says they may.
        if ch == ' ' || ch.is_control() {
            return false;
        }
        s.draft.focused = true;
        s.draft.insert(&ch.to_string());
        return true;
    }
    let idx = (ch as u8 - b'1') as usize;
    let Some(option) = request.options.get(idx) else {
        return false;
    };
    answer_decision(s, option, tx);
    true
}

pub(super) fn card(s: &State, p: &Palette) -> Option<El> {
    let (teammate_name, standing, request) = active_decision(s)?;
    let l = crate::look::controls::Look::new(p.dark);
    let title = decision_title(teammate_name.as_deref());

    let head = row()
        .w_full()
        .gap(8.0)
        .center_y()
        .child(
            row()
                .gap(6.0)
                .center_y()
                .grow(1.0)
                .child(rune::icon(&crate::look::icons::DIFF, 13.0).color(l.c("text-secondary")))
                .child(text(title).font(l.ty.label).color(l.c("text-primary"))),
        )
        .child(text(standing).font(l.ty.label).color(l.c("text-muted")));

    let mut options_col = col().w_full().gap(6.0);
    for (index, option) in request.options.iter().enumerate() {
        let opt = option.clone();
        let num = (index + 1).to_string();
        let key_badge = row()
            .w(20.0)
            .h(20.0)
            .center()
            .radius(4.0)
            .fill(l.c("bg-window"))
            .border(1.0, l.c("border-control"))
            .child(
                text(num)
                    .font(rune::TextStyle::new(11.0, 500.0).mono())
                    .color(l.c("text-secondary")),
            );
        let mut body = col()
            .grow(1.0)
            .gap(2.0)
            .child(text(option.label.clone()).font(l.ty.body).color(l.c("text-primary")).wrap());
        if let Some(note) = &option.note {
            body = body.child(
                text(note.clone())
                    .font(l.ty.label)
                    .color(l.c("text-secondary"))
                    .wrap(),
            );
        }
        let btn = row()
            .key(format!("decision-option-{}", index + 1))
            .w_full()
            .gap(10.0)
            .padding(10.0)
            .center_y()
            .radius(6.0)
            .fill(l.c("bg-control"))
            .border(1.0, l.c("border-control"))
            .hover(l.c("hover-surface"))
            .focusable(true)
            .focus_ring(2.0, 2.0, l.c("focus-ring"))
            .role("button")
            .label(option.label.clone())
            .child(key_badge)
            .child(body)
            .on_press_tx(move |s: &mut State, tx: &mut Tx| answer_decision(s, &opt, tx));
        options_col = options_col.child(btn);
    }

    // `.lc-decision`: raised as a pending card is (bg-pending, shadow-pending), never amber — the run has ended and
    // nothing is waiting against the folder.
    let mut card = col()
        .key("decision-card")
        .w_full()
        .gap(10.0)
        .padding(14.0)
        .radius(l.t.px("radius-card"))
        .fill(l.c("bg-pending"))
        .border(1.0, l.c("border-card"));
    for shadow in l.t.shadows("shadow-pending") {
        card = card.box_shadow(shadow);
    }
    Some(
        card
            .child(head)
            .child(
                text(request.question)
                    .font(l.ty.body)
                    .color(l.c("text-primary"))
                    .w_full()
                    .wrap(),
            )
            .child(options_col)
            .child(
                text("Or reply below — you are not limited to these")
                    .font(rune::TextStyle::new(11.0, 400.0).mono())
                    .color(l.c("text-muted")),
            ),
    )
}

fn decision_title(teammate_name: Option<&str>) -> String {
    match teammate_name {
        Some(name) => format!("{name} needs a decision to continue"),
        None => "A decision is needed to continue".into(),
    }
}

#[cfg(test)]
#[path = "decision_number_tests.rs"]
mod decision_number_tests;

#[cfg(test)]
mod renderer_tests {
    use super::*;
    #[test]
    fn says_a_decision_is_needed_and_no_word_for_the_machinery_when_nobody_asked_by_name() {
        let title = decision_title(None);
        assert_eq!(title, "A decision is needed to continue");
        assert!(!title.contains("runtime"));
    }
    #[test]
    fn names_the_teammate_who_asked() {
        assert_eq!(decision_title(Some("Wren")), "Wren needs a decision to continue");
    }
}
