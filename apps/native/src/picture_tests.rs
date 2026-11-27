//! Picture tests: a screen approved by eye keeps its look (notes/PICTURES.md).
//!
//! Each locked scene is rendered in-process the way `locust-native --scene NAME --snap out.png` renders it
//! (`crate::picture_render`), at a fixed time and scale, and compared with its approved picture in `pictures/NAME.png`.
//! A test never writes a reference: a person who has looked at the render approves it with
//! `scripts/picture-lock.sh NAME`. On a failure the new render and a diff image land beside the reference as
//! `NAME.new.png` and `NAME.diff.png` (ignored by git), and the message names the scene and the changed share.

use std::path::{Path, PathBuf};

use rune::text::pixel_compare::Image;

use super::*;

/// The scenes whose look is locked, as `--scene` names them (notes/PICTURES.md says why these, and which were left out).
pub(crate) const LOCKED: &[(&str, fn(&mut State))] = &[
    ("settings", settings_panel::scene),
    ("settings-light", settings_panel::light_scene),
    ("people", people_scene),
    ("people-light", people_light_scene),
    ("thread-rich", thread_rich_scene),
    ("thread-rich-light", thread_rich_light_scene),
    ("empty", empty_scene),
    ("empty-light", empty_light_scene),
];

/// The render: one second in (past the opening fades; frame 0 is still loading and differs run to run), at 1x so the
/// eight references stay under a megabyte in all. scripts/picture-lock.sh passes the same two.
pub(crate) const AT: &str = "1";
pub(crate) const SCALE: &str = "1";

/// Elements left out of the comparison, by key, each with why. Every locked scene is built on the team fixture, whose
/// Argon is working, so the sidebar's first Recent row shimmers; the band runs on the wall clock
/// (`working_shimmer::clock`, an `Instant` started at the first shimmer drawn), not Rune's frame clock, so where it
/// stands in a snap depends on how fast this machine drew the frames before it: up to ~500 pixels of that title
/// moved by up to 134 levels between two runs of the same binary (26 Nov). The rectangle is read from each render's
/// own element tree, so it follows the row if the layout moves it; everything else on the screen is compared.
///
/// The Settings header's meta line ("Locust 0.883.0 · local only") is the other: it names the version, which every
/// release bumps, and a debug build (the tests) adds " · development build" that the release binary approved
/// pictures are drawn with never shows (`cfg!(debug_assertions)` in settings_panel.rs). The element grows across the
/// header, so its rectangle is the strip right of "Settings" up to Close.
pub(crate) const UNLOCKED: &[(&str, &str)] = &[
    ("conversation-title-sample-2", "the working row's title shimmers on the wall clock (src/ui/working_shimmer.rs clock())"),
    ("settings-meta", "the version, and \" · development build\" in a debug build"),
];

/// THE TOLERANCE. A pixel counts as changed when any of its red, green or blue moves by more than CHANNEL_TOLERANCE
/// (of 255); a scene fails when more than MAX_CHANGED_SHARE of its compared pixels changed.
/// Why these two: anti-aliasing noise (a glyph edge rasterised a hair differently by another GPU or driver) moves an
/// edge pixel by a few levels, so 8 levels lets it through, while any real recolour of an element moves every pixel
/// it covers by more. The share then lets a few hundred stray pixels through (0.02% is ~190 of 1215x800) but not a
/// moved, recoloured or missing element: the smallest of those, one short label, already covers several hundred.
pub(crate) const CHANNEL_TOLERANCE: u8 = 8;
pub(crate) const MAX_CHANGED_SHARE: f64 = 0.0002;
/// The most of a picture the UNLOCKED elements may cover (the shimmering title is ~0.25%, the settings meta strip ~1.6%).
pub(crate) const MAX_UNLOCKED_SHARE: f64 = 0.025;

/// How one render differs from its reference.
#[derive(Debug)]
pub(crate) struct Difference {
    pub changed: usize,
    /// The pixels compared: all of them but the unlocked rectangles.
    pub total: usize,
    /// Left, top, right, bottom (inclusive) of the changed pixels; None when none changed.
    pub bounds: Option<[u32; 4]>,
    /// Which pixels changed, row by row.
    pub mask: Vec<bool>,
}

impl Difference {
    pub fn share(&self) -> f64 {
        if self.total == 0 { 0.0 } else { self.changed as f64 / self.total as f64 }
    }
    pub fn passes(&self) -> bool {
        self.share() <= MAX_CHANGED_SHARE
    }
}

