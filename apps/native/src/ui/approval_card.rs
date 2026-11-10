use super::*;

pub(super) fn sync_approvals(s: &mut State) -> bool {
    if s.engine != Some(Kind::Native) {
        if s.pending_approval.is_some() {
            s.pending_approval = None;
            return true;
        }
        return false;
    }
    let waiting = native::waiting_approvals();
    let mission = s.ledger.as_ref().map(|pane| pane.snapshot.tip.as_str());
    let next = waiting
        .into_iter()
        .find(|request| Some(request.mission_id.as_str()) == mission);
    let changed = match (&s.pending_approval, &next) {
        (Some(a), Some(b)) => a.approval_id != b.approval_id,
        (None, None) => false,
        _ => true,
    };
    if changed {
        s.pending_approval = next;
        s.approval_shown = std::time::Instant::now();
        s.approval_denying = false;
        s.approval_reason.clear();
        if s.pending_approval.is_some() {
            let id = selected_conversation(s)
                .map(|(t, _)| t.id.clone())
                .unwrap_or_default();
            set_status(s, &id, "Needs you", StatusTone::Amber);
        }
    } else if s.pending_approval.is_some() {
        // Keep the amber "Needs you" while a card waits, even if Working… would overwrite it.
        if !s.status.starts_with("Needs you") && !s.status.contains("need you") {
            s.status = approval::needs_you_status(1);
            s.status_tone = StatusTone::Amber;
        }
    }
    changed
}

pub(super) fn decide_pending(s: &mut State, decision: approval::Decision) {
    let Some(card) = s.pending_approval.clone() else {
        return;
    };
    if decision != approval::Decision::Deny && s.approval_shown.elapsed().as_millis() < 500 {
        return;
    }
    let reason = s
        .approval_reason
        .text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let reason = if reason.is_empty() {
        None
    } else {
        Some(reason.chars().take(500).collect())
    };
    match native::decide_approval(&card.approval_id, decision, reason) {
        Ok(()) => {
            s.pending_approval = None;
            s.approval_notice = None;
            s.status = "Working…".into();
            s.status_tone = StatusTone::Green;
        }
        Err(words) => {
            s.pending_approval = None;
            s.approval_notice = Some(words.clone());
            s.status = words;
            s.status_tone = StatusTone::Amber;
        }
    }
}

