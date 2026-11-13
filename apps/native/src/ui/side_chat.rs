//! Ask about a selection (`SelectionAsk.tsx`) and the read-only side chat (`SideChat.tsx`).

use super::*;
use locust_core::{
    approval::{Decision, Mode, Request},
    engine_api::api,
    native::{Origin, Send},
    side_question::can_fork_runtime,
};
use serde_json::json;

/// A quote longer than this is cut, and says so: the box is for a message, not a copy of the reply.
pub const MAX_QUOTE: usize = 1_500;

/// Format selected reply words as a Markdown blockquote, matching Electron `SelectionAsk.tsx:quoteOf`.
pub fn quote_of(text: &str) -> String {
    let norm = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut flat = String::with_capacity(norm.len());
    let mut newlines = 0usize;
    for ch in norm.chars() {
        if ch == '\n' {
            newlines += 1;
            if newlines <= 2 {
                flat.push(ch);
            }
        } else {
            newlines = 0;
            flat.push(ch);
        }
    }
    let flat = flat.trim();
    let cut = if flat.encode_utf16().count() <= MAX_QUOTE {
        flat.to_owned()
    } else {
        let mut units = 0usize;
        let mut end = 0usize;
        for (idx, ch) in flat.char_indices() {
            let len = ch.len_utf16();
            if units + len > MAX_QUOTE - 1 {
                break;
            }
            units += len;
            end = idx + ch.len_utf8();
        }
        format!("{}\u{2026}", flat[..end].trim_end())
    };
    cut.split('\n')
        .map(|line| {
            if line.trim().is_empty() {
                ">".to_owned()
            } else {
                format!("> {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectionAskState {
    pub scope: String,
    pub text: String,
}

pub struct SideTurn {
    pub mission_id: String,
    pub prompt: String,
    pub items: Vec<ledger::Item>,
    pub phase: String,
    pub error: Option<String>,
    pub session: Option<ledger::Session>,
}

pub struct SideChatState {
    pub of: String,
    pub runtime: String,
    pub model: String,
    pub raw_model: Option<String>,
    pub teammate: Teammate,
    pub turns: Vec<SideTurn>,
    pub draft: TextEdit,
    pub problem: Option<String>,
    pub approvals: Vec<Request>,
    pub asking: bool,
}

const GOING: &[&str] = &["starting", "running", "cancelling"];

pub fn is_going(phase: &str) -> bool {
    GOING.contains(&phase)
}

pub fn can_open(runtime: Option<&str>, conversation_id: Option<&str>) -> bool {
    conversation_id.is_some_and(|id| !id.is_empty())
        && runtime.is_some_and(can_fork_runtime)
}

fn can_open_for_state(s: &State) -> bool {
    let Some((teammate, conversation)) = selected_conversation(s) else {
        return false;
    };
    let tip = s
        .ledger
        .as_ref()
        .map(|p| p.snapshot.tip.as_str())
        .filter(|t| !t.is_empty())
        .or(conversation.conversation_id.as_deref());
    let runtime = s
        .picks
        .get(&teammate.id)
        .and_then(|p| p.runtime.as_deref())
        .or(teammate.runtime.as_deref());
    can_open(runtime, tip)
}

pub(super) fn open_from_state(s: &mut State, tx: &mut Tx) {
    let Some((teammate, conversation)) = selected_conversation(s) else {
        return;
    };
    let teammate = teammate.clone();
    let Some(of) = s
        .ledger
        .as_ref()
        .map(|p| p.snapshot.tip.clone())
        .filter(|t| !t.is_empty())
        .or_else(|| conversation.conversation_id.clone())
    else {
        return;
    };
    let runtime = s
        .picks
        .get(&teammate.id)
        .and_then(|p| p.runtime.clone())
        .or_else(|| teammate.runtime.clone())
        .unwrap_or_else(|| "codex".into());
    if !can_fork_runtime(&runtime) {
        return;
    }
    let raw_model = s
        .picks
        .get(&teammate.id)
        .map(|p| p.model.clone())
        .or_else(|| teammate.model.clone());
    let model = raw_model
        .clone()
        .filter(|m| !m.is_empty() && m != "account-default")
        .unwrap_or_else(|| "this model".into());
    if let Some(existing) = s.side_chat.as_mut().filter(|c| c.of == of) {
        existing.draft.focused = true;
        return;
    }
    close(s);
    let mut draft = TextEdit::new("");
    draft.focused = true;
    s.side_chat = Some(SideChatState {
        of,
        runtime,
        model,
        raw_model,
        teammate: teammate.clone(),
        turns: Vec::new(),
        draft,
        problem: None,
        approvals: Vec::new(),
        asking: false,
    });
    let _ = tx;
}

pub(super) fn close(s: &mut State) {
    let Some(side) = s.side_chat.take() else {
        return;
    };
    for turn in &side.turns {
        if is_going(&turn.phase) {
            let _ = api().stop(&turn.mission_id);
        }
    }
}

pub(super) fn send(s: &mut State) {
    let Some(side) = s.side_chat.as_mut() else {
        return;
    };
    let question = side.draft.text.trim().to_owned();
    let answering = side.turns.last().is_some_and(|t| is_going(&t.phase));
    if question.is_empty() || side.asking || answering {
        return;
    }
    side.asking = true;
    side.problem = None;
    let of = side
        .turns
        .last()
        .map(|t| t.mission_id.clone())
        .unwrap_or_else(|| side.of.clone());
    let question_num = side.turns.len() + 1;
    let mut teammate = side.teammate.clone();
    teammate.runtime = Some(side.runtime.clone());
    let job = Send {
        teammate,
        conversation: None,
        prompt: question.clone(),
        model: side
            .raw_model
            .clone()
            .filter(|m| !m.is_empty() && m != "account-default"),
        effort: None,
    };
    let origin = Origin {
        mode: Some(Mode::Ask),
        folder: None,
        started_by: Some(json!({
            "kind": "side",
            "of": of,
            "question": question_num,
        })),
        runtime: Some(side.runtime.clone()),
        account: None,
        reference_ids: None,
        slot: None,
    };
    let result = api().send(job, origin);
    let Some(side) = s.side_chat.as_mut() else {
        return;
    };
    side.asking = false;
    match result {
        Ok(started) => {
            let session = s
                .data
                .as_ref()
                .and_then(|store| store.ledger_dir().ok())
                .map(|dir| native::session(dir, started.conversation_id.clone()));
            side.turns.push(SideTurn {
                mission_id: started.conversation_id,
                prompt: question,
                items: Vec::new(),
                phase: "running".into(),
                error: None,
                session,
            });
            side.draft.clear();
        }
        Err(why) => {
            side.problem = Some(why);
        }
    }
}

pub(super) fn stop_latest(s: &mut State) {
    let Some(side) = s.side_chat.as_mut() else {
        return;
    };
    let Some(latest) = side.turns.last_mut() else {
        return;
    };
    if !is_going(&latest.phase) {
        return;
    }
    match api().stop(&latest.mission_id) {
        Ok(_) => {
            latest.phase = "cancelling".into();
        }
        Err(why) => {
            side.problem = Some(why);
        }
    }
}

pub(super) fn poll(s: &mut State) -> bool {
    let Some(side) = s.side_chat.as_mut() else {
        return false;
    };
    let mut changed = false;
    for turn in &mut side.turns {
        if let Some(session) = turn.session.as_mut()
            && let Ok(snap) = session.poll()
        {
            if snap.items != turn.items {
                turn.items = snap.items;
                changed = true;
            }
            let stopped = turn.items.iter().any(|item| {
                matches!(item, ledger::Item::Note { text, .. } if text == "Stopped")
            });
            let next_phase = if snap.running {
                if turn.phase == "cancelling" {
                    "cancelling"
                } else {
                    "running"
                }
            } else if stopped {
                "cancelled"
            } else if snap.failed {
                "failed"
            } else {
                "completed"
            };
            if turn.phase != next_phase {
                turn.phase = next_phase.into();
                changed = true;
            }
        }
    }
    let waiting: Vec<Request> = side
        .turns
        .iter()
        .flat_map(|t| api().waiting_for(&t.mission_id))
        .collect();
    let same_approvals = waiting.len() == side.approvals.len()
        && waiting
            .iter()
            .zip(side.approvals.iter())
            .all(|(a, b)| a.approval_id == b.approval_id);
    if !same_approvals {
        side.approvals = waiting;
        changed = true;
    }
    changed
}

pub(super) fn header_button(s: &State, p: &Palette) -> Option<El> {
    if !can_open_for_state(s) {
        return None;
    }
    let l = crate::look::controls::Look::new(p.dark);
    let active = s.side_chat.is_some();
    Some(
        el().key("ask-on-side-button")
            .px(10.0)
            .py(5.0)
            .radius(l.t.px("radius-control"))
            .fill(if active { l.c("bg-selected") } else { l.c("bg-control") })
            .border(1.0, l.c("border-strong"))
            .hover(l.c("bg-selected"))
            .child(text("Ask on the side").font(l.ty.label).color(l.c("text-primary")))
            .role("button")
            .label("Ask on the side")
            .on_press_tx(|s: &mut State, tx| open_from_state(s, tx)),
    )
}

pub(super) fn selection_ask_chip(s: &State, p: &Palette) -> Option<El> {
    let ask = s.selection_ask.as_ref()?;
    if ask.text.trim().is_empty() {
        return None;
    }
    let l = crate::look::controls::Look::new(p.dark);
    let selected_text = ask.text.clone();
    Some(
        el().key("selection-ask")
            .pin(None, None, Some(22.0), Some(14.0))
            .px(12.0)
            .py(6.0)
            .radius(999.0)
            .fill(l.c("bg-raised-2"))
            .border(1.0, l.c("border-strong"))
            .hover(l.c("bg-selected"))
            .child(
                text("Ask about this")
                    .font(l.ty.label)
                    .color(l.c("text-primary")),
            )
            .role("button")
            .label("Ask about this")
            // Electron's App: the quote goes into the conversation's own message box (Composer `quoteIn`), above
            // whatever is being written there. The side chat opens from its own button.
            .on_press(move |s: &mut State| {
                let quote = quote_of(&selected_text);
                let next = if s.draft.text.trim().is_empty() {
                    format!("{quote}\n\n")
                } else {
                    format!("{quote}\n\n{}", s.draft.text)
                };
                s.draft.replace_text(next);
                s.draft.focused = true;
                s.selection_ask = None;
            }),
    )
}

pub(super) fn panel(s: &State, p: &Palette) -> Option<El> {
    let side = s.side_chat.as_ref()?;
    Some(render_panel(side, p))
}

pub(crate) fn render_panel(side: &SideChatState, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let latest = side.turns.last();
    let answering = latest.is_some_and(|t| is_going(&t.phase));

    let mut thread = col().w_full().gap(10.0).padding(12.0);
    if side.turns.is_empty() {
        thread = thread.child(
            text(
                "Ask anything about this conversation: why it chose something, or what it has done so far. It keeps working while you ask. Nothing asked here reaches it. The answer comes from a read-only copy, which changes no file here; your connectors still work in it, as they do everywhere.",
            )
            .font(type_::BODY)
            .color(p.muted)
            .w_full()
            .wrap(),
        );
    } else {
        for turn in &side.turns {
            thread = thread.child(
                col()
                    .w_full()
                    .padding(10.0)
                    .radius(8.0)
                    .fill(p.your_bubble)
                    .child(
                        text(turn.prompt.clone())
                            .font(type_::BODY)
                            .color(p.text)
                            .w_full()
                            .wrap_anywhere(),
                    ),
            );
            let mut has_reply = false;
            for item in &turn.items {
                match item {
                    ledger::Item::Assistant { text: body, .. } => {
                        let shown = locust_core::protocol_tags::shown_text(body);
                        if !shown.is_empty() {
                            has_reply = true;
                            thread = thread.child(
                                text(shown)
                                    .font(type_::BODY)
                                    .color(p.text)
                                    .w_full()
                                    .wrap_anywhere()
                                    .selectable(true),
                            );
                        }
                    }
                    ledger::Item::Note { level, text: body } => {
                        let line = if level.is_empty() {
                            body.clone()
                        } else {
                            format!("{level}: {body}")
                        };
                        thread = thread.child(
                            text(line)
                                .font(type_::LABEL)
                                .color(p.muted)
                                .w_full()
                                .wrap_anywhere(),
                        );
                    }
                    _ => {}
                }
            }
            if let Some(err) = &turn.error {
                thread = thread.child(
                    text(err.clone())
                        .font(type_::LABEL)
                        .color(p.red)
                        .w_full()
                        .wrap_anywhere(),
                );
            } else if !has_reply && is_going(&turn.phase) {
                thread = thread.child(
                    text("Answering...")
                        .font(type_::LABEL)
                        .color(p.muted)
                        .w_full(),
                );
            } else if turn.phase == "cancelled" {
                thread = thread.child(
                    text("Stopped")
                        .font(type_::LABEL)
                        .color(p.muted)
                        .w_full(),
                );
            }
        }
    }

    for req in &side.approvals {
        let approval_id = req.approval_id.clone();
        let deny_id = req.approval_id.clone();
        let mut card = col()
            .w_full()
            .gap(6.0)
            .padding(10.0)
            .radius(8.0)
            .fill(p.raised)
            .border(1.0, p.amber)
            .child(
                text(req.summary.clone())
                    .font(type_::BODY)
                    .color(p.text)
                    .w_full()
                    .wrap_anywhere(),
            );
        if !req.detail.trim().is_empty() {
            card = card.child(
                text(req.detail.clone())
                    .font(type_::LABEL)
                    .color(p.muted)
                    .w_full()
                    .wrap_anywhere(),
            );
        }
        card = card.child(
            row()
                .w_full()
                .gap(8.0)
                .child(
                    el().key(format!("side-approve-{}", req.approval_id))
                        .px(10.0)
                        .py(5.0)
                        .radius(6.0)
                        .fill(p.selected)
                        .child(
                            text("Approve once")
                                .font(type_::LABEL)
                                .color(p.text),
                        )
                        .role("button")
                        .label("Approve once")
                        .on_press(move |_s: &mut State| {
                            let _ = api().decide(&approval_id, Decision::ApproveOnce, None);
                        }),
                )
                .child(
                    el().key(format!("side-deny-{}", req.approval_id))
                        .px(10.0)
                        .py(5.0)
                        .radius(6.0)
                        .fill(p.field)
                        .child(text("Deny").font(type_::LABEL).color(p.secondary))
                        .role("button")
                        .label("Deny")
                        .on_press(move |_s: &mut State| {
                            let _ = api().decide(&deny_id, Decision::Deny, None);
                        }),
                ),
        );
        thread = thread.child(card);
    }

    let mut form = col()
        .w_full()
        .gap(6.0)
        .padding(10.0)
        .border_sides([1.0, 0.0, 0.0, 0.0], p.hairline);
    if let Some(problem) = &side.problem {
        form = form.child(
            text(problem.clone())
                .font(type_::LABEL)
                .color(p.red)
                .w_full()
                .wrap_anywhere(),
        );
    }
    let placeholder = if answering {
        "Answering..."
    } else {
        "Ask about this conversation..."
    };
    let input = text_input(&side.draft, type_::BODY)
        .key("side-chat-input")
        .placeholder(placeholder, p.muted)
        .color(p.text)
        .padding(8.0)
        .radius(6.0)
        .fill(p.field)
        .border(1.0, p.hairline)
        .grow(1.0)
        .on_edit(|s: &mut State| &mut s.side_chat.as_mut().expect("side_chat").draft)
        .on_submit(|s: &mut State, _tx: &mut Tx| send(s));

    let send_or_stop = if answering {
        let cancelling = latest.is_some_and(|t| t.phase == "cancelling");
        let mut btn = el()
            .key("side-chat-stop")
            .px(10.0)
            .py(6.0)
            .radius(6.0)
            .fill(p.red)
            .child(text("Stop").font(type_::LABEL).color(p.text))
            .role("button")
            .label("Stop this answer");
        if !cancelling {
            btn = btn.on_press(|s: &mut State| stop_latest(s));
        }
        btn
    } else {
        let disabled = side.draft.text.trim().is_empty() || side.asking;
        let mut btn = el()
            .key("side-chat-send")
            .px(10.0)
            .py(6.0)
            .radius(6.0)
            .fill(if disabled { p.field } else { p.selected })
            .child(
                text("Ask")
                    .font(type_::LABEL)
                    .color(if disabled { p.muted } else { p.text }),
            )
            .role("button")
            .label("Ask on the side");
        if !disabled {
            btn = btn.on_press(|s: &mut State| send(s));
        }
        btn
    };

    form = form.child(row().w_full().gap(8.0).center_y().child(input).child(send_or_stop));

    col()
        .key("side-chat-panel")
        .w(340.0)
        .h_full()
        .fill(p.sidebar)
        .border_sides([0.0, 0.0, 0.0, 1.0], p.hairline)
        .role("complementary")
        .label("On the side")
        .child(
            row()
                .w_full()
                .center_y()
                .gap(8.0)
                .px(12.0)
                .py(10.0)
                .border_sides([0.0, 0.0, 1.0, 0.0], p.hairline)
                .child(
                    col()
                        .grow(1.0)
                        .shrink()
                        .gap(2.0)
                        .child(
                            text("On the side")
                                .font(type_::TITLE)
                                .color(p.text)
                                .truncate(),
                        )
                        .child(
                            text(format!("{} · a copy of this conversation", side.model))
                                .font(type_::LABEL)
                                .color(p.muted)
                                .truncate(),
                        ),
                )
                .child(
                    el().key("side-chat-close")
                        .px(8.0)
                        .py(4.0)
                        .radius(l.t.px("radius-control"))
                        .hover(l.c("hover-surface"))
                        .child(text("Close").font(type_::LABEL).color(p.muted))
                        .role("button")
                        .label("Close")
                        .on_press(|s: &mut State| close(s)),
                ),
        )
        .child(
            col()
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec { stick_bottom: true })
                .child(thread),
        )
        .child(form)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Env, Runtime};

    fn sample_side(phase: &str, approvals: Vec<Request>) -> SideChatState {
        SideChatState {
            of: "mission_convo".into(),
            runtime: "claude".into(),
            model: "Opus 5.5".into(),
            raw_model: None,
            teammate: Teammate {
                id: "tm_1".into(),
                name: "Codec".into(),
                role: "Code".into(),
                runtime: Some("claude".into()),
                model: None,
                account: None,
                busy: false,
                runs_in: "ask".into(),
                folder: None,
            },
            turns: vec![SideTurn {
                mission_id: "mission_side".into(),
                prompt: "Why did it pick Postgres?".into(),
                items: Vec::new(),
                phase: phase.into(),
                error: None,
                session: None,
            }],
            draft: TextEdit::new(""),
            problem: None,
            approvals,
            asking: false,
        }
    }

    fn render_texts_and_labels(side: SideChatState) -> (Vec<String>, Vec<Option<String>>) {
        let mut s = State::default();
        s.side_chat = Some(side);
        let mut rt = Runtime::new(s, |s, _| panel(s, &Palette::new(true)).unwrap_or_else(el), (400.0, 600.0), Env::new(true, 1.0));
        rt.frame(0.0);
        let elems = rt.elements();
        let texts = elems.iter().filter_map(|e| e.text.clone()).collect();
        let labels = elems.iter().map(|e| e.label.clone()).collect();
        (texts, labels)
    }

    // a-part-of-a-reply-can-be-asked-about.test.tsx
    #[test]
    fn a_part_of_a_reply_is_quoted_line_by_line_and_capped() {
        assert_eq!(
            quote_of("The rank is at most 3.\n\n\n\nSo AB is singular."),
            "> The rank is at most 3.\n>\n> So AB is singular."
        );
        let long = "x".repeat(MAX_QUOTE + 50);
        let quote = quote_of(&long);
        assert!(quote.chars().count() <= MAX_QUOTE + 2);
        assert!(quote.ends_with('\u{2026}'));
    }

    // a-side-question-can-be-approved-and-stopped.test.tsx
    #[test]
    fn the_side_panel_shows_approval_with_buttons() {
        let approval = Request {
            approval_id: "ap_1".into(),
            mission_id: "mission_side".into(),
            kind: locust_core::approval::Kind::Connector,
            summary: "Use create_issue on github".into(),
            detail: "{\n  \"title\": \"x\"\n}".into(),
            cwd: Some("C:/work".into()),
            requested_at: "2026-09-29T12:00:00.000Z".into(),
            runtime: "claude".into(),
            always_covers: None,
            always_key: None,
            data_sent_says: None,
            reversible_says: None,
            claude_input: None,
        };
        let (texts, _) = render_texts_and_labels(sample_side("running", vec![approval]));
        assert!(texts.iter().any(|t| t == "Use create_issue on github"));
        assert!(texts.iter().any(|t| t == "Approve once"));
    }

    // a-side-question-can-be-approved-and-stopped.test.tsx
    #[test]
    fn the_side_panel_offers_stop_while_answering_and_ask_when_done() {
        let (_, running_labels) = render_texts_and_labels(sample_side("running", Vec::new()));
        assert!(running_labels.iter().any(|l| l.as_deref() == Some("Stop this answer")));
        assert!(!running_labels.iter().any(|l| l.as_deref() == Some("Ask on the side")));

        let (_, done_labels) = render_texts_and_labels(sample_side("completed", Vec::new()));
        assert!(!done_labels.iter().any(|l| l.as_deref() == Some("Stop this answer")));
        assert!(done_labels.iter().any(|l| l.as_deref() == Some("Ask on the side")));
    }
}