/// Pixel by pixel under the tolerance above, skipping the pixels inside `skip` (left, top, right, bottom, exclusive
/// of right and bottom). Different sizes are an error, never a pass.
pub(crate) fn compare(reference: &Image, render: &Image, skip: &[[u32; 4]]) -> Result<Difference, String> {
    if (reference.width, reference.height) != (render.width, render.height) {
        return Err(format!(
            "size changed: reference {}x{}, render {}x{}",
            reference.width, reference.height, render.width, render.height
        ));
    }
    let width = reference.width;
    let skipped = |i: usize| {
        let (x, y) = ((i as u32) % width, (i as u32) / width);
        skip.iter().any(|[l, t, r, b]| x >= *l && x < *r && y >= *t && y < *b)
    };
    let mut bounds: Option<[u32; 4]> = None;
    let mut total = 0;
    let mask: Vec<bool> = reference
        .rgb
        .chunks_exact(3)
        .zip(render.rgb.chunks_exact(3))
        .enumerate()
        .map(|(i, (a, b))| {
            if skipped(i) {
                return false;
            }
            total += 1;
            a.iter().zip(b).any(|(x, y)| x.abs_diff(*y) > CHANNEL_TOLERANCE)
        })
        .collect();
    for (i, _) in mask.iter().enumerate().filter(|(_, changed)| **changed) {
        let (x, y) = ((i as u32) % width, (i as u32) / width);
        bounds = Some(match bounds {
            None => [x, y, x, y],
            Some([l, t, r, b]) => [l.min(x), t.min(y), r.max(x), b.max(y)],
        });
    }
    Ok(Difference { changed: mask.iter().filter(|c| **c).count(), total, bounds, mask })
}

/// How many pixels the unlocked elements took out of the comparison, when that is more than MAX_UNLOCKED_SHARE: they
/// are one title row, never a hole the size of a panel.
pub(crate) fn left_out_too_much(difference: &Difference) -> Option<usize> {
    let left_out = difference.mask.len() - difference.total;
    (left_out as f64 > MAX_UNLOCKED_SHARE * difference.mask.len() as f64).then_some(left_out)
}

/// The reference dimmed to a third, with every changed pixel in magenta: where to look.
pub(crate) fn diff_image(reference: &Image, difference: &Difference) -> Vec<u8> {
    reference
        .rgb
        .chunks_exact(3)
        .zip(&difference.mask)
        .flat_map(|(p, changed)| if *changed { [255, 0, 255, 255] } else { [p[0] / 3, p[1] / 3, p[2] / 3, 255] })
        .collect()
}

pub(crate) fn pictures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("pictures")
}

/// The device-pixel rectangles of the elements keyed `keys` in a `--tree` dump, rounded outward.
pub(crate) fn rects_of(tree: &serde_json::Value, keys: &[&str], scale: f32) -> Vec<[u32; 4]> {
    fn walk(e: &serde_json::Value, keys: &[&str], scale: f32, out: &mut Vec<[u32; 4]>) {
        if let (Some(key), Some([x, y, w, h])) = (e["key"].as_str(), e["rect"].as_array().map(|r| r.as_slice()))
            && keys.contains(&key)
        {
            let [x, y, w, h] = [x, y, w, h].map(|v| v.as_f64().unwrap_or(0.0) as f32 * scale);
            out.push([x.floor().max(0.0) as u32, y.floor().max(0.0) as u32, (x + w).ceil() as u32, (y + h).ceil() as u32]);
        }
        for child in e["children"].as_array().into_iter().flatten().chain(e["elements"].as_array().into_iter().flatten()) {
            walk(child, keys, scale, out);
        }
    }
    let mut out = Vec::new();
    walk(tree, keys, scale, &mut out);
    out
}

/// One render read back: the picture, the unlocked rectangles, and the time it was taken at (the `--tree` dump's `t`).
struct Rendered {
    image: Image,
    skip: Vec<[u32; 4]>,
    at: Option<f64>,
}

/// Renders `scene` as `--snap` does, at the locked time.
fn render(scene: &str) -> Rendered {
    render_at(scene, AT)
}

