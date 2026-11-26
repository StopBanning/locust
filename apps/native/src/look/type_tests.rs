//! The type holds Electron's: shell.css's smoothing and the windows that must carry it. Expected values are read from
//! Electron's stylesheet, never from the constants under test.
use super::*;

const SHELL_CSS: &str = include_str!("../../../desktop/src/renderer/src/shell.css");
const MAIN_RS: &str = include_str!("../main.rs");
const QUICK_RS: &str = include_str!("../quick.rs");

/// The line in a window's option chain that takes smoothing from this module.
const SMOOTHING_LINE: &str = ".font_smoothing(crate::look::type_::SMOOTHING)";

#[test]
fn electron_smooths_the_body_with_antialiased_and_that_is_runes_off() {
    let body = SHELL_CSS.split("\nbody {").nth(1).and_then(|rest| rest.split('}').next()).expect("shell.css has a body rule");
    assert!(body.contains("-webkit-font-smoothing: antialiased;"), "Electron's body: {body}");
    assert_eq!(SMOOTHING, rune::FontSmoothing::Off, "antialiased is the thin mode, FontSmoothing::Off");
}

#[test]
fn the_main_window_and_quick_ask_are_built_with_that_smoothing() {
    for (name, source) in [("main.rs", MAIN_RS), ("quick.rs", QUICK_RS)] {
        // A mention in a comment or in this test's own text does not count: only a chain line that begins with it.
        let chained = source.lines().filter(|l| l.trim_start().starts_with(SMOOTHING_LINE)).count();
        assert_eq!(chained, 1, "{name}: the window's option chain takes its smoothing from look::type_::SMOOTHING");
    }
}

// ---- Row 2: Locust's own BODY, LABEL and TITLE -------------------------------------------------------------------

/// The declarations in Electron's body rule (shell.css), as `property: value` pairs.
fn electron_body_rule() -> &'static str {
    SHELL_CSS.split("\nbody {").nth(1).and_then(|rest| rest.split('}').next()).expect("shell.css has a body rule")
}

#[test]
fn electron_text_is_weight_400_unless_a_rule_says_otherwise_and_body_and_label_are_400() {
    // The body rule sets no font-weight, so everything inherits 400 (CSS `normal`).
    assert!(!electron_body_rule().contains("font-weight"), "Electron's body sets a weight: {}", electron_body_rule());
    assert_eq!(BODY.weight, 400.0);
    assert_eq!(LABEL.weight, 400.0, "the toolkit's LABEL is 500, one step heavier than the same words in Electron");
}

#[test]
fn the_sizes_are_the_tokens_the_stylesheet_names_for_them() {
    let t = super::super::tokens(true);
    // 13 px is `--lc-text-description`, Electron's commonest size; 12 px is `--lc-text-ask`, the supporting line.
    assert_eq!(BODY.size, t.px("text-description"));
    assert_eq!(TITLE.size, t.px("text-description"));
    assert_eq!(LABEL.size, t.px("text-ask"));
}

#[test]
fn title_is_weight_600_which_electron_uses_for_names_and_headings() {
    assert_eq!(TITLE.weight, 600.0);
    let uses = SHELL_CSS.matches("font-weight: 600").count();
    assert!(uses >= 40, "shell.css gives 600 to {uses} rules");
    assert!(TITLE.weight > BODY.weight);
}

#[test]
fn main_and_quick_take_their_type_from_this_module_not_the_toolkits() {
    // main.rs's own `type_` is what every screen's `use crate::{.., type_}` reaches.
    assert_eq!(crate::type_::LABEL, LABEL);
    assert_eq!(crate::type_::BODY, BODY);
    assert_eq!(crate::type_::TITLE, TITLE);
    for (name, source) in [("main.rs", MAIN_RS), ("quick.rs", QUICK_RS)] {
        assert_eq!(source.lines().filter(|l| *l == "use crate::look::type_;").count(), 1, "{name}: the one import of type_");
        assert!(!source.contains("use rune::tok::type_"), "{name} still imports the toolkit's sample scale");
    }
}

// ---- Row 3: the line height Electron's text has ------------------------------------------------------------------

#[test]
fn body_and_label_sit_on_electrons_line_height() {
    let t = super::super::tokens(true);
    let leading: f32 = t.raw("leading-prose").parse().expect("--lc-leading-prose is a number");
    // shell.css gives `body` that leading, and text that writes no line-height inherits it.
    assert!(SHELL_CSS.contains("line-height: var(--lc-leading-prose);"), "body's line-height");
    assert_eq!(leading, 1.55);
    for (name, style) in [("BODY", BODY), ("LABEL", LABEL)] {
        assert!((style.line_px() - style.size * leading).abs() < 1e-3, "{name}: {} px lines for {} px text", style.line_px(), style.size);
    }
    // Neither is the toolkit's 1.25 any more; a heading keeps the toolkit's, as Row 3 of notes/TYPE-NOTES.md says.
    assert!(BODY.line_px() > BODY.size * 1.5 && LABEL.line_px() > LABEL.size * 1.5);
    assert_eq!(TITLE.line_px(), TITLE.size * 1.25);
}

