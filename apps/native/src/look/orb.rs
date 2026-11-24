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

fn sample_time(preset: &Preset, tick: Option<u64>) -> f64 {
    tick.map_or(STILL_TIME, |tick| tick as f64 * preset.speed / FPS)
}

/// Draw at most once per beat of the existing live word clock. The same window poll drives the
/// live word and the plan's moving surfaces. Reduced motion uses the library's still pose.
pub(crate) fn view(
    key: String,
    state: &'static str,
    surface: Surface,
    s: &crate::State,
    dark: bool,
) -> Option<El> {
    let preset = preset(state, surface)?;
    let tick = (s.window_focused && !s.reduced_motion.get())
        .then(|| (crate::working_shimmer::clock() * FPS) as u64);
    let width = surface.width();
    let drawing = if preset.size == 20 { 20 } else { width };
    Some(rune::memo::memo(
        key.clone(),
        (state, surface, dark, tick),
        || {
            let pixels = Pixels::new();
            let side = drawing as usize * PIXELS_PER_POINT;
            let factor = if preset.size == 64 {
                PAINT_DOWN_FACTOR
            } else {
                2
            };
            pixels.set(
                side as u32,
                side as u32,
                paint(
                    &preset.frame(sample_time(&preset, tick)),
                    side,
                    factor,
                    dark,
                ),
            );
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
