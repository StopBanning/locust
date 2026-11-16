//! only-the-face-you-talk-to-hops.test.ts (main): "which faces hop". Electron reads its own source for the call sites that ask
//! for `motion="full"`, so a third hopping copy of the same teammate is a decision and not a paste. This reads the
//! window's source the same way, for `BotMotionLevel::Full`.
//!
//! Rust's list is shorter than Electron's three (the New teammate preview; the live line in a conversation and the call
//! with a teammate are not drawn by the window yet), and `BotFaceOpts::default()` asks for Full where Electron's
//! `TeammateBot` is subtle unless told otherwise: nothing but a test builds one from the default.

use std::path::Path;

fn sources(dir: &Path, out: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if name.ends_with("_tests.rs") || name == "tests.rs" {
                continue;
            }
            out.push((path.to_string_lossy().replace('\\', "/"), std::fs::read_to_string(&path).unwrap()));
        }
    }
}

fn window_sources() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut all = Vec::new();
    sources(&root, &mut all);
    all
}

/// The source before its test module (`#[cfg(test)]` over a `mod` or a `#[path]`), whatever helpers come before it.
fn without_tests(text: &str) -> &str {
    let at = ["#[cfg(test)]\nmod ", "#[cfg(test)]\n#[path"].iter().filter_map(|marker| text.find(marker)).min();
    &text[..at.unwrap_or(text.len())]
}

// "is full in three places only" -- here, in the places the window asks for it: the New teammate preview, and the default.
#[test]
fn the_window_asks_for_full_motion_in_the_look_pickers_preview_and_the_default_and_nowhere_else() {
    let mut asking: Vec<(String, usize)> = window_sources()
        .into_iter()
        .filter(|(name, _)| !name.ends_with("look/bot_stage.rs") && !name.ends_with("look/face_motion_sites_tests.rs"))
        .map(|(name, text)| {
            let at = name.split("/src/").nth(1).unwrap_or(&name).to_owned();
            (at, without_tests(&text).matches("BotMotionLevel::Full").count())
        })
        .filter(|(_, n)| *n > 0)
        .collect();
    asking.sort();
    assert_eq!(asking, vec![("look/faces.rs".to_owned(), 1), ("ui/look_picker.rs".to_owned(), 1)]);
}

// "is never computed, so the list above is the whole list".
#[test]
fn the_motion_a_face_asks_for_is_never_computed() {
    for (name, text) in window_sources() {
        for line in without_tests(&text).lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("motion:") {
                let rest = rest.trim();
                // A declaration of the type (`motion: BotMotionLevel,`), or one literal level spelled out: not an `if`, a
                // `match` or a variable.
                let tail = rest.rsplit("BotMotionLevel::").next().unwrap_or("");
                let literal = rest.matches("BotMotionLevel::").count() == 1 && (tail == "Full," || tail == "Subtle,");
                assert!(rest == "BotMotionLevel," || literal, "{name}: {line}");
            }
        }
    }
}

// "leaves the header and the sidebar subtle".
#[test]
fn the_sidebar_and_the_rail_and_settings_leave_faces_subtle() {
    for file in ["ui/sidebar.rs", "ui/rail.rs", "ui/settings_panel.rs"] {
        let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(file)).unwrap();
        assert!(!without_tests(&text).contains("BotMotionLevel::Full"), "{file}");
    }
    let sidebar = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/sidebar.rs")).unwrap();
    assert!(sidebar.contains("BotMotionLevel::Subtle"));
}
