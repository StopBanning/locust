//! A MAC'S WINDOW IS GLASS BEHIND THE PAGE (Electron: main/window-chrome.ts, main/index.ts, renderer shell.css
//! `[data-glass='mac']`). On a Mac the window is made transparent and its whole ground is macOS's sidebar material,
//! following the window (Rune's `SystemMaterial::Sidebar`, the local patch in RUNE.md), as Finder's, Notes' and Mail's
//! are. The page lets the glass through the sidebar and the title bar under a light wash of its own ground, and every
//! other pane keeps a ground of its own. Off a Mac, and in a headless run, nothing changes.
use super::*;
use locust_core::workspace_settings::LookChoice;
use rune::SystemMaterial;

/// The wash over the glass in the sidebar and the title bar (tokens.css `--lc-glass-tint`): a wash, not a fill, so the
/// glass comes through in both themes.
pub(crate) fn glass_tint(dark: bool) -> Color {
    // rgba(18, 20, 21, 0.42) and rgba(246, 245, 241, 0.46).
    if dark { color("#121415").alpha(0.42) } else { color("#f6f5f1").alpha(0.46) }
}

/// The window's options: transparent on a Mac, so the material behind the page shows (window-chrome.ts gives Windows
/// and Linux no vibrancy).
pub(crate) fn window_options(os: &str) -> rune::WindowOptions {
    rune::WindowOptions { transparent: os == "macos", ..Default::default() }
}

/// Whether this window is drawn on glass: the Mac's own window only, never a headless run (main.tsx marks the page
/// only in the Mac window, never in a browser, the loading window or the cover).
pub(crate) fn marks_glass(os: &str, headless: bool) -> bool {
    os == "macos" && !headless
}

/// The window's whole ground: the sidebar material behind the page, its `fill` the stand-in where no native view backs
/// it. Without glass, the ground as it was.
pub(crate) fn window_ground(app: El, s: &State, p: &Palette) -> El {
    let app = app.fill(p.bg);
    if liquid(s) {
        // Liquid glass in place of the sidebar material, never over it (mac-glass.ts takes the vibrancy off first).
        app.system_material(SystemMaterial::Glass)
    } else if s.mac_glass {
        app.system_material(SystemMaterial::Sidebar)
    } else {
        app
    }
}

/// The sidebar's (and the compact rail's) own ground: the glass wash on glass, else its solid ground.
pub(crate) fn sidebar_ground(s: &State, p: &Palette) -> Color {
    if liquid(s) {
        Color::TRANSPARENT
    } else if s.mac_glass {
        glass_tint(p.dark)
    } else {
        p.sidebar
    }
}

/// What an unpicked row in the sidebar paints: nothing on glass (the sidebar's wash is under it), else the sidebar's
/// ground.
pub(crate) fn sidebar_row_ground(s: &State, p: &Palette) -> Color {
    if s.mac_glass { Color::TRANSPARENT } else { p.sidebar }
}

/// The title bar's ground: the glass wash on glass, else the chrome's.
pub(crate) fn title_bar_ground(s: &State, p: &Palette, l: &crate::look::controls::Look) -> Color {
    if liquid(s) {
        Color::TRANSPARENT
    } else if s.mac_glass {
        glass_tint(p.dark)
    } else {
        l.c("bg-chrome")
    }
}

/// Every pane of the body beside the sidebar keeps a ground (`.lc-body > :not(.lc-sidebar)` gets the window's): on
/// glass it paints the window's ground itself, since the window's is now the glass.
pub(crate) fn pane_ground(s: &State, p: &Palette) -> Color {
    if s.mac_glass && !liquid(s) { p.bg } else { Color::TRANSPARENT }
}

/// The hairline at the sidebar's and the title bar's edge: none in Liquid glass, where they float on the panel.
pub(crate) fn edge(s: &State, hairline: Color) -> Color {
    if liquid(s) { Color::TRANSPARENT } else { hairline }
}

/// The sidebar's width: in Liquid glass the inset wider, so the panel is set in and the rows keep their width.
pub(crate) fn sidebar_width(s: &State) -> f32 {
    SIDEBAR_WIDTH + sidebar_inset(s)
}

/// The sidebar's own inset from its left edge: the panel's, in Liquid glass.
pub(crate) fn sidebar_inset(s: &State) -> f32 {
    if liquid(s) { GLASS_INSET } else { 0.0 }
}

/// The sidebar's Standard width.
pub(crate) const SIDEBAR_WIDTH: f32 = 248.0;


// LIQUID GLASS (Electron 0.855: main/mac-glass.ts, renderer glassPanel.ts, shell.css `[data-look='liquid']`). On a Mac
// with macOS 26's glass, Settings > Appearance > Look puts the window on two of macOS's own glass views: the window's
// whole ground, and a floating panel under the sidebar, set in 8 points from the window's edges with its corners
// rounded. The sidebar, the title bar and the panes paint nothing; a smoked ground lies all round the panel (its spread
// shadow, which the native glass cuts out of its own interior), dark or light enough to read on over any desktop.

