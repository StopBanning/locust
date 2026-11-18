//! Window size and bounds parity tests (Electron `main/window-size.test.ts`, `main/window-bounds.test.ts`).

use crate::window_bounds::{
    OpeningPlacement, Rect, SavedWindow, Size, minimum_size, opening_placement, opening_size,
    read_saved_window,
};
use serde_json::json;

const LAPTOP: Rect = Rect { x: 0, y: 0, width: 1920, height: 1040 };
const WORK: Size = Size { width: 1920, height: 1040 };

#[test]
fn takes_a_fraction_of_the_display_rather_than_most_of_it() {
    let on1920 = opening_size(Size { width: 1920, height: 1080 });
    assert!(on1920.width <= 1215);
    assert!(on1920.height <= 800);
}

#[test]
fn never_opens_below_what_the_layout_needs_on_a_display_that_has_it() {
    let small = opening_size(Size { width: 1180, height: 760 });
    assert_eq!(small.width, 1120);
    assert_eq!(small.height, 720);
}

#[test]
fn never_opens_taller_or_wider_than_a_work_area_smaller_than_the_minimum() {
    assert_eq!(opening_size(Size { width: 1280, height: 672 }), Size { width: 1120, height: 672 });
    assert_eq!(opening_size(Size { width: 1024, height: 640 }), Size { width: 1024, height: 640 });
    assert_eq!(minimum_size(Size { width: 1280, height: 672 }), Size { width: 1120, height: 672 });
    assert_eq!(minimum_size(Size { width: 2560, height: 1392 }), Size { width: 1120, height: 720 });
}

#[test]
fn scales_down_on_a_small_display_instead_of_using_the_cap() {
    let modest = opening_size(Size { width: 1440, height: 900 });
    assert_eq!(modest.width, 1181);
    assert_eq!(modest.height, 774);
}

#[test]
fn uses_the_opening_size_when_nothing_was_saved() {
    let placement = opening_placement(None, &[LAPTOP], WORK);
    assert_eq!(placement.width, 1215);
    assert_eq!(placement.x, None);
    assert!(!placement.maximized);
}

#[test]
fn reopens_where_and_how_the_person_left_it() {
    let saved = SavedWindow { x: 220, y: 90, width: 1500, height: 900, maximized: false };
    let placement = opening_placement(Some(&saved), &[LAPTOP], WORK);
    assert_eq!(
        placement,
        OpeningPlacement {
            x: Some(220),
            y: Some(90),
            width: 1500,
            height: 900,
            maximized: false,
            min_width: 1120,
            min_height: 720,
        }
    );
}

#[test]
fn keeps_a_saved_size_larger_than_the_default_opening_size() {
    let saved = SavedWindow { x: 0, y: 0, width: 1800, height: 1000, maximized: false };
    assert_eq!(opening_placement(Some(&saved), &[LAPTOP], WORK).width, 1800);
}

#[test]
fn reopens_maximized_when_it_was_closed_maximized() {
    let saved = SavedWindow { x: 10, y: 10, width: 1400, height: 880, maximized: true };
    let placement = opening_placement(Some(&saved), &[LAPTOP], WORK);
    assert!(placement.maximized);
    assert_eq!(placement.width, 1400);
}

#[test]
fn drops_a_position_on_a_monitor_that_is_no_longer_plugged_in() {
    let saved = SavedWindow { x: 2400, y: 200, width: 1400, height: 880, maximized: false };
    let placement = opening_placement(Some(&saved), &[LAPTOP], WORK);
    assert_eq!(placement.x, None);
    assert_eq!(placement.y, None);
    assert_eq!(placement.width, 1400);
}

#[test]
fn keeps_a_position_on_a_second_display_that_is_still_attached() {
    let second = Rect { x: 1920, y: 0, width: 1920, height: 1040 };
    let saved = SavedWindow { x: 2400, y: 200, width: 1400, height: 880, maximized: false };
    assert_eq!(opening_placement(Some(&saved), &[LAPTOP, second], WORK).x, Some(2400));
}

#[test]
fn rejects_a_window_hanging_off_the_edge_with_only_a_sliver_showing() {
    let saved = SavedWindow { x: 1860, y: 400, width: 1400, height: 880, maximized: false };
    assert_eq!(opening_placement(Some(&saved), &[LAPTOP], WORK).x, None);
}

#[test]
fn never_restores_below_what_the_layout_needs() {
    let saved = SavedWindow { x: 40, y: 40, width: 300, height: 200, maximized: false };
    let placement = opening_placement(Some(&saved), &[LAPTOP], WORK);
    assert_eq!(placement.width, 1120);
    assert_eq!(placement.height, 720);
}

#[test]
fn never_restores_taller_than_a_work_area_smaller_than_the_minimum() {
    let small = Rect { x: 0, y: 0, width: 1280, height: 672 };
    let saved = SavedWindow { x: 0, y: 0, width: 1120, height: 650, maximized: false };
    let placement = opening_placement(Some(&saved), &[small], Size { width: 1280, height: 672 });
    assert_eq!(placement.height, 672);
    assert_eq!(placement.min_height, 672);
}

#[test]
fn never_restores_taller_than_the_display_it_lands_on() {
    let saved = SavedWindow { x: 0, y: 0, width: 4000, height: 3000, maximized: false };
    let placement = opening_placement(Some(&saved), &[LAPTOP], WORK);
    assert_eq!(placement.width, 1920);
    assert_eq!(placement.height, 1040);
}

#[test]
fn accepts_a_whole_rectangle() {
    assert_eq!(
        read_saved_window(&json!({ "x": 1, "y": 2, "width": 1200, "height": 800, "maximized": true })),
        Some(SavedWindow { x: 1, y: 2, width: 1200, height: 800, maximized: true })
    );
}

#[test]
fn refuses_invalid_saved_window_records() {
    for value in [
        json!("nonsense"),
        json!(null),
        json!({ "width": 1200, "height": 800 }),
        json!({ "x": 1, "y": 2 }),
        json!({ "x": 1, "y": 2, "width": 0, "height": 800 }),
    ] {
        assert_eq!(read_saved_window(&value), None);
    }
}

#[test]
fn treats_a_missing_maximized_flag_as_not_maximized() {
    assert_eq!(
        read_saved_window(&json!({ "x": 1, "y": 2, "width": 1200, "height": 800 }))
            .map(|w| w.maximized),
        Some(false)
    );
}
