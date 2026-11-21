//! Electron's `the-message-box-focus-meets-3-to-1.test.ts` (2 cases): the focused box's edge is the box token, and
//! that token meets 3:1 against the page and the box, read from the tokens.css Rust draws from.
use super::*;

type Rgb = [f64; 3];

/// A token as the stylesheet wrote it (sRGB 0..1): Rune keeps colours linear, and a browser blends in sRGB.
fn srgb(c: rune::Color) -> (Rgb, f64) {
    let e = |v: f32| {
        let v = v as f64;
        if v <= 0.0031308 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
    };
    ([e(c.r), e(c.g), e(c.b)], c.a as f64)
}

fn over(top: rune::Color, ground: Rgb) -> Rgb {
    let (c, a) = srgb(top);
    [c[0] * a + ground[0] * (1.0 - a), c[1] * a + ground[1] * (1.0 - a), c[2] * a + ground[2] * (1.0 - a)]
}

fn luminance(c: Rgb) -> f64 {
    let f = |v: f64| if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2])
}

fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

// case: draws its edge in the box token
#[test]
fn draws_its_edge_in_the_box_token() {
    // As Electron reads its stylesheet: the box's own source, where a later edit to the edge is seen.
    let source = include_str!("controls.rs");
    let at = source.find("pub fn composer_box(").expect("composer_box");
    let body = &source[at..at + source[at..].find("\n}").unwrap()];
    assert!(body.contains(r#".focus_within_ring(1.0, 0.0, l.c("focus-edge-box"))"#), "{body}");
}

// case: is at least 3:1 against the page and against the box
#[test]
fn is_at_least_3_to_1_against_the_page_and_against_the_box() {
    // The dark theme, as Electron's test reads it (the first value of each token in tokens.css). The light theme's edge
    // is 2.4:1 on its page in Electron too; that is a colour for Electron's tokens, not one Rust invents.
    for dark in [true] {
        let t = tokens(dark);
        let (page, _) = srgb(t.color("bg-app"));
        let box_ = over(t.color("chat-bg"), page);
        let edge = over(t.color("focus-edge-box"), page);
        assert!(contrast(edge, page) >= 3.0, "dark {dark}: edge on page {}", contrast(edge, page));
        assert!(contrast(edge, box_) >= 3.0, "dark {dark}: edge on box {}", contrast(edge, box_));
    }
}
