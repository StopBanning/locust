//! A WORKING ROW SHIMMERS, AS CLAUDE CODE'S DOES (Electron 0.839, shell.css `.lc-conv__title.is-working`). Colin,
//! 2026-10-28: a turning mark on every running row was "too distracting" with many chats going; a light band passes
//! slowly through the title instead. Electron clips a 250%-wide gradient to the glyphs and slides it with CSS; Rune
//! has no text clip, so each character takes the gradient's value at its place, which is the same band at a
//! character's grain.

use rune::{Color, Span};

/// One pass of the band (`animation: lc-shimmer 2s linear infinite`).
pub(crate) const PASS_SECONDS: f64 = 2.0;
/// The grey is the theme's own text at a third (`color-mix(in srgb, var(--lc-text-primary) 34%, transparent)`),
/// never the muted grey, which Liquid glass lifts to near white (0.861).
pub(crate) const DIM: f32 = 0.34;
/// The band crosses a title one and a half widths in two seconds, about twenty characters a second, so a frame every
/// 50 ms moves it about one character: faster would redraw the window for nothing a person can see.
pub(crate) const FRAME_MS: u64 = 50;

/// The band's and the sweep's steps a second: they stand on a grid of `FRAME_MS` steps of the window's clock, so a
/// frame between two steps draws them where the last step put them, and only a new step moves them (speed-3 row 1).
const STEPS: f64 = 1000.0 / FRAME_MS as f64;
/// A clock within this of a step is at it: a wake asked for a step lands there to within Rune's clock arithmetic.
const AT_STEP: f64 = 1e-6;

/// The step the window's clock `now` stands in.
pub(crate) fn step(now: f64) -> i64 {
    ((now + AT_STEP) * STEPS).floor() as i64
}

/// Where the band and the sweep stand at `now`: the start of its step, on the window's own clock.
pub(crate) fn on_step(now: f64) -> f64 {
    step(now) as f64 / STEPS
}

/// The wake for the next step, unless one is asked for already (`asked`, the step it was asked for). Every poll while
/// the band moves comes here, but the window holds one wake a step: a wake asked 50 ms from each poll started a chain
/// of its own at every frame between two steps (a hover, a landing reply), and the chains never met, so a running row
/// framed and rebuilt the window up to 60 times a second (speed-3 row 0).
pub(crate) fn next_step_wake(now: f64, asked: &mut Option<i64>) -> Option<std::time::Duration> {
    let next = step(now) + 1;
    if asked.is_some_and(|step| step >= next) {
        return None;
    }
    *asked = Some(next);
    Some(std::time::Duration::from_secs_f64((next as f64 / STEPS - now).max(0.0)))
}

/// The gradient at `u` across its image (`dim 0%, dim 32%, primary 50%, dim 68%, dim 100%`).
fn gradient(u: f64) -> f32 {
    let off = (u - 0.5).abs();
    if off >= 0.18 {
        DIM
    } else {
        DIM + (1.0 - DIM) * (1.0 - off / 0.18) as f32
    }
}

/// How bright the title is at `x` (0 its left edge, 1 its right) `seconds` into the shimmer. The image is 250% of
/// the title (`background-size: 250% 100%`) and its position runs from 100% to 0% (`lc-shimmer`), so the band enters
/// on the left and leaves on the right.
pub(crate) fn brightness_at(x: f64, seconds: f64) -> f32 {
    let position = 1.0 - (seconds / PASS_SECONDS).rem_euclid(1.0);
    gradient((x + 1.5 * position) / 2.5)
}

/// The title's characters, each in the theme's text at the band's brightness where it stands.
pub(crate) fn spans(title: &str, seconds: f64, ink: Color) -> Vec<Span> {
    let count = title.chars().count().max(1) as f64;
    title
        .char_indices()
        .enumerate()
        .map(|(i, (at, ch))| {
            let x = (i as f64 + 0.5) / count;
            Span::new(at, at + ch.len_utf8()).color(ink.alpha(ink.a * brightness_at(x, seconds)))
        })
        .collect()
}

