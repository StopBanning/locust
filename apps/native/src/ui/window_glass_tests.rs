//! a-macs-sidebar-is-glass.test.ts, drawn: the whole window drawn headless with the Mac's glass on and off, read
//! through the element info (Rune's `ElementInfo::material`, the local patch in RUNE.md).
use super::*;
use rune::{Env, Runtime, SystemMaterial};

fn window(glass: bool, dark: bool, layout: &str) -> Vec<rune::ElementInfo> {
    let mut s = State::default();
    s.mac_glass = glass;
    s.appearance = Some(dark);
    s.layout_pref = layout.to_owned();
    let mut rt = Runtime::new(s, crate::view, (1400.0, 900.0), Env::new(dark, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().filter(|e| !e.exiting).collect()
}

fn find<'a>(all: &'a [rune::ElementInfo], key: &str) -> &'a rune::ElementInfo {
    all.iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no {key}"))
}

// case: a Mac window is made with the sidebar material, following the window; Windows and Linux get none
#[test]
fn a_mac_window_is_made_with_the_sidebar_material_following_the_window_and_windows_and_linux_get_none() {
    assert!(window_options("macos").transparent);
    for os in ["windows", "linux"] {
        assert!(!window_options(os).transparent, "{os} gets no material");
    }
    for dark in [true, false] {
        let all = window(true, dark, "wide");
        assert_eq!(find(&all, "app-window").material, Some(SystemMaterial::Sidebar), "the window's ground is the sidebar material");
        // One material only: the page lets it through, it does not put more glass on top.
        assert_eq!(all.iter().filter(|e| e.material.is_some()).count(), 1);
    }
    // Off glass, the window is made as before.
    let all = window(false, true, "wide");
    assert!(all.iter().all(|e| e.material.is_none()));
}

// case: its ground is clear, set after the theme's, and a theme change does not paint over it
#[test]
fn its_ground_is_clear_set_after_the_themes_and_a_theme_change_does_not_paint_over_it() {
    let mut s = State::default();
    s.mac_glass = true;
    s.appearance = Some(true);
    let mut rt = Runtime::new(s, crate::view, (1400.0, 900.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let ground = |rt: &Runtime<State>| rt.elements().into_iter().find(|e| e.key.as_deref() == Some("app-window")).map(|e| e.material);
    assert_eq!(ground(&rt), Some(Some(SystemMaterial::Sidebar)));
    // The theme's ground is only the stand-in under the material: the light theme keeps the glass.
    rt.state.appearance = Some(false);
    rt.frame(16.0);
    assert_eq!(ground(&rt), Some(Some(SystemMaterial::Sidebar)), "a theme change keeps the glass");
    assert_eq!(rt.elements().into_iter().find(|e| e.key.as_deref() == Some("app-window")).unwrap().fill, Palette::new(false).bg);
}

// case: the page marks itself glass only in the Mac window, never in a browser, the loading window or the cover
#[test]
fn the_page_marks_itself_glass_only_in_the_mac_window_never_in_a_headless_run_or_off_a_mac() {
    assert!(marks_glass("macos", false));
    assert!(!marks_glass("macos", true), "a headless run (--snap, --scene) is not the Mac's window");
    assert!(!marks_glass("windows", false));
    assert!(!marks_glass("linux", false));
    // The state starts unmarked: nothing but the real window's setup marks it.
    assert!(!State::default().mac_glass);
}

// case: the sheet lets the glass through the sidebar and the title bar, and every other pane keeps a ground
#[test]
fn the_glass_comes_through_the_sidebar_and_the_title_bar_and_every_other_pane_keeps_a_ground() {
    for dark in [true, false] {
        let p = Palette::new(dark);
        let tint = glass_tint(dark);
        // A wash, not a fill: the glass must come through in both themes.
        assert!(tint.a > 0.0 && tint.a < 0.6, "tint {}", tint.a);
        let all = window(true, dark, "wide");
        assert_eq!(find(&all, "sidebar").fill, tint);
        assert_eq!(find(&all, "titlebar").fill, tint);
        assert_eq!(find(&all, "app-panes").fill, p.bg, "the panes beside the sidebar keep the window's ground");
        // The compact rail is the sidebar too.
        let rail = window(true, dark, "compact");
        assert_eq!(find(&rail, "rail").fill, tint);
        // Off glass: the solid grounds, as before.
        let l = crate::look::controls::Look::new(dark);
        let off = window(false, dark, "wide");
        assert_eq!(find(&off, "sidebar").fill, p.sidebar);
        assert_eq!(find(&off, "titlebar").fill, l.c("bg-chrome"));
        assert_eq!(find(&off, "app-panes").fill.a, 0.0);
        assert_eq!(find(&window(false, dark, "compact"), "rail").fill, p.sidebar);
    }
    assert_ne!(glass_tint(true), glass_tint(false));
}

// The rows in the sidebar paint nothing on glass, so the wash under them is all that shows (drawn, in the real sidebar).
#[test]
fn an_unpicked_sidebar_row_paints_nothing_on_glass() {
    for glass in [false, true] {
        let mut s = State::default();
        s.mac_glass = glass;
        s.appearance = Some(true);
        let at = "2026-11-23T05:00:00.000Z";
        s.recent.push(ConversationRow { id: "mission_glass".into(), title: "yo".into(), teammate: String::new(),
            started_by: String::new(), state: "completed".into(), updated: at.into() });
        let mut rt = Runtime::new(s, crate::view, (1400.0, 900.0), Env::new(true, 1.0));
        rt.frame(0.0);
        let row = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("recent-mission_glass")).expect("the row");
        if glass {
            assert_eq!(row.fill.a, 0.0, "on glass the row paints nothing");
        } else {
            assert_eq!(row.fill, Palette::new(true).sidebar);
        }
    }
}
