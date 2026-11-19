//! `a-thread-shows-local-pictures.test.tsx`, drawn: a step's pictures and a reply's local pictures.

use super::*;

/// A 4x2 red PNG, made by hand here, never a real screenshot.
const TINY_PNG: &[u8] = &[137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 4, 0, 0, 0, 2, 8, 6, 0, 0, 0, 127, 168, 125, 99, 0, 0, 0, 18, 73, 68, 65, 84, 120, 156, 99, 56, 161, 161, 241, 31, 25, 51, 160, 11, 0, 0, 25, 213, 16, 185, 253, 47, 204, 55, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130];

fn tiny_png() -> Vec<u8> {
    TINY_PNG.to_vec()
}

pub(super) struct Folder(pub(super) std::path::PathBuf);
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(super) fn state_in_folder() -> (State, Folder) {
    let dir = std::env::temp_dir().join(format!("locust-pictures-{}", crate::composer::new_queue_id()));
    std::fs::create_dir_all(&dir).unwrap();
    let dir = dir.canonicalize().unwrap();
    std::fs::write(dir.join("chart.png"), tiny_png()).unwrap();
    std::fs::write(dir.join("after.png"), tiny_png()).unwrap();
    std::fs::write(dir.join("notes.md"), "words").unwrap();
    let mut s = State::default();
    s.image_cache = Some(std::sync::Arc::new(rune::ImageCache::new(std::sync::Arc::new(|| {}))));
    s.teammates = vec![Teammate {
        id: "tm_codec".into(),
        name: "Codec".into(),
        role: "Code & Migrations".into(),
        runtime: Some("codex".into()),
        model: None,
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: Some(dir.to_string_lossy().into_owned()),
    }];
    s.selected = 0;
    s.conversations.insert("tm_codec".into(), Conversation::default());
    (s, Folder(dir))
}

pub(super) fn tool(id: &str, name: &str, summary: &str) -> ledger::Item {
    ledger::Item::Tool {
        name: name.into(),
        status: ledger::Status::Done,
        summary: summary.into(),
        detail: Box::new(ledger::ToolDetail { id: id.into(), finished: true, ..Default::default() }),
    }
}

fn pictures_drawn(items: &[ledger::Item], s: State) -> (Vec<String>, String) {
    let mut rt = rune::Runtime::new(
        (s, items.to_vec()),
        |(s, items), _| activity_group(items, &Palette::new(true), s, "conv"),
        (700.0, 700.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    let elements = rt.elements();
    let pictures = elements.iter().filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("picture:"))).map(|e| e.label.clone().unwrap_or_default()).collect();
    let words = elements.iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n");
    (pictures, words)
}

#[test]
fn a_step_that_read_a_picture_shows_it_while_folded_and_keeps_its_line() {
    let (s, _dir) = state_in_folder();
    let (pictures, words) = pictures_drawn(&[tool("t1", "Read", "chart.png"), tool("t2", "Bash", "ls")], s);
    assert_eq!(pictures, ["Picture chart.png"]);
    assert!(words.contains("Read a file, listed a folder"), "{words}");
}

#[test]
fn two_pictures_sit_side_by_side_as_one_strip() {
    let (s, _dir) = state_in_folder();
    let (pictures, _) = pictures_drawn(&[tool("t1", "Read", "chart.png"), tool("t2", "view_file", "after.png")], s);
    assert_eq!(pictures, ["Picture chart.png", "Picture after.png"]);
}

#[test]
fn a_picture_that_may_not_be_drawn_leaves_the_step_as_it_was() {
    let (s, _dir) = state_in_folder();
    let (pictures, _) = pictures_drawn(
        &[tool("t1", "Read", "../outside.png"), tool("t2", "Read", "missing.png"), tool("t3", "Read", "notes.md"), tool("t4", "Grep", "chart.png")],
        s,
    );
    assert!(pictures.is_empty(), "{pictures:?}");
}

#[test]
fn a_picture_is_held_to_320_points_and_never_enlarged_or_cropped() {
    assert_eq!(fitted([4, 2], PICTURE_MAX_WIDTH, PICTURE_MAX_HEIGHT), (4.0, 2.0), "never enlarged");
    assert_eq!(fitted([1000, 2000], PICTURE_MAX_WIDTH, PICTURE_MAX_HEIGHT), (160.0, 320.0), "a tall screenshot keeps its shape");
    assert_eq!(fitted([2400, 1200], PICTURE_MAX_WIDTH, PICTURE_MAX_HEIGHT), (560.0, 280.0), "a wide one is held to the column");
    assert_eq!(fitted([1000, 2000], PAIR_MAX_WIDTH, PAIR_MAX_HEIGHT), (110.0, 220.0));
}

#[test]
fn a_step_shows_at_most_six_pictures() {
    let (s, dir) = state_in_folder();
    let items: Vec<ledger::Item> = (0..8)
        .map(|n| {
            std::fs::write(dir.0.join(format!("shot{n}.png")), tiny_png()).unwrap();
            tool(&format!("t{n}"), "Read", &format!("shot{n}.png"))
        })
        .collect();
    let (pictures, _) = pictures_drawn(&items, s);
    assert_eq!(pictures.len(), STEP_PICTURES_SHOWN);
}

fn reply_drawn(body: &str, s: State) -> (Vec<String>, String) {
    let body = body.to_owned();
    let mut rt = rune::Runtime::new(
        (s, body),
        |(s, body), _| crate::reply_actions::reply_markdown_for_tests(s, body, &Palette::new(true)),
        (700.0, 700.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    let elements = rt.elements();
    let pictures = elements.iter().filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("picture:"))).map(|e| e.label.clone().unwrap_or_default()).collect();
    let words = elements.iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n");
    (pictures, words)
}

#[test]
fn a_replys_local_picture_is_drawn_with_its_alt_text_as_the_caption() {
    let (s, _dir) = state_in_folder();
    let (pictures, words) = reply_drawn("The result:\n\n![Revenue chart](chart.png)\n", s);
    assert_eq!(pictures, ["Revenue chart"]);
    assert!(words.contains("Revenue chart"), "{words}");
    assert!(!words.contains("!["), "{words}");
}

#[test]
fn a_reply_draws_no_outside_network_svg_or_missing_picture() {
    for src in ["../private.png", "https://example.com/graph.png", "x.svg", "missing.png"] {
        let (s, _dir) = state_in_folder();
        let (pictures, _) = reply_drawn(&format!("Before\n\n![chart]({src})\n\nafter."), s);
        assert!(pictures.is_empty(), "{src}: {pictures:?}");
    }
}

#[test]
fn a_picture_written_as_code_stays_its_words() {
    let (s, _dir) = state_in_folder();
    let (pictures, words) = reply_drawn("`![literal](chart.png)`", s);
    assert!(pictures.is_empty(), "{pictures:?}");
    assert!(words.contains("![literal](chart.png)"), "{words}");
}