/// The panel's inset from the window's edges and its corner radius (glassPanel.ts GLASS_INSET, GLASS_RADIUS; shell.css
/// `--lc-liquid-inset`, `--lc-liquid-radius`).
pub(crate) const GLASS_INSET: f32 = 8.0;
pub(crate) const GLASS_RADIUS: f32 = 16.0;

/// Where the sidebar's glass panel goes, in points from the window's top left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GlassPanel {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub radius: f32,
}

/// The panel for a sidebar `sidebar_width` wide in a window `window_height` tall (glassPanelFor): from the window's top
/// inset to its bottom inset, as wide as the sidebar less the inset, or none when there is no room for one.
pub(crate) fn glass_panel_for(sidebar_width: Option<f32>, window_height: f32) -> Option<GlassPanel> {
    let width = panel_width(sidebar_width?)?;
    let height = (window_height - 2.0 * GLASS_INSET).round();
    if window_height < 4.0 * GLASS_INSET || !(2.0 * GLASS_INSET..=PANEL_DIMENSION_MAX).contains(&height) {
        return None;
    }
    Some(GlassPanel {
        x: GLASS_INSET,
        y: GLASS_INSET,
        width,
        height,
        radius: GLASS_RADIUS,
    })
}

// mac-glass.ts readGlassPanel rejects dimensions above 20,000 points.
const PANEL_DIMENSION_MAX: f32 = 20_000.0;

fn panel_width(sidebar_width: f32) -> Option<f32> {
    if sidebar_width < 2.0 * GLASS_INSET {
        return None;
    }
    let width = (sidebar_width - GLASS_INSET).round();
    (GLASS_INSET..=PANEL_DIMENSION_MAX).contains(&width).then_some(width)
}

/// Whether Liquid glass is offered (mac-glass.ts `offered`): only in the Mac's own window, and only where macOS has its
/// glass view (`NSGlassEffectView`, macOS 26); never in a headless run or off a Mac.
pub(crate) fn liquid_offered(os: &str, headless: bool, has_glass_view: bool) -> bool {
    os == "macos" && !headless && has_glass_view
}

/// Whether this Mac has macOS 26's glass view.
pub(crate) fn mac_has_glass_view() -> bool {
    #[cfg(target_os = "macos")]
    {
        objc2::runtime::AnyClass::get(c"NSGlassEffectView").is_some()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// A window is drawn in Liquid glass only when it is chosen and offered (shared/appearance.ts drawsLiquid).
pub(crate) fn draws_liquid(look: LookChoice, offered: bool) -> bool {
    look == LookChoice::Liquid && offered
}

/// This window is drawn in Liquid glass now: it is on a Mac's glass, and the look is chosen and offered.
pub(crate) fn liquid(s: &State) -> bool {
    s.mac_glass && draws_liquid(s.settings_panel.look, s.settings_panel.liquid_offered)
}

/// The washes over the glass: the smoked ground all round the panel, the panel's own wash, and its rim
/// (`--lc-liquid-ground`, `--lc-liquid-panel`, `--lc-liquid-rim`), each theme its own.
pub(crate) fn liquid_washes(dark: bool) -> (Color, Color, Color) {
    if dark {
        (color("#0d0e0f").alpha(0.62), color("#0d0e0f").alpha(0.4), Color::WHITE.alpha(0.13))
    } else {
        (color("#faf9f6").alpha(0.62), Color::WHITE.alpha(0.38), Color::WHITE.alpha(0.55))
    }
}

/// The sidebar's glass panel, under everything else in the window, for a sidebar (or rail) `sidebar_width` wide; none
/// unless the window is drawn in Liquid glass.
pub(crate) fn liquid_panel(s: &State, p: &Palette, sidebar_width: f32) -> Option<El> {
    if !liquid(s) {
        return None;
    }
    let width = panel_width(sidebar_width)?;
    let (ground, wash, rim) = liquid_washes(p.dark);
    let top_rim = Color::WHITE.alpha(if p.dark { 0.20 } else { 0.85 });
    Some(
        el().key("liquid-panel-frame")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .padding(GLASS_INSET)
            .a11y_hidden()
            .child(
                el().key("liquid-panel")
                    .w(width)
                    .h_full()
                    .radius(GLASS_RADIUS)
                    .system_material(SystemMaterial::Glass)
                    // The smoked ground, everywhere but the panel: the native glass cuts its own interior out of it.
                    .box_shadow(rune::Shadow { spread: 4000.0, color: ground, ..Default::default() })
                    .child(
                        el().key("liquid-panel-wash").w_full().h_full()
                            .radius(GLASS_RADIUS).fill(wash).border(0.5, rim).clip_rounded(GLASS_RADIUS)
                            // The one-point inset shine above the regular rim (shell.css --lc-liquid-rim-top).
                            .child(el().key("liquid-panel-top-rim").absolute(0.0, 0.0).w_full().h(1.0).fill(top_rim)),
                    ),
            ),
    )
}

#[cfg(test)]
#[path = "window_glass_tests.rs"]
mod window_glass_tests;
#[cfg(test)]
#[path = "liquid_glass_tests.rs"]
mod liquid_glass_tests;
