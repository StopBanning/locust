//! Electron's `ui-face-is-bundled.test.ts`, the half that is about the face itself: the app's own words are Figtree,
//! compiled into the binary in both styles, covering the title weight, with its licence beside it. Electron reads the
//! `@font-face` blocks and `.woff2` files; Rust reads the same `tokens.css` at build time and ships TrueType copies
//! (`assets/fonts/README.md`), so these hold the copies Rust actually draws with. The title half is drawn in
//! `ui/titles_as_drawn_tests.rs`.
use super::*;

/// The family names a face's `name` table spells (UTF-16, as Windows/Unicode records keep them).
fn names(face: &[u8], family: &str) -> bool {
    let utf16: Vec<u8> = family.encode_utf16().flat_map(|u| u.to_be_bytes()).collect();
    face.windows(utf16.len()).any(|w| w == utf16.as_slice())
}

fn u16_at(b: &[u8], at: usize) -> usize {
    u16::from_be_bytes([b[at], b[at + 1]]) as usize
}

fn fixed_at(b: &[u8], at: usize) -> f32 {
    i32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]) as f32 / 65536.0
}

/// The face's `wght` axis as (min, max), read from its `fvar` table; `None` for a face that is not variable in weight.
fn weight_axis(face: &[u8]) -> Option<(f32, f32)> {
    let tables = u16_at(face, 4);
    let fvar = (0..tables).map(|i| 12 + i * 16).find(|&r| &face[r..r + 4] == b"fvar")?;
    let at = u32::from_be_bytes([face[fvar + 8], face[fvar + 9], face[fvar + 10], face[fvar + 11]]) as usize;
    let (axes, count, size) = (at + u16_at(face, at + 4), u16_at(face, at + 8), u16_at(face, at + 10));
    (0..count).map(|i| axes + i * size).find(|&a| &face[a..a + 4] == b"wght").map(|a| (fixed_at(face, a + 4), fixed_at(face, a + 12)))
}

/// Bit 1 of `head.macStyle` (offset 44), set by an italic face.
fn italic_bit(face: &[u8]) -> bool {
    let tables = u16_at(face, 4);
    let head = (0..tables).map(|i| 12 + i * 16).find(|&r| &face[r..r + 4] == b"head").expect("a head table");
    let at = u32::from_be_bytes([face[head + 8], face[head + 9], face[head + 10], face[head + 11]]) as usize;
    u16_at(face, at + 44) & 0b10 != 0
}

/// The first family `--lc-font-ui` names, as a browser would try it first.
fn ui_face() -> String {
    tokens(true).raw("font-ui").split(',').next().unwrap_or("").trim().trim_matches('\'').to_owned()
}

#[test]
fn is_figtree_with_geist_behind_it_for_what_figtree_does_not_draw() {
    // case: is Figtree, with Geist behind it for what Figtree does not draw
    assert_eq!(ui_face(), "Figtree");
    assert!(tokens(true).raw("font-ui").starts_with("'Figtree', 'Geist',"), "{}", tokens(true).raw("font-ui"));
    // The family the window is started with is the one the CSS names first, and both Figtree files are that family.
    assert_eq!(FAMILIES.0, ui_face());
    assert_eq!(families_for("geist").0, ui_face());
    assert_eq!(families_for("paper").0, ui_face());
    assert!(names(FONTS[0], &ui_face()) && names(FONTS[1], &ui_face()));
    // Geist (the sans) is not bundled: Rune's `AppSpec::families` takes one sans family, and a glyph Figtree lacks
    // falls to Rune's system fallbacks (`rune::text::fallback`), not to a second bundled face.
}

#[test]
fn is_a_real_face_on_disk_for_both_styles_covering_the_title_weight() {
    // case: is a real woff2 on disk for both styles, covering the title weight
    let (upright, italic) = (FONTS[0], FONTS[1]);
    for (name, face) in [("upright", upright), ("italic", italic)] {
        assert!(face.len() > 20_000, "{name}: {} bytes", face.len());
        assert_eq!(&face[..4], &[0, 1, 0, 0], "{name} is not TrueType");
        assert!(names(face, "Figtree"), "{name} does not name Figtree");
        // A variable range that stops short of 700 would synthesise the titles (Electron: `font-weight: 300 900`).
        let (min, max) = weight_axis(face).unwrap_or_else(|| panic!("{name} is not variable in weight"));
        assert!(min <= 300.0 && max >= 900.0, "{name}: wght {min}..{max}");
        let title: f32 = tokens(true).raw("title-weight").parse().unwrap();
        assert!((min..=max).contains(&title), "{name}: the title weight {title} is outside {min}..{max}");
    }
    // Two styles, not one face twice: only the second sets the italic bit of its `head` table's macStyle.
    assert!(italic_bit(italic) && !italic_bit(upright));
}

#[test]
fn carries_its_licence_beside_it() {
    // case: carries its licence beside it
    let licence = include_str!("../../assets/fonts/FIGTREE-LICENSE.txt");
    assert!(licence.contains("SIL Open Font License"), "{licence}");
    assert!(licence.contains("The Figtree Project Authors"), "the licence beside Figtree is another face's");
}
