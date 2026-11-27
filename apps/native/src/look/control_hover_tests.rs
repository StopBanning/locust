//! Hover on the switch and the button, drawn in a window and read from what it paints.
//!
//! Electron: `button.lc-switch:hover:not(:disabled):not(.is-on) { background: var(--lc-switch-off-hover) }`
//! (apps/desktop/src/renderer/src/shell.css:18926-18928) on `transition: background 160ms var(--lc-ease-out)`
//! (shell.css:18923); `.lc-button:hover:not(:disabled) { background: var(--lc-hover-surface); border-color:
//! var(--lc-hover-border) }` (shell.css:701-704) on a 140ms `ease` (shell.css:698); both show the hand
//! (`cursor: pointer`, shell.css:18922, 697) and a disabled one the arrow (shell.css:3782-3785, 710-713).
use super::*;
use rune::{Cursor, ElementInfo, Env, Runtime};

#[derive(Clone, Copy)]
struct S {
    dark: bool,
    on: bool,
    disabled: bool,
}

fn view(s: &S, _: &mut rune::Cx<S>) -> El {
    let l = Look::new(s.dark);
    col()
        .padding(20.0)
        .gap(20.0)
        .child(switch(&l, s.on, "Scout", s.disabled).key("switch").on_press(|_: &mut S| {}))
        .child(button(&l, "Save", Tone::Neutral, s.disabled).key("button").on_press(|_: &mut S| {}))
        .child(primary_button(&l, "Send", s.disabled).key("primary").on_press(|_: &mut S| {}))
        // What Electron paints under the pointer, drawn plainly to read back in the same blend.
        .child(row().key("switch-lit").w(32.0).h(18.0).radius(999.0).fill(l.c("switch-off-hover")))
        .child(row().key("button-edge").w(60.0).h(30.0).radius(l.t.px("radius-control")).fill(l.c("bg-control")).border(1.0, l.c("hover-border")))
        .child(row().key("button-rest-edge").w(60.0).h(30.0).radius(l.t.px("radius-control")).fill(l.c("bg-control")).border(1.0, l.c("border-control")))
        .child(row().key("away").w(60.0).h(30.0))
}

fn window(s: S) -> Runtime<S> {
    let mut rt = Runtime::new(s, view, (300.0, 400.0), Env::new(s.dark, 1.0));
    rt.frame(0.0);
    rt
}

fn info(rt: &Runtime<S>, key: &str) -> ElementInfo {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no {key}"))
}

/// The instance the element's own fill and border draw in: (fill, border), premultiplied.
fn paint(rt: &Runtime<S>, key: &str) -> ([f32; 4], [f32; 4]) {
    let r = info(rt, key).rect;
    rt.instances
        .iter()
        .find(|i| (0..4).all(|k| (i.rect[k] - r[k]).abs() < 0.75) && i.fill[3] > 0.0)
        .map(|i| (i.fill, i.border))
        .unwrap_or_else(|| panic!("{key} draws nothing"))
}

fn near(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 2e-3)
}

fn point_at(rt: &mut Runtime<S>, key: &str) {
    let e = info(rt, key);
    rt.pointer_move(e.rect[0] + e.rect[2] / 2.0, e.rect[1] + e.rect[3] / 2.0);
    rt.frame(1.0 / 60.0);
}

fn run(rt: &mut Runtime<S>, seconds: f32) {
    for _ in 0..(seconds * 60.0).round() as usize {
        rt.frame(1.0 / 60.0);
    }
}

#[test]
fn an_off_switch_lightens_to_switch_off_hover_under_the_pointer_in_both_themes() {
    for dark in [true, false] {
        let mut rt = window(S { dark, on: false, disabled: false });
        let rest = paint(&rt, "switch").0;
        point_at(&mut rt, "switch");
        run(&mut rt, 0.3);
        let (lit, want) = (paint(&rt, "switch").0, paint(&rt, "switch-lit").0);
        assert!(near(lit, want), "dark {dark}: {lit:?}, Electron's {want:?}");
        assert!(!near(lit, rest), "dark {dark}: it changed");
        assert_eq!(rt.cursor(), Cursor::Pointer, "dark {dark}: the hand");
        point_at(&mut rt, "away");
        run(&mut rt, 0.3);
        assert!(near(paint(&rt, "switch").0, rest), "dark {dark}: back to switch-off");
    }
}

