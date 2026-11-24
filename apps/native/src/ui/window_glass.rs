//! A MAC'S WINDOW IS GLASS BEHIND THE PAGE (Electron: main/window-chrome.ts, main/index.ts, renderer shell.css
//! `[data-glass='mac']`). On a Mac the window is made transparent and its whole ground is macOS's sidebar material,
//! following the window (Rune's `SystemMaterial::Sidebar`, the local patch in RUNE.md), as Finder's, Notes' and Mail's
//! are. The page lets the glass through the sidebar and the title bar under a light wash of its own ground, and every
//! other pane keeps a ground of its own. Off a Mac, and in a headless run, nothing changes.
use super::*;
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
    if s.mac_glass { app.system_material(SystemMaterial::Sidebar) } else { app }
}

/// The sidebar's (and the compact rail's) own ground: the glass wash on glass, else its solid ground.
pub(crate) fn sidebar_ground(s: &State, p: &Palette) -> Color {
    if s.mac_glass { glass_tint(p.dark) } else { p.sidebar }
}

/// What an unpicked row in the sidebar paints: nothing on glass (the sidebar's wash is under it), else the sidebar's
/// ground.
pub(crate) fn sidebar_row_ground(s: &State, p: &Palette) -> Color {
    if s.mac_glass { Color::TRANSPARENT } else { p.sidebar }
}

/// The title bar's ground: the glass wash on glass, else the chrome's.
pub(crate) fn title_bar_ground(s: &State, p: &Palette, l: &crate::look::controls::Look) -> Color {
    if s.mac_glass { glass_tint(p.dark) } else { l.c("bg-chrome") }
}

/// Every pane of the body beside the sidebar keeps a ground (`.lc-body > :not(.lc-sidebar)` gets the window's): on
/// glass it paints the window's ground itself, since the window's is now the glass.
pub(crate) fn pane_ground(s: &State, p: &Palette) -> Color {
    if s.mac_glass { p.bg } else { Color::TRANSPARENT }
}

#[cfg(test)]
#[path = "window_glass_tests.rs"]
mod window_glass_tests;
