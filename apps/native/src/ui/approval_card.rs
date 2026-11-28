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

/// The pane can change before the engine poll replaces the cached card. Never display or answer the old turn's card
/// in that gap. A standalone card (without a running window engine) is used in the app's scenes and previews.
pub(super) fn active_approval(s: &State) -> Option<&approval::Request> {
    s.pending_approval.as_ref().filter(|card| {
        s.engine != Some(Kind::Native)
            || s.ledger.as_ref().is_some_and(|pane| pane.snapshot.tip == card.mission_id)
    })
}

pub(super) fn decide_pending(s: &mut State, decision: approval::Decision) {
    let Some(card) = active_approval(s).cloned() else {
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

/// U+2060 WORD JOINER: drawn between a flag's dashes and its first letter, never copied.
pub(super) const FLAG_HOLD: char = '\u{2060}';

/// CommandText.tsx (0.837): a command that wraps at its spaces. A line may break after any hyphen, so a flag could be cut
/// from its dash (`powershell.exe" -` / `Command`); each flag's dashes (`(?<=^|\s)-+[^\s-]`) are held to its first letter
/// by a word joiner, and every other place a line could break stays. Returns the text to draw and the joiners' byte
/// ranges, which draw but are not copied: what a copy takes is the command exactly as sent.
pub(super) fn held_command(command: &str) -> (String, Vec<std::ops::Range<usize>>) {
    let chars: Vec<char> = command.chars().collect();
    let (mut shown, mut holds) = (String::with_capacity(command.len()), Vec::new());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '-' && (i == 0 || chars[i - 1].is_whitespace()) {
            let mut end = i;
            while end < chars.len() && chars[end] == '-' {
                end += 1;
            }
            shown.extend(&chars[i..end]);
            if end < chars.len() && !chars[end].is_whitespace() {
                let at = shown.len();
                shown.push(FLAG_HOLD);
                holds.push(at..shown.len());
            }
            i = end;
            continue;
        }
        shown.push(chars[i]);
        i += 1;
    }
    (shown, holds)
}

/// `prefix` then `command` held at its flags (`held_command`), in one text element whose joiners are not copied.
pub(super) fn command_text(prefix: &str, command: &str) -> El {
    let (shown, holds) = held_command(command);
    let mut line = text(format!("{prefix}{shown}"));
    for hold in holds {
        line = line.uncopied(prefix.len() + hold.start..prefix.len() + hold.end);
    }
    line
}

fn plain_action_for(card: &approval::Request) -> Option<locust_core::command_plain::PlainCommand> {
    (card.kind == approval::Kind::Command)
        .then(|| locust_core::command_plain::plain_command(&card.detail))
        .flatten()
}

pub(super) fn approval_card(s: &State, p: &Palette) -> El {
    let Some(card) = active_approval(s) else {
        return col().w_full();
    };
    let l = crate::look::controls::Look::new(p.dark);
    // `.lc-card__head` (ApprovalCard.tsx): the shield and the title in amber on the left; on the right which program is
    // asking, its mark then its name in the code face (`lc-rail__meta`), so a card says who asks at a glance.
    let amber = l.c("amber-text");
    // THE CARD SAYS WHO WANTS WHAT, IN PLAIN WORDS (REVIEW-ui-changes-2026-11-26.md change 5, Colin's yes 26 Nov).
    // Electron titles every card "Approval required — exact action" and repeats itself with "Action: Run a command"
    // (ApprovalCard.tsx:366, :387-434); here the title is the teammate and what it wants, the command right under it.
    let who = s.ledger.as_ref().map(|pane| pane.header.name.trim().to_owned()).filter(|name| !name.is_empty())
        .unwrap_or_else(|| runtime_display_name(&card.runtime).to_owned());
    let title = match card.kind {
        approval::Kind::Question => format!("{who} is asking you something"),
        approval::Kind::Command => format!("{who} wants to run a command"),
        approval::Kind::FileChange => format!("{who} wants to change files"),
        approval::Kind::Connector => format!("{who} wants to use a connector"),
    };
    let mut asker = row().key("approval-asker").gap(5.0).center_y().shrink();
    if let Some(mark) = crate::look::controls::agent_mark(&l, &card.runtime, 12.0, l.c("text-muted")) {
        asker = asker.child(mark.key(format!("approval-mark-{}", card.runtime)));
    }
    asker = asker.child(
        text(format!("{}{}", runtime_display_name(&card.runtime), if card.cwd.is_none() { " · this workspace" } else { "" }))
            .key("approval-runtime")
            .font(l.ty.mono)
            .color(l.c("text-muted"))
            .shrink()
            .wrap_anywhere(),
    );
    let head = row()
        .key("approval-head")
        .w_full()
        .center_y()
        .gap(12.0)
        .py(8.0)
        .px(12.0)
        .child(
            row()
                .key("approval-title")
                .gap(6.0)
                .center_y()
                .grow(1.0)
                .shrink()
                .child(crate::look::controls::glyph(&l, &crate::look::icons::SHIELD, 13.0, Some(amber)).key("approval-shield"))
                .child(
                    text(title)
                        .font(l.ty.control)
                        .color(amber)
                        .shrink()
                        .wrap(),
                ),
        )
        .child(asker);
    // `.lc-receipt`: what is asked, as a list of facts, each its label in a column of its own and its value beside it, the
    // values starting at one edge (`grid-template-columns: max-content minmax(0, 60ch)`). The labels are the interface's
    // face, sentence case, a weight up and a shade down (shell.css 0.862, which retired the spaced mono capitals); the
    // exact command and the folder are in the code face (`lc-mono`).
    let secondary = l.c("text-secondary");
    let words = |said: String, ink: rune::Color| text(said).font(l.ty.body).color(ink).wrap_anywhere();
    let mut facts: Vec<(&'static str, String, El, String)> = Vec::new();
    let asked_by = s.ledger.as_ref().and_then(|pane| {
        approval::helper_asked_by(approval::tool_use_of(&card.approval_id).as_deref(), &pane.items, Some(pane.header.name.as_str()))
    });
    // A command's title already says what it is, so the command itself is the first row and no "Action" row repeats
    // the title; any other card keeps Electron's order (Asked by, Action, Exact).
    let command_first = card.kind == approval::Kind::Command && !card.detail.is_empty();
    if !command_first {
        if let Some(asked_by) = asked_by.clone() {
            facts.push(("Asked by", "approval-asked-by".into(), words(asked_by.clone(), secondary), asked_by));
        }
        facts.push(("Action", "approval-action".into(), words(card.summary.clone(), secondary), card.summary.clone()));
    }
    if !card.detail.is_empty() {
        // A command is drawn held at its flags (ApprovalCard.tsx `CommandText`); a connector call's detail as it is.
        let exact = if card.kind == approval::Kind::Command { command_text("", &card.detail) } else { text(card.detail.clone()) };
        facts.push((
            if card.kind == approval::Kind::Question { "Question" } else { "Exact" },
            "approval-exact".into(),
            exact.font(l.ty.body.mono()).color(secondary).wrap_anywhere(),
            card.detail.clone(),
        ));
    }
    if command_first && let Some(asked_by) = asked_by {
        facts.push(("Asked by", "approval-asked-by".into(), words(asked_by.clone(), secondary), asked_by));
    }
    let careful = plain_action_for(card).is_some_and(|plain| plain.careful);
    if let Some(plain) = plain_action_for(card) {
        let ink = if plain.careful { amber } else { secondary };
        facts.push(("In short", "approval-plain".into(), words(plain.said.clone(), ink), plain.said));
    }
    if let Some(reach) = approval::command_reach_of(card) {
        facts.push(("Reaches", "approval-reach".into(), words(reach.said.clone(), amber), reach.said));
    }
    let place = card.cwd.clone().unwrap_or_else(|| "the project folder".into());
    facts.push(("Where", "approval-where".into(), text(place.clone()).font(l.ty.body.mono()).color(secondary).wrap_anywhere(), place));
    // The two lines of "Effects: Unknown…" on every command are boilerplate a person learns to skip, and then skips the
    // rows that matter beside them (change 5): they show only for a command the card already calls careful. Known
    // effects (what is sent, whether it can be undone) always show.
    for (label, said) in approval::effect_lines(card).into_iter().filter(|(label, _)| *label != "Effects" || careful) {
        let key = format!("approval-{}", label.to_lowercase().replace(' ', "-"));
        facts.push((label, key, words(said.clone(), secondary), said));
    }
    let label_face = crate::look::type_::FACT_LABEL;
    let label_column = facts.iter().map(|(label, ..)| rune::text::measure(label, &[], label_face, None).0).fold(0.0, f32::max);
    let value_cap = rune::text::measure("0", &[], l.ty.body, None).0 * 60.0;
    let mut receipt = col().key("approval-receipt").w_full().gap(6.0).px(12.0).pb(12.0);
    for (label, key, value, said) in facts {
        receipt = receipt.child(
            row()
                .key(format!("approval-row-{}", key.trim_start_matches("approval-")))
                .label(format!("{label}: {said}"))
                .w_full()
                .gap(16.0)
                .child(text(label).key(format!("approval-label-{}", key.trim_start_matches("approval-"))).font(label_face).color(secondary).w(label_column).pt(2.0))
                .child(value.key(key).grow(1.0).shrink().max_w(value_cap)),
        );
    }
    let mut body = col().key("approval-body").w_full().gap(8.0).px(12.0).pb(12.0);
    // `.lc-card.is-pending.is-amber`: raised toward the reader on the pending fill, a translucent amber edge, a 3-point
    // amber ring and the pending shadow (shell.css).
    let shadows = l.t.shadows("shadow-pending");
    let (card_key, radius, fill, edge, glow) =
        (format!("approval-{}", card.approval_id), l.t.px("radius-card"), l.c("bg-pending"), l.c("amber-border"), l.c("amber-tint"));
    let finish = move |body: El| -> El {
        let mut card_box = col()
            .key(card_key)
            .w_full()
            .radius(radius)
            .fill(fill)
            .border(1.0, edge)
            .box_shadow(rune::Shadow { spread: 3.0, color: glow, ..rune::Shadow::default() })
            .role("group")
            .label("Approval required");
        for shadow in shadows {
            card_box = card_box.box_shadow(shadow);
        }
        card_box.child(head).child(receipt).child(body)
    };
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
                // DiffView.tsx draws the change (diff_view.rs): the first two hunks, the unchanged lines folded, what
                // changed within a line marked, and a closing statement of completeness.
                let reported = if patch.truncated { (patch.added, patch.removed) } else { file.counts() };
                let copy = file.after().map(|after| crate::reply_actions::copy_button(s.copy_feedback.as_ref(), after, p));
                patch_box = patch_box.child(crate::diff_view::diff_view(
                    s,
                    crate::diff_view::Place::of("approval", &file),
                    &file,
                    patch.truncated,
                    Some(reported),
                    (None, false),
                    copy,
                    p,
                ));
            }
        }
        body = body.child(patch_box);
    }
    // A question is answered, never approved: its choices take the buttons' place.
    if card.kind == approval::Kind::Question && !card.questions.is_empty() {
        body = body.child(crate::question_card::questions(s, card, p));
        if let Some(notice) = &s.approval_notice {
            body = body.child(text(notice.clone()).font(type_::LABEL).color(p.amber).w_full().wrap());
        }
        return finish(body);
    }
    if !s.approval_denying {
        body = body.child(
            row()
                .w_full()
                .gap(8.0)
                .child(
                    crate::look::controls::primary_button(&l, "Allow once", false)
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
                    crate::look::controls::ghost_button(&l, "Allow this command from now on")
                        .key("dont-ask-again")
                        .on_press(|s: &mut State| {
                            if let Some(card) = active_approval(s).cloned() {
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
                                    if let Some(card) = active_approval(s).cloned() {
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
    finish(body)
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
            skippable: false, answer_in: None, claude_input: None,
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

    /// the-runtimes-wear-their-marks.test.tsx: "an approval card says which program is asking with its mark".
    #[test]
    fn an_approval_card_says_which_program_is_asking_with_its_mark() {
        let mut s = State::default();
        s.pending_approval = Some(request("copilot"));
        let mut rt = Runtime::new(s, |s, _| approval_card(s, &Palette::new(true)), (600.0, 800.0), Env::new(true, 1.0));
        rt.frame(0.0);
        let elements = rt.elements();
        assert!(runtime_display_name("copilot").contains("Copilot"));
        let mark = elements.iter().position(|e| e.key.as_deref() == Some("approval-mark-copilot")).expect("the mark");
        let name = elements.iter().position(|e| e.key.as_deref() == Some("approval-runtime") && e.text.as_deref().is_some_and(|t| t.starts_with(&runtime_display_name("copilot")))).expect("the name");
        assert!(mark < name, "the mark, then its name");
    }

    #[test]
    fn still_says_codex_for_a_request_recorded_before_runtimes_were_named_on_it() {
        let recorded = card_text("");
        assert!(recorded.contains("The change, as Codex CLI would apply it"), "{recorded}");
    }
}

#[cfg(test)]
#[path = "commands_card_tests.rs"]
mod commands_card_tests;

#[cfg(test)]
#[path = "approval_shape_tests.rs"]
mod approval_shape_tests;

#[cfg(test)]
#[path = "command_wraps_tests.rs"]
mod command_wraps_tests;

#[cfg(test)]
#[path = "approval_active_thread_tests.rs"]
mod approval_active_thread_tests;
