//! THIS COMPUTER'S WINDOW WHILE A BROWSER HAS IT (Electron `ElsewhereApp.tsx` + `TitleBar.tsx`, 0.830).
//!
//! While a paired browser has the window, this computer shows a cover saying so and offering
//! "Use Locust here" to take it back, with no app behind it. In a browser (and on macOS, where
//! the system draws traffic lights), the window draws no Minimize / Maximize / Close controls of
//! its own.

use rune::{El, col, row, text};
use std::sync::Arc;

use crate::look::controls::{Look, Tone, button, ghost_button};
use crate::{Palette, State, type_};

pub(crate) const COVER_TITLE: &str = "Locust is open in a browser";
pub(crate) const COVER_BODY_BASE: &str =
    "Your teammates keep working on this computer, and the browser shows what they do.";
pub(crate) const TAKE_BACK_LABEL: &str = "Use Locust here";

#[derive(Default)]
pub(crate) struct ElsewhereCover {
    pub(crate) covered: bool,
    pub(crate) device: Option<String>,
    pub(crate) busy: bool,
    pub(crate) on_take_back: Option<Arc<dyn Fn() + Send + Sync>>,
}

/// Whether the title bar draws Locust's own Minimize / Maximize / Close controls.
/// In a browser the browser's own window has them; on macOS (`darwin`) macOS draws its own.
pub(crate) fn show_window_controls(in_browser: bool, platform: &str) -> bool {
    !in_browser && platform != "darwin"
}

/// The window controls strip (`lc-windowcontrols`), or `None` in a browser or on macOS.
pub(crate) fn window_controls_bar(in_browser: bool, platform: &str, p: &Palette) -> Option<El> {
    if !show_window_controls(in_browser, platform) {
        return None;
    }
    let l = Look::new(p.dark);
    Some(
        row()
            .key("window-controls")
            .gap(4.0)
            .child(ghost_button(&l, "Minimize").key("window-control-minimize").label("Minimize"))
            .child(ghost_button(&l, "Maximize").key("window-control-maximize").label("Maximize"))
            .child(ghost_button(&l, "Close").key("window-control-close").label("Close")),
    )
}

/// The cover's sentence: names the browser/device when known, and always says teammates keep working here.
pub(crate) fn cover_body(device: Option<&str>) -> String {
    match device {
        Some(name) if !name.is_empty() => format!("{name} has it now. {COVER_BODY_BASE}"),
        _ => COVER_BODY_BASE.to_owned(),
    }
}

/// Update the cover state when `DeskHost` hands the window to a browser (`covered = true`) or takes it back.
pub(crate) fn apply_cover(s: &mut State, covered: bool, device: Option<String>) {
    s.elsewhere_cover.covered = covered;
    s.elsewhere_cover.device = if covered { device } else { None };
    s.elsewhere_cover.busy = false;
}

/// Take the window back to this computer (`desk:take-back` / `DeskHost::take_back`).
pub(crate) fn take_back(s: &mut State) {
    if s.elsewhere_cover.busy {
        return;
    }
    s.elsewhere_cover.busy = true;
    let cb = s.elsewhere_cover.on_take_back.clone();
    if let Some(cb) = cb {
        cb();
    }
    s.elsewhere_cover.covered = false;
    s.elsewhere_cover.device = None;
    s.elsewhere_cover.busy = false;
}

/// Full-window cover shown on this computer while a browser has the window (`ElsewhereApp`).
pub(crate) fn view(s: &State, p: &Palette) -> El {
    let l = Look::new(p.dark);
    let body = cover_body(s.elsewhere_cover.device.as_deref());
    let disabled = s.elsewhere_cover.busy;
    col()
        .key("elsewhere-cover")
        .label(COVER_TITLE)
        .w_full()
        .h_full()
        .fill(p.bg)
        .padding(32.0)
        .gap(16.0)
        .child(
            col()
                .key("elsewhere-box")
                .w(460.0)
                .gap(12.0)
                .padding(24.0)
                .fill(p.raised)
                .radius(12.0)
                .child(
                    text(COVER_TITLE.to_owned())
                        .role("heading")
                        .level(1)
                        .font(type_::TITLE)
                        .color(p.text),
                )
                .child(
                    text(body)
                        .font(type_::BODY)
                        .color(p.secondary)
                        .wrap(),
                )
                .child(
                    button(&l, TAKE_BACK_LABEL, Tone::Neutral, disabled)
                        .key("elsewhere-take-back")
                        .label(TAKE_BACK_LABEL)
                        .on_press(|s: &mut State| {
                            take_back(s);
                        }),
                ),
        )
}

#[cfg(test)]
#[path = "elsewhere_cover_tests.rs"]
mod elsewhere_cover_tests;