/// Renders `scene` as `--snap` does at `at` seconds, into scratch files, and reads them back.
fn render_at(scene: &str, at: &str) -> Rendered {
    // Its own files: tests that render the same scene run at once under `cargo test` (one process, many threads).
    static RENDERS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = RENDERS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let base = std::env::temp_dir().join(format!("locust-picture-{}-{n}-{scene}", std::process::id()));
    let (out, tree) = (base.with_extension("png"), base.with_extension("json"));
    let args = ["--scene", scene, "--snap", &out.to_string_lossy(), "--tree", &tree.to_string_lossy(), "--at", at, "--scale", SCALE];
    crate::picture_render(LOCKED, args.iter().map(|a| a.to_string()).collect());
    let image = Image::read_png(&out).unwrap_or_else(|e| panic!("{scene}: the render was not written: {e}"));
    let tree: serde_json::Value = serde_json::from_slice(&std::fs::read(&tree).unwrap_or_default()).unwrap_or_default();
    let keys: Vec<&str> = UNLOCKED.iter().map(|(key, _)| *key).collect();
    let skip = rects_of(&tree, &keys, SCALE.parse().unwrap_or(1.0));
    let at = tree["t"].as_f64();
    let _ = (std::fs::remove_file(&out), std::fs::remove_file(base.with_extension("json")));
    Rendered { image, skip, at }
}

/// Err when a render was not taken at AT. The pixels cannot say so themselves: every locked scene is settled by
/// frame 0 but for the title bar's working dot, which pulses on the frame clock, and that 6x6 dot (with thread-rich's
/// own pulse, 96 pixels at most) is under MAX_CHANGED_SHARE. A render at 0 passed every picture test (27 Nov).
fn taken_at_the_locked_time(rendered: &Rendered) -> Result<(), String> {
    let locked: f64 = AT.parse().expect("AT is a number of seconds");
    match rendered.at {
        Some(t) if (t - locked).abs() < 1e-6 => Ok(()),
        t => Err(format!("rendered at {t:?} s, not at the locked {AT} s the approved pictures are drawn at")),
    }
}

/// The picture test for one locked scene against the approved pictures in `dir`: Err says what changed, after writing
/// the new render (and, when the sizes match, the diff image) beside the reference.
fn check(scene: &str, dir: &Path) -> Result<(), String> {
    check_at(scene, dir, AT)
}

/// `check`, with the render taken at `at` seconds: anything but AT fails before the pixels are compared.
fn check_at(scene: &str, dir: &Path, at: &str) -> Result<(), String> {
    let (reference_path, new_path, diff_path) =
        (dir.join(format!("{scene}.png")), dir.join(format!("{scene}.new.png")), dir.join(format!("{scene}.diff.png")));
    let _ = (std::fs::remove_file(&new_path), std::fs::remove_file(&diff_path));
    let rendered = render_at(scene, at);
    taken_at_the_locked_time(&rendered).map_err(|e| format!("{scene}: {e}"))?;
    let Rendered { image: now, skip, .. } = rendered;
    let write_new = || {
        let rgba: Vec<u8> = now.rgb.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect();
        rune::render::write_png(&new_path, now.width, now.height, &rgba).expect("write the new render");
    };
    let reference = match Image::read_png(&reference_path) {
        Ok(image) => image,
        Err(e) => {
            write_new();
            return Err(format!(
                "{scene}: no approved picture ({e}); look at {} and approve it with scripts/picture-lock.sh {scene}",
                new_path.display()
            ));
        }
    };
    let difference = compare(&reference, &now, &skip).map_err(|e| {
        write_new();
        format!("{scene}: {e}; the new render is {}", new_path.display())
    })?;
    if let Some(left_out) = left_out_too_much(&difference) {
        return Err(format!(
            "{scene}: {left_out} pixels were left out of the comparison (UNLOCKED: {skip:?}); at most {:.1}% may be",
            MAX_UNLOCKED_SHARE * 100.0
        ));
    }
    if difference.passes() {
        return Ok(());
    }
    write_new();
    let diff = diff_image(&reference, &difference);
    rune::render::write_png(&diff_path, reference.width, reference.height, &diff).expect("write the diff image");
    Err(format!(
        "{scene}: the look changed: {:.3}% of pixels ({} of {}) moved by more than {CHANNEL_TOLERANCE} levels \
         (allowed {:.3}%), within {:?}. New render {}, changed pixels in magenta in {}. If the change was asked for, \
         look at it and approve it with scripts/picture-lock.sh {scene}.",
        difference.share() * 100.0,
        difference.changed,
        difference.total,
        MAX_CHANGED_SHARE * 100.0,
        difference.bounds,
        new_path.display(),
        diff_path.display(),
    ))
}

