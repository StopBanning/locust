//! Unit and headless Rune tests for the Stop button's quiet mono beam (`src/look/stop_beam.rs`).
//! Expected numbers come from Electron's `the-beams-are-mono-and-quiet.test.tsx`, `Composer.tsx`, and `shell.css`
//! (1.96 s turn, `steps(59)` ≈ 30.1 a second, 1.5 px rim round the 40×40 Stop disc, conic stops at 54/60/66/70/78%).

use super::*;
use crate::look::controls::{Look, SendState, send_circle};

fn wedge_opacities(rt: &rune::Runtime<(Mode, bool)>) -> Vec<f32> {
    let all = rt.elements();
    (0..59)
        .map(|k| {
            let key = format!("stop-beam-wedge-{k}");
            all.iter()
                .find(|e| e.key.as_deref() == Some(key.as_str()) && !e.exiting)
                .unwrap_or_else(|| panic!("missing {key}"))
                .opacity
        })
        .collect()
}

fn peak_wedge(opacities: &[f32]) -> (usize, f32) {
    opacities
        .iter()
        .copied()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap()
}

fn runtime(mode: Mode, dark: bool) -> rune::Runtime<(Mode, bool)> {
    let mut rt = rune::Runtime::new(
        (mode, dark),
        |&(mode, dark), _| {
            let l = Look::new(dark);
            let button = send_circle(&l, SendState::Stop, "Stop").key("send");
            rune::row().padding(16.0).child(wrap(button, mode, dark))
        },
        (120.0, 120.0),
        rune::Env::new(dark, 1.0),
    );
    rt.frame(0.0);
    rt
}

#[test]
fn constants_and_mono_profile_match_electron() {
    assert_eq!(PERIOD_SECS, 1.96);
    assert_eq!(STEPS, 59);
    assert!((FPS - 59.0 / 1.96).abs() < 1e-5, "fps {FPS}");
    assert!((STEP_SECONDS - 1.96 / 59.0).abs() < 1e-9, "step {STEP_SECONDS}");
    assert_eq!(RIM, 1.5);
    assert_eq!(RING_SIZE, 43.0);
    assert_eq!(STRENGTH, 1.0);

    // Conic stops from `.lc-coverbeam__light::before` at strength 1.
    assert_eq!(profile_alpha(0.0), 0.0);
    assert_eq!(profile_alpha(0.54), 0.0);
    assert!((profile_alpha(0.60) - 0.08).abs() < 1e-6);
    assert!((profile_alpha(0.66) - 0.45).abs() < 1e-6);
    assert!((profile_alpha(0.70) - 0.75).abs() < 1e-6);
    assert_eq!(profile_alpha(0.78), 0.0);
    assert_eq!(profile_alpha(0.95), 0.0);

    // Mono in both themes: neutral greyscale with zero hue shift.
    let dark = ink(true);
    let light = ink(false);
    assert_eq!((dark.r, dark.g, dark.b, dark.a), (1.0, 1.0, 1.0, 1.0));
    assert_eq!(light.r, light.g);
    assert_eq!(light.g, light.b);
    assert!(light.r < 0.2 && light.a == 1.0);

    // Mode matrix: cancelling turns the light off; reduced motion holds a still frame; otherwise moving.
    assert_eq!(mode(false, false), Mode::Moving);
    assert_eq!(mode(false, true), Mode::Still);
    assert_eq!(mode(true, false), Mode::Off);
    assert_eq!(mode(true, true), Mode::Off);
}

#[test]
fn stepping_holds_steady_within_each_of_the_59_steps_and_wraps_at_1_96_seconds() {
    // Inside the first 1.96 / 59 ≈ 0.03322 s step, 60 Hz and 120 Hz instants do not glide.
    for t in [0.0, 0.008, 0.016, 0.030] {
        assert_eq!(step_index(t), 0, "t={t}");
        assert_eq!(step_turn(t), 0.0, "t={t}");
    }
    assert_eq!(step_index(0.034), 1);
    assert!((step_turn(0.034) - 1.0 / 59.0).abs() < 1e-6);
    assert_eq!(step_index(1.96 * 29.5 / 59.0), 29);
    assert_eq!(step_index(1.96 * 58.5 / 59.0), 58);
    assert_eq!(step_index(1.96), 0);

    // Verify Rune's Cycle samples the exact 59-step staircase with zero gliding between ticks.
    let at = |t: f64| (0..59).map(|k| wedge_cycle(k).sample(t)).collect::<Vec<_>>();
    let s0 = at(0.0);
    assert_eq!(at(0.008), s0, "120 Hz sub-step must hold step 0");
    assert_eq!(at(0.016), s0, "60 Hz sub-step must hold step 0");
    assert_eq!(at(0.030), s0, "late within step 0 must still hold step 0");

    let (p0, v0) = peak_wedge(&s0);
    assert_eq!(p0, 41, "peak at step 0 sits at 0.70 turn (wedge 41 of 59)");
    assert!(v0 > 0.70 && v0 <= 0.75, "peak alpha {v0}");
    assert_eq!(s0.iter().filter(|&&a| a > 0.0).count(), 14, "lit arc spans 0.54..0.78 (14 of 59 wedges)");

    // Step 1 (past 1.96 / 59 s): peak jumps clockwise by one wedge, from 41 to 42.
    let s1 = at(0.034);
    let (p1, v1) = peak_wedge(&s1);
    assert_eq!(p1, 42);
    assert!((v1 - v0).abs() < 1e-6);

    // One full 1.96 s turn wraps cleanly back to step 0.
    assert_eq!(at(1.96), s0);

    // Cycle cadence and reduced-motion still pose.
    let c41 = wedge_cycle(41);
    assert_eq!(c41.period, 1.96);
    assert!((c41.fps - 59.0 / 1.96).abs() < 1e-5);
    assert!((c41.still - v0).abs() < 1e-6, "still pose keeps step 0's peak");
    assert_eq!(wedge_cycle(0).still, 0.0);
}

