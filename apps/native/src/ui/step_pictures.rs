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
    Some(
        el().key(format!("picture:{path}"))
            .size(w, h)
            .pixels(image.pixels())
            .radius(10.0)
            .border(1.0, p.hairline)
            .role("image")
            .label(format!("Picture {path}"))
            .help(path.to_owned()),
    )
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
#[path = "step_pictures_tests.rs"]
mod tests;
