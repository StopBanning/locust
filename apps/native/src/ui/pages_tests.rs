//! Live hour 1, fault 40: with Routines open, pressing Board did nothing; with Settings open, pressing Team did
//! nothing; each worked only after that page's own Close. Every entry now opens from any page: for each pair of pages A
//! then B, the page the window shows is B. The window's own selector (`page_shown`) is what the view draws from.
use super::*;
use rune::{Env, Runtime};

/// A page the sidebar reaches: the key of its entry, and what the window then shows.
const ENTRIES: [(&str, Page); 6] = [
    ("sidebar-place-conversations", Page::Missions),
    ("sidebar-place-rooms", Page::Rooms),
    ("sidebar-place-routines", Page::Routines),
    ("sidebar-footer-settings", Page::Settings),
    ("sidebar-footer-board", Page::Board),
    ("sidebar-faces-team", Page::Team),
];

fn sidebar_window() -> Runtime<State> {
    let mut s = State::default();
    crate::team_scene(&mut s);
    let mut rt = Runtime::new(s, |s, _| crate::sidebar::sidebar(s, &Palette::new(true)), (crate::window_glass::SIDEBAR_WIDTH, 8000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn press(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no {key} in the sidebar"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.0);
}

#[test]
fn a_new_window_shows_the_conversation() {
    assert_eq!(crate::page_shown(&State::default()), Page::Conversation);
}

#[test]
fn every_sidebar_entry_opens_from_every_page() {
    for (from, _) in ENTRIES {
        for (to, shown) in ENTRIES {
            if from == to {
                continue;
            }
            let mut rt = sidebar_window();
            press(&mut rt, from);
            press(&mut rt, to);
            assert_eq!(crate::open_pages(&rt.state), [shown], "{from} then {to}: one page open, the one asked for");
        }
    }
}

#[test]
fn board_opens_over_routines_and_team_over_settings() {
    // The two found driving the live app.
    let mut rt = sidebar_window();
    press(&mut rt, "sidebar-place-routines");
    assert_eq!(crate::page_shown(&rt.state), Page::Routines);
    press(&mut rt, "sidebar-footer-board");
    assert_eq!(crate::page_shown(&rt.state), Page::Board);
    let mut rt = sidebar_window();
    press(&mut rt, "sidebar-footer-settings");
    press(&mut rt, "sidebar-faces-team");
    assert_eq!(crate::page_shown(&rt.state), Page::Team);
}

#[test]
fn rooms_opens_from_home() {
    let mut rt = sidebar_window();
    rt.update(|s, _| crate::home_panel::open_home(s));
    assert_eq!(crate::page_shown(&rt.state), Page::Home);
    press(&mut rt, "sidebar-place-rooms");
    assert_eq!(crate::page_shown(&rt.state), Page::Rooms);
}

#[test]
fn board_and_home_open_over_any_page_however_they_are_reached() {
    // The palette and Home's own links call these directly, not through a sidebar press.
    let openers: [fn(&mut State); 6] = [
        crate::missions_screen::open,
        crate::room_screen::open_list,
        crate::routines_panel::open,
        crate::settings_panel::open,
        crate::people_panel::open,
        crate::benchmarks_panel::open,
    ];
    for (n, open) in openers.into_iter().enumerate() {
        let mut s = State::default();
        crate::team_scene(&mut s);
        open(&mut s);
        crate::home_panel::open_board(&mut s);
        assert_eq!(crate::open_pages(&s), [Page::Board], "page {n} then Board");
        open(&mut s);
        crate::home_panel::open_home(&mut s);
        assert_eq!(crate::open_pages(&s), [Page::Home], "page {n} then Home");
        open(&mut s);
        crate::home_panel::open_boot(&mut s);
        assert_eq!(crate::open_pages(&s), [Page::Boot], "page {n} then the install check");
    }
}

#[test]
fn closing_every_page_leaves_the_conversation() {
    for open in [crate::benchmarks_panel::open, crate::routines_panel::open, crate::settings_panel::open, crate::people_panel::open, crate::room_screen::open_list, crate::missions_screen::open] {
        let mut s = State::default();
        crate::team_scene(&mut s);
        open(&mut s);
        assert_ne!(crate::page_shown(&s), Page::Conversation);
        close_all(&mut s);
        assert_eq!(crate::page_shown(&s), Page::Conversation);
    }
}

#[test]
fn a_changes_page_and_a_comparison_close_too() {
    let mut s = State::default();
    s.changes.open = true;
    s.compare.active_id = Some("compare_1".into());
    assert_eq!(crate::open_pages(&s), [Page::Changes, Page::Compare]);
    crate::home_panel::open_board(&mut s);
    assert_eq!(crate::open_pages(&s), [Page::Board]);
}

#[test]
fn the_compact_rail_opens_every_entry_from_every_page_too() {
    // The rail's places: Teammates & Memory over Settings, and Settings over Routines.
    let rail_window = || {
        let mut s = State::default();
        crate::team_scene(&mut s);
        let mut rt = Runtime::new(s, |s, _| crate::rail::rail(s, &Palette::new(true)), (crate::window_glass::SIDEBAR_WIDTH, 8000.0), Env::new(true, 1.0));
        rt.frame(0.0);
        rt
    };
    let mut rt = rail_window();
    rt.update(|s, _| crate::settings_panel::open(s));
    press(&mut rt, "rail-place-teammates-memory");
    assert_eq!(crate::open_pages(&rt.state), [Page::Team]);
    let mut rt = rail_window();
    rt.update(|s, _| crate::routines_panel::open(s));
    press(&mut rt, "rail-place-settings");
    assert_eq!(crate::open_pages(&rt.state), [Page::Settings]);
    press(&mut rt, "rail-place-board");
    assert_eq!(crate::open_pages(&rt.state), [Page::Board]);
}

#[test]
fn benchmarks_and_conversations_open_over_any_page_too() {
    use locust_core::compare::Compare;
    let with_a_rerun = Compare {
        compare_id: "cmp_1".into(), teammate_id: None, prompt: "p".into(), created_at: "2026-11-26T09:00:00.000Z".into(), slots: vec![],
        changes: false, changes_in: None, blind: false, kept: None, judge: None, base: None, rerun_of: None,
        reruns: vec!["cmp_2".into()], settled: false, point_drafts: vec![],
    };
    let window = || {
        let mut s = State::default();
        crate::team_scene(&mut s);
        s.compare.compares = vec![with_a_rerun.clone()];
        let mut rt = Runtime::new(s, |s, _| crate::sidebar::sidebar(s, &Palette::new(true)), (crate::window_glass::SIDEBAR_WIDTH, 8000.0), Env::new(true, 1.0));
        rt.frame(0.0);
        rt
    };
    let mut rt = window();
    press(&mut rt, "sidebar-place-conversations");
    press(&mut rt, "sidebar-place-benchmarks");
    assert_eq!(crate::open_pages(&rt.state), [Page::Benchmarks]);
    let mut rt = window();
    rt.update(|s, _| s.compare.active_id = Some("cmp_1".into()));
    press(&mut rt, "sidebar-place-conversations");
    assert_eq!(crate::open_pages(&rt.state), [Page::Missions]);
}