/// A working title: the band at `seconds` on the window's own clock (its poll's `now`, the same for every working row
/// in that window), or for a person who asked for less motion the theme's text standing still
/// (`@media (prefers-reduced-motion: reduce)`: `animation: none; color: var(--lc-text-primary)`). The window's clock,
/// not the wall clock: a snap at one second shows the band where one second puts it, however fast the frames before it
/// were drawn (the picture tests once had to leave the shimmering title out, harden-1).
pub(crate) fn title(words: &str, ink: Color, reduced: bool, seconds: f64) -> rune::El {
    rune::text(words.to_owned()).color(element_ink(ink, reduced)).spans(title_spans(words, ink, reduced, seconds))
}

/// The element's own colour, which paints what the spans do not cover: the "…" a long title is cut with is the grey
/// the band passes over, as Electron's clip paints it, and the theme's text when it stands still.
pub(crate) fn element_ink(ink: Color, reduced: bool) -> Color {
    if reduced { ink } else { ink.alpha(ink.a * DIM) }
}

/// The spans a working title wears: none (the plain text) under less motion, else the band at `seconds`.
pub(crate) fn title_spans(words: &str, ink: Color, reduced: bool, seconds: f64) -> Vec<Span> {
    if reduced { Vec::new() } else { spans(words, seconds, ink) }
}

// ---- THE LIVE WORD'S SWEEP (Electron shell.css `.lc-sweep`, ported from libraries.dev's `.t-shimmer`) -------------
//
// A solid word with a highlight passing over it: the word is painted in the muted grey and stays painted, and only an
// overlay drawn from its own text is clipped to a moving gradient (`content: attr(data-text)`). Anything that stops the
// overlay leaves the word, never a blank. Rune has no text clip, so each character takes the overlay's value at its
// place, mixed over the word's grey.

/// The overlay image is four widths of the word (`background-size: 400% 100%`).
pub(crate) const SWEEP_IMAGE: f64 = 4.0;
/// One pass (`animation: lc-sweep 2s linear infinite`).
pub(crate) const SWEEP_SECONDS: f64 = 2.0;

/// The overlay's gradient at `u` across its image (`transparent 0%, transparent 40%, name 50%, transparent 60%,
/// transparent 100%`): 0 is none of the highlight, 1 all of it.
fn sweep_gradient(u: f64) -> f32 {
    let off = (u - 0.5).abs();
    if off >= 0.1 { 0.0 } else { (1.0 - off / 0.1) as f32 }
}

/// How much highlight is over the word at `x` (0 its left edge, 1 its right) `seconds` in. The position runs from 100%
/// to 0% (`@keyframes lc-sweep`), so the band enters on the left and leaves on the right.
pub(crate) fn sweep_at(x: f64, seconds: f64) -> f32 {
    let position = 1.0 - (seconds / SWEEP_SECONDS).rem_euclid(1.0);
    sweep_gradient((x + (SWEEP_IMAGE - 1.0) * position) / SWEEP_IMAGE)
}

/// The live word's characters: the muted word with the highlight (`--lc-text-name`) over it where the band stands.
/// Under less motion only the highlight goes (`.lc-sweep::before { display: none }`): no spans, the plain word.
pub(crate) fn sweep_spans(words: &str, seconds: f64, word: Color, highlight: Color, reduced: bool) -> Vec<Span> {
    if reduced {
        return Vec::new();
    }
    let count = words.chars().count().max(1) as f64;
    words
        .char_indices()
        .enumerate()
        .map(|(i, (at, ch))| {
            let x = (i as f64 + 0.5) / count;
            Span::new(at, at + ch.len_utf8()).color(word.mix(highlight, sweep_at(x, seconds)))
        })
        .collect()
}

/// The live word as a label rather than a machine string (`.lc-livestep__register`): 14px in the UI face at 500.
pub(crate) const LIVE_WORD: rune::TextStyle = rune::TextStyle::new(14.0, 500.0);

/// The live word, drawn at `seconds` on the window's own clock, as [`title`]: the word's own colour is the muted grey
/// whatever the band does, so the "…" a long word is cut with, and every character once the band has passed, are the
/// word itself.
pub(crate) fn live_word(words: &str, seconds: f64, word: Color, highlight: Color, reduced: bool) -> rune::El {
    rune::text(words.to_owned()).font(LIVE_WORD).color(word).spans(sweep_spans(words, seconds, word, highlight, reduced))
}

#[cfg(test)]
#[path = "working_shimmer_tests.rs"]
mod working_shimmer_tests;
