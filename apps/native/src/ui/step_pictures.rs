//! A step's pictures, and a reply's local Markdown pictures, drawn as rounded cards (Electron's `ThreadImage`, laid
//! out the way Claude's own app shows a turn's screenshots). Which paths a step brings is
//! `locust_core::step_pictures`; whether one may be drawn is `workspace_image` (inside the conversation's folder, 8 MB,
//! never SVG). A picture that may not be drawn, or does not decode, leaves its row exactly as it was.

use super::*;

/// Electron's `.lc-threadimage__picture`: never taller than 320 points, never cropped.
pub(super) const PICTURE_MAX_HEIGHT: f32 = 320.0;
/// Two pictures side by side are each held lower, so a pair reads as one strip.
pub(super) const PAIR_MAX_HEIGHT: f32 = 220.0;
/// The widest a lone picture draws.
pub(super) const PICTURE_MAX_WIDTH: f32 = 560.0;
/// The widest each of a pair draws.
pub(super) const PAIR_MAX_WIDTH: f32 = 300.0;
/// The picture viewer's width inside its 340-point pane.
pub(super) const VIEWER_WIDTH: f32 = 308.0;
/// The picture viewer's pane height.
pub(super) const VIEWER_HEIGHT: f32 = 420.0;
/// Maximum zoom factor (ZoomableImage.tsx `MAX_SCALE`).
pub(super) const MAX_ZOOM_SCALE: f64 = 8.0;

/// Where the picture sits in the viewer pane and how big (ZoomableImage.tsx `ImageView`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ImageView {
    pub(super) scale: f64,
    pub(super) x: f64,
    pub(super) y: f64,
}

/// The scale a picture opens at: the pane's width, never larger than its own pixels (ZoomableImage.tsx `fitScale`).
pub(super) fn fit_scale(image: (f64, f64), pane: (f64, f64)) -> f64 {
    (pane.0 / image.0).min(1.0)
}

/// The smallest it zooms out to: the whole picture in the pane (ZoomableImage.tsx `wholeScale`).
pub(super) fn whole_scale(image: (f64, f64), pane: (f64, f64)) -> f64 {
    (pane.0 / image.0).min(pane.1 / image.1).min(1.0)
}

/// Keep the picture in the pane: centred where smaller than the pane, edges clamped to the pane where larger
/// (ZoomableImage.tsx `keepInPane`).
pub(super) fn keep_in_pane(view: ImageView, image: (f64, f64), pane: (f64, f64)) -> ImageView {
    let clamp_axis = |at: f64, size: f64, room: f64| -> f64 {
        if size <= room {
            (room - size) / 2.0
        } else {
            at.max(room - size).min(0.0)
        }
    };
    ImageView {
        scale: view.scale,
        x: clamp_axis(view.x, image.0 * view.scale, pane.0),
        y: clamp_axis(view.y, image.1 * view.scale, pane.1),
    }
}

/// Zoom by `factor` keeping the point under the pointer `(px, py)` where it is, between `min_scale` and
/// `MAX_ZOOM_SCALE` (ZoomableImage.tsx `zoomAround`).
pub(super) fn zoom_around(view: ImageView, factor: f64, px: f64, py: f64, min_scale: f64) -> ImageView {
    let scale = (view.scale * factor).max(min_scale).min(MAX_ZOOM_SCALE);
    let ratio = scale / view.scale;
    ImageView {
        scale,
        x: px - (px - view.x) * ratio,
        y: py - (py - view.y) * ratio,
    }
}
/// How many pictures a step shows; the rest stay as their rows.
pub(super) const STEP_PICTURES_SHOWN: usize = 6;
/// Pictures remembered across frames before the oldest are let go.
const PICTURES_KEPT: usize = 64;

/// Fit a picture of `size` pixels inside `max_w` by `max_h` points, keeping its shape, never enlarging it.
pub(super) fn fitted(size: [u32; 2], max_w: f32, max_h: f32) -> (f32, f32) {
    let (w, h) = (size[0].max(1) as f32, size[1].max(1) as f32);
    let scale = (max_w / w).min(max_h / h).min(1.0);
    ((w * scale).round().max(1.0), (h * scale).round().max(1.0))
}

