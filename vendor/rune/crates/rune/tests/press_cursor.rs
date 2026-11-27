//! The hand on everything pressable: an element with `on_press` shows `Cursor::Pointer` while the
//! pointer is over it unless it sets its own `El::cursor`, and a disabled one shows the arrow.
//!
//! Electron: `.lc-button { cursor: pointer }` and `.lc-iconbutton { cursor: pointer }`
//! (apps/desktop/src/renderer/src/shell.css:686-697, 248-259); `.lc-button:disabled { cursor:
//! default }` and `.lc-iconbutton:disabled { cursor: default }` (shell.css:710-713, 271-274).
mod support;

use rune::{Color, Cursor, Cx, El, Env, Link, Runtime, Span, TextEdit, TextStyle, col, el, rich, row, text, text_input};
use support::{centre, find};

const MESSAGE: &str = "read the docs now";

#[derive(Default)]
struct S {
    pressed: Vec<&'static str>,
    field: TextEdit,
}

fn view(s: &S, _: &mut Cx<S>) -> El {
    let style = TextStyle::new(14.0, 400.0).lh(20.0);
    let docs = MESSAGE.find("docs").unwrap();
    col().w_full().h_full().padding(10.0).gap(10.0)
        .child(el().key("button").size(80.0, 24.0).fill(Color::BLACK).on_press(|s: &mut S| s.pressed.push("button")))
        .child(el().key("disabled").size(80.0, 24.0).fill(Color::BLACK).disabled(true).on_press(|s: &mut S| s.pressed.push("disabled")))
        .child(el().key("grab").size(80.0, 24.0).fill(Color::BLACK).cursor(Cursor::Grab).on_press(|s: &mut S| s.pressed.push("grab")))
        .child(el().key("plain").size(80.0, 24.0).fill(Color::BLACK).on_hover_change(|_: &mut S, _, _| {}))
        .child(row().key("row").size(360.0, 30.0).fill(Color::WHITE).on_press(|s: &mut S| s.pressed.push("row"))
            .child(text("label").key("label").font(style))
            // A disabled button left with its press handler, and one drawn as a button with none.
            .child(el().key("inner-disabled").size(40.0, 20.0).fill(Color::BLACK).disabled(true).on_press(|s: &mut S| s.pressed.push("inner")))
            .child(el().key("inner-dimmed").role("button").hit().size(40.0, 20.0).fill(Color::BLACK).disabled(true))
            .child(text_input(&s.field, style).key("field").w(100.0).on_edit(|s: &mut S| &mut s.field)))
        .child(rich(MESSAGE, vec![Span::new(docs, docs + 4)]).key("message").font(style).w(300.0)
            .link(Link::new(docs..docs + 4, "https://docs"))
            .on_link(|_: &mut S, _, _| {}))
}

fn runtime() -> Runtime<S> {
    let mut rt = Runtime::new(S::default(), view, (400.0, 400.0), Env::default());
    rt.frame(0.0);
    rt
}

fn cursor_over(rt: &mut Runtime<S>, key: &str) -> Cursor {
    let (x, y) = centre(&find(rt, key));
    rt.pointer_move(x, y);
    rt.frame(1.0 / 60.0);
    rt.cursor()
}

#[test]
fn a_pressable_element_shows_the_hand() {
    let mut rt = runtime();
    assert_eq!(cursor_over(&mut rt, "button"), Cursor::Pointer);
}

#[test]
fn a_disabled_pressable_element_shows_the_arrow() {
    let mut rt = runtime();
    assert_eq!(cursor_over(&mut rt, "disabled"), Cursor::Default);
}

#[test]
fn an_explicit_cursor_wins_over_the_hand() {
    let mut rt = runtime();
    assert_eq!(cursor_over(&mut rt, "grab"), Cursor::Grab);
}

#[test]
fn a_plain_element_shows_the_arrow() {
    let mut rt = runtime();
    assert_eq!(cursor_over(&mut rt, "plain"), Cursor::Default);
}

#[test]
fn a_link_span_still_shows_the_hand() {
    let mut rt = runtime();
    let message = find(&rt, "message");
    let docs = rt.elements().into_iter().find(|e| e.path.starts_with(&message.path) && e.role == "link").expect("the link span");
    let (x, y) = centre(&docs);
    rt.pointer_move(x, y);
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.cursor(), Cursor::Pointer, "over the link's glyphs");
    rt.pointer_move(message.rect[0] + 2.0, y);
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.cursor(), Cursor::Default, "plain text beside the link");
}

#[test]
fn what_sits_inside_a_pressable_row_shows_the_hand() {
    let mut rt = runtime();
    assert_eq!(cursor_over(&mut rt, "label"), Cursor::Pointer, "the row's label");
    let row = find(&rt, "row");
    rt.pointer_move(row.rect[0] + row.rect[2] - 4.0, row.rect[1] + row.rect[3] / 2.0);
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.cursor(), Cursor::Pointer, "the row's empty end");
}

#[test]
fn a_field_and_a_disabled_control_in_a_pressable_row_keep_their_own() {
    let mut rt = runtime();
    assert_eq!(cursor_over(&mut rt, "field"), Cursor::Text, "the field keeps its I-beam");
    assert_eq!(cursor_over(&mut rt, "inner-disabled"), Cursor::Default, "a disabled control shows the arrow, not the row's hand");
    assert_eq!(cursor_over(&mut rt, "inner-dimmed"), Cursor::Default, "a disabled button without a handler too");
}

#[test]
fn the_hand_stays_while_the_press_is_held() {
    let mut rt = runtime();
    let (x, y) = centre(&find(&rt, "button"));
    rt.pointer_move(x, y);
    rt.frame(1.0 / 60.0);
    rt.pointer_down(x, y);
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.cursor(), Cursor::Pointer);
    rt.pointer_up(x, y);
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.pressed, ["button"]);
}
