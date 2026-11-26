//! Locust's own type: how glyphs are drawn (here) and the three styles the screens set text in (`BODY`, `LABEL`,
//! `TITLE`, below once rows 2 and 3 of notes/TYPE-NOTES.md land). Electron's are `shell.css` and `tokens.css`.

/// How the windows draw glyphs. Electron's `body` sets `-webkit-font-smoothing: antialiased` (shell.css:30), which
/// Rune names `FontSmoothing::Off`: macOS browsers draw antialiased text without CoreText's dilation, so strokes are
/// thinner than Rune's Mac default, `On`. The main window (main.rs) and Quick Ask (quick.rs) both take it from here.
pub const SMOOTHING: rune::FontSmoothing = rune::FontSmoothing::Off;

#[cfg(test)]
#[path = "type_tests.rs"]
mod type_tests;