#[test]
fn wrapped_stop_button_steps_at_30_hz_holds_still_under_reduced_motion_and_sleeps_when_off() {
    // Active moving beam: 40×40 seat, 43×43 ring 1.5 px outside on every side, ~30.1 Hz wake deadline.
    let mut rt = runtime(Mode::Moving, true);
    let all = rt.elements();
    let beam_idx = all.iter().position(|e| e.key.as_deref() == Some("stop-beam")).expect("stop-beam");
    let send_idx = all.iter().position(|e| e.key.as_deref() == Some("send")).expect("send");
    let ring = all.iter().find(|e| e.key.as_deref() == Some("stop-beam-light")).expect("stop-beam-light");
    assert!(send_idx > beam_idx, "stop-beam wraps send");
    assert_eq!(all[send_idx].parent, Some(beam_idx));
    assert_eq!((all[beam_idx].rect[2], all[beam_idx].rect[3]), (40.0, 40.0));
    assert_eq!((all[send_idx].rect[2], all[send_idx].rect[3]), (40.0, 40.0));
    assert_eq!(ring.rect[0], all[send_idx].rect[0] - 1.5);
    assert_eq!(ring.rect[1], all[send_idx].rect[1] - 1.5);
    assert_eq!((ring.rect[2], ring.rect[3]), (43.0, 43.0));
    assert_eq!(ring.radius, 21.5);

    // Cycle sleeps between 1.96/59 s beats instead of asking for continuous 60/120 Hz frames.
    let wake = rt.wake_deadline().expect("active beam schedules next step");
    assert!((wake - 1.96 / 59.0).abs() < 1e-5, "wake {wake}");

    let op0 = wedge_opacities(&rt);
    assert_eq!(peak_wedge(&op0).0, 41);

    // Advancing 16 ms (60 Hz) stays inside step 0: zero gliding.
    assert!(!rt.frame(0.016));
    assert_eq!(wedge_opacities(&rt), op0);

    // Crossing 1.96 / 59 s (34 ms total) advances clockwise by one step to wedge 42.
    assert!(!rt.frame(0.018));
    assert_eq!(peak_wedge(&wedge_opacities(&rt)).0, 42);

    // Completing 1.96 s returns to wedge 41.
    assert!(!rt.frame(1.96 - 0.034));
    assert_eq!(wedge_opacities(&rt), op0);

    // Flipping Rune's reduced-motion flag freezes at step 0 and cancels wake deadlines.
    rt.reduced_motion = true;
    assert!(!rt.frame(0.1));
    assert_eq!(rt.wake_deadline(), None);
    assert_eq!(peak_wedge(&wedge_opacities(&rt)).0, 41);

    // Mode::Still (reduced motion): draws the calm still frame, no loops, no repaints.
    let mut still = runtime(Mode::Still, false);
    assert_eq!(still.wake_deadline(), None);
    let still_op = wedge_opacities(&still);
    assert_eq!(peak_wedge(&still_op).0, 41);
    assert!(still.elements().iter().filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("stop-beam-wedge-"))).all(|e| e.loop_phase.is_none()));
    assert!(!still.frame(0.2));
    assert_eq!(still.wake_deadline(), None);
    assert_eq!(wedge_opacities(&still), still_op);

    // Mode::Off (cancelling): wrapper stays round the Stop button, light is gone, no repaints.
    let mut off = runtime(Mode::Off, true);
    assert!(off.find("stop-beam").is_some());
    assert!(off.find("stop-beam-light").is_none());
    assert_eq!(off.wake_deadline(), None);
    assert!(!off.frame(0.2));
    assert_eq!(off.wake_deadline(), None);
}