fn holds(scene: &str) {
    if let Err(failure) = check(scene, &pictures_dir()) {
        panic!("{failure}");
    }
}

#[test]
fn the_settings_screen_keeps_its_approved_look() {
    holds("settings");
}

#[test]
fn the_light_settings_screen_keeps_its_approved_look() {
    holds("settings-light");
}

#[test]
fn the_people_screen_keeps_its_approved_look() {
    holds("people");
}

#[test]
fn the_light_people_screen_keeps_its_approved_look() {
    holds("people-light");
}

#[test]
fn a_rich_thread_keeps_its_approved_look() {
    holds("thread-rich");
}

#[test]
fn a_light_rich_thread_keeps_its_approved_look() {
    holds("thread-rich-light");
}

#[test]
fn the_empty_window_keeps_its_approved_look() {
    holds("empty");
}

#[test]
fn the_light_empty_window_keeps_its_approved_look() {
    holds("empty-light");
}

// ---- The harness itself ----

fn flat(width: u32, height: u32, rgb: [u8; 3]) -> Image {
    Image { width, height, rgb: rgb.iter().copied().cycle().take((width * height * 3) as usize).collect() }
}

fn paint(image: &mut Image, [left, top, right, bottom]: [u32; 4], rgb: [u8; 3]) {
    for y in top..bottom {
        for x in left..right {
            let i = ((y * image.width + x) * 3) as usize;
            image.rgb[i..i + 3].copy_from_slice(&rgb);
        }
    }
}

#[test]
fn every_locked_scene_has_a_test_and_an_approved_picture() {
    let source = include_str!("picture_tests.rs");
    for (scene, _) in LOCKED {
        assert!(source.contains(&format!("holds(\"{scene}\")")), "{scene} is locked but no test holds it");
        assert!(pictures_dir().join(format!("{scene}.png")).is_file(), "{scene} has no approved picture");
    }
}

#[test]
fn the_same_picture_passes() {
    let mut a = flat(200, 100, [20, 22, 28]);
    paint(&mut a, [10, 10, 60, 30], [230, 230, 235]);
    let d = compare(&a, &a, &[]).unwrap();
    assert_eq!((d.changed, d.bounds), (0, None));
    assert!(d.passes());
}

#[test]
fn anti_aliasing_noise_passes() {
    // Every edge pixel of a label a few levels off, and a few stray pixels further off: what another rasteriser gives.
    let a = flat(400, 300, [20, 22, 28]);
    let mut b = flat(400, 300, [20, 22, 28]);
    paint(&mut b, [0, 0, 400, 300], [26, 28, 34]);
    for x in 0..20 {
        paint(&mut b, [x * 7, 5, x * 7 + 1, 6], [90, 90, 90]);
    }
    let d = compare(&a, &b, &[]).unwrap();
    assert_eq!(d.changed, 20, "a shift of 6 levels is noise; only the 20 strays count");
    assert!(d.passes(), "20 of 120000 pixels is within {MAX_CHANGED_SHARE}: {d:?}");
}

#[test]
fn a_recoloured_label_fails() {
    // A 60x10 label at 1x, its text one shade lighter (16 levels): a changed colour token.
    let mut a = flat(1215, 800, [20, 22, 28]);
    paint(&mut a, [100, 100, 160, 110], [150, 150, 160]);
    let mut b = flat(1215, 800, [20, 22, 28]);
    paint(&mut b, [100, 100, 160, 110], [166, 166, 176]);
    let d = compare(&a, &b, &[]).unwrap();
    assert_eq!(d.changed, 600);
    assert_eq!(d.bounds, Some([100, 100, 159, 109]));
    assert!(!d.passes(), "a recoloured label must fail: {d:?}");
}

#[test]
fn a_moved_element_fails() {
    // A 40x12 chip moved down 3 px.
    let mut a = flat(1215, 800, [20, 22, 28]);
    paint(&mut a, [500, 300, 540, 312], [90, 120, 220]);
    let mut b = flat(1215, 800, [20, 22, 28]);
    paint(&mut b, [500, 303, 540, 315], [90, 120, 220]);
    let d = compare(&a, &b, &[]).unwrap();
    assert_eq!(d.changed, 240, "three rows uncovered, three rows covered");
    assert!(!d.passes(), "a moved chip must fail: {d:?}");
}

