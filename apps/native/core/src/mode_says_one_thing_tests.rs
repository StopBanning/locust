//! Electron `renderer/src/mode-says-one-thing.test.ts`: a mode means one thing, and it is written down once. The
//! registers (modeLabel, modeSummary, modeSentence and the picker's consequence) are allowed to differ; the facts are
//! not. Which Rust surface uses which register is held in `src/ui/mode_registers_tests.rs`.

use super::{MODE_FACTS, fact, mode_label, mode_sentence, mode_summary};

const EVERY_MODE: [&str; 5] = ["ask", "accept-edits", "approve-each", "plan", "auto"];

// case: covers the whole union exactly once
#[test]
fn covers_the_whole_union_exactly_once() {
    let mut modes: Vec<&str> = MODE_FACTS.iter().map(|f| f.mode).collect();
    modes.sort_unstable();
    let mut every = EVERY_MODE.to_vec();
    every.sort_unstable();
    assert_eq!(modes, every);
}

// case: names each mode the same way everywhere it is named
#[test]
fn names_each_mode_the_same_way_everywhere_it_is_named() {
    for f in MODE_FACTS {
        assert!(mode_summary(f.mode).starts_with(&format!("{} ", f.name)), "{}", f.mode);
        assert!(mode_label(Some(f.mode)).starts_with(&f.name.to_lowercase()), "{}", f.mode);
    }
}

// case: says read-only for every mode that is read-only, on every surface
#[test]
fn says_read_only_for_every_mode_that_is_read_only_on_every_surface() {
    for mode in ["ask", "plan"] {
        assert_eq!(MODE_FACTS.iter().find(|f| f.mode == mode).and_then(|f| f.scope), Some("read-only"));
        assert!(mode_label(Some(mode)).contains("read-only"), "{mode}");
        let said = format!("{} {}", mode_summary(mode), mode_sentence(mode));
        assert!(said.contains("refused") || said.contains("changes nothing"), "{said}");
    }
}

// case: says of Auto, on every surface, that it reaches past this folder
#[test]
fn says_of_auto_on_every_surface_that_it_reaches_past_this_folder() {
    assert!(mode_label(Some("auto")).contains("whole machine"));
    assert!(mode_summary("auto").contains("anything on this machine"));
    assert!(mode_sentence("auto").contains("anywhere on this machine"));
}

/// The registers in Electron's exact words, and a teammate with no mode yet.
#[test]
fn each_register_says_what_electron_says() {
    assert_eq!(mode_label(Some("ask")), "ask \u{b7} read-only");
    assert_eq!(mode_label(Some("approve-each")), "approve each action");
    assert_eq!(mode_label(None), "not set yet");
    assert_eq!(mode_summary("accept-edits"), "Edit \u{b7} may change files in this workspace");
    assert_eq!(mode_sentence("approve-each"), "Every change waits for your approval before it lands.");
    assert_eq!(fact("accept-edits").scope, None);
}
