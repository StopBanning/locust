//! the-whole-window-can-be-liquid-glass.test.ts and the two panel cases of
//! the-look-is-offered-only-where-it-can-be-drawn.test.tsx, drawn: the whole window headless with the look chosen,
//! offered or not, read through Rune's `ElementInfo::material` (the local patch in RUNE.md).
use super::*;
use locust_core::workspace_settings::{
    liquid_from_arguments, load_appearance, save_appearance, AppearancePrefs, GlassGround, ThemeChoice,
};
use rune::{Env, Runtime, SystemMaterial};

const SIZE: (f32, f32) = (1400.0, 900.0);

fn state(look: LookChoice, offered: bool, dark: bool) -> State {
    let mut s = State::default();
    s.mac_glass = true;
    s.appearance = Some(dark);
    s.layout_pref = "wide".to_owned();
    s.settings_panel.look = look;
    s.settings_panel.liquid_offered = offered;
    s
}

fn drawn(s: State) -> Runtime<State> {
    let mut rt = Runtime::new(s, crate::view, SIZE, Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn info(rt: &Runtime<State>, key: &str) -> Option<rune::ElementInfo> {
    rt.elements().into_iter().find(|e| !e.exiting && e.key.as_deref() == Some(key))
}

fn materials(rt: &Runtime<State>) -> Vec<(Option<String>, SystemMaterial)> {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| Some((e.key.clone(), e.material?))).collect()
}

struct Dir(std::path::PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn store() -> (Dir, crate::store::Store) {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "locust-liquid-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("native")).unwrap();
    std::fs::create_dir_all(root.join("electron")).unwrap();
    let store = crate::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    (Dir(root), store)
}

// case: the look is kept beside the theme, Standard when it is missing or unreadable, and saving one keeps the other
#[test]
fn the_look_is_kept_beside_the_theme_standard_when_missing_or_unreadable_and_saving_one_keeps_the_other() {
    let (_dir, store) = store();
    assert_eq!(load_appearance(&store).look, LookChoice::Standard, "missing");
    std::fs::write(store.root().join("appearance.json"), "not json").unwrap();
    assert_eq!(load_appearance(&store).look, LookChoice::Standard, "unreadable");
    std::fs::write(store.root().join("appearance.json"), r#"{"look":"frosted"}"#).unwrap();
    assert_eq!(load_appearance(&store).look, LookChoice::Standard, "unknown");
    let prefs = AppearancePrefs { theme: ThemeChoice::Light, look: LookChoice::Liquid, glass_ground: GlassGround::Desktop };
    save_appearance(&store, prefs).unwrap();
    let read = load_appearance(&store);
    assert_eq!((read.theme, read.look), (ThemeChoice::Light, LookChoice::Liquid));
}

// case: what the glass shows is kept with the look: the desktop unless Rainbow was chosen, and kept by every save
#[test]
fn what_the_glass_shows_is_kept_with_the_look_the_desktop_unless_rainbow_was_chosen() {
    let (_dir, store) = store();
    assert_eq!(load_appearance(&store).glass_ground, GlassGround::Desktop);
    std::fs::write(store.root().join("appearance.json"), r#"{"glassGround":"sparkles"}"#).unwrap();
    assert_eq!(load_appearance(&store).glass_ground, GlassGround::Desktop);
    let prefs = AppearancePrefs { theme: ThemeChoice::Dark, look: LookChoice::Liquid, glass_ground: GlassGround::Rainbow };
    save_appearance(&store, prefs).unwrap();
    assert_eq!(load_appearance(&store), prefs);
}

// case: a window is drawn in it only when it is chosen and offered, and starts marked so from its own command line
#[test]
fn a_window_is_drawn_in_it_only_when_it_is_chosen_and_offered_and_starts_marked_so_from_its_command_line() {
    assert!(draws_liquid(LookChoice::Liquid, true));
    assert!(!draws_liquid(LookChoice::Liquid, false));
    assert!(!draws_liquid(LookChoice::Standard, true));
    assert!(liquid_from_arguments(&["--x", "--locust-look=liquid"]));
    assert!(!liquid_from_arguments(&["--locust-look=standard"]));
    assert!(!liquid_from_arguments::<&str>(&[]));
    // Drawn: chosen and offered, the window's ground is macOS's glass.
    let rt = drawn(state(LookChoice::Liquid, true, true));
    assert_eq!(info(&rt, "app-window").unwrap().material, Some(SystemMaterial::Glass));
    for (look, offered) in [(LookChoice::Liquid, false), (LookChoice::Standard, true)] {
        let rt = drawn(state(look, offered, true));
        assert_eq!(info(&rt, "app-window").unwrap().material, Some(SystemMaterial::Sidebar), "{look:?} offered {offered}");
    }
}

// case: is offered only on a Mac whose glass the addon can reach
#[test]
fn is_offered_only_on_a_mac_whose_glass_view_is_there() {
    assert!(!liquid_offered("windows", false, true));
    assert!(!liquid_offered("linux", false, true));
    assert!(!liquid_offered("macos", false, false), "no NSGlassEffectView before macOS 26");
    assert!(!liquid_offered("macos", true, true), "never in a headless run");
    assert!(liquid_offered("macos", false, true));
    // Nothing offers it until the real window's setup says so.
    assert!(!State::default().settings_panel.liquid_offered);
}

// case: changes the window in place: its material off before the glass goes on, and the lights into the panel
#[test]
fn changes_the_window_in_place_its_material_off_and_the_glass_on() {
    let mut rt = drawn(state(LookChoice::Standard, true, true));
    assert_eq!(materials(&rt), vec![(Some("app-window".to_owned()), SystemMaterial::Sidebar)]);
    rt.state.settings_panel.look = LookChoice::Liquid;
    rt.frame(16.0);
    let now = materials(&rt);
    // The sidebar material is gone, not left under the glass; the window and the panel are glass.
    assert!(now.iter().all(|(_, m)| *m == SystemMaterial::Glass), "{now:?}");
    assert!(now.contains(&(Some("app-window".to_owned()), SystemMaterial::Glass)));
    assert!(now.contains(&(Some("liquid-panel".to_owned()), SystemMaterial::Glass)));
    // Asked again, nothing is laid twice.
    rt.frame(32.0);
    assert_eq!(materials(&rt).len(), 2);
}

// case: goes back exactly: the glass off, the material and the lights where the window was made with them
#[test]
fn goes_back_exactly_the_glass_off_and_the_material_where_the_window_was_made_with_it() {
    let mut rt = drawn(state(LookChoice::Liquid, true, false));
    rt.state.settings_panel.look = LookChoice::Standard;
    rt.frame(16.0);
    assert_eq!(materials(&rt), vec![(Some("app-window".to_owned()), SystemMaterial::Sidebar)]);
    assert!(info(&rt, "liquid-panel").is_none());
    assert_eq!(info(&rt, "sidebar").unwrap().fill, glass_tint(false));
    assert_eq!(info(&rt, "titlebar").unwrap().fill, glass_tint(false));
    // A window that was never on glass (off a Mac) is left alone.
    let mut off = state(LookChoice::Liquid, true, true);
    off.mac_glass = false;
    assert!(materials(&drawn(off)).is_empty());
}

// case: a glass that will not go on leaves the window as it was made
#[test]
fn a_glass_that_is_not_offered_leaves_the_window_as_it_was_made() {
    let rt = drawn(state(LookChoice::Liquid, false, true));
    assert_eq!(materials(&rt), vec![(Some("app-window".to_owned()), SystemMaterial::Sidebar)]);
    assert!(info(&rt, "liquid-panel").is_none());
    assert_eq!(info(&rt, "sidebar").unwrap().fill, glass_tint(true));
}

// case: places the sidebar's panel only on a window drawn in it, and a drive's picture only when one is given
#[test]
fn places_the_sidebars_panel_only_on_a_window_drawn_in_it() {
    assert!(info(&drawn(state(LookChoice::Standard, true, true)), "liquid-panel").is_none());
    let rt = drawn(state(LookChoice::Liquid, true, true));
    let panel = info(&rt, "liquid-panel").expect("the panel");
    assert_eq!(panel.material, Some(SystemMaterial::Glass));
    let side = info(&rt, "sidebar").unwrap();
    let want = glass_panel_for(Some(side.rect[0] + side.rect[2]), SIZE.1).unwrap();
    assert_eq!(panel.rect, [want.x, want.y, want.width, want.height]);
    assert_eq!(panel.radius, GLASS_RADIUS);
    // The compact rail gets its own, as narrow as the rail.
    let mut s = state(LookChoice::Liquid, true, true);
    s.layout_pref = "compact".to_owned();
    let rt = drawn(s);
    assert_eq!(info(&rt, "liquid-panel").unwrap().rect[2], crate::rail::RAIL_WIDTH - GLASS_INSET);
}

// case: takes a panel from the page only when every number is finite, positive and sane
#[test]
fn takes_a_panel_only_when_every_number_is_finite_positive_and_sane() {
    assert_eq!(glass_panel_for(Some(276.0), 800.0), Some(GlassPanel { x: 8.0, y: 8.0, width: 268.0, height: 784.0, radius: 16.0 }));
    for (width, height) in [(None, 800.0), (Some(0.0), 800.0), (Some(f32::NAN), 800.0), (Some(276.0), f32::NAN),
        (Some(276.0), 20.0), (Some(-1.0), 800.0), (Some(f32::INFINITY), 800.0)] {
        assert_eq!(glass_panel_for(width, height), None, "{width:?} x {height}");
    }
}

// case: the sheet and the window agree on where the lights sit
#[test]
fn the_sidebar_and_the_title_bar_paint_nothing_and_light_has_its_own_washes() {
    for dark in [true, false] {
        let rt = drawn(state(LookChoice::Liquid, true, dark));
        assert_eq!(info(&rt, "sidebar").unwrap().fill.a, 0.0, "the sidebar paints nothing");
        assert_eq!(info(&rt, "titlebar").unwrap().fill.a, 0.0, "the title bar paints nothing");
        assert_eq!(info(&rt, "app-panes").unwrap().fill.a, 0.0, "the panes lie on the smoked ground");
        assert_eq!(info(&rt, "liquid-panel-wash").unwrap().fill, liquid_washes(dark).1);
    }
    assert_ne!(liquid_washes(true), liquid_washes(false), "light has its own washes");
}

// case: the panel runs from the window's top inset to its bottom inset, as wide as the sidebar, or is none
#[test]
fn the_panel_runs_from_the_windows_top_inset_to_its_bottom_inset_as_wide_as_the_sidebar_or_is_none() {
    assert_eq!(glass_panel_for(Some(276.0), 800.0), Some(GlassPanel { x: GLASS_INSET, y: GLASS_INSET, width: 276.0 - GLASS_INSET, height: 800.0 - 2.0 * GLASS_INSET, radius: GLASS_RADIUS }));
    assert_eq!(glass_panel_for(Some(82.0), 660.0).map(|p| p.width), Some(82.0 - GLASS_INSET));
    assert_eq!(glass_panel_for(None, 800.0), None);
    assert_eq!(glass_panel_for(Some(0.0), 800.0), None);
    assert_eq!(glass_panel_for(Some(276.0), 20.0), None);
}

// case: the panel the page asks for is the shape the sheet washes
#[test]
fn the_panel_the_window_asks_for_is_the_shape_its_wash_paints() {
    let rt = drawn(state(LookChoice::Liquid, true, true));
    let panel = info(&rt, "liquid-panel").unwrap();
    let wash = info(&rt, "liquid-panel-wash").unwrap();
    assert_eq!(wash.rect, panel.rect);
    assert_eq!((wash.radius, panel.radius), (GLASS_RADIUS, GLASS_RADIUS));
    assert_eq!((GLASS_INSET, GLASS_RADIUS), (8.0, 16.0));
}

// Lane check 23 Nov: in Liquid glass the sidebar's contents sit inside the floating panel, GLASS_INSET (8 points,
// `--lc-liquid-inset`) further in than in Standard.
#[test]
fn the_sidebars_contents_move_in_by_the_panels_inset_in_liquid_glass() {
    let left_of_words = |look| {
        let rt = drawn(state(look, true, true));
        let side = info(&rt, "sidebar").unwrap().rect;
        rt.elements()
            .into_iter()
            .filter(|e| !e.exiting && e.text.is_some() && e.rect[0] >= side[0] && e.rect[0] + e.rect[2] <= side[0] + side[2])
            .map(|e| e.rect[0])
            .fold(f32::INFINITY, f32::min)
    };
    let standard = left_of_words(LookChoice::Standard);
    let liquid = left_of_words(LookChoice::Liquid);
    assert!(standard.is_finite(), "the sidebar has words");
    assert_eq!(liquid - standard, 8.0);
}
