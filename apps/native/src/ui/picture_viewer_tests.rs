//! ThreadImage's press: a picture in the thread opens larger beside the conversation.

use super::*;
use super::tests::{state_in_folder, tool};

#[test]
fn pressing_a_steps_picture_opens_it_beside_the_conversation() {
    let (s, _dir) = state_in_folder();
    thread_local! {
        static ITEMS: Vec<ledger::Item> = vec![tool("t1", "Read", "chart.png")];
    }
    let mut rt = rune::Runtime::new(
        s,
        |s, _| ITEMS.with(|items| activity_group(items, &Palette::new(true), s, "conv")),
        (700.0, 700.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    let (x, y) = rt.find("picture:chart.png").expect("the picture card");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.1);
    let (target, body) = rt.state.file_view.clone().expect("the viewer opened");
    assert_eq!(target.path, "chart.png");
    assert_eq!(body, Ok(String::new()), "a picture is never read as text");
}

#[test]
fn the_viewer_draws_the_picture_and_says_so_when_it_cannot() {
    let (mut s, _dir) = state_in_folder();
    s.file_view = Some((crate::reply_actions::FileTarget { path: "chart.png".into(), start: 0, end: 0 }, Ok(String::new())));
    let mut rt = rune::Runtime::new(s, |s, _| crate::reply_actions::viewer(s, &Palette::new(true)).unwrap_or_else(el), (900.0, 700.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let elements = rt.elements();
    assert!(elements.iter().any(|e| e.key.as_deref() == Some("viewer-picture:chart.png")));
    assert!(elements.iter().any(|e| e.text.as_deref() == Some("chart.png")), "the title is the path, without a line range");

    let (mut s, _dir) = state_in_folder();
    s.file_view = Some((crate::reply_actions::FileTarget { path: "../outside.png".into(), start: 0, end: 0 }, Ok(String::new())));
    let mut rt = rune::Runtime::new(s, |s, _| crate::reply_actions::viewer(s, &Palette::new(true)).unwrap_or_else(el), (900.0, 700.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some("This picture cannot be shown here.")));
}
