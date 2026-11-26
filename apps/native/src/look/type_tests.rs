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
