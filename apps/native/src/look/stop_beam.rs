//! The Stop button's quiet mono beam (`the-beams-are-mono-and-quiet.test.tsx`, `Composer.tsx` `<Beam className="lc-stopbeam">`,
//! `shell.css` `.lc-stopbeam`): a mono conic light reusing Home's cover beam (`home_panel::cover_beam`) at full strength
//! (`strength={1}`), travelling round the 40×40 Stop disc in 59 linear steps per 1.96 s turn (~30.1 a second,
//! `BOT_FRAMES_PER_SECOND`) on Rune's own window frame clock (`Cycle`), never gliding between steps, holding a calm still
//! frame under reduced motion, and requesting no repaints when nothing runs.

use super::controls::SEND_SIZE;
use rune::{Color, Cycle, El, Shape, col};

/// One full turn of the stop beam (`shell.css` `.lc-stopbeam`: 59 steps a turn of the package's 1.96 s).
pub const PERIOD_SECS: f64 = 1.96;
/// Linear steps per turn (`animation-timing-function: steps(59), ease`).
pub const STEPS: usize = 59;
/// Step rate: 59 / 1.96 ≈ 30.1 a second, matching `BOT_FRAMES_PER_SECOND`.
pub const FPS: f32 = STEPS as f32 / PERIOD_SECS as f32;
/// Seconds per step (`1.96 / 59 ≈ 0.03322 s`).
pub const STEP_SECONDS: f64 = PERIOD_SECS / STEPS as f64;
/// Outer rim width round the 40×40 Stop disc, matching `home_panel::cover_beam`'s 1.5 px rim.
pub const RIM: f32 = 1.5;
/// Diameter of the beam ring wrapping the 40×40 Stop disc (`43.0` pt).
pub const RING_SIZE: f32 = SEND_SIZE + 2.0 * RIM;
/// Beam intensity (`Composer.tsx`: `<Beam size="sm" strength={1} ...>`).
pub const STRENGTH: f32 = 1.0;
/// Still step held under reduced motion.
pub const STILL_STEP: usize = 0;

/// Conic gradient stops from `.lc-coverbeam__light::before` (`home_panel::beam_light`): clear to 54%, white at 0.08,
/// 0.45 and 0.75 by 60, 66 and 70%, clear again by 78%.
const PROFILE_STOPS: [(f32, f32); 5] = [
    (0.54, 0.0),
    (0.60, 0.08),
    (0.66, 0.45),
    (0.70, 0.75),
    (0.78, 0.0),
];

/// How the Stop button's beam is drawn while Stop is offered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Active run, motion enabled: 59-step travelling mono beam on Rune's frame clock.
    Moving,
    /// Active run, reduced motion: calm still frame at `STILL_STEP`, no repaints.
    Still,
    /// Stop already pressed (`cancelling`): wrapper stays round the Stop button, light off, no repaints.
    Off,
}

/// Which mode the Stop button's beam wears (`Composer.tsx`: `active={!cancelling}`, `Beam.tsx`: reduced motion).
pub const fn mode(cancelling: bool, reduced_motion: bool) -> Mode {
    if cancelling {
        Mode::Off
    } else if reduced_motion {
        Mode::Still
    } else {
        Mode::Moving
    }
}

/// Mono beam colour for the drawn theme: pure neutral greyscale (no hue shift) visible against the button's ground.
pub const fn ink(dark: bool) -> Color {
    if dark {
        Color::WHITE
    } else {
        Color {
            r: 0.11,
            g: 0.11,
            b: 0.11,
            a: 1.0,
        }
    }
}

/// Conic alpha at `turn` in `[0, 1)` (0 is straight up, clockwise), matching `home_panel::beam_light`.
pub const fn profile_alpha(turn: f32) -> f32 {
    if turn <= PROFILE_STOPS[0].0 || turn >= PROFILE_STOPS[4].0 {
        return 0.0;
    }
    let mut i = 0;
    while i < 4 {
        let (t0, a0) = PROFILE_STOPS[i];
        let (t1, a1) = PROFILE_STOPS[i + 1];
        if turn <= t1 {
            return a0 + (a1 - a0) * (turn - t0) / (t1 - t0);
        }
        i += 1;
    }
    0.0
}

/// Which of the 59 steps the beam stands on at `seconds` (`steps(59)` / `jump-end`: holds steady across each step).
pub fn step_index(seconds: f64) -> usize {
    let phase = (seconds / PERIOD_SECS).rem_euclid(1.0);
    ((phase * STEPS as f64).floor() as usize) % STEPS
}

/// Quantized clockwise turn in `[0, 1)` at `seconds` (`step_index(seconds) / 59`).
pub fn step_turn(seconds: f64) -> f32 {
    step_index(seconds) as f32 / STEPS as f32
}

