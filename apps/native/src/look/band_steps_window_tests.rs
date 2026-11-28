//! speed-3 row 1: a working row's band and the live word's sweep stand on a grid of 50 ms steps of the window's clock.
//! The window holds one wake a step however many polls ask (`working_shimmer::next_step_wake`), and its poll reports a
//! change only when the step moved. Before, every poll asked a wake 50 ms on and reported a change, so each frame
//! between two steps (a hover, a landing reply) started a chain of wakes of its own, and a window with a running row
//! framed and rebuilt itself up to 60 times a second (row 0's measure: 40 to 60 commits a second, 21-33% of one core).
use super::*;
use crate::lane_running_window_measure_tests::{next_frame, step, window};
use crate::working_shimmer::{next_step_wake, on_step};
use rune::Runtime;

/// Frames the window as its host does, from `clock` for `secs` seconds of window time: (frames, commits).
fn run(rt: &mut Runtime<State>, clock: &mut f64, secs: f64, mut each: impl FnMut(&Runtime<State>)) -> (u32, u64) {
    let (start, commits, mut frames, mut animating) = (*clock, rt.stats.commits, 0, false);
    while *clock - start < secs {
        let at = next_frame(rt, *clock, animating);
        animating = step(rt, clock, at).2;
        frames += 1;
        each(rt);
    }
    (frames, rt.stats.commits - commits)
}

/// The running row's title as the window painted it: each glyph's colour, in order.
fn title_colouring(rt: &Runtime<State>) -> Vec<[u8; 4]> {
    let ([x0, y0, w, h], _) = rt.presented("conversation-title-m_other").expect("the running row's title is drawn");
    rt.instances
        .iter()
        .filter(|i| i.rect[0] >= x0 - 1.0 && i.rect[0] <= x0 + w + 1.0 && i.rect[1] >= y0 - 2.0 && i.rect[1] <= y0 + h + 2.0)
        .map(|i| i.fill.map(|c| (c * 255.0).round() as u8))
        .collect()
}

/// The window not focused: a teammate's face moves only in a focused window, on a beat of its own (30 a second) that
/// frames the window without rebuilding it (speed-2 row 1). The status rail's green dot still pulses while a turn runs
/// (rail.rs, a Rune `Cycle` at 30 frames a second): its frames rebuild nothing either, but they are frames.
fn unfocused(mut rt: Runtime<State>) -> Runtime<State> {
    rt.update(|s, _| s.window_focused = false);
    rt.frame(0.0);
    rt
}

/// State A's frames and rebuilds over two seconds after half a second's start, with `extra` frames put in between two
/// steps first, as a hover's or a landing reply's come: no floor.
fn after_extra_frames(extra: usize) -> (u32, u64) {
    let mut rt = unfocused(window("m_other", false));
    let mut clock = 0.0;
    run(&mut rt, &mut clock, 0.5, |_| {});
    for _ in 0..extra {
        let wake = rt.wake_deadline().expect("the band asks for its next step");
        let at = (clock + 0.007).min(wake - 0.001).max(clock + 0.001);
        step(&mut rt, &mut clock, at);
        let at = next_frame(&rt, clock, false);
        step(&mut rt, &mut clock, at);
    }
    run(&mut rt, &mut clock, 2.0, |_| {})
}

#[test]
fn a_running_rows_band_frames_the_window_once_a_step_however_often_something_else_draws() {
    let _engine = locust_core::native::hooks::no_saved_rules();
    let (frames, commits) = after_extra_frames(0);
    // 20 steps a second: 40 rebuilds in two seconds, with room for the step the run starts in.
    assert!(commits <= 42, "{commits} rebuilds in two seconds: a poll between steps rebuilt the window");
    assert!(commits >= 36, "the band still steps: {commits} rebuilds in two seconds");
    // Frames between steps (here three) start no chain of wakes of their own: the window frames as often after them
    // as it did with none. A wake asked from every poll took one extra frame to 60 frames a second, for good.
    let (frames_after, commits_after) = after_extra_frames(3);
    assert!(frames_after <= frames + 2, "{frames_after} frames in two seconds after three extra frames, {frames} with none: a wake chain came back");
    assert!(commits_after <= 42, "{commits_after} rebuilds in two seconds after three extra frames");

    // Focused, Wren's face (Wren's row runs) frames the window on its own beat too; those frames rebuild nothing.
    let mut rt = window("m_other", false);
    let mut clock = 0.0;
    run(&mut rt, &mut clock, 0.5, |_| {});
    let at = clock + 0.02;
    step(&mut rt, &mut clock, at);
    let (frames, commits) = run(&mut rt, &mut clock, 2.0, |_| {});
    assert!(u64::from(frames) > commits, "the face and the dot frame the focused window ({frames} frames)");
    assert!(commits <= 42, "{commits} rebuilds in two seconds of a focused window: a frame between steps rebuilt it");
}