/// The picture for `path` in this conversation, when it may be drawn and has decoded.
pub(super) fn thread_picture(s: &State, path: &str) -> Option<rune::Image> {
    let folder = std::path::PathBuf::from(selected_conversation(s)?.0.folder.as_deref()?);
    let cache = s.image_cache.as_ref()?;
    let key = (folder.clone(), path.to_owned());
    let mut kept = s.thread_pictures.borrow_mut();
    if !kept.contains_key(&key) {
        if kept.len() >= PICTURES_KEPT {
            kept.clear();
        }
        let picture = crate::workspace_image::workspace_image(path, &folder, std::slice::from_ref(&folder))
            .ok()
            .map(|real| cache.file(real, PICTURE_MAX_HEIGHT, 2.0));
        kept.insert(key.clone(), picture);
    }
    let image = kept.get(&key)?.clone()?;
    // A snapshot, a video or a test draws one frame: it needs the decode done before paint.
    if HEADLESS.load(std::sync::atomic::Ordering::Relaxed) || cfg!(test) {
        let _ = image.wait();
    }
    (image.is_ready() && image.error().is_none()).then_some(image)
}

fn card(image: &rune::Image, path: &str, max_w: f32, max_h: f32, p: &Palette) -> Option<El> {
    let (w, h) = fitted(image.size()?, max_w, max_h);
    let target = crate::reply_actions::FileTarget { path: path.to_owned(), start: 0, end: 0 };
    Some(
        el().key(format!("picture:{path}"))
            .size(w, h)
            .pixels(image.pixels())
            .radius(10.0)
            .border(1.0, p.hairline)
            .role("image")
            .label(format!("Picture {path}"))
            .help(path.to_owned())
            // ThreadImage's button: press for the larger view beside the conversation.
            .on_press(move |s: &mut State| crate::reply_actions::open_file(s, target.clone())),
    )
}

/// The picture in the file viewer, with zoom and pan controls beside it (ZoomableImage.tsx).
pub(super) fn viewer_picture(s: &State, image: &rune::Image, path: &str, p: &Palette) -> Option<El> {
    let size = image.size()?;
    let natural = (size[0].max(1) as f64, size[1].max(1) as f64);
    let pane = (VIEWER_WIDTH as f64, VIEWER_HEIGHT as f64);
    let fitted = keep_in_pane(ImageView { scale: fit_scale(natural, pane), x: 0.0, y: 0.0 }, natural, pane);
    let shown = s.viewer_image_view.map(|v| keep_in_pane(v, natural, pane)).unwrap_or(fitted);
    let min_scale = whole_scale(natural, pane);
    let at_fit = (shown.scale - fitted.scale).abs() < 1e-6;
    let percent = (shown.scale * 100.0).round() as i64;
    let drawn_w = (natural.0 * shown.scale).round().max(1.0) as f32;
    let drawn_h = (natural.1 * shown.scale).round().max(1.0) as f32;
    let can_zoom_out = shown.scale > min_scale + 1e-6;
    let can_zoom_in = shown.scale < MAX_ZOOM_SCALE - 1e-6;

    let zoom_to = move |s: &mut State, factor: f64| {
        let cur = s.viewer_image_view.unwrap_or(fitted);
        let next = zoom_around(cur, factor, pane.0 / 2.0, pane.1 / 2.0, min_scale);
        s.viewer_image_view = Some(keep_in_pane(next, natural, pane));
    };

    let l = crate::look::controls::Look::new(p.dark);
    let tool_btn = |key: &str, words: &str, label: &str, disabled: bool| {
        text(words.to_owned())
            .font(type_::LABEL)
            .color(if disabled { p.muted } else { p.secondary })
            .px(6.0)
            .py(3.0)
            .radius(4.0)
            .fill(p.raised)
            .hover(l.c("hover-surface"))
            .key(key.to_owned())
            .role("button")
            .label(label.to_owned())
    };

    let mut zoom_out = tool_btn("viewer-zoom-out", "−", "Zoom out", !can_zoom_out);
    if can_zoom_out {
        zoom_out = zoom_out.on_press(move |s: &mut State| zoom_to(s, 0.8));
    }
    let fit_words = if at_fit { "Fit".to_owned() } else { format!("{percent}%") };
    let fit_btn = tool_btn("viewer-zoom-fit", &fit_words, "Fit to the pane", false)
        .on_press(|s: &mut State| s.viewer_image_view = None);
    let mut zoom_in = tool_btn("viewer-zoom-in", "+", "Zoom in", !can_zoom_in);
    if can_zoom_in {
        zoom_in = zoom_in.on_press(move |s: &mut State| zoom_to(s, 1.25));
    }
    let actual_btn = tool_btn("viewer-zoom-actual", "100%", "Actual size", false)
        .on_press(move |s: &mut State| {
            let cur = s.viewer_image_view.unwrap_or(fitted);
            let next = zoom_around(cur, 1.0 / cur.scale, pane.0 / 2.0, pane.1 / 2.0, min_scale);
            s.viewer_image_view = Some(keep_in_pane(next, natural, pane));
        });

    let tools = row()
        .key("viewer-zoom-tools")
        .role("toolbar")
        .label("Zoom")
        .gap(6.0)
        .center_y()
        .child(zoom_out)
        .child(fit_btn)
        .child(zoom_in)
        .child(actual_btn);

    let picture_el = el()
        .key(format!("viewer-picture:{path}"))
        .size(drawn_w, drawn_h)
        .pixels(image.pixels())
        .radius(6.0)
        .border(1.0, p.hairline)
        .role("image")
        .label(format!("Picture {path}"));

    let pane_el = col()
        .key(format!("viewer-picture-pane:{path}"))
        .w(VIEWER_WIDTH)
        .h(VIEWER_HEIGHT)
        .scroll(ScrollSpec { stick_bottom: false })
        .role("group")
        .label(format!("{path}, {percent}%"))
        .focusable(true)
        .on_key(move |s: &mut State, event: &KeyEvent, _tx: &mut Tx| {
            match &event.key {
                Key::Text(t) if t == "+" || t == "=" => { zoom_to(s, 1.25); true }
                Key::Text(t) if t == "-" || t == "_" => { zoom_to(s, 0.8); true }
                Key::Text(t) if t == "0" => { s.viewer_image_view = None; true }
                Key::Text(t) if t == "1" => {
                    let cur = s.viewer_image_view.unwrap_or(fitted);
                    let next = zoom_around(cur, 1.0 / cur.scale, pane.0 / 2.0, pane.1 / 2.0, min_scale);
                    s.viewer_image_view = Some(keep_in_pane(next, natural, pane));
                    true
                }
                _ => false,
            }
        })
        .child(picture_el);

    Some(col().w_full().gap(8.0).child(pane_el).child(tools))
}