/// Opacity of angular wedge `wedge` (`0..STEPS`, clockwise from 12 o'clock) when the beam stands at `step` (`0..STEPS`).
pub const fn wedge_alpha(wedge: usize, step: usize) -> f32 {
    let rel = (wedge % STEPS + STEPS - (step % STEPS)) % STEPS;
    let mid = (rel as f32 + 0.5) / STEPS as f32;
    let a = profile_alpha(mid) * STRENGTH;
    if a > 1.0 { 1.0 } else { a }
}

/// Exact 59-step staircase for Rune's `Cycle`: each `1/59` bucket `[j/59, (j+1)/59]` holds one constant opacity so
/// `Cycle::sample` steps discretely rather than gliding between ticks even if another surface repaints faster.
const fn build_cycle_stops() -> [(f32, f32); STEPS * 2] {
    let mut out = [(0.0_f32, 0.0_f32); STEPS * 2];
    let mut j = 0;
    while j < STEPS {
        let rel = (STEPS - j) % STEPS;
        let alpha = wedge_alpha(rel, 0);
        let start = j as f32 / STEPS as f32;
        let end = if j + 1 == STEPS {
            1.0
        } else {
            (j + 1) as f32 / STEPS as f32
        };
        out[2 * j] = (start, alpha);
        out[2 * j + 1] = (end, alpha);
        j += 1;
    }
    out
}

static CYCLE_STOPS: [(f32, f32); STEPS * 2] = build_cycle_stops();

/// The `Cycle` for wedge `wedge` (`0..STEPS`): period 1.96 s, 59 discrete steps (~30.1 fps), still frame at `STILL_STEP`.
pub fn wedge_cycle(wedge: usize) -> Cycle {
    // Tiny negative phase bias (0.001 of one step ≈ 33 µs) keeps exact step boundaries strictly inside their bucket
    // against IEEE-754 rounding in `(t / period - phase).rem_euclid(1.0)`.
    let phase = (wedge as f32 - 0.001) / STEPS as f32;
    Cycle {
        period: PERIOD_SECS as f32,
        phase,
        stops: &CYCLE_STOPS,
        still: wedge_alpha(wedge, STILL_STEP),
        fps: FPS,
    }
}

/// Annular wedge `wedge` (`0..STEPS`) around the circular Stop disc: inner radius sits 1 pt inside the 20 pt button
/// edge (hidden behind the disc) and outer radius reaches past the clipped 21.5 pt rim so `clip_rounded` cuts a crisp arc.
pub fn wedge_shape(size: f32, wedge: usize, colour: Color) -> Shape {
    let c = size / 2.0;
    let r_in = (SEND_SIZE / 2.0 - 1.0).max(0.0);
    let r_out = c + 0.5;
    let a0 = wedge as f32 / STEPS as f32 * std::f32::consts::TAU;
    let a1 = (wedge + 1) as f32 / STEPS as f32 * std::f32::consts::TAU;
    // 0 is straight up, clockwise, matching CSS `conic-gradient(from 0deg, ...)` and `home_panel::beam_light`.
    let pt = |r: f32, a: f32| [c + r * a.sin(), c - r * a.cos()];
    Shape::new()
        .view_box(size, size)
        .fill(vec![pt(r_in, a0), pt(r_out, a0), pt(r_out, a1), pt(r_in, a1)], colour)
}

/// Wraps the composer's 40×40 Stop button in its mono beam without changing its layout footprint.
/// Draws the light behind the button (1.5 px outside on every side), so the button receives all input unobstructed.
pub fn wrap(button: El, mode: Mode, dark: bool) -> El {
    let mut wrapper = col().w(SEND_SIZE).h(SEND_SIZE).key("stop-beam");
    if mode != Mode::Off {
        let colour = ink(dark);
        let radius = RING_SIZE / 2.0;
        let mut ring = col()
            .absolute(-RIM, -RIM)
            .size(RING_SIZE, RING_SIZE)
            .radius(radius)
            .clip_rounded(radius)
            .key("stop-beam-light")
            .a11y_hidden();
        for k in 0..STEPS {
            let mut wedge = rune::canvas(wedge_shape(RING_SIZE, k, colour))
                .size(RING_SIZE, RING_SIZE)
                .absolute(0.0, 0.0)
                .key(format!("stop-beam-wedge-{k}"))
                .a11y_hidden();
            wedge = match mode {
                Mode::Moving => wedge.cycle(wedge_cycle(k)),
                Mode::Still => wedge.opacity(wedge_alpha(k, STILL_STEP)),
                Mode::Off => unreachable!(),
            };
            ring = ring.child(wedge);
        }
        wrapper = wrapper.child(ring);
    }
    wrapper.child(button)
}

#[cfg(test)]
#[path = "stop_beam_tests.rs"]
mod tests;
