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

// ---- an-image-can-be-zoomed-and-panned.test.ts ------------------------------------------------------------------

#[test]
fn opens_at_the_pane_width_as_it_always_did_and_never_larger_than_its_own_pixels() {
    // case: opens at the pane width, as it always did, and never larger than its own pixels
    assert_eq!(fit_scale((2000.0, 1000.0), (500.0, 800.0)), 0.25);
    assert_eq!(fit_scale((1000.0, 4000.0), (500.0, 800.0)), 0.5);
    assert_eq!(fit_scale((100.0, 50.0), (500.0, 800.0)), 1.0);
}

#[test]
fn zooms_out_as_far_as_the_whole_picture_in_the_pane() {
    // case: zooms out as far as the whole picture in the pane
    assert_eq!(whole_scale((1000.0, 4000.0), (500.0, 800.0)), 0.2);
    assert_eq!(whole_scale((100.0, 50.0), (500.0, 800.0)), 1.0);
}

#[test]
fn opens_a_tall_picture_at_its_top_and_a_small_one_in_the_middle() {
    // case: opens a tall picture at its top, and a small one in the middle
    let pane = (500.0, 800.0);
    assert_eq!(
        keep_in_pane(ImageView { scale: fit_scale((1000.0, 4000.0), pane), x: 0.0, y: 0.0 }, (1000.0, 4000.0), pane),
        ImageView { scale: 0.5, x: 0.0, y: 0.0 }
    );
    assert_eq!(
        keep_in_pane(ImageView { scale: 1.0, x: 0.0, y: 0.0 }, (100.0, 50.0), pane),
        ImageView { scale: 1.0, x: 200.0, y: 375.0 }
    );
}

#[test]
fn zooms_around_the_point_under_the_pointer_which_stays_where_it_was() {
    // case: zooms around the point under the pointer, which stays where it was
    let view = ImageView { scale: 0.5, x: 10.0, y: 20.0 };
    let (px, py) = (210.0, 120.0);
    let (ix, iy) = ((px - view.x) / view.scale, (py - view.y) / view.scale);
    let next = zoom_around(view, 2.0, px, py, 0.25);
    assert_eq!(next.scale, 1.0);
    assert!((next.x + ix * next.scale - px).abs() < 1e-6);
    assert!((next.y + iy * next.scale - py).abs() < 1e-6);
}

#[test]
fn stops_at_the_fitted_size_going_down_and_at_eight_times_its_pixels_going_up() {
    // case: stops at the fitted size going down and at eight times its pixels going up
    assert_eq!(zoom_around(ImageView { scale: 0.3, x: 0.0, y: 0.0 }, 0.1, 0.0, 0.0, 0.25).scale, 0.25);
    assert_eq!(zoom_around(ImageView { scale: 6.0, x: 0.0, y: 0.0 }, 10.0, 0.0, 0.0, 0.25).scale, 8.0);
}

#[test]
fn stays_centred_where_it_is_smaller_than_the_pane_and_keeps_its_edges_at_the_pane_where_it_is_larger() {
    // case: stays centred where it is smaller than the pane, and keeps its edges at the pane where it is larger
    let pane = (500.0, 400.0);
    let small = keep_in_pane(ImageView { scale: 1.0, x: -300.0, y: 900.0 }, (100.0, 100.0), pane);
    assert_eq!(small, ImageView { scale: 1.0, x: 200.0, y: 150.0 });
    let large = keep_in_pane(ImageView { scale: 2.0, x: 50.0, y: -5000.0 }, (1000.0, 1000.0), pane);
    assert_eq!(large.x, 0.0);
    assert_eq!(large.y, 400.0 - 2000.0);
}

#[test]
fn is_what_the_file_viewer_draws_for_a_picture_with_the_zoom_controls_beside_it() {
    // case: is what the file viewer draws for a picture, with the zoom controls beside it
    let (mut s, _dir) = state_in_folder();
    s.file_view = Some((crate::reply_actions::FileTarget { path: "chart.png".into(), start: 0, end: 0 }, Ok(String::new())));
    let mut rt = rune::Runtime::new(s, |s, _| crate::reply_actions::viewer(s, &Palette::new(true)).unwrap_or_else(el), (900.0, 800.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let elements = rt.elements();
    assert!(elements.iter().any(|e| e.key.as_deref() == Some("viewer-picture:chart.png")));
    for label in ["Zoom out", "Zoom in", "Fit to the pane", "Actual size"] {
        assert!(elements.iter().any(|e| e.label.as_deref() == Some(label)), "missing zoom control {label:?}");
    }
    // Pressing Zoom in scales the picture up and shows its percentage on the Fit button.
    let (x, y) = rt.find("viewer-zoom-in").expect("Zoom in button");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
    let zoomed = rt.state.viewer_image_view.expect("Zoom in sets viewer_image_view");
    assert!((zoomed.scale - 1.25).abs() < 1e-6, "expected 1.25x, got {}", zoomed.scale);
    assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some("125%")));
    // Pressing Fit resets back to fitted.
    let (fx, fy) = rt.find("viewer-zoom-fit").expect("Fit to the pane button");
    rt.pointer_down(fx, fy);
    rt.pointer_up(fx, fy);
    rt.frame(0.05);
    assert!(rt.state.viewer_image_view.is_none());
}
