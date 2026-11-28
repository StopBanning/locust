//! Keep the native roster's live canvas on the correct side of window focus.
use crate::{State, Teammate, apply_team, view};
use rune::{Env, Runtime};

const SLOT: &str = "roster-tm_sweep_focus";

fn window() -> Runtime<State> {
    let mut s = State::default();
    s.layout_pref = "wide".into();
    apply_team(&mut s, vec![Teammate {
        id: "tm_sweep_focus".into(), name: "Wren".into(), role: String::new(),
        runtime: Some("codex".into()), model: None, account: None, busy: true,
        runs_in: "ask".into(), folder: None,
    }]);
    let mut rt = Runtime::new(s, view, (1100.0, 780.0), Env::new(true, 1.0));
    // The app's stage call, without unrelated browser, engine and platform polling.
    rt.poll = Some(|s, now, tx| s.bot_stage.poll(now, s.reduced_motion.get(), s.window_focused, tx));
    for _ in 0..12 { rt.frame(1.0 / 60.0); }
    assert!(rt.state.bot_stage.wake_count() > 0, "the working roster face had asked to animate");
    rt
}

fn shape() -> String {
    format!("{:?}", crate::look::faces::drawn_shape(SLOT).expect("the real roster face"))
}

// case: asks for no frames and draws nothing more, holding where it was
#[test]
fn sweep_face_behind_its_window_neither_draws_nor_schedules() {
    let mut rt = window();
    rt.window_focus(false);
    rt.frame(0.0);
    assert!(!rt.state.window_focused, "the real window's blur callback");
    let (before, wakes) = (shape(), rt.state.bot_stage.wake_count());
    for _ in 0..12 { rt.frame(1.0 / 30.0); }
    assert_eq!(rt.state.bot_stage.wake_count(), wakes);
    assert_eq!(shape(), before);
}

// case: lets go of its window when it stops, and asks for nothing after
#[test]
fn sweep_face_removed_from_the_roster_is_dropped_and_cannot_schedule() {
    let mut rt = window();
    assert!(rt.state.bot_stage.desired_activity(SLOT).is_some());
    rt.update(|s, _| apply_team(s, Vec::new()));
    for _ in 0..4 { rt.frame(1.0 / 30.0); }
    assert!(rt.state.bot_stage.desired_activity(SLOT).is_none());
    let wakes = rt.state.bot_stage.wake_count();
    rt.window_focus(false);
    rt.frame(1.0 / 30.0);
    rt.window_focus(true);
    for _ in 0..12 { rt.frame(1.0 / 30.0); }
    assert_eq!(rt.state.bot_stage.wake_count(), wakes);
}
