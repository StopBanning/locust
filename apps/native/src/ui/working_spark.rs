//! THE WORKING SPARK (Electron 0.392, `WorkingSpark.tsx`, shell.css `.lc-spark`): Claude Code's own spinner, the
//! glyphs its terminal turns through, · ✢ ✳ ✶ ✻ ✽ and back, one every 120 ms. Since 0.839 Electron's sidebar rows
//! shimmer instead (working_shimmer.rs); the spark is what a comparison's working column, the judge reading, a
//! benchmark's rerun and the post-edit check wear.
//!
//! Electron steps a pseudo-element's `content` with CSS, so running marks cost no renders. Here each of the six
//! glyphs is a layer on the window's own frame clock (a wall-clock opacity `Cycle`), lit only in its 120 ms slots:
//! Rune wakes the window for them only while a spark is drawn, and none is drawn when nothing works. Under reduced
//! motion Rune holds every layer at its `still`, which is the spark at rest, ✻ (`.lc-spark::before`'s own content).

use super::*;
use rune::Cycle;

/// `@keyframes lc-spark`, a frame to each of its twelve slots (`steps(1, end)`: the 100% frame is never held), every
/// dingbat in its text form (FE0E) so no face draws it as an emoji.
pub(super) const FRAMES: [&str; 12] = [
    "\u{00B7}",
    "\u{2722}\u{FE0E}",
    "\u{2733}\u{FE0E}",
    "\u{2736}\u{FE0E}",
    "\u{273B}\u{FE0E}",
    "\u{273D}\u{FE0E}",
    "\u{273D}\u{FE0E}",
    "\u{273B}\u{FE0E}",
    "\u{2736}\u{FE0E}",
    "\u{2733}\u{FE0E}",
    "\u{2722}\u{FE0E}",
    "\u{00B7}",
];
/// One turn (`animation: lc-spark 1.44s`): twelve slots of 120 ms.
pub(super) const PERIOD_SECONDS: f32 = 1.44;
/// The spark at rest (`.lc-spark::before { content: '\273B\FE0E' }`), held under reduced motion.
pub(super) const STILL: &str = "\u{273B}\u{FE0E}";
/// The footprint, the finished dot's (`.lc-spark`: 6 by 6, the glyph spilling out of it).
pub(super) const FOOTPRINT: f32 = 6.0;
/// How often the window wakes for a spark: three times a slot, so a frame turns at most 40 ms late.
const WAKES_PER_SECOND: f32 = 25.0;

const S: f32 = 1.0 / 12.0;
/// Glyph `i` of the first six frames is lit in slots `i` and `11 - i`; the middle two slots are both ✽.
const LIT: [&[(f32, f32)]; 6] = [
    &[(0.0, 1.0), (S, 1.0), (S, 0.0), (11.0 * S, 0.0), (11.0 * S, 1.0), (1.0, 1.0)],
    &[(0.0, 0.0), (S, 0.0), (S, 1.0), (2.0 * S, 1.0), (2.0 * S, 0.0), (10.0 * S, 0.0), (10.0 * S, 1.0), (11.0 * S, 1.0), (11.0 * S, 0.0), (1.0, 0.0)],
    &[(0.0, 0.0), (2.0 * S, 0.0), (2.0 * S, 1.0), (3.0 * S, 1.0), (3.0 * S, 0.0), (9.0 * S, 0.0), (9.0 * S, 1.0), (10.0 * S, 1.0), (10.0 * S, 0.0), (1.0, 0.0)],
    &[(0.0, 0.0), (3.0 * S, 0.0), (3.0 * S, 1.0), (4.0 * S, 1.0), (4.0 * S, 0.0), (8.0 * S, 0.0), (8.0 * S, 1.0), (9.0 * S, 1.0), (9.0 * S, 0.0), (1.0, 0.0)],
    &[(0.0, 0.0), (4.0 * S, 0.0), (4.0 * S, 1.0), (5.0 * S, 1.0), (5.0 * S, 0.0), (7.0 * S, 0.0), (7.0 * S, 1.0), (8.0 * S, 1.0), (8.0 * S, 0.0), (1.0, 0.0)],
    &[(0.0, 0.0), (5.0 * S, 0.0), (5.0 * S, 1.0), (7.0 * S, 1.0), (7.0 * S, 0.0), (1.0, 0.0)],
];

/// The six glyph layers' cycles, in the order of the first six frames.
fn layers() -> impl Iterator<Item = (&'static str, Cycle)> {
    FRAMES[..6].iter().zip(LIT).map(|(glyph, stops)| {
        let still = if *glyph == STILL { 1.0 } else { 0.0 };
        (*glyph, Cycle { period: PERIOD_SECONDS, phase: 0.0, stops, still, fps: WAKES_PER_SECOND })
    })
}

/// The spark: the dot's 6-point footprint, the glyph centred over it and allowed to spill (`translate: -50% -52%`),
/// 13 points, in the theme's text (monochrome since 0.429), hidden from a screen reader as Electron's is.
pub(super) fn working_spark(l: &crate::look::controls::Look, key: impl Into<String>) -> El {
    const BOX: f32 = 20.0;
    let ink = l.c("text-primary");
    let mut glyphs = el().absolute((FOOTPRINT - BOX) / 2.0, (FOOTPRINT - BOX) / 2.0 - 0.02 * 13.0).size(BOX, BOX);
    for (index, (glyph, cycle)) in layers().enumerate() {
        glyphs = glyphs.child(
            row().absolute(0.0, 0.0).size(BOX, BOX).center()
                .child(text(glyph).font(rune::TextStyle::new(13.0, 400.0).lh(13.0)).color(ink))
                .cycle(cycle)
                .key(format!("spark-glyph-{index}")),
        );
    }
    el().key(key).size(FOOTPRINT, FOOTPRINT).a11y_hidden().child(glyphs)
}

#[cfg(test)]
#[path = "working_spark_tests.rs"]
mod working_spark_tests;
