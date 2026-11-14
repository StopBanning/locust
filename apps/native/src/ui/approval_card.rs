use super::*;

pub(super) fn sync_approvals(s: &mut State) -> bool {
    if s.engine != Some(Kind::Native) {
        if s.pending_approval.is_some() {
            s.pending_approval = None;
            return true;
        }
        return false;
    }
    let waiting = locust_core::engine_api::api().waiting();
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
        s.question_answers = crate::question_card::Answers::default();
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
    match locust_core::engine_api::api().decide(&card.approval_id, decision, reason) {
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

fn runtime_display_name(runtime: &str) -> &'static str {
    match runtime {
        "claude" => "Claude Code",
        "acp" => "ACP agent",
        "opencode" => "OpenCode",
        "kilo" => "Kilo",
        "cursor" => "Cursor",
        "copilot" => "GitHub Copilot",
        "antigravity" => "Antigravity",
        "muse" => "Muse Code",
        _ => "Codex CLI",
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
    let asking = card.kind == approval::Kind::Question && !card.questions.is_empty();
    let mut body = col()
        .key(format!("approval-{}", card.approval_id))
        .w_full()
        .padding(14.0)
        .gap(8.0)
        .radius(8.0)
        .fill(p.raised)
        .border(1.0, p.amber)
        .child(
            text(if asking { "A question for you" } else { "Approval required — exact action" })
                .font(type_::TITLE)
                .color(p.text)
                .w_full()
                .wrap(),
        )
        .child(
            text(format!(
                "{}{}",
                runtime_display_name(&card.runtime),
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
    // A card a helper raised says whose it is (helperAskedBy).
    if let Some(pane) = &s.ledger
        && let Some(asked_by) = approval::helper_asked_by(approval::tool_use_of(&card.approval_id).as_deref(), &pane.items, Some(pane.header.name.as_str()))
    {
        body = body.child(text(format!("Asked by {asked_by}")).font(type_::LABEL).color(p.secondary).w_full().wrap().key("approval-asked-by"));
    }
    if let Some(plain) = approval::plain_action(card) {
        body = body.child(
            text(format!("In short: {plain}"))
                .font(type_::LABEL)
                .color(p.text)
                .w_full()
                .wrap(),
        );
    }
    if let Some(reach) = approval::command_reach_of(card) {
        body = body.child(
            text(format!("Reaches: {}", reach.said))
                .font(type_::LABEL)
                .color(p.amber)
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
    if let Some(patch) = &card.patch {
        let who = runtime_display_name(&card.runtime);
        let mut patch_box = col()
            .w_full()
            .gap(4.0)
            .padding(8.0)
            .radius(6.0)
            .fill(p.bg)
            .border(1.0, p.hairline)
            .child(
                row()
                    .w_full()
                    .gap(8.0)
                    .child(
                        text(format!("The change, as {who} would apply it"))
                            .font(type_::LABEL)
                            .color(p.text)
                            .wrap(),
                    )
                    .child(
                        text(format!("+{} −{}", patch.added, patch.removed))
                            .font(type_::LABEL)
                            .color(p.muted),
                    ),
            );
        let files = locust_core::changes::parse(&patch.text);
        if files.is_empty() {
            patch_box = patch_box.child(
                text("Codex sent a change this build could not read as a diff.")
                    .font(type_::LABEL)
                    .color(p.muted)
                    .w_full()
                    .wrap(),
            );
        } else {
            for file in files {
                patch_box = patch_box.child(
                    text(format!("{} · {}", file.path, file.status))
                        .font(type_::LABEL)
                        .color(p.secondary)
                        .w_full()
                        .wrap_anywhere(),
                );
                for hunk in file.hunks {
                    for row_line in hunk.rows.iter().take(40) {
                        let prefix = row_line.kind;
                        let color = match prefix {
                            '+' => p.green,
                            '-' => p.red,
                            _ => p.muted,
                        };
                        patch_box = patch_box.child(
                            text(format!("{prefix}{}", row_line.text))
                                .font(type_::LABEL)
                                .color(color)
                                .w_full()
                                .wrap_anywhere(),
                        );
                    }
                }
            }
        }
        body = body.child(patch_box);
    }
    let l = crate::look::controls::Look::new(p.dark);
    // A question is answered, never approved: its choices take the buttons' place.
    if card.kind == approval::Kind::Question && !card.questions.is_empty() {
        body = body.child(crate::question_card::questions(s, card, p));
        if let Some(notice) = &s.approval_notice {
            body = body.child(text(notice.clone()).font(type_::LABEL).color(p.amber).w_full().wrap());
        }
        return body;
    }
    if !s.approval_denying {
        body = body.child(
            row()
                .w_full()
                .gap(8.0)
                .child(
                    crate::look::controls::primary_button(&l, "Approve once", false)
                        .key("approve-once")
                        .on_press(|s: &mut State| {
                            decide_pending(s, approval::Decision::ApproveOnce)
                        }),
                )
                .child(if approval::asks_every_time(card) {
                    col()
                } else {
                    crate::look::controls::ghost_button(&l, "Always allow this session")
                        .key("approve-always")
                        .on_press(|s: &mut State| {
                            decide_pending(s, approval::Decision::ApproveAlways)
                        })
                })
                .child(if approval::can_save_allow_rule(card) {
                    crate::look::controls::ghost_button(&l, "Yes, and don't ask again")
                        .key("dont-ask-again")
                        .on_press(|s: &mut State| {
                            if let Some(card) = s.pending_approval.clone() {
                                match locust_core::engine_api::api().save_rule_and_decide(
                                    &card,
                                    crate::approval_rules::RuleEffect::Allow,
                                ) {
                                    Ok(()) => decide_pending(s, approval::Decision::ApproveOnce),
                                    Err(words) => {
                                        s.approval_notice = Some(words);
                                    }
                                }
                            }
                        })
                } else {
                    col()
                })
                .child(
                    crate::look::controls::deny_button(&l, "Deny…")
                        .key("deny")
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
                        crate::look::controls::deny_button(
                            &l,
                            if s.approval_reason.text.trim().is_empty() { "Deny" } else { "Deny and say why" },
                        )
                        .on_press(|s: &mut State| decide_pending(s, approval::Decision::Deny)),
                    )
                    .child(
                        crate::look::controls::ghost_button(&l, "Back")
                            .on_press(|s: &mut State| {
                                s.approval_denying = false;
                                s.approval_reason.clear();
                            }),
                    )
                    .child(
                        if s.pending_approval
                            .as_ref()
                            .is_some_and(approval::can_save_allow_rule)
                        {
                            crate::look::controls::deny_button(&l, "Never allow this (saves a rule)")
                                .on_press(|s: &mut State| {
                                    if let Some(card) = s.pending_approval.clone() {
                                        match locust_core::engine_api::api().save_rule_and_decide(
                                            &card,
                                            crate::approval_rules::RuleEffect::Deny,
                                        ) {
                                            Ok(()) => decide_pending(s, approval::Decision::Deny),
                                            Err(words) => {
                                                s.approval_notice = Some(words);
                                            }
                                        }
                                    }
                                })
                        } else {
                            col()
                        },
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

#[cfg(test)]
mod tests {
    use super::*;
    use locust_core::approval_patch::ToolPatch;
    use rune::{Env, Runtime};

    fn request(runtime: &str) -> approval::Request {
        approval::Request {
            approval_id: "ap_1".into(),
            mission_id: "mission_1".into(),
            runtime: runtime.into(),
            kind: approval::Kind::FileChange,
            summary: "Edit approval.txt".into(),
            detail: "approval.txt".into(),
            cwd: None,
            requested_at: "2026-09-27T10:00:00.000Z".into(),
            always_covers: None,
            always_key: None,
            data_sent_says: None,
            reversible_says: None,
            patch: Some(ToolPatch {
                text: "--- a/approval.txt\n+++ b/approval.txt\n@@ -1 +1 @@\n-draft\n+keep\n"
                    .into(),
                added: 1,
                removed: 1,
                truncated: false,
            }),
            claude_input: None,
            questions: Vec::new(),
        }
    }

    fn card_text(runtime: &str) -> String {
        let mut s = State::default();
        s.pending_approval = Some(request(runtime));
        let mut rt = Runtime::new(
            s,
            |s, _| approval_card(s, &Palette::new(true)),
            (600.0, 800.0),
            Env::new(true, 1.0),
        );
        rt.frame(0.0);
        rt.elements()
            .into_iter()
            .filter_map(|e| e.text)
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn is_headed_by_the_runtime_asking_not_by_codex() {
        let opencode = card_text("opencode");
        assert!(opencode.contains("The change, as OpenCode would apply it"), "{opencode}");
        assert!(!opencode.contains("as Codex"), "{opencode}");
        let claude = card_text("claude");
        assert!(claude.contains("The change, as Claude Code would apply it"), "{claude}");
    }

    #[test]
    fn still_says_codex_for_a_request_recorded_before_runtimes_were_named_on_it() {
        let recorded = card_text("");
        assert!(recorded.contains("The change, as Codex CLI would apply it"), "{recorded}");
    }
}