pub(super) fn approval_card(s: &State, p: &Palette) -> El {
    let Some(card) = &s.pending_approval else {
        return col().w_full();
    };
    let detail = if card.detail.is_empty() {
        "—".into()
    } else {
        card.detail.clone()
    };
    let mut body = col()
        .key(format!("approval-{}", card.approval_id))
        .w_full()
        .padding(14.0)
        .gap(8.0)
        .radius(8.0)
        .fill(p.raised)
        .border(1.0, p.amber)
        .child(
            text("Approval required — exact action")
                .font(type_::TITLE)
                .color(p.text)
                .w_full()
                .wrap(),
        )
        .child(
            text(format!(
                "{}{}",
                match card.runtime.as_str() {
                    "claude" => "Claude Code",
                    "acp" => "ACP agent",
                    "opencode" => "OpenCode",
                    "kilo" => "Kilo",
                    "cursor" => "Cursor",
                    "copilot" => "GitHub Copilot",
                    "antigravity" => "Antigravity",
                    "muse" => "Muse Code",
                    _ => "Codex CLI",
                },
                if card.cwd.is_none() {
                    " · this workspace"
                } else {
                    ""
                }
            ))
            .font(type_::LABEL)
            .color(p.muted)
            .w_full()
            .wrap_anywhere(),
        )
        .child(
            text(format!("Action: {}", card.summary))
                .font(type_::BODY)
                .color(p.text)
                .w_full()
                .wrap(),
        )
        .child(
            text(format!("Exact: {detail}"))
                .font(type_::LABEL)
                .color(p.secondary)
                .w_full()
                .wrap_anywhere(),
        );
    if let Some(plain) = approval::plain_action(card) {
        body = body.child(
            text(format!("In short: {plain}"))
                .font(type_::LABEL)
                .color(p.text)
                .w_full()
                .wrap(),
        );
    }
    body = body.child(
        text("Where: ".to_string() + card.cwd.as_deref().unwrap_or("the project folder"))
            .font(type_::LABEL)
            .color(p.muted)
            .w_full()
            .wrap_anywhere(),
    );
    for (label, words) in approval::effect_lines(card) {
        body = body.child(
            text(format!("{label}: {words}"))
                .font(type_::LABEL)
                .color(p.secondary)
                .w_full()
                .wrap(),
        );
    }
    if !s.approval_denying {
        body = body.child(
            row()
                .w_full()
                .gap(8.0)
                .child(
                    el().key("approve-once")
                        .h(36.0)
                        .px(14.0)
                        .center()
                        .radius(8.0)
                        .fill(p.selected)
                        .border(1.0, p.hairline)
                        .child(text("Approve once").font(type_::LABEL).color(p.text))
                        .on_press(|s: &mut State| {
                            decide_pending(s, approval::Decision::ApproveOnce)
                        }),
                )
                .child(if approval::asks_every_time(card) {
                    col()
                } else {
                    el().key("approve-always")
                        .h(36.0)
                        .px(14.0)
                        .center()
                        .radius(8.0)
                        .fill(p.raised)
                        .border(1.0, p.hairline)
                        .child(
                            text("Always allow this session")
                                .font(type_::LABEL)
                                .color(p.text),
                        )
                        .on_press(|s: &mut State| {
                            decide_pending(s, approval::Decision::ApproveAlways)
                        })
                })
                .child(
                    el().key("deny")
                        .h(36.0)
                        .px(14.0)
                        .center()
                        .radius(8.0)
                        .fill(p.raised)
                        .border(1.0, p.red)
                        .child(text("Deny…").font(type_::LABEL).color(p.red))
                        .on_press(|s: &mut State| {
                            s.approval_denying = true;
                        }),
                ),
        );
    }
    if s.approval_denying {
        body = body
            .child(
                text_input(&s.approval_reason, type_::BODY)
                    .key("approval-reason")
                    .single_line()
                    .placeholder(
                        "Why? Optional. The teammate reads it and can change course.",
                        p.muted,
                    )
                    .w_full()
                    .h(36.0)
                    .on_edit(|s: &mut State| &mut s.approval_reason)
                    .on_submit(|s: &mut State, _: &mut Tx| {
                        decide_pending(s, approval::Decision::Deny)
                    })
                    .on_input_key(|s: &mut State, event: &KeyEvent, _: &mut Tx| {
                        if event.key == Key::Escape {
                            s.approval_denying = false;
                            s.approval_reason.clear();
                            true
                        } else {
                            false
                        }
                    }),
            )
            .child(
                row()
                    .gap(8.0)
                    .child(
                        el().h(36.0)
                            .px(12.0)
                            .child(
                                text(if s.approval_reason.text.trim().is_empty() {
                                    "Deny"
                                } else {
                                    "Deny and say why"
                                })
                                .color(p.red),
                            )
                            .on_press(|s: &mut State| decide_pending(s, approval::Decision::Deny)),
                    )
                    .child(
                        el().h(36.0)
                            .px(12.0)
                            .child(text("Back").color(p.text))
                            .on_press(|s: &mut State| {
                                s.approval_denying = false;
                                s.approval_reason.clear();
                            }),
                    ),
            );
    }
    body = body.child(
        text(approval::card_note(card))
            .font(type_::LABEL)
            .color(p.muted)
            .w_full()
            .wrap(),
    );
    if let Some(notice) = &s.approval_notice {
        body = body.child(
            text(notice.clone())
                .font(type_::LABEL)
                .color(p.amber)
                .w_full()
                .wrap(),
        );
    }
    body
}
