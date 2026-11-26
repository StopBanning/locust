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
const ENDED: &[&str] = &["completed", "failed", "cancelled"];

pub fn is_going(phase: &str) -> bool {
    GOING.contains(&phase)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunLike {
    pub phase: String,
    pub data_run_id: Option<String>,
}

/// Filter approval cards against known runs so finished, failed, or cancelled runs drop their cards.
///
/// Returns `None` when every card stays so UI state does not churn; returns `Some(kept)` when one or
/// more cards belong to an ended run (`completed`, `failed`, or `cancelled`).
pub fn approvals_of_live_runs(
    approvals: &[Request],
    runs: &HashMap<String, RunLike>,
) -> Option<Vec<Request>> {
    let kept: Vec<Request> = approvals
        .iter()
        .filter(|approval| {
            let run = runs.get(&approval.mission_id).or_else(|| {
                runs.values().find(|entry| {
                    entry.data_run_id.as_deref() == Some(approval.mission_id.as_str())
                })
            });
            run.is_none_or(|r| !ENDED.contains(&r.phase.as_str()))
        })
        .cloned()
        .collect();
    if kept.len() == approvals.len() {
        None
    } else {
        Some(kept)
    }
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
        ..Default::default()
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
    let raw_waiting: Vec<Request> = side
        .turns
        .iter()
        .flat_map(|t| api().waiting_for(&t.mission_id))
        .collect();
    if apply_waiting_approvals(side, raw_waiting) {
        changed = true;
    }
    changed
}

fn apply_waiting_approvals(side: &mut SideChatState, raw_waiting: Vec<Request>) -> bool {
    let runs: HashMap<String, RunLike> = side
        .turns
        .iter()
        .map(|t| {
            (
                t.mission_id.clone(),
                RunLike {
                    phase: t.phase.clone(),
                    data_run_id: None,
                },
            )
        })
        .collect();
    let waiting = approvals_of_live_runs(&raw_waiting, &runs).unwrap_or(raw_waiting);
    let same_approvals = waiting.len() == side.approvals.len()
        && waiting
            .iter()
            .zip(side.approvals.iter())
            .all(|(a, b)| a.approval_id == b.approval_id);
    if !same_approvals {
        side.approvals = waiting;
        return true;
    }
    false
}

/// What the side chat's header icon is called and says on hover: Electron's "Ask on the side" (App.tsx `headerActions`,
/// SideChat.tsx `aria-label`) and what the panel says it asks (SideChat.tsx `{model} · a copy of this conversation`).
const ASK_ON_THE_SIDE: &str = "Ask on the side";
const ASK_ON_THE_SIDE_TIP: &str = "Ask on the side · a copy of this conversation";

pub(super) fn header_button(s: &State, p: &Palette) -> Option<El> {
    if !can_open_for_state(s) {
        return None;
    }
    Some(side_icon(s.side_chat.is_some(), p))
}

/// The header's own icon button (`.lc-headicon`, as Changes, Browser and More actions are drawn beside it): the
/// muted glyph on nothing, the header's hover wash under the pointer, pressed while the side chat is open, and its
/// words in the tooltip rather than on its face.
fn side_icon(active: bool, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    crate::look::controls::head_toggle(&crate::look::icons::MESSAGE, ASK_ON_THE_SIDE, ASK_ON_THE_SIDE_TIP, active, None)
        .on_press_tx(|s: &mut State, tx| open_from_state(s, tx))
        .build(l.k)
        .key("ask-on-side-button")
        .checked(active)
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
#[path = "ask_about_a_part_tests.rs"]
mod ask_about_a_part_tests;

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

    fn info(rt: &Runtime<State>, key: &str) -> rune::ElementInfo {
        rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no {key}"))
    }

    /// A Claude Code teammate's conversation is shown: a runtime that can copy its session, and a turn to copy.
    fn forkable() -> State {
        let mut s = State::default();
        let mut teammate = sample_side("completed", Vec::new()).teammate;
        teammate.id = "tm_codec".into();
        s.teammates = vec![teammate];
        s.selected = 0;
        s.conversations.insert("tm_codec".into(), Conversation { conversation_id: Some("m_1".into()), turns: Vec::new() });
        s
    }

    /// The side chat's header button beside the Browser toggle, another of the header's icons, as thread_view.rs
    /// lays out the header's actions.
    fn header_pair() -> Runtime<State> {
        let mut rt = Runtime::new(
            forkable(),
            |s, _| {
                let p = Palette::new(true);
                rune::row().pt(40.0).px(40.0).gap(4.0).child(crate::browser_panel::toggle(s, &p)).children(header_button(s, &p))
            },
            (400.0, 200.0),
            Env::new(true, 1.0),
        );
        rt.frame(0.0);
        rt
    }

    /// `frame` takes the clock: forty frames from `from` seconds, long enough for a hover to finish washing in.
    fn settle(rt: &mut Runtime<State>, from: f64) {
        for i in 0..40 {
            rt.frame(from + f64::from(i) * 0.016);
        }
    }

    // App.tsx: offered only where the conversation's runtime can copy its session and there is a turn to copy.
    #[test]
    fn ask_on_the_side_is_offered_only_for_a_conversation_it_can_copy() {
        let p = Palette::new(true);
        assert!(header_button(&forkable(), &p).is_some());
        assert!(header_button(&State::default(), &p).is_none());
    }

    // App.tsx header `.lc-headicon` + SideChat.tsx: "Ask on the side" is a header icon, its words on hover.
    #[test]
    fn ask_on_the_side_is_a_keyed_header_icon_named_and_tipped_in_electrons_words() {
        let rt = header_pair();
        let button = info(&rt, "ask-on-side-button");
        assert_eq!(button.role, "button");
        assert_eq!(button.label.as_deref(), Some("Ask on the side"));
        assert_eq!(button.help.as_deref(), Some("Ask on the side · a copy of this conversation"));
        let words: Vec<String> = rt.elements().into_iter().filter_map(|e| e.text).collect();
        assert!(!words.iter().any(|w| w == "Ask on the side"), "its words are in the tooltip, not on its face: {words:?}");
        let browser = info(&rt, "browser-toggle");
        assert_eq!((button.rect[2], button.rect[3]), (browser.rect[2], browser.rect[3]), "the same size as the header's other icons");
    }

    // Lane check of small-2 row 2: a header icon that opens a panel reads as on while its panel is open, as App.tsx's
    // header icons do (`aria-pressed={changesOpen}`, App.tsx:10386; `aria-pressed={inspectorOpen}`, :10354), heard as
    // AccessKit's toggled state ("checked"/"unchecked", head_icons_tests.rs).
    #[test]
    fn ask_on_the_side_reads_as_on_while_the_side_chat_is_open() {
        fn toggled(rt: &Runtime<State>) -> Vec<String> {
            fn find<'a>(tree: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
                if tree["key"] == key {
                    return Some(tree);
                }
                tree["children"].as_array()?.iter().find_map(|c| find(c, key))
            }
            let tree = rt.accessibility_json();
            let node = find(&tree, "ask-on-side-button").expect("in the tree");
            node["states"].as_array().into_iter().flatten().filter_map(|v| v.as_str()).filter(|s| s.ends_with("checked")).map(str::to_owned).collect()
        }
        let mut rt = header_pair();
        assert_eq!(toggled(&rt), ["unchecked"], "closed, it reads as off");
        rt.update(|s, _| s.side_chat = Some(sample_side("completed", Vec::new())));
        rt.frame(0.0);
        assert_eq!(toggled(&rt), ["checked"], "open, it reads as on");
    }

    // shell.css `.lc-headicon { background: transparent }` and `.lc-headicon:hover:not(:disabled) { background:
    // var(--lc-hover-surface) }`: nothing under it at rest, the header's wash under the pointer, as its neighbours.
    // (Rune lists an element's resting fill and how far its hover has come, not the wash's colour, so the wash is
    // judged as the Browser icon's: the same resting fill, corners and size, and the same hover reached.)
    #[test]
    fn ask_on_the_side_takes_the_headers_hover_wash() {
        let mut rt = header_pair();
        let rest = info(&rt, "ask-on-side-button");
        assert_eq!(rest.fill.a, 0.0, "transparent at rest, as `.lc-headicon`");
        assert_eq!(rest.hover, 0.0);
        let (x, y) = rt.find("ask-on-side-button").unwrap();
        rt.pointer_move(x, y);
        settle(&mut rt, 0.1);
        let hovered = info(&rt, "ask-on-side-button");
        assert!(hovered.hover > 0.99, "it has a hover state: {}", hovered.hover);

        let (bx, by) = rt.find("browser-toggle").unwrap();
        rt.pointer_move(bx, by);
        settle(&mut rt, 0.1);
        let browser = info(&rt, "browser-toggle");
        assert!(browser.hover > 0.99);
        assert!(info(&rt, "ask-on-side-button").hover < 0.01, "the wash leaves with the pointer");
        assert_eq!(
            (hovered.fill, hovered.radius, hovered.rect[2], hovered.rect[3]),
            (browser.fill, browser.radius, browser.rect[2], browser.rect[3]),
            "drawn as the Browser icon beside it"
        );
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
            patch: None,
            skippable: false, answer_in: None, claude_input: None,
            questions: Vec::new(),
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

    fn card_for(run_id: &str) -> Request {
        Request {
            approval_id: format!("ap_{run_id}"),
            mission_id: run_id.into(),
            kind: locust_core::approval::Kind::Command,
            summary: format!("Run something on {run_id}"),
            detail: String::new(),
            cwd: None,
            requested_at: "2026-09-29T12:00:00.000Z".into(),
            runtime: "codex".into(),
            always_covers: None,
            always_key: None,
            data_sent_says: None,
            reversible_says: None,
            patch: None,
            skippable: false, answer_in: None, claude_input: None,
            questions: Vec::new(),
        }
    }

    fn run_like(phase: &str, data_run_id: Option<&str>) -> RunLike {
        RunLike {
            phase: phase.into(),
            data_run_id: data_run_id.map(str::to_owned),
        }
    }

    // an-approval-card-goes-with-its-run.test.ts
    #[test]
    fn go_when_their_run_is_stopped_fails_or_finishes() {
        for phase in ["cancelled", "failed", "completed"] {
            let runs = HashMap::from([(String::from("run_1"), run_like(phase, None))]);
            let kept = approvals_of_live_runs(&[card_for("run_1")], &runs)
                .expect("ended run should drop its approval card");
            assert!(kept.is_empty());
        }
        let by_data = HashMap::from([(String::from("pending_2"), run_like("completed", Some("run_2")))]);
        let kept = approvals_of_live_runs(&[card_for("run_2")], &by_data)
            .expect("ended run matched by data_run_id should drop its approval card");
        assert!(kept.is_empty());
    }

    // an-approval-card-goes-with-its-run.test.ts
    #[test]
    fn stay_while_their_run_is_live_found_by_key_or_by_its_run_id() {
        let runs = HashMap::from([
            (String::from("run_1"), run_like("running", None)),
            (String::from("pending_2"), run_like("waiting", Some("run_2"))),
        ]);
        let list = vec![card_for("run_1"), card_for("run_2")];
        assert!(approvals_of_live_runs(&list, &runs).is_none());
    }

    // an-approval-card-goes-with-its-run.test.ts
    #[test]
    fn stay_when_the_window_does_not_know_the_run_yet() {
        let list = vec![card_for("run_new")];
        assert!(approvals_of_live_runs(&list, &HashMap::new()).is_none());
    }

    // an-approval-card-goes-with-its-run.test.ts
    #[test]
    fn keeps_the_same_list_when_nothing_changes_so_the_state_does_not_churn() {
        let runs = HashMap::from([(String::from("run_1"), run_like("running", None))]);
        let list = vec![card_for("run_1")];
        assert!(approvals_of_live_runs(&list, &runs).is_none());
    }

    #[test]
    fn side_chat_drops_waiting_cards_once_the_turn_ends() {
        let mut side = sample_side("running", Vec::new());
        assert!(apply_waiting_approvals(&mut side, vec![card_for("mission_side")]));
        assert_eq!(side.approvals.len(), 1);
        side.turns[0].phase = "completed".into();
        assert!(apply_waiting_approvals(&mut side, vec![card_for("mission_side")]));
        assert!(side.approvals.is_empty());
    }
}
