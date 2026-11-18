//! Where the window opens and how big it is (Electron `main/window-size.ts`, `main/window-bounds.ts`).
//!
//! Pure placement logic: saved bounds and display work areas in, opening size, minimum size, and position out.

use serde_json::{Value, json};
use std::path::Path;

/// The smallest the layout is designed to work at.
pub const MIN_WINDOW_WIDTH: i32 = 1120;
pub const MIN_WINDOW_HEIGHT: i32 = 720;

/// A titlebar's worth of window must land on a display for a saved position to be usable.
const MIN_VISIBLE_WIDTH: i32 = 160;
const MIN_VISIBLE_HEIGHT: i32 = 48;

const DEFAULT_MAX_WIDTH: i32 = 1215;
const DEFAULT_MAX_HEIGHT: i32 = 800;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SavedWindow {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    /// Maximized windows restore maximized, but keep the size to un-maximize to.
    pub maximized: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpeningPlacement {
    pub width: i32,
    pub height: i32,
    /// `None` means no remembered position; the caller centers instead.
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub maximized: bool,
    /// The window's own minimum, for the work area it opens on (M21).
    pub min_width: i32,
    pub min_height: i32,
}

/// The smallest the window may be on a work area of this size: the layout's minimum, unless the work area itself is
/// smaller -- then the work area, so the window fits and the layout scrolls.
pub fn minimum_size(work: Size) -> Size {
    Size {
        width: MIN_WINDOW_WIDTH.min(1.max(work.width)),
        height: MIN_WINDOW_HEIGHT.min(1.max(work.height)),
    }
}

/// How big the window opens when nothing was saved: a fraction of the work area, capped at 1215x800, never below
/// `minimum_size`.
pub fn opening_size(work: Size) -> Size {
    let floor = minimum_size(work);
    Size {
        width: floor.width.max(DEFAULT_MAX_WIDTH.min((work.width as f64 * 0.82).round() as i32)),
        height: floor.height.max(DEFAULT_MAX_HEIGHT.min((work.height as f64 * 0.86).round() as i32)),
    }
}

/// Parse a saved window bounds JSON value as untrusted input.
pub fn read_saved_window(value: &Value) -> Option<SavedWindow> {
    let obj = value.as_object()?;
    let x = obj.get("x").and_then(Value::as_f64).filter(|n| n.is_finite())?;
    let y = obj.get("y").and_then(Value::as_f64).filter(|n| n.is_finite())?;
    let width = obj.get("width").and_then(Value::as_f64).filter(|n| n.is_finite())?;
    let height = obj.get("height").and_then(Value::as_f64).filter(|n| n.is_finite())?;
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    Some(SavedWindow {
        x: x.round() as i32,
        y: y.round() as i32,
        width: width.round() as i32,
        height: height.round() as i32,
        maximized: obj.get("maximized").and_then(Value::as_bool) == Some(true),
    })
}

fn overlaps(window: Rect, display: Rect) -> bool {
    let width = (window.x + window.width).min(display.x + display.width) - window.x.max(display.x);
    let height = (window.y + window.height).min(display.y + display.height) - window.y.max(display.y);
    width >= MIN_VISIBLE_WIDTH && height >= MIN_VISIBLE_HEIGHT
}

/// What to open with, given whatever was saved and the displays that exist now.
pub fn opening_placement(
    saved: Option<&SavedWindow>,
    displays: &[Rect],
    primary_work: Size,
) -> OpeningPlacement {
    let Some(saved) = saved else {
        let fallback = opening_size(primary_work);
        let floor = minimum_size(primary_work);
        return OpeningPlacement {
            width: fallback.width,
            height: fallback.height,
            x: None,
            y: None,
            maximized: false,
            min_width: floor.width,
            min_height: floor.height,
        };
    };

    let saved_rect = Rect {
        x: saved.x,
        y: saved.y,
        width: saved.width,
        height: saved.height,
    };
    let home = displays.iter().copied().find(|display| overlaps(saved_rect, *display));
    let bound = home.map(|d| Size { width: d.width, height: d.height }).unwrap_or(primary_work);
    let floor = minimum_size(bound);
    let width = floor.width.max(saved.width.min(bound.width));
    let height = floor.height.max(saved.height.min(bound.height));

    OpeningPlacement {
        width,
        height,
        x: home.map(|_| saved.x),
        y: home.map(|_| saved.y),
        maximized: saved.maximized,
        min_width: floor.width,
        min_height: floor.height,
    }
}

/// Read saved window bounds from disk, returning `None` if missing or malformed.
pub fn read_saved_window_file(path: &Path) -> Option<SavedWindow> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    read_saved_window(&value)
}

/// Persist window bounds to disk atomically enough for profile state.
pub fn write_saved_window_file(path: &Path, saved: &SavedWindow) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Could not create bounds directory: {e}"))?;
    }
    let value = json!({
        "x": saved.x,
        "y": saved.y,
        "width": saved.width,
        "height": saved.height,
        "maximized": saved.maximized,
    });
    let json = serde_json::to_string_pretty(&value).map_err(|e| format!("Could not serialize window bounds: {e}"))?;
    std::fs::write(path, format!("{json}\n")).map_err(|e| format!("Could not write window bounds: {e}"))
}
