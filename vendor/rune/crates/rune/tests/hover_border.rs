//! `El::hover_border`: the border's colour while hovered, as CSS `:hover { border-color }` sets
//! it, on the same curves as `El::hover`.
//!
//! Electron: `.lc-button:hover:not(:disabled) { border-color: var(--lc-hover-border) }` with
//! `transition: … border-color var(--lc-motion-hover) var(--lc-ease)`
//! (apps/desktop/src/renderer/src/shell.css:686-702).
mod support;

use rune::{Color, Cx, El, Env, Runtime, col, el, text};
use rune::{Curve, Easing};
use support::{centre, find};

const REST: &str = "#336699";
const LIT: &str = "#cc3300";

#[derive(Default)]
struct S {
    presses: u32,
    slow: bool,
    within: bool,
    inert: bool,
}

fn hex(s: &str) -> Color {
    rune::color::hex(s).unwrap()
}

fn view(s: &S, _: &mut Cx<S>) -> El {
    let mut card = el().key("card").size(120.0, 40.0).fill(Color::BLACK).border(1.0, hex(REST)).hover_border(hex(LIT))
        .on_press(|s: &mut S| s.presses += 1)
        .child(text(format!("pressed {}", s.presses)).key("label"));
    if s.slow {
        card = card.hover_motion(Curve::tween(1.0, Easing::LINEAR), Curve::tween(1.0, Easing::LINEAR));
    }
    if s.inert {
        card = card.inert();
    }
    let mut root = col().key("root").w_full().h_full().padding(20.0)
        .child(card)
        .child(el().key("away").size(120.0, 40.0).fill(Color::WHITE));
    if s.within {
        root = root.hover_motion_within(Curve::tween(1.0, Easing::LINEAR), Curve::tween(1.0, Easing::LINEAR));
    }
    root
}

fn runtime(s: S) -> Runtime<S> {
    let mut rt = Runtime::new(s, view, (300.0, 200.0), Env::default());
    rt.frame(0.0);
    rt
}

/// The border colour the card's own instance draws, premultiplied.
fn border(rt: &Runtime<S>) -> [f32; 4] {
    let r = find(rt, "card").rect;
    rt.instances.iter()
        .find(|i| (0..4).all(|k| (i.rect[k] - r[k]).abs() < 0.75) && i.border[3] > 0.0)
        .map(|i| i.border)
        .expect("the card draws a border")
}

fn is(c: [f32; 4], want: &str) -> bool {
    c.iter().zip(hex(want).premul(1.0)).all(|(a, b)| (a - b).abs() < 2e-3)
}

fn hover(rt: &mut Runtime<S>, key: &str) {
    let (x, y) = centre(&find(rt, key));
    rt.pointer_move(x, y);
    rt.frame(1.0 / 60.0);
}

fn run(rt: &mut Runtime<S>, seconds: f32) {
    for _ in 0..(seconds * 60.0).round() as usize {
        rt.frame(1.0 / 60.0);
    }
}

#[test]
fn the_border_takes_its_hover_colour_under_the_pointer_and_rests_again_after() {
    let mut rt = runtime(S::default());
    assert!(is(border(&rt), REST), "at rest: {:?}", border(&rt));
    hover(&mut rt, "card");
    assert!(is(border(&rt), LIT), "lands at once on the way in, as `hover` does: {:?}", border(&rt));
    hover(&mut rt, "away");
    assert!(!is(border(&rt), REST) && !is(border(&rt), LIT), "fades on the way out: {:?}", border(&rt));
    run(&mut rt, 0.3);
    assert!(is(border(&rt), REST), "back at rest after the 0.1 s fade: {:?}", border(&rt));
}

#[test]
fn it_follows_the_elements_own_hover_curves() {
    let mut rt = runtime(S { slow: true, ..S::default() });
    hover(&mut rt, "card");
    run(&mut rt, 0.5);
    let mid = border(&rt);
    let (rest, lit) = (hex(REST).premul(1.0), hex(LIT).premul(1.0));
    assert!(mid[0] > rest[0] + 0.05 && mid[0] < lit[0] - 0.05, "halfway through a 1 s curve: {mid:?}");
    run(&mut rt, 0.7);
    assert!(is(border(&rt), LIT));
}

#[test]
fn it_follows_hover_motion_within_from_an_ancestor() {
    let mut rt = runtime(S { within: true, ..S::default() });
    hover(&mut rt, "card");
    run(&mut rt, 0.5);
    assert!(!is(border(&rt), REST) && !is(border(&rt), LIT), "the root's 1 s curve: {:?}", border(&rt));
    run(&mut rt, 0.7);
    assert!(is(border(&rt), LIT));
}

#[test]
fn a_commit_while_hovered_keeps_the_hover_border() {
    let mut rt = runtime(S::default());
    hover(&mut rt, "card");
    let (x, y) = centre(&find(&rt, "card"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.presses, 1);
    assert_eq!(find(&rt, "label").text.as_deref(), Some("pressed 1"), "the view committed");
    assert!(is(border(&rt), LIT), "still lit after the commit: {:?}", border(&rt));
    run(&mut rt, 0.3);
    assert!(is(border(&rt), LIT), "and stays lit: {:?}", border(&rt));
    hover(&mut rt, "away");
    run(&mut rt, 0.3);
    assert!(is(border(&rt), REST), "then rests when the pointer leaves: {:?}", border(&rt));
}

#[test]
fn an_inert_element_keeps_its_rest_border() {
    let mut rt = runtime(S { inert: true, ..S::default() });
    hover(&mut rt, "card");
    run(&mut rt, 0.3);
    assert!(is(border(&rt), REST), "{:?}", border(&rt));
}
