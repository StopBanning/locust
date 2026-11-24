//! "In the background" side panel (`apps/desktop/src/renderer/src/components/BackgroundRuns.tsx`).
//!
//! Shows Claude Code `--bg` turns started from Locust: Locust watches them, never answers approval
//! prompts for them, and brings finished runs back into their conversation.

use super::*;
use crate::look::controls::{self, Look, Tone};
use locust_core::claude_background::{
    self, BACKGROUND_TRADE, BackgroundRun, BackgroundState, background_is_live,
    background_state_words,
};

pub(super) const TITLE: &str = "In the background";
pub(super) const SUBTITLE: &str = "Claude Code";
pub(super) const EMPTY_NOTE: &str =
    "Nothing in the background yet. Pick Background in the chat type menu, then send.";
pub(super) const SETUP_LABEL: &str = "Open Claude Code in this folder";
pub(super) const OPEN_TO_ANSWER_LABEL: &str = "Open it to answer";
pub(super) const OPEN_TERMINAL_LABEL: &str = "Open in a terminal";
pub(super) const STOP_LABEL: &str = "Stop";
pub(super) const SHOW_IN_CONVERSATION_LABEL: &str = "Show in the conversation";
pub(super) const REMOVE_LABEL: &str = "Remove from this list";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct BackgroundNote {
    pub text: String,
    pub set_up: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Panel {
    pub(super) open: bool,
    pub(super) runs: Vec<BackgroundRun>,
    pub(super) note: Option<BackgroundNote>,
    pub(super) last_opened_id: Option<String>,
    pub(super) last_stopped_id: Option<String>,
    pub(super) last_shown_mission_id: Option<String>,
    pub(super) setup_requested: bool,
}

/// Number of currently live background runs (for the header badge).
pub(super) fn live_count(runs: &[BackgroundRun]) -> usize {
    runs.iter().filter(|run| background_is_live(run.state)).count()
}

/// Runs ordered newest-first by `started_at` ISO timestamp, matching `BackgroundRuns.tsx`.
pub(super) fn sorted_newest_first(runs: &[BackgroundRun]) -> Vec<BackgroundRun> {
    let mut out = runs.to_vec();
    out.sort_by(|left, right| right.started_at.cmp(&left.started_at));
    out
}

/// Dismisses an ended run from the panel state (live runs stay in the list).
pub(super) fn dismiss_local(panel: &mut Panel, id: &str) -> bool {
    let Some(pos) = panel
        .runs
        .iter()
        .position(|run| run.id == id && !background_is_live(run.state))
    else {
        return false;
    };
    panel.runs.remove(pos);
    true
}

/// Renders the "In the background" side panel when open.
pub(super) fn view(s: &State, p: &Palette) -> Option<El> {
    let panel = &s.background_runs;
    if !panel.open {
        return None;
    }
    let l = Look::new(p.dark);
    let newest_first = sorted_newest_first(&panel.runs);

    let header = row()
        .key("background-runs-head")
        .w_full()
        .gap(8.0)
        .center_y()
        .child(text(TITLE).font(l.ty.control).color(l.c("text-primary")))
        .child(text(SUBTITLE).font(l.ty.mono).color(l.c("text-secondary")))
        .child(
            controls::ghost_button(&l, "Close")
                .key("background-runs-close")
                .on_press(|s: &mut State| {
                    s.background_runs.open = false;
                }),
        );

    let mut body = col()
        .key("background-runs-panel")
        .role("region")
        .label(TITLE)
        .w_full()
        .gap(8.0)
        .px(16.0)
        .py(12.0)
        .fill(p.raised)
        .border(1.0, p.hairline)
        .child(header);

    if let Some(ref note) = panel.note {
        let mut note_box = col()
            .key("background-runs-note")
            .gap(6.0)
            .child(
                text(note.text.clone())
                    .key("background-runs-problem")
                    .font(l.ty.body)
                    .color(l.c("amber-text"))
                    .wrap(),
            );
        if note.set_up {
            note_box = note_box.child(
                controls::button(&l, SETUP_LABEL, Tone::Neutral, false)
                    .key("background-runs-setup")
                    .on_press(|s: &mut State| {
                        s.background_runs.setup_requested = true;
                    }),
            );
        }
        body = body.child(note_box);
    }

    body = body.child(
        text(BACKGROUND_TRADE)
            .key("background-runs-trade")
            .font(l.ty.body)
            .color(l.c("text-secondary"))
            .wrap(),
    );

    if newest_first.is_empty() && panel.note.is_none() {
        body = body.child(
            text(EMPTY_NOTE)
                .key("background-runs-empty")
                .font(l.ty.body)
                .color(l.c("text-secondary"))
                .wrap(),
        );
    }

    for run in newest_first {
        let live = background_is_live(run.state);
        let state_line = background_state_words(&run);
        let id = run.id.clone();
        let mut actions = row().key(format!("bg-actions-{}", run.id)).gap(6.0);
        if live {
            let open_label = if run.state == BackgroundState::Blocked {
                OPEN_TO_ANSWER_LABEL
            } else {
                OPEN_TERMINAL_LABEL
            };
            let tone = if run.state == BackgroundState::Blocked {
                Tone::Active
            } else {
                Tone::Neutral
            };
            let open_id = id.clone();
            actions = actions.child(
                controls::button(&l, open_label, tone, false)
                    .key(format!("bg-open-{}", run.id))
                    .on_press(move |s: &mut State| {
                        s.background_runs.last_opened_id = Some(open_id.clone());
                    }),
            );
            let stop_id = id.clone();
            actions = actions.child(
                controls::ghost_button(&l, STOP_LABEL)
                    .key(format!("bg-stop-{}", run.id))
                    .on_press(move |s: &mut State| {
                        s.background_runs.last_stopped_id = Some(stop_id.clone());
                    }),
            );
        } else {
            if let Some(ref mission_id) = run.brought_in {
                let target = mission_id.clone();
                actions = actions.child(
                    controls::ghost_button(&l, SHOW_IN_CONVERSATION_LABEL)
                        .key(format!("bg-show-{}", run.id))
                        .on_press(move |s: &mut State| {
                            s.background_runs.last_shown_mission_id = Some(target.clone());
                        }),
                );
            }
            let dismiss_id = id.clone();
            actions = actions.child(
                controls::ghost_button(&l, REMOVE_LABEL)
                    .key(format!("bg-dismiss-{}", run.id))
                    .on_press(move |s: &mut State| {
                        dismiss_local(&mut s.background_runs, &dismiss_id);
                    }),
            );
        }

        let card = col()
            .key(format!("bg-card-{}", run.id))
            .w_full()
            .gap(4.0)
            .px(10.0)
            .py(8.0)
            .border(1.0, p.hairline)
            .child(
                text(run.prompt.clone())
                    .key(format!("bg-prompt-{}", run.id))
                    .font(l.ty.control)
                    .color(l.c("text-primary"))
                    .wrap(),
            )
            .child(
                text(state_line)
                    .key(format!("bg-state-{}", run.id))
                    .font(l.ty.body)
                    .color(l.c("text-secondary"))
                    .wrap(),
            )
            .child(actions);

        body = body.child(card);
    }

    let _ = claude_background::background_is_live(BackgroundState::Working);
    Some(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_run(id: &str, state: BackgroundState, started_at: &str) -> BackgroundRun {
        BackgroundRun {
            account: None,
            id: id.into(),
            session_id: Some(format!("{id}-full")),
            folder: "/tmp/work".into(),
            prompt: format!("Prompt for {id}"),
            started_at: started_at.into(),
            conversation: Some("mission_1".into()),
            teammate_id: None,
            model: Some("haiku".into()),
            state,
            waiting_for: None,
            name: None,
            brought_in: None,
            not_brought_in: None,
        }
    }

    #[test]
    fn background_runs_panel_sorts_newest_first_counts_live_and_renders_states() {
        let mut s = State::default();
        let p = Palette::new(true);
        assert!(view(&s, &p).is_none());

        let mut blocked = sample_run("ad39859a", BackgroundState::Blocked, "2026-10-06T22:00:00Z");
        blocked.waiting_for = Some("permission prompt".into());
        let mut done = sample_run("bc123456", BackgroundState::Done, "2026-10-06T22:05:00Z");
        done.brought_in = Some("mission_back".into());

        s.background_runs.open = true;
        s.background_runs.runs = vec![blocked.clone(), done.clone()];
        s.background_runs.note = Some(BackgroundNote {
            text: "Claude Code has not been told to trust this folder.".into(),
            set_up: true,
        });

        assert_eq!(live_count(&s.background_runs.runs), 1);
        let ordered = sorted_newest_first(&s.background_runs.runs);
        assert_eq!(ordered[0].id, "bc123456");
        assert_eq!(ordered[1].id, "ad39859a");

        // Panel renders when open.
        assert!(view(&s, &p).is_some());

        // Live run cannot be dismissed; finished run can.
        assert!(!dismiss_local(&mut s.background_runs, "ad39859a"));
        assert_eq!(s.background_runs.runs.len(), 2);
        assert!(dismiss_local(&mut s.background_runs, "bc123456"));
        assert_eq!(s.background_runs.runs.len(), 1);
        assert_eq!(s.background_runs.runs[0].id, "ad39859a");
    }
}
