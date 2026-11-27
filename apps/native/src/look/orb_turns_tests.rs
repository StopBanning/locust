//! The orb turns while a turn runs whether or not the window is in front, as Electron's canvas does on its own loop
//! (components/Orb.tsx:202-232: stopped only for reduced motion or when scrolled out of view, never for a window
//! behind). It turns by redrawing its own image in place, so the window's poll does not mark the view changed for it;
//! at rest, and under reduced motion, nothing is drawn. Driven through the window's own poll, as the app runs it.

use super::*;
use std::{cell::Cell, rc::Rc};

fn window(build: fn(&mut crate::State), focused: bool, reduced: bool) -> rune::Runtime<crate::State> {
    let mut s = crate::State::default();
    build(&mut s);
    s.window_focused = focused;
    // The view reads the person's setting from the window each build (`cx.is_reduced_motion()`).
    fn still(s: &crate::State, cx: &mut rune::Cx<crate::State>) -> El {
        cx.reduced_motion(true);
        crate::view(s, cx)
    }
    let view: fn(&crate::State, &mut rune::Cx<crate::State>) -> El = if reduced { still } else { crate::view };
    let mut rt = rune::Runtime::new(s, view, (1215.0, 800.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

/// `beats` polls a beat apart from `from` seconds: the orb frames drawn, and how many of the polls after the first
/// marked the window's view changed.
fn polled(rt: &mut rune::Runtime<crate::State>, from: f64, beats: usize) -> (u64, usize) {
    let before = frames_drawn();
    let changed = Rc::new(Cell::new(0));
    for n in 0..beats {
        let now = from + n as f64 / FPS;
        let seen = changed.clone();
        rt.update(move |s, tx| {
            if crate::poll(s, now, tx) && n > 0 {
                seen.set(seen.get() + 1);
            }
        });
        rt.frame(1.0 / FPS);
    }
    (frames_drawn() - before, changed.get())
}

#[test]
fn a_running_turn_turns_its_three_orbs_with_the_window_behind() {
    let mut rt = window(crate::scenes::native_scene_live, false, false);
    let (drawn, changed) = polled(&mut rt, 1.0, 12);
    // The live line's orb, the plan's head and its running step, each redrawn on every beat after the first.
    assert!(drawn >= 3 * 11, "three orbs a beat behind other windows: {drawn} frames");
    assert_eq!(changed, 0, "turning the orb never marks the window's view changed");
}

#[test]
fn a_running_turn_turns_its_orbs_with_the_window_in_front() {
    let mut rt = window(crate::scenes::native_scene_live, true, false);
    let (drawn, _) = polled(&mut rt, 1.0, 12);
    assert!(drawn >= 3 * 11, "{drawn} frames");
}

#[test]
fn at_rest_no_orb_frame_is_drawn() {
    let mut rt = window(crate::scenes::ledger_transcript_scene, false, false);
    assert_eq!(polled(&mut rt, 1.0, 12).0, 0);
    let mut rt = window(crate::scenes::ledger_transcript_scene, true, false);
    assert_eq!(polled(&mut rt, 1.0, 12).0, 0);
}

#[test]
fn reduced_motion_holds_the_still_pose() {
    let mut rt = window(crate::scenes::native_scene_live, false, true);
    assert_eq!(polled(&mut rt, 1.0, 12).0, 0);
    assert!(rt.elements().iter().any(|e| e.key.as_deref() == Some("live-step-orb")), "the orb still shows, standing still");
}

#[test]
fn a_turn_that_ends_stops_its_orbs_and_the_next_turn_turns_again() {
    let mut rt = window(crate::scenes::native_scene_live, false, false);
    assert!(polled(&mut rt, 1.0, 4).0 > 0);
    rt.update(|s, _| crate::scenes::native_scene_idle(s));
    rt.frame(1.0 / FPS);
    assert_eq!(polled(&mut rt, 2.0, 6).0, 0, "nothing turns once the turn is over");
    rt.update(|s, _| crate::scenes::native_scene_live(s));
    rt.frame(1.0 / FPS);
    assert!(polled(&mut rt, 3.0, 6).0 >= 3 * 4, "a new turn's orbs turn: not an image left from the last one");
}

fn asked() -> locust_core::approval::Request {
    locust_core::approval::Request {
        approval_id: "ap_orbs".into(),
        mission_id: "mission_1".into(),
        runtime: "codex".into(),
        kind: locust_core::approval::Kind::Command,
        summary: "Run a command".into(),
        detail: "touch hello.txt".into(),
        cwd: Some("/tmp/project".into()),
        requested_at: "2026-11-26T00:00:00.000Z".into(),
        always_covers: None,
        always_key: None,
        data_sent_says: None,
        reversible_says: None,
        patch: None,
        skippable: false,
        answer_in: None,
        claude_input: None,
        questions: Vec::new(),
    }
}

/// An approval card holds the live line (Thread.tsx `awaitingDecision`) while the plan and its orbs stay on screen.
/// Once it is answered every orb turns again: the plan's are drawn into images the beat redraws, not left on the ones
/// it let go of while the card waited.
#[test]
fn after_an_approval_is_answered_every_orb_turns_again() {
    let mut rt = window(crate::scenes::native_scene_live, false, false);
    assert!(polled(&mut rt, 1.0, 4).0 > 0);
    rt.update(|s, _| s.pending_approval = Some(asked()));
    rt.frame(1.0 / FPS);
    polled(&mut rt, 2.0, 3);
    rt.update(|s, _| s.pending_approval = None);
    rt.frame(1.0 / FPS);
    let (drawn, _) = polled(&mut rt, 3.0, 6);
    assert!(drawn >= 3 * 5, "the live line's orb and both of the plan's turn again: {drawn} frames");
}

/// REVIEW-ui-changes-2026-11-26.md change 7: once Stop is pressed the orb goes still until the agent answers, and stays
/// on screen.
#[test]
fn a_pressed_stop_holds_the_orb_still_until_it_lands() {
    let mut rt = window(crate::scenes::live_shell_scene, false, false);
    assert!(polled(&mut rt, 1.0, 8).0 >= 7, "the turn's orb turns before Stop is pressed");
    // Stop pressed: the same turn, with a stop on its way that has not landed.
    rt.update(|s, _| crate::scenes::live_stopping_scene(s));
    rt.frame(1.0 / FPS);
    assert!(rt.state.stopping.is_pending());
    assert_eq!(polled(&mut rt, 2.0, 8).0, 0, "no orb frame while the stop is on its way");
    assert!(rt.elements().iter().any(|e| e.key.as_deref() == Some("live-step-orb")), "the orb stays, standing still");
}