#[test]
fn a_missing_element_fails() {
    let mut a = flat(1215, 800, [20, 22, 28]);
    paint(&mut a, [900, 40, 930, 56], [240, 240, 240]);
    let b = flat(1215, 800, [20, 22, 28]);
    let d = compare(&a, &b, &[]).unwrap();
    assert!(!d.passes(), "a removed 30x16 button must fail: {d:?}");
}

#[test]
fn a_render_of_another_size_fails() {
    let error = compare(&flat(10, 10, [0, 0, 0]), &flat(10, 11, [0, 0, 0]), &[]).unwrap_err();
    assert!(error.contains("size changed"), "{error}");
}

#[test]
fn the_diff_image_marks_the_changed_pixels_in_magenta() {
    let a = flat(4, 1, [30, 60, 90]);
    let mut b = flat(4, 1, [30, 60, 90]);
    paint(&mut b, [2, 0, 3, 1], [200, 60, 90]);
    let d = compare(&a, &b, &[]).unwrap();
    assert_eq!(diff_image(&a, &d), [10, 20, 30, 255, 10, 20, 30, 255, 255, 0, 255, 255, 10, 20, 30, 255]);
}

/// A scratch folder of approved pictures for the failure-path tests, removed afterwards.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("locust-pictures-{}-{name}", std::process::id())); // one per test
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_changed_screen_fails_naming_the_scene_and_share_and_leaves_the_render_and_a_diff() {
    // The empty window held to the settings screen's picture: a real render, a real difference.
    let dir = scratch("changed");
    std::fs::copy(pictures_dir().join("settings.png"), dir.join("empty.png")).unwrap();
    let failure = check("empty", &dir).expect_err("the empty window is not the settings screen");
    assert!(failure.starts_with("empty: the look changed: "), "{failure}");
    assert!(failure.contains("% of pixels"), "{failure}");
    let new = Image::read_png(&dir.join("empty.new.png")).expect("the new render is left beside the reference");
    let diff = Image::read_png(&dir.join("empty.diff.png")).expect("the diff image is left beside the reference");
    assert_eq!((new.width, new.height, diff.width, diff.height), (1215, 800, 1215, 800));
    assert!(diff.rgb.chunks_exact(3).any(|p| p == [255, 0, 255]), "the diff marks changed pixels in magenta");
    let reference = std::fs::read(dir.join("empty.png")).unwrap();
    assert_eq!(reference, std::fs::read(pictures_dir().join("settings.png")).unwrap(), "a test never rewrites a reference");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_scene_with_no_approved_picture_fails_and_leaves_its_render() {
    let dir = scratch("missing");
    let failure = check("empty", &dir).expect_err("nothing approved yet");
    assert!(failure.contains("empty: no approved picture"), "{failure}");
    assert!(failure.contains("scripts/picture-lock.sh empty"), "{failure}");
    assert!(dir.join("empty.new.png").is_file());
    assert!(!dir.join("empty.png").exists(), "a test never approves a picture itself");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unlocked_rectangle_is_left_out_and_nothing_beside_it() {
    let a = flat(100, 100, [20, 22, 28]);
    let mut b = flat(100, 100, [20, 22, 28]);
    paint(&mut b, [10, 10, 30, 20], [200, 200, 200]);
    let d = compare(&a, &b, &[[10, 10, 30, 20]]).unwrap();
    assert_eq!((d.changed, d.total), (0, 10_000 - 200), "the change inside is not counted, nor are its pixels");
    paint(&mut b, [30, 10, 31, 20], [200, 200, 200]);
    let d = compare(&a, &b, &[[10, 10, 30, 20]]).unwrap();
    assert_eq!((d.changed, d.bounds), (10, Some([30, 10, 30, 19])), "the column just right of it is compared");
}

#[test]
fn the_shimmering_title_is_found_in_each_render_and_is_only_that_row() {
    let skip = render("empty").skip;
    assert_eq!(skip.len(), 1, "the working title, and no settings header on this screen: {skip:?}");
    let [left, top, right, bottom] = skip[0];
    assert!(right <= 248 && bottom - top <= 20, "inside the sidebar, one line high: {skip:?}");
    assert!(((right - left) * (bottom - top)) as f64 <= MAX_UNLOCKED_SHARE * 1215.0 * 800.0, "{skip:?}");
}

#[test]
fn a_tree_rectangle_is_rounded_outward_into_device_pixels() {
    let tree = serde_json::json!({"elements": [{"key": "a", "rect": [48.0, 319.88, 136.0, 17.5], "children": [
        {"key": "b", "rect": [1.5, 2.5, 3.25, 4.0]}]}]});
    assert_eq!(rects_of(&tree, &["a"], 1.0), [[48, 319, 184, 338]]);
    // At 2x: 3 to 9.5 across, 5 to 13 down; the half pixel on the right is taken in.
    assert_eq!(rects_of(&tree, &["b"], 2.0), [[3, 5, 10, 13]]);
    assert!(rects_of(&tree, &["c"], 1.0).is_empty());
}

#[test]
fn leaving_out_more_than_a_row_fails() {
    let a = flat(100, 100, [20, 22, 28]);
    let rows = compare(&a, &a, &[[0, 0, 100, 2]]).unwrap();
    assert_eq!(left_out_too_much(&rows), None, "two rows of 100 are 2%: allowed");
    let panel = compare(&a, &a, &[[0, 0, 50, 50]]).unwrap();
    assert_eq!(left_out_too_much(&panel), Some(2500), "a quarter of the picture is not");
}

#[test]
fn the_settings_header_leaves_out_only_its_meta_line() {
    let skip = render("settings").skip;
    assert_eq!(skip.len(), 2, "the working title and the meta line: {skip:?}");
    let meta = skip.iter().find(|[_, top, _, _]| *top < 100).expect("the meta line is in the header");
    assert!(meta[0] > 248 && meta[3] - meta[1] <= 20, "right of the sidebar, one line high: {meta:?}");
}


#[test]
fn a_render_is_taken_at_the_locked_time_and_one_at_zero_fails() {
    let locked = render("empty");
    assert_eq!(taken_at_the_locked_time(&locked), Ok(()), "the picture tests render at AT");
    let early = render_at("empty", "0");
    let failure = taken_at_the_locked_time(&early).expect_err("a render at 0 is not the approved time");
    assert!(failure.contains("rendered at Some(0.0) s, not at the locked 1 s"), "{failure}");
    // Why the time is checked and not left to the pixels: at 0 only the title bar's pulsing working dot differs.
    let reference = Image::read_png(&pictures_dir().join("empty.png")).unwrap();
    let d = compare(&reference, &early.image, &early.skip).unwrap();
    let [left, top, right, bottom] = d.bounds.expect("the working dot pulses on the frame clock, so 0 differs");
    assert!(left >= 1120 && right <= 1135 && top >= 10 && bottom <= 25, "only the dot moves: {:?}, {} pixels", d.bounds, d.changed);
    assert!(d.passes(), "and it is under the share, which is why the time itself is held: {:?}", d.bounds);
    // So the picture test itself fails on it, before comparing a pixel.
    let dir = scratch("early");
    std::fs::copy(pictures_dir().join("empty.png"), dir.join("empty.png")).unwrap();
    let failure = check_at("empty", &dir, "0").expect_err("a render at 0 fails the picture test");
    assert!(failure.starts_with("empty: rendered at Some(0.0) s"), "{failure}");
    assert_eq!(check_at("empty", &dir, AT), Ok(()), "and the same check at AT passes");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The body of `fn NAME` in main.rs, up to its closing brace at the start of a line.
fn main_rs_fn(name: &str) -> &'static str {
    let source = include_str!("main.rs");
    let start = source.find(&format!("fn {name}")).unwrap_or_else(|| panic!("main.rs has no fn {name}"));
    let body = &source[start..];
    &body[..body.find("\n}\n").expect("the fn ends")]
}

#[test]
fn the_app_window_and_the_picture_render_take_their_look_from_one_place() {
    // main()'s window and picture_render's both go through main_window_look: glass, smoothing and frame in one place,
    // so what the picture tests lock is what the app opens with.
    for name in ["main()", "picture_render"] {
        assert_eq!(
            main_rs_fn(name).matches("main_window_look(App::new(AppSpec {").count(),
            1,
            "{name} builds its App inside main_window_look"
        );
    }
    let look = main_rs_fn("main_window_look");
    for part in ["window_glass::window_options(", ".font_smoothing(crate::look::type_::SMOOTHING)", ".chrome(WINDOW_CHROME)"] {
        assert!(look.contains(part), "main_window_look sets {part}");
    }
}
