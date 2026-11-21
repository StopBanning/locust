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

/// Seconds since the window first drew a shimmer: the band's clock, the same for every working row.
pub(crate) fn clock() -> f64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64()
}

/// A working title: the band, or for a person who asked for less motion the theme's text standing still
/// (`@media (prefers-reduced-motion: reduce)`: `animation: none; color: var(--lc-text-primary)`).
pub(crate) fn title(words: &str, ink: Color, reduced: bool) -> rune::El {
    rune::text(words.to_owned()).color(element_ink(ink, reduced)).spans(title_spans(words, ink, reduced, clock()))
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

#[cfg(test)]
#[path = "working_shimmer_tests.rs"]
mod working_shimmer_tests;
