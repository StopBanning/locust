//! Electron's `shared/image-files.test.ts`, one Rust test for each of its seven cases, in its order, with its values.
use super::*;

#[test]
fn names_the_media_type_for_the_formats_a_browser_paints() {
    assert_eq!(image_media_type("shot.png"), Some("image/png"));
    assert_eq!(image_media_type("photo.jpg"), Some("image/jpeg"));
    assert_eq!(image_media_type("photo.jpeg"), Some("image/jpeg"));
    assert_eq!(image_media_type("anim.gif"), Some("image/gif"));
    assert_eq!(image_media_type("modern.webp"), Some("image/webp"));
}

#[test]
fn does_not_care_how_the_extension_is_cased() {
    // A screenshot off a camera or a Windows share is routinely SHOT.PNG.
    assert_eq!(image_media_type("SHOT.PNG"), Some("image/png"));
    assert!(is_image_path("Photo.JPG"));
}

#[test]
fn reads_the_last_extension_not_the_first() {
    assert_eq!(image_media_type("archive.png.txt"), None);
    assert_eq!(image_media_type("report.final.png"), Some("image/png"));
}

#[test]
fn leaves_everything_else_as_a_file() {
    for path in ["notes.md", "main.ts", "archive.tar.gz", "README", "data.json"] {
        assert!(!is_image_path(path), "{path}");
        assert_eq!(image_media_type(path), None, "{path}");
    }
}

#[test]
fn refuses_svg_on_purpose() {
    // THE test. An SVG is a document that can carry script and fetch remote content, and this would be one chosen from
    // anywhere on the machine and painted inside the app. It still attaches and still reaches the runtime; it is
    // simply not drawn here.
    assert!(!is_image_path("icon.svg"));
}

#[test]
fn handles_a_path_with_directories_and_a_name_with_no_dot_at_all() {
    assert_eq!(image_media_type("src/assets/logo.png"), Some("image/png"));
    assert_eq!(image_media_type("Makefile"), None);
    assert_eq!(image_media_type(""), None);
    // A dotfile is not an extension.
    assert_eq!(image_media_type(".gitignore"), None);
}

#[test]
fn bounds_a_preview_well_above_a_screenshot_and_well_below_trouble() {
    assert!(MAX_PREVIEW_BYTES > 2 * 1024 * 1024);
    assert!(MAX_PREVIEW_BYTES <= 16 * 1024 * 1024);
}

// What Electron's source does and its test is silent on. Values taken from running `shared/image-files.ts`.

#[test]
fn beyond_electron_every_format_on_the_list_has_its_media_type_and_no_other_is_drawn() {
    for (path, media) in [
        ("a.png", "image/png"), ("a.jpg", "image/jpeg"), ("a.jpeg", "image/jpeg"), ("a.gif", "image/gif"), ("a.webp", "image/webp"),
        ("a.bmp", "image/bmp"), ("a.avif", "image/avif"), ("a.ico", "image/x-icon"), ("x.ICO", "image/x-icon"), ("a.JPEG", "image/jpeg"),
        ("a.Png", "image/png"),
    ] {
        assert_eq!(image_media_type(path), Some(media), "{path}");
    }
    assert_eq!(IMAGE_TYPES.len(), 8);
    for path in ["x.svg", "x.tiff", "x.heic", "x.jfif", "x.apng"] {
        assert_eq!(image_media_type(path), None, "{path}");
    }
}

#[test]
fn beyond_electron_the_extension_is_whatever_follows_the_last_dot_anywhere_in_the_path() {
    // A dot in a folder name is the last dot when the file has none (`dir.png/file`), and then it is not an image; a
    // dotfile named like an extension is one; nothing is trimmed, so a trailing space or line break is not an image.
    for path in ["a.png.", "a.jpg/", "dir.png/file", "dir.png\\file", "a.png\n", "a. png", "."] {
        assert_eq!(image_media_type(path), None, "{path:?}");
    }
    for path in [".png", "x/.png", " .png", "C:\\a\\b.PNG"] {
        assert_eq!(image_media_type(path), Some("image/png"), "{path:?}");
    }
}

#[test]
fn beyond_electron_the_extension_is_lowercased_the_way_javascript_does_it() {
    // `İ.png` (a dotted capital I in the name) is still a png; the Kelvin sign in the extension is not a `k`.
    assert_eq!(image_media_type("\u{130}.png"), Some("image/png"));
    assert_eq!(image_media_type("a.\u{212a}eep"), None);
}

#[test]
fn beyond_electron_a_preview_is_eight_megabytes_and_a_file_past_it_is_named_not_drawn() {
    assert_eq!(MAX_PREVIEW_BYTES, 8 * 1024 * 1024);
}
