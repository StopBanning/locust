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
    let last_final = latest_items.iter().rev().find_map(|item| match item {
        ledger::Item::Assistant {
            text, r#final: true, ..
        } => Some(text.as_str()),
        _ => None,
    })?;
    let request = protocol_tags::parse_decision(last_final)?;
    let teammate_name = if teammate.id == LEDGER_TARGET || teammate.name.trim().is_empty() {
        None
    } else {
        Some(teammate.name.clone())
    };
    let standing = decision_standing(s, teammate, latest_items);
    Some((teammate_name, standing, request))
}

/// What may honestly be said about the workspace (`missionView.ts` `decisionStanding`).
pub(super) fn decision_standing(
    s: &State,
    teammate: &Teammate,
    latest_items: &[ledger::Item],
) -> &'static str {
    let mode = s
        .composer_mode
        .as_deref()
        .filter(|m| !m.is_empty())
        .or_else(|| (!teammate.runs_in.is_empty()).then_some(teammate.runs_in.as_str()));
    if matches!(mode, Some("ask") | Some("plan")) {
        return "stopped here · nothing was changed";
    }
    let wrote = latest_items.iter().any(|item| match item {
        ledger::Item::Tool { name, .. } => {
            let lower = name.to_lowercase();
            lower.contains("edit")
                || lower.contains("write")
                || lower.contains("patch")
                || lower.contains("filechange")
        }
        _ => false,
    });
    if wrote {
        "stopped here · work already done is kept"
    } else {
        "stopped here · this run could edit files"
    }
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
    if chars.next().is_some() || !('1'..='9').contains(&ch) {
        return false;
    }
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
    let title = match teammate_name {
        Some(name) => format!("{name} needs a decision to continue"),
        None => "A decision is needed to continue".into(),
    };

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

    Some(
        col()
            .key("decision-card")
            .w_full()
            .gap(10.0)
            .padding(14.0)
            .radius(l.t.px("radius-card"))
            .fill(l.c("bg-raised"))
            .border(1.0, l.c("border-card"))
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
