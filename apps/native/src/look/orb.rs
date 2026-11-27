//! The thread's thinking orb, using the same drawings and presets as Electron's Orb.tsx.

#[path = "orb_shapes.rs"]
mod shapes;

use rune::{El, Pixels, el};
use shapes::{Frame, Preset, Shape};

#[derive(Clone, Copy, Hash)]
pub(crate) enum Surface {
    Live,
    PlanHead,
    PlanStep,
}

impl Surface {
    fn width(self) -> u32 {
        match self {
            Self::Live => 26,
            Self::PlanHead => 24,
            Self::PlanStep => 20,
        }
    }
}

fn preset(state: &str, surface: Surface) -> Option<Preset> {
    let (shape, size, speed) = match state {
        "composing" => (Shape::Ribbon, 64, 2.34),
        "listening" if matches!(surface, Surface::PlanStep) => (Shape::Wave, 20, 3.998),
        "listening" => (Shape::Wave, 64, 4.388),
        "solving" => (Shape::Rubik, 64, 1.82),
        "searching" => (Shape::Globe, 64, 2.015),
        "connecting" => (Shape::Web, 64, 3.315),
        "weaving" => (Shape::Braid, 64, 1.625),
        "shaping" => (Shape::Morph, 20, 2.08),
        "breathing" => (Shape::Ring, 20, 3.78),
        "working" => (Shape::Orbits, 20, 3.9),
        _ => return None,
    };
    Some(Preset { shape, size, speed })
}

const FPS: f64 = 30.0;
const STILL_TIME: f64 = 0.6;
const PAINT_DOWN_FACTOR: usize = 4;
const PIXELS_PER_POINT: usize = 2;

/// How often a turning orb is redrawn: Electron's frame beat (frameBeat.ts:41, Orb.tsx:219), 30 a second.
pub(crate) const FRAME_MS: u64 = 33;

fn sample_time(preset: &Preset, tick: Option<u64>) -> f64 {
    tick.map_or(STILL_TIME, |tick| tick as f64 * preset.speed / FPS)
}

/// An orb on screen while a turn runs: one image, redrawn in place on its own beat.
struct Turning {
    pixels: Pixels,
    preset: Preset,
    side: usize,
    factor: usize,
    dark: bool,
    state: &'static str,
    /// The beat it last drew.
    tick: u64,
}

#[derive(Default)]
struct Orbs {
    /// Every orb is drawn only while a turn's line sweeps (`messages::live_sweeping`, the plan's `underway`), the
    /// same condition that keeps this set, so an orb is never on screen with its image out of the set.
    turning: std::collections::HashMap<String, Turning>,
    /// The beat of the window's last poll; a new orb starts there.
    tick: u64,
    /// Every redraw, for the window's own tests.
    drawn: u64,
}

thread_local! {
    static ORBS: std::cell::RefCell<Orbs> = std::cell::RefCell::new(Orbs::default());
}

/// THE ORB TURNS BEHIND OTHER WINDOWS, AS ELECTRON'S DOES. Electron's orb is a canvas on its own loop
/// (components/Orb.tsx:202-232), stopped only for reduced motion or when scrolled out of view, never for a window
/// behind; the Rust orb was given a clock only while the window was in front and otherwise drew one still pose, so side
/// by side it froze (REVIEW-visual-live-rust-2026-11-26.md, Motion). Each orb is now one image redrawn in place on a
/// 33 ms beat while a turn runs (`Pixels::set` wakes only what shows it), never by rebuilding the window's view. At rest
/// the set is empty: nothing is drawn and no beat is asked for. Called from the window's poll; true while an orb turns,
/// so the poll asks to be woken for the next beat.
pub(crate) fn tick(s: &crate::State, now: f64) -> bool {
    advance(crate::messages::live_sweeping(s) && moving(s), now)
}

/// Whether an orb on screen moves: not under reduced motion, and not once Stop is pressed and the stop has not landed
/// (REVIEW-ui-changes-2026-11-26.md change 7: "the orb goes still"), so a still orb with "Stopping…" says the press
/// took.
fn moving(s: &crate::State) -> bool {
    !s.reduced_motion.get() && !s.stopping.is_pending()
}

fn advance(turning: bool, now: f64) -> bool {
    ORBS.with(|orbs| {
        let mut orbs = orbs.borrow_mut();
        if !turning {
            orbs.turning.clear();
            return false;
        }
        let tick = (now.max(0.0) * FPS) as u64;
        orbs.tick = tick;
        let mut drawn = 0;
        for orb in orbs.turning.values_mut().filter(|orb| orb.tick != tick) {
            orb.tick = tick;
            orb.pixels.set(orb.side as u32, orb.side as u32, paint(&orb.preset.frame(sample_time(&orb.preset, Some(tick))), orb.side, orb.factor, orb.dark));
            drawn += 1;
        }
        orbs.drawn += drawn;
        !orbs.turning.is_empty()
    })
}

/// How many orb frames have been drawn on this thread since it started (the window's own tests).
#[cfg(test)]
pub(crate) fn frames_drawn() -> u64 {
    ORBS.with(|orbs| orbs.borrow().drawn)
}

/// The image an orb draws into: the turning one for `key`, made (and drawn at the current beat) when it is new or its
/// state or theme changed.
fn turning_pixels(key: &str, state: &'static str, preset: Preset, side: usize, factor: usize, dark: bool) -> Pixels {
    ORBS.with(|orbs| {
        let mut orbs = orbs.borrow_mut();
        let tick = orbs.tick;
        if let Some(orb) = orbs.turning.get(key).filter(|orb| orb.state == state && orb.dark == dark && orb.side == side) {
            return orb.pixels.clone();
        }
        let pixels = Pixels::new();
        pixels.set(side as u32, side as u32, paint(&preset.frame(sample_time(&preset, Some(tick))), side, factor, dark));
        orbs.turning.insert(key.to_owned(), Turning { pixels: pixels.clone(), preset, side, factor, dark, state, tick });
        pixels
    })
}

