//! HANDED OUT, UNDER THE TURN (Electron `HandedOut` in `components/PeerThread.tsx`, drawn by `CallTodoList`). The
//! peer exchanges keep the whole conversation; this card sums up who has each ask a lead's turn handed out and how it
//! is going, one line each, with Open on each one already delivered.
use super::*;
use locust_core::call_todos::{self, Handed, TodoState, Tone};

/// The turn a posted message reached: the conversation on screen first (a live turn before the history refreshes),
/// then every turn the window has summaries of. Never the sender's own.
fn received_in(s: &State, message_id: &str) -> Option<String> {
    let mut live: Vec<(&str, Vec<ledger::PeerLink>)> = Vec::new();
    if let Some(pane) = &s.ledger {
        for (mission, link) in &pane.snapshot.peer_links {
            match live.iter_mut().find(|(m, _)| *m == mission.as_str()) {
                Some((_, links)) => links.push(link.clone()),
                None => live.push((mission.as_str(), vec![link.clone()])),
            }
        }
    }
    let held = live.iter().map(|(m, links)| (*m, links.as_slice()));
    let summaries = s.ledger_summaries.values().map(|sum| (sum.mission_id.as_str(), sum.peer_links.as_slice()));
    call_todos::peer_run_for(message_id, held.chain(summaries)).map(str::to_owned)
}

/// One to-do's state, from the same facts the sidebar and the Board read: a card waiting on the person, else how the
/// receiving turn's record ends. A turn the window has not seen yet is Starting.
pub(super) fn state_of(s: &State, mission_id: &str) -> TodoState {
    let needs_you = s.home.waiting_for.contains_key(mission_id)
        || s.pending_approval.as_ref().is_some_and(|card| card.mission_id == mission_id);
    let phase = s.ledger_summaries.get(mission_id).map(|sum| {
        if sum.running {
            "running"
        } else if sum.failed {
            "failed"
        } else if sum.cancelled {
            "cancelled"
        } else {
            "completed"
        }
    });
    let live = if s.seen_live.iter().any(|id| id == mission_id) {
        Some("running")
    } else {
        s.ledger.as_ref().filter(|pane| pane.snapshot.tip == mission_id)
            .map(|pane| if pane.snapshot.running { "running" } else if pane.snapshot.failed { "failed" } else { phase.unwrap_or("completed") })
    };
    call_todos::todo_state(live, phase, needs_you)
}

/// The card's lines for one turn's peer messages.
pub(super) fn handed(s: &State, messages: &[locust_core::peer_thread::Message]) -> Vec<Handed> {
    call_todos::handed_out(
        messages.iter().map(|m| (m.message_id.as_str(), m.direction.as_str(), m.to_name.as_str(), m.text.as_deref())),
        |id| received_in(s, id),
    )
}

/// The chip, in its tone's text and tint (`.lc-status--{tone}`).
fn chip(word: &str, tone: Tone, l: &crate::look::controls::Look) -> El {
    let (ink, tint) = match tone {
        Tone::Quiet => (l.c("text-muted"), l.c("chip-bg")),
        other => (l.c(&format!("{}-text", other.name())), l.c(&format!("{}-tint", other.name()))),
    };
    el().px(6.0).py(1.0).radius(4.0).fill(tint).child(text(word.to_owned()).font(l.ty.label).color(ink))
}

/// The card under the turn `mission_id`, or None when it handed nothing out.
pub(super) fn card(s: &State, mission_id: &str, messages: &[locust_core::peer_thread::Message], p: &Palette) -> Option<El> {
    let handed = handed(s, messages);
    let label = call_todos::handed_out_label(&handed)?;
    let l = crate::look::controls::Look::new(p.dark);
    let mut card = col()
        .key(format!("handed-out-{mission_id}"))
        .w_full()
        .gap(6.0)
        .padding(10.0)
        .radius(8.0)
        .border(1.0, p.hairline)
        .fill(l.c("bg-raised"))
        .label(label.clone())
        .child(text(label).key(format!("handed-out-label-{mission_id}")).font(type_::LABEL.mono()).color(l.c("text-muted")));
    for todo in handed {
        let state = todo.mission_id.as_deref().map_or(TodoState::Starting, |m| state_of(s, m));
        let (word, tone) = state.words();
        let id = todo.message_id.clone();
        let mut line = row()
            .key(format!("handed-out-todo-{id}"))
            .w_full()
            .gap(8.0)
            .center_y()
            .child(text(todo.name.clone()).key(format!("handed-out-who-{id}")).font(l.ty.label).color(l.c("text-primary")))
            // The ask on one line, the whole of it as its hover (`title={todo.task}`).
            .child(text(todo.task.clone()).key(format!("handed-out-task-{id}")).font(l.ty.label).color(l.c("text-secondary")).grow(1.0).shrink().truncate().help(todo.task.clone()))
            .child(chip(word, tone, &l).key(format!("handed-out-state-{id}")).label(word.to_owned()));
        if let Some(target) = todo.mission_id.clone() {
            let who = todo.name.clone();
            let task = todo.task.clone();
            line = line.child(
                crate::look::controls::button(&l, "Open", crate::look::controls::Tone::Neutral, false)
                    .key(format!("handed-out-open-{id}"))
                    .label(format!("Open {who}\u{2019}s conversation: {task}"))
                    .on_press_tx(move |s: &mut State, tx: &mut Tx| peer_thread::open_mission_by_id(s, target.clone(), tx)),
            );
        }
        card = card.child(line);
    }
    Some(card)
}

#[cfg(test)]
#[path = "handed_out_tests.rs"]
mod tests;
