//! "In the background" side panel (`apps/desktop/src/renderer/src/components/BackgroundRuns.tsx`).
//!
//! Shows Claude Code `--bg` turns started from Locust: Locust watches them, never answers approval
//! prompts for them, and brings finished runs back into their conversation.

use super::*;
use crate::look::controls::{self, Look, Tone};
use locust_core::claude_background::{
    BACKGROUND_TRADE, BackgroundRun, BackgroundRuns, BackgroundState, background_is_live,
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
const POLL_INTERVAL_SECS: f64 = 5.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct BackgroundNote {
    pub text: String,
    pub set_up: bool,
}

#[derive(Default)]
pub(crate) struct Panel {
    pub(super) watcher: Option<BackgroundRuns<native::NativeBackgroundHost>>,
    pub(super) next_poll_at: f64,
    pub(super) open: bool,
    pub(super) runs: Vec<BackgroundRun>,
    pub(super) note: Option<BackgroundNote>,
    pub(super) last_opened_id: Option<String>,
    pub(super) last_stopped_id: Option<String>,
    pub(super) last_shown_mission_id: Option<String>,
    pub(super) setup_requested: bool,
}

/// Creates the watcher once at window startup and resumes any persisted open background runs.
pub(super) fn setup(s: &mut State) {
    if s.background_runs.watcher.is_some() {
        return;
    }
    let Some(store) = s.data.as_ref() else {
        return;
    };
    let mut watcher = native::background_runs_for(store);
    watcher.resume();
    s.background_runs.runs = watcher.list();
    s.background_runs.watcher = Some(watcher);
}

/// Polls active background runs from the window's main poll (every 5s while any run is live; idle at rest).
pub(super) fn poll(s: &mut State, now: f64) -> bool {
    if s.background_runs.watcher.is_none() {
        setup(s);
    }
    let Some(watcher) = s.background_runs.watcher.as_mut() else {
        return false;
    };
    if watcher.watching().is_empty() {
        return false;
    }
    if now < s.background_runs.next_poll_at {
        return false;
    }
    s.background_runs.next_poll_at = now + POLL_INTERVAL_SECS;
    let _ = watcher.poll_once();
    let latest = watcher.list();
    if latest != s.background_runs.runs {
        s.background_runs.runs = latest;
        return true;
    }
    false
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

/// Stops a live run through the watcher and records the requested id.
pub(super) fn stop_run(s: &mut State, id: &str) {
    if let Some(watcher) = s.background_runs.watcher.as_mut() {
        let _ = watcher.stop(id);
    }
    s.background_runs.last_stopped_id = Some(id.to_string());
}

/// Dismisses an ended run from the watcher and panel state (live runs stay in the list).
pub(super) fn dismiss_run(s: &mut State, id: &str) -> bool {
    if let Some(watcher) = s.background_runs.watcher.as_mut() {
        if watcher.dismiss(id) {
            s.background_runs.runs = watcher.list();
            return true;
        }
        return false;
    }
    dismiss_local(&mut s.background_runs, id)
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

/// Header badge shown whenever Locust has background runs (or a background setup note) to inspect.
pub(super) fn header_badge(s: &State, p: &Palette) -> Option<El> {
    let panel = &s.background_runs;
    if panel.runs.is_empty() && panel.note.is_none() {
        return None;
    }
    let l = Look::new(p.dark);
    let live = live_count(&panel.runs);
    let label = if live > 0 {
        format!("{TITLE} · {live} live")
    } else {
        format!("{TITLE} · {}", panel.runs.len())
    };
    Some(
        row()
            .key("background-runs-badge")
            .w_full()
            .px(12.0)
            .py(4.0)
            .gap(8.0)
            .center_y()
            .fill(p.raised)
            .border(1.0, p.hairline)
            .child(
                controls::ghost_button(&l, &label)
                    .key("background-runs-badge-button")
                    .on_press(|s: &mut State| {
                        s.background_runs.open = !s.background_runs.open;
                    }),
            ),
    )
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
                        stop_run(s, &stop_id);
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
                        dismiss_run(s, &dismiss_id);
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

    Some(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use locust_core::claude_background::{
        BackgroundSandbox, StartBackgroundOutcome, StartBackgroundRequest,
    };
    use std::path::PathBuf;

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

    fn fixture_script() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("scripts")
            .join("fixtures")
            .join("claude-bg.py")
    }

    fn words(rt: &rune::Runtime<State>) -> String {
        rt.elements()
            .iter()
            .filter_map(|e| e.text.clone().or(e.label.clone()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn background_runs_panel_sorts_newest_first_counts_live_and_renders_states() {
        let mut s = State::default();
        let p = Palette::new(true);
        assert!(view(&s, &p).is_none());
        assert!(header_badge(&s, &p).is_none());

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

        // Panel and header badge render when open/populated.
        assert!(header_badge(&s, &p).is_some());
        assert!(view(&s, &p).is_some());

        // Live run cannot be dismissed; finished run can.
        assert!(!dismiss_local(&mut s.background_runs, "ad39859a"));
        assert_eq!(s.background_runs.runs.len(), 2);
        assert!(dismiss_local(&mut s.background_runs, "bc123456"));
        assert_eq!(s.background_runs.runs.len(), 1);
        assert_eq!(s.background_runs.runs[0].id, "ad39859a");
    }

    #[test]
    fn background_run_from_fake_claude_shows_in_window_through_main_poll() {
        let (_dir, store) = crate::org::tests::temp_store();
        let script = fixture_script();
        assert!(script.exists(), "fixture script missing: {}", script.display());
        let agents_file = store.root().join("fake-agents.json");
        std::fs::write(
            &agents_file,
            r#"[{"id":"ad39859a","kind":"background","state":"blocked","waitingFor":"permission prompt"}]"#,
        )
        .expect("write fake agents");

        // Start a run through native::background_runs_for_with_program using the fake script.
        let mut watcher = native::background_runs_for_with_program(
            &store,
            "python3",
            vec![script.to_string_lossy().into_owned()],
            vec![(
                "CLAUDE_BG_AGENTS_FILE".to_string(),
                agents_file.to_string_lossy().into_owned(),
            )],
        );
        let started = watcher.start(StartBackgroundRequest {
            prompt: "Run the release checklist.".into(),
            folder: store.root().to_string_lossy().into_owned(),
            sandbox: BackgroundSandbox::ReadOnly,
            ask_each: false,
            conversation: None,
            teammate_id: None,
            model: Some("haiku".into()),
            effort: None,
            account: None,
        });
        assert!(matches!(started, StartBackgroundOutcome::Started(_)));

        // Verify setup() loads persisted runs via native::background_runs_for(store).
        let mut boot_state = State::default();
        boot_state.data = Some(store.clone());
        setup(&mut boot_state);
        assert!(boot_state.background_runs.watcher.is_some());
        assert_eq!(boot_state.background_runs.runs.len(), 1);
        assert_eq!(boot_state.background_runs.runs[0].id, "ad39859a");

        // Wire the fake-script watcher into a window State and drive it through main::poll + main::view.
        let mut s = State::default();
        s.data = Some(store);
        s.background_runs.watcher = Some(watcher);
        assert!(crate::poll(&mut s, 0.0, &mut Tx::default()));
        assert_eq!(s.background_runs.runs.len(), 1);
        assert_eq!(s.background_runs.runs[0].state, BackgroundState::Blocked);
        assert_eq!(
            s.background_runs.runs[0].waiting_for.as_deref(),
            Some("permission prompt")
        );

        // Render the real main window: header badge is drawn before the panel is opened.
        let mut rt = rune::Runtime::new(
            s,
            crate::view,
            (1280.0, 800.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        let initial_words = words(&rt);
        assert!(
            initial_words.contains("In the background · 1 live"),
            "expected header badge in window, got:\n{initial_words}"
        );
        assert!(
            !initial_words.contains("Run the release checklist."),
            "panel should stay closed until opened"
        );

        // Click the header badge button to open the panel and verify the run card is drawn.
        let (x, y) = rt
            .find("background-runs-badge-button")
            .expect("header badge button on screen");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.016);
        let open_words = words(&rt);
        assert!(
            open_words.contains("Run the release checklist."),
            "expected prompt in open panel, got:\n{open_words}"
        );
        assert!(
            open_words.contains("Waiting for you (permission prompt). Open it to answer."),
            "expected blocked state words in open panel, got:\n{open_words}"
        );
        assert!(
            open_words.contains("Open it to answer"),
            "expected Open it to answer action in open panel, got:\n{open_words}"
        );
    }
}