/// The orb's image element. While motion is wanted it is the turning image, redrawn in place by `tick`, so the view
/// keeps one element and is never rebuilt to move it; under reduced motion it is the library's still pose.
pub(crate) fn view(
    key: String,
    state: &'static str,
    surface: Surface,
    s: &crate::State,
    dark: bool,
) -> Option<El> {
    let preset = preset(state, surface)?;
    let turning = moving(s);
    let width = surface.width();
    let drawing = if preset.size == 20 { 20 } else { width };
    Some(rune::memo::memo(
        key.clone(),
        (state, surface, dark, turning),
        || {
            let side = drawing as usize * PIXELS_PER_POINT;
            let factor = if preset.size == 64 {
                PAINT_DOWN_FACTOR
            } else {
                2
            };
            let pixels = if turning {
                turning_pixels(&key, state, preset, side, factor, dark)
            } else {
                let pixels = Pixels::new();
                pixels.set(side as u32, side as u32, paint(&preset.frame(sample_time(&preset, None)), side, factor, dark));
                pixels
            };
            let image = el()
                .key(key)
                .trace_name("thinking-orb")
                .trace_namespace("thinking-orb", state)
                .role("image")
                .size(drawing as f32, drawing as f32)
                .pixels(&pixels)
                .a11y_hidden();
            if drawing == width {
                image
            } else {
                rune::row()
                    .size(width as f32, width as f32)
                    .center()
                    .child(image)
            }
        },
    ))
}

#[derive(Clone, Copy, Default)]
struct Sample {
    ink: f32,
    alpha: f32,
}

impl Sample {
    fn over(&mut self, ink: f32, alpha: f32) {
        self.ink = ink * alpha + self.ink * (1.0 - alpha);
        self.alpha = alpha + self.alpha * (1.0 - alpha);
    }
}

struct Raster {
    side: usize,
    pixels: Vec<Sample>,
}

impl Raster {
    /// Fill the samples inside a dot or a line's strip. Coordinates are in the supersampled image.
    fn paint(
        &mut self,
        [left, top, right, bottom]: [f64; 4],
        ink: f64,
        alpha: f64,
        covers: impl Fn(f64, f64) -> bool,
    ) {
        let bound = |edge: f64| (edge as usize).min(self.side);
        for y in bound(top.floor())..bound(bottom.ceil()) {
            for x in bound(left.floor())..bound(right.ceil()) {
                if covers(x as f64 + 0.5, y as f64 + 0.5) {
                    self.pixels[y * self.side + x].over(ink as f32, alpha as f32);
                }
            }
        }
    }

    fn dot(&mut self, x: f64, y: f64, r: f64, ink: f64, alpha: f64) {
        self.paint([x - r, y - r, x + r, y + r], ink, alpha, |u, v| {
            (x - u).powi(2) + (y - v).powi(2) <= r * r
        });
    }

    fn line(&mut self, [x, y]: [f64; 2], [u, v]: [f64; 2], r: f64, ink: f64, alpha: f64) {
        let (dx, dy) = (u - x, v - y);
        let squared = dx * dx + dy * dy;
        self.paint(
            [x.min(u) - r, y.min(v) - r, x.max(u) + r, y.max(v) + r],
            ink,
            alpha,
            |px, py| {
                let (px, py) = (px - x, py - y);
                let along = px * dx + py * dy;
                (0.0..=squared).contains(&along) && (px * dy - py * dx).powi(2) <= r * r * squared
            },
        );
    }

    /// The output texel is the area average of every covered sample, in premultiplied colour.
    fn downsample(&self, side: usize, factor: usize) -> Vec<u8> {
        let mut pixels = Vec::with_capacity(side * side * 4);
        for y in 0..side {
            for x in 0..side {
                let mut sum = Sample::default();
                for row in y * factor..(y + 1) * factor {
                    for column in x * factor..(x + 1) * factor {
                        let pixel = self.pixels[row * self.side + column];
                        sum.alpha += pixel.alpha;
                        sum.ink += pixel.ink;
                    }
                }
                let alpha = (255.0 * sum.alpha / (factor * factor) as f32).round() as u8;
                let ink = if sum.alpha > 0.0 {
                    (255.0 * sum.ink / sum.alpha).round() as u8
                } else {
                    0
                };
                pixels.extend_from_slice(&[ink, ink, ink, alpha]);
            }
        }
        pixels
    }
}

fn paint(frame: &Frame, side: usize, factor: usize, dark: bool) -> Vec<u8> {
    let high = side * factor;
    let scale = high as f64 / frame.size;
    let mut raster = Raster {
        side: high,
        pixels: vec![Sample::default(); high * high],
    };
    let ink = |shade: f64| {
        let shade = shade.clamp(0.0, 1.0);
        if dark { 1.0 - shade } else { shade }
    };
    for line in &frame.lines {
        let [x, y, _] = line.from.map(|v| v * scale);
        let [u, v, _] = line.to.map(|v| v * scale);
        raster.line(
            [x, y],
            [u, v],
            line.width * scale / 2.0,
            ink(0.42),
            line.alpha,
        );
    }
    for dot in &frame.dots {
        let [x, y, _] = dot.center.map(|v| v * scale);
        raster.dot(x, y, dot.radius * scale, ink(dot.shade), dot.alpha);
    }
    raster.downsample(side, factor)
}

#[cfg(test)]
#[path = "orb_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "orb_turns_tests.rs"]
mod orb_turns_tests;
