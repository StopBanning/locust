//! the-rail-can-be-used.test.ts: the 64px rail's `+` add menu (drawn as the window's right-click menu) and Home mark.
use super::*;
use rune::{Env, Key, Runtime};

fn window(size: (f32, f32), layout_pref: &str) -> Runtime<State> {
    let mut s = State::default();
    s.demo = true;
    s.layout_pref = layout_pref.into();
    s.teammates.push(Teammate {
        id: "tm_wren".into(),
        name: "Wren".into(),
        role: "Custom".into(),
        runtime: None,
        model: None,
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: None,
    });
    let mut rt = Runtime::new(s, crate::view, size, Env::new(true, 1.0));
    for _ in 0..6 {
        rt.frame(0.05);
    }
    rt
}

fn element(rt: &Runtime<State>, key: &str) -> rune::ElementInfo {
    rt.elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key) && !e.exiting)
        .unwrap_or_else(|| panic!("no element {key}"))
}

fn click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..4 {
        rt.frame(0.05);
    }
}

#[test]
// case: is the right-click menu, hung from the +
fn add_menu_is_the_right_click_menu_hung_from_the_plus() {
    let mut rt = window((900.0, 720.0), "compact");
    let plus = element(&rt, "rail-add-button");
    click(&mut rt, "rail-add-button");
    let menu = element(&rt, "row-menu");
    assert_eq!(menu.role, "menu");
    assert_eq!(menu.label.as_deref(), Some("Add"));
    assert!(rt.find("sidebar-add-menu").is_none(), "no bespoke sidebar-add-menu");
    assert!((menu.rect[0] - plus.rect[0]).abs() < 2.0, "hung from the + left edge");
    assert!((menu.rect[1] - (plus.rect[1] + plus.rect[3] + 4.0)).abs() < 2.0, "hung below the +");
}

#[test]
// case: offers what the + adds, each with its key -- and no New group since folders sort (0.460)
fn add_menu_offers_what_the_plus_adds_with_keys_and_no_new_group() {
    let mut rt = window((900.0, 720.0), "compact");
    click(&mut rt, "rail-add-button");
    let menu = rt.state.row_menu.as_ref().expect("add menu open");
    let labels: Vec<&str> = menu.items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"New teammate"), "{labels:?}");
    assert!(labels.contains(&"New room"), "{labels:?}");
    assert!(!labels.contains(&"New group"), "no New group: {labels:?}");
    assert_eq!(element(&rt, "row-menu-item-new-teammate-key").text.as_deref(), Some("T"));
    assert_eq!(element(&rt, "row-menu-item-new-room-key").text.as_deref(), Some("R"));

    // Pressing `t` opens the New teammate form; pressing `r` opens New room.
    rt.key(Key::Text("t".into()).into());
    rt.frame(0.016);
    assert!(rt.state.people_panel.open && rt.state.people_panel.form_open, "t opens New teammate");
    assert!(rt.state.row_menu.is_none());

    click(&mut rt, "rail-add-button");
    rt.key(Key::Text("r".into()).into());
    rt.frame(0.016);
    assert!(matches!(rt.state.rooms.active, crate::room_screen::RoomView::New), "r opens New room");
}

#[test]
// case: leaves its own button alone, so a second press closes it instead of reopening it
fn add_menu_second_press_closes_instead_of_reopening() {
    let mut rt = window((900.0, 720.0), "compact");
    let (x, y) = rt.find("rail-add-button").expect("rail + button");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
    assert!(rt.state.row_menu.is_some(), "first press opens");

    // Second press on the same button closes it rather than reopening it.
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
    assert!(rt.state.row_menu.is_none(), "second press closes");

    // And the anchor toggle itself closes an already-anchored menu rather than reopening it.
    let at = rt.state.add_menu_at;
    crate::context_menu::toggle_add_menu(&mut rt.state, "rail-add-button", at);
    assert!(rt.state.row_menu.is_some());
    crate::context_menu::toggle_add_menu(&mut rt.state, "rail-add-button", at);
    assert!(rt.state.row_menu.is_none(), "toggle on same anchor closes instead of reopening");
}

#[test]
// case: may still hang out of the rail, as the rail’s flyouts do
fn add_menu_hangs_out_of_the_64px_rail_unclipped() {
    let mut rt = window((900.0, 720.0), "compact");
    click(&mut rt, "rail-add-button");
    let ([left, _top, width, height], _) = rt.presented("row-menu").expect("add menu drawn");
    assert!(left + width > RAIL_WIDTH + 60.0, "hangs past the 64px rail: {left} + {width}");
    assert!(height >= 56.0, "tall enough to hold its items: {height}");
    // An item past x=64 is clickable and opens its screen.
    let item = element(&rt, "row-menu-item-new-room");
    let click_x = RAIL_WIDTH + 40.0;
    assert!(click_x < item.rect[0] + item.rect[2], "item reaches past x=64");
    rt.pointer_down(click_x, item.rect[1] + item.rect[3] * 0.5);
    rt.pointer_up(click_x, item.rect[1] + item.rect[3] * 0.5);
    rt.frame(0.016);
    assert!(matches!(rt.state.rooms.active, crate::room_screen::RoomView::New));
}

#[test]
// case: is the mark, and it is a button
fn way_home_is_the_mark_and_is_a_button() {
    let mut rt = window((1400.0, 720.0), "wide");
    let home = element(&rt, "sidebar-home-button");
    assert_eq!(home.role, "button");
    assert_eq!(home.label.as_deref(), Some("Home"));
    rt.state.home.home_open = false;
    click(&mut rt, "sidebar-home-button");
    assert!(rt.state.home.home_open, "pressing the brand mark goes Home");
}

#[test]
// case: keeps a real size in the rail, where the wordmark collapses to nothing
fn way_home_keeps_a_real_size_in_the_rail() {
    let mut rt = window((900.0, 720.0), "compact");
    let home = element(&rt, "rail-home-button");
    assert_eq!(home.role, "button");
    assert_eq!(home.label.as_deref(), Some("Home"));
    assert!(home.rect[2] >= 24.0 && home.rect[3] >= 15.0, "real clickable area in the rail: {:?}", home.rect);
    rt.state.home.home_open = false;
    click(&mut rt, "rail-home-button");
    assert!(rt.state.home.home_open, "clickable in the 64px rail");
}
