//! the-title-bar-is-a-macs-on-a-mac.test.tsx (2 cases) and a-mac-window-has-its-own-traffic-lights.test.ts (3 cases).
//! On a Mac the window keeps its frame with the title bar hidden and inset, the lights in the bar's left column, and the
//! bar draws no controls of its own. Windows and Linux are Electron's control: there it is frameless and draws its own
//! controls; rune keeps those platforms' native frame with the platform's own controls, so those two control cases
//! do not apply to this app and have no test here.
use super::*;
use rune::{Env, Runtime};

fn bar() -> Vec<rune::ElementInfo> {
    let mut s = State::default();
    s.home.folder_path = Some("/work/tides".into());
    let mut rt = Runtime::new(s, |s, _| col().w_full().child(crate::rail::title_bar(s, &Palette::new(true), false, 0)), (900.0, 200.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements()
}

// case: draws no window controls of its own, and nothing in its left column
#[test]
fn the_mac_title_bar_draws_no_controls_and_nothing_on_its_left() {
    let all = bar();
    for name in ["Close", "Minimize", "Maximize", "Restore"] {
        assert!(!all.iter().any(|e| e.label.as_deref() == Some(name)), "no {name} of its own");
    }
    let bar_at = all.iter().position(|e| e.key.as_deref() == Some("titlebar")).expect("the title bar");
    let kids: Vec<&rune::ElementInfo> = all.iter().filter(|e| e.parent == Some(bar_at)).collect();
    assert!(kids[0].text.is_none() && !all.iter().any(|e| e.parent == Some(all.iter().position(|x| std::ptr::eq(x, kids[0])).unwrap())), "the left column is empty");
    assert_eq!(kids[1].text.as_deref(), Some("tides"), "then the title");
    // The lights sit inside that empty left column.
    let lights = WINDOW_CHROME.traffic_light_rects().expect("lights are placed");
    let last = lights[2];
    assert!(last[0] + last[2] <= kids[0].rect[0] + kids[0].rect[2], "the lights end inside the left column: {lights:?} vs {:?}", kids[0].rect);
}

// case: on a Mac keeps its frame, with the title bar hidden and inset and the lights in its left column
#[test]
fn a_mac_window_keeps_its_frame_with_the_lights_at_14_13() {
    assert!(WINDOW_CHROME.full_size_content, "the title bar is hidden and the content runs under it");
    assert_eq!(WINDOW_CHROME.traffic_lights, Some((14.0, 13.0)));
    assert!(WINDOW_CHROME.drag_regions, "only the drag regions move the window");
}

// case: is what the main window is made with
#[test]
fn the_main_window_is_made_with_it() {
    let main = include_str!("../main.rs");
    assert_eq!(main.matches(".chrome(WINDOW_CHROME)").count(), 2, "every window Locust opens is made with it");
    assert!(!main.contains("traffic_lights: Some((14.0, 13.0)),\n        drag_regions"), "no second copy of it");
}