#[test]
fn a_frame_between_two_steps_rebuilds_nothing_for_the_band() {
    let _engine = locust_core::native::hooks::no_saved_rules();
    let mut rt = unfocused(window("m_other", false));
    let mut clock = 0.0;
    run(&mut rt, &mut clock, 0.5, |_| {});
    // Frame at the start of the step the band asked a wake for: the next step is 50 ms on.
    let at = rt.state.band_wake.expect("the band asked for a step") as f64 * crate::working_shimmer::FRAME_MS as f64 / 1000.0;
    step(&mut rt, &mut clock, at);
    let asked = rt.state.band_wake;
    let (commits, drawn) = (rt.stats.commits, title_colouring(&rt));
    let at = clock + 0.01;
    step(&mut rt, &mut clock, at);
    assert_eq!(rt.stats.commits, commits, "a frame inside a step polls, and the band has not moved");
    assert_eq!(title_colouring(&rt), drawn);
    assert_eq!(rt.state.band_wake, asked, "and asks no wake of its own: the step's one stands");
}

#[test]
fn the_band_still_moves_and_stands_still_under_less_motion() {
    let _engine = locust_core::native::hooks::no_saved_rules();
    let mut rt = window("m_other", false);
    let mut clock = 0.0;
    let mut colourings = std::collections::HashSet::new();
    run(&mut rt, &mut clock, 2.0, |rt| {
        colourings.insert(title_colouring(rt));
    });
    assert!(colourings.len() >= 10, "the band passed through {} colourings of the title in two seconds", colourings.len());

    let mut still = window("m_other", false);
    still.set_system_reduced_motion(true);
    let mut clock = 0.0;
    still.frame(0.0);
    // Rune fades a colour change briefly under less motion: the title settles first.
    for _ in 0..20 {
        let at = clock + 0.05;
        step(&mut still, &mut clock, at);
    }
    let mut colourings = std::collections::HashSet::new();
    // No wake comes for a still band: the host frames every 50 ms here regardless, and nothing moves.
    for _ in 0..40 {
        let at = clock + 0.05;
        step(&mut still, &mut clock, at);
        colourings.insert(title_colouring(&still));
    }
    assert_eq!(colourings.len(), 1, "under less motion the title stands still");
}

#[test]
fn every_asker_in_a_step_shares_its_one_wake_on_the_windows_clock() {
    let mut asked = None;
    let first = next_step_wake(1.012, &mut asked).expect("the first poll of a step asks");
    assert!((first.as_secs_f64() - 0.038).abs() < 1e-9, "{first:?}: the next step, 1.05 s");
    assert_eq!(asked, Some(21));
    // Another poll in the same step, however soon or late: no wake of its own.
    assert_eq!(next_step_wake(1.03, &mut asked), None);
    assert_eq!(next_step_wake(1.0499, &mut asked), None);
    // At the step (to within the clock's arithmetic) the next one is asked.
    let next = next_step_wake(1.05 - 1e-9, &mut asked).expect("the step came: the next is asked");
    assert!((next.as_secs_f64() - 0.05).abs() < 1e-6, "{next:?}");
    assert_eq!(asked, Some(22));
    // The band stands at its step's start.
    assert_eq!(on_step(1.012), 1.0);
    assert_eq!(on_step(1.0499), 1.0);
    assert_eq!(on_step(1.05 - 1e-9), 1.05);
    assert_eq!(on_step(1.5), 1.5);
}