#[test]
fn the_screens_reach_those_line_heights() {
    // Through main.rs's `type_`, the one every screen's import reaches.
    assert_eq!(crate::type_::BODY.line_px(), BODY.line_px());
    assert_eq!(crate::type_::LABEL.line_px(), LABEL.line_px());
}

// ---- Row 6: the list of screens that still write a size of their own ---------------------------------------------

/// The files under src/ui that write `TextStyle::new(` with numbers of their own, and how many times each does today
/// (110 in 24 files, 26 Nov 2026). THE LIST MAY ONLY SHRINK: take a file out, or lower its count, when a screen moves
/// onto `type_::BODY / LABEL / TITLE` or a `Look` style; never add a file or raise a count (that is what this test is for).
const WRITES_ITS_OWN_SIZE: &[(&str, usize)] = &[
    ("activity_drawers.rs", 3),
    ("benchmarks_panel.rs", 3),
    ("changes_panel.rs", 5),
    ("compare_chart.rs", 3),
    ("compare_view.rs", 2),
    ("decision_card.rs", 2),
    ("github_section.rs", 1),
    ("home_panel.rs", 10),
    ("look_picker.rs", 1),
    ("messages.rs", 1),
    ("people_panel.rs", 11),
    ("rail.rs", 13),
    ("review_changes.rs", 1),
    ("room_screen.rs", 3),
    ("scout_cards.rs", 28),
    ("scout_setting.rs", 3),
    ("setup_card.rs", 1),
    ("sidebar.rs", 8),
    ("sidebar_rooms.rs", 1),
    ("terminal_panel.rs", 1),
    ("thread_view.rs", 3),
    ("tune_teammate.rs", 4),
    ("working_shimmer.rs", 1),
    ("working_spark.rs", 1),
];

/// Every `.rs` file directly under src/ui and the number of `TextStyle::new(` in it.
fn own_sizes_by_file() -> Vec<(String, usize)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui");
    let mut found = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("src/ui is readable") {
        let path = entry.expect("a directory entry").path();
        if path.extension().is_some_and(|e| e == "rs") {
            let n = std::fs::read_to_string(&path).expect("a source file").matches("TextStyle::new(").count();
            if n > 0 {
                found.push((path.file_name().expect("a name").to_string_lossy().into_owned(), n));
            }
        }
    }
    found.sort();
    found
}

#[test]
fn no_screen_writes_a_size_of_its_own_beyond_the_list_and_the_list_only_shrinks() {
    let found = own_sizes_by_file();
    let mut broken = Vec::new();
    for (file, n) in &found {
        match WRITES_ITS_OWN_SIZE.iter().find(|(f, _)| f == file) {
            None => broken.push(format!("{file} is new on the list: {n} `TextStyle::new(` of its own; use type_::BODY / LABEL / TITLE or a Look style")),
            Some((_, allowed)) if n > allowed => broken.push(format!("{file} went from {allowed} to {n} `TextStyle::new(`")),
            Some(_) => {}
        }
    }
    assert!(broken.is_empty(), "the type list may only shrink:\n{}", broken.join("\n"));
}

#[test]
fn the_list_is_sorted_has_no_empty_rows_and_names_files_that_exist() {
    let names: Vec<&str> = WRITES_ITS_OWN_SIZE.iter().map(|(f, _)| *f).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted, "keep the list sorted");
    assert!(WRITES_ITS_OWN_SIZE.iter().all(|(_, n)| *n > 0), "a file at 0 comes off the list");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui");
    for (file, _) in WRITES_ITS_OWN_SIZE {
        assert!(dir.join(file).exists(), "{file} is not under src/ui any more: take it off the list");
    }
}

#[test]
fn the_list_holds_what_the_screens_hold_today_or_less() {
    // The sum of today's list; it only goes down. (110 on 26 Nov 2026, from `grep -rc 'TextStyle::new(' src/ui`.)
    let listed: usize = WRITES_ITS_OWN_SIZE.iter().map(|(_, n)| n).sum();
    assert!(listed <= 110, "the list is {listed} and may only shrink from 110");
    let drawn: usize = own_sizes_by_file().iter().map(|(_, n)| n).sum();
    assert!(drawn <= listed, "{drawn} `TextStyle::new(` in src/ui against {listed} allowed");
}