#[test]
fn the_switch_eases_over_160ms() {
    let mut rt = window(S { dark: true, on: false, disabled: false });
    let rest = paint(&rt, "switch").0;
    let lit = paint(&rt, "switch-lit").0;
    point_at(&mut rt, "switch");
    let early = paint(&rt, "switch").0;
    assert!(early[3] < lit[3] - 1e-3 && !near(early, lit), "not there at once: {early:?}");
    run(&mut rt, 0.05);
    let mid = paint(&rt, "switch").0;
    assert!(mid[3] > rest[3] + 1e-3 && mid[3] < lit[3] - 1e-3, "on its way: {mid:?}");
    run(&mut rt, 0.15);
    assert!(near(paint(&rt, "switch").0, lit), "there by 160ms");
}

#[test]
fn an_on_switch_and_a_disabled_one_keep_their_fill() {
    for (on, disabled) in [(true, false), (false, true)] {
        let mut rt = window(S { dark: true, on, disabled });
        let rest = paint(&rt, "switch").0;
        point_at(&mut rt, "switch");
        run(&mut rt, 0.3);
        assert!(near(paint(&rt, "switch").0, rest), "on {on}, disabled {disabled}");
        let want = if disabled { Cursor::Default } else { Cursor::Pointer };
        assert_eq!(rt.cursor(), want, "on {on}, disabled {disabled}");
    }
}

#[test]
fn a_button_lifts_its_border_to_hover_border_under_the_pointer() {
    for dark in [true, false] {
        let mut rt = window(S { dark, on: false, disabled: false });
        let rest = paint(&rt, "button").1;
        assert!(near(rest, paint(&rt, "button-rest-edge").1), "dark {dark}: border-control at rest");
        point_at(&mut rt, "button");
        run(&mut rt, 0.3);
        let (lit, want) = (paint(&rt, "button").1, paint(&rt, "button-edge").1);
        assert!(near(lit, want), "dark {dark}: {lit:?}, Electron's hover-border {want:?}");
        assert!(info(&rt, "button").hover > 0.99, "dark {dark}: the hover surface too");
        assert_eq!(rt.cursor(), Cursor::Pointer, "dark {dark}: the hand");
        point_at(&mut rt, "away");
        run(&mut rt, 0.3);
        assert!(near(paint(&rt, "button").1, rest), "dark {dark}: back to border-control");
    }
}

#[test]
fn the_button_border_eases_over_140ms() {
    let mut rt = window(S { dark: true, on: false, disabled: false });
    let (rest, lit) = (paint(&rt, "button").1, paint(&rt, "button-edge").1);
    point_at(&mut rt, "button");
    run(&mut rt, 0.05);
    let mid = paint(&rt, "button").1;
    assert!(mid[3] > rest[3] + 1e-3 && mid[3] < lit[3] - 1e-3, "on its way: {mid:?}");
    run(&mut rt, 0.15);
    assert!(near(paint(&rt, "button").1, lit));
}

#[test]
fn a_disabled_button_keeps_its_border_and_shows_the_arrow() {
    let mut rt = window(S { dark: true, on: false, disabled: true });
    let rest = paint(&rt, "button").1;
    point_at(&mut rt, "button");
    run(&mut rt, 0.3);
    assert!(near(paint(&rt, "button").1, rest));
    assert_eq!(rt.cursor(), Cursor::Default);
    point_at(&mut rt, "primary");
    assert_eq!(rt.cursor(), Cursor::Default, "a disabled primary button too");
}