/// The strip under a step's line: its pictures in pairs, each a rounded card, or nothing when it has none to draw.
pub(super) fn step_picture_strip(s: &State, items: &[ledger::Item], scope: &str, p: &Palette) -> Option<El> {
    let workspace = selected_conversation(s).and_then(|(t, _)| t.folder.as_deref());
    let shown: Vec<(String, rune::Image)> = locust_core::step_pictures::step_picture_paths(items, workspace)
        .into_iter()
        .filter_map(|path| thread_picture(s, &path).map(|image| (path, image)))
        .take(STEP_PICTURES_SHOWN)
        .collect();
    if shown.is_empty() {
        return None;
    }
    let (max_w, max_h) = if shown.len() == 1 { (PICTURE_MAX_WIDTH, PICTURE_MAX_HEIGHT) } else { (PAIR_MAX_WIDTH, PAIR_MAX_HEIGHT) };
    let mut strip = col().key(format!("{scope}/pictures")).w_full().gap(8.0).pt(4.0).pb(4.0);
    for pair in shown.chunks(2) {
        let mut line = row().gap(8.0);
        for (path, image) in pair {
            if let Some(card) = card(image, path, max_w, max_h, p) {
                line = line.child(card);
            }
        }
        strip = strip.child(line);
    }
    Some(strip)
}

/// The pictures a reply's `![alt](path)` images name inside the conversation's folder, decoded, for Markdown's image
/// hook (which cannot reach the state while it draws).
pub(super) fn reply_pictures(s: &State, body: &str) -> HashMap<String, rune::Image> {
    let mut out = HashMap::new();
    if !body.contains("![") {
        return out;
    }
    for found in locust_core::invariants::regex(r"!\[[^\]]*\]\(\s*<?([^)\s>]+)>?").captures_iter(body) {
        let src = found[1].to_owned();
        if !out.contains_key(&src)
            && let Some(image) = thread_picture(s, &src)
        {
            out.insert(src, image);
        }
    }
    out
}

/// A reply's picture as a card, with its alt text as the caption below it (`.lc-threadimage__caption`).
pub(super) fn reply_picture_card(image: &rune::Image, src: &str, alt: &str, p: &Palette) -> Option<El> {
    let picture = card(image, src, PICTURE_MAX_WIDTH, PICTURE_MAX_HEIGHT, p)?.label(if alt.is_empty() { format!("Picture {src}") } else { alt.to_owned() });
    let mut out = col().gap(6.0).py(8.0).child(picture);
    if !alt.is_empty() {
        out = out.child(text(alt).font(type_::LABEL).color(p.muted));
    }
    Some(out)
}

#[cfg(test)]
#[path = "picture_viewer_tests.rs"]
mod viewer_tests;

#[cfg(test)]
#[path = "step_pictures_tests.rs"]
mod tests;
