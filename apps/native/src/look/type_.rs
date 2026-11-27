//! Locust's own type: how glyphs are drawn, and the three styles the screens set text in (`BODY`, `LABEL`, `TITLE`),
//! under the names rune's sample scale (`rune::tok::type_`) uses, so a screen's `type_::LABEL` is Locust's, not the
//! toolkit's demo. Electron's are `shell.css` and `tokens.css`; see notes/TYPE-NOTES.md for each row.
use rune::TextStyle;

/// How the windows draw glyphs. Electron's `body` sets `-webkit-font-smoothing: antialiased` (shell.css:30), which
/// Rune names `FontSmoothing::Off`: macOS browsers draw antialiased text without CoreText's dilation, so strokes are
/// thinner than Rune's Mac default, `On`. The main window (main.rs) and Quick Ask (quick.rs) both take it from here.
pub const SMOOTHING: rune::FontSmoothing = rune::FontSmoothing::Off;

/// The line height of text that sets none of its own: Electron's `body { line-height: var(--lc-leading-prose) }`
/// (shell.css:29, tokens.css `--lc-leading-prose: 1.55`) is inherited by the ~270 rules at the body's own sizes that
/// write no `line-height`; the 25 that do write 1.35 to 1.6 (1.5 the commonest). The toolkit's default is 1.25.
pub const LEADING: f32 = 1.55;

/// Running text: 13 px at weight 400 (`--lc-text-description`; Electron's default weight is 400, shell.css sets
/// `font-weight` only on the rules that mean it).
pub const BODY: TextStyle = TextStyle::new(13.0, 400.0).lh(13.0 * LEADING);

/// The small supporting line: 12 px at weight 400. The toolkit's own is 500, one weight step heavier than the same
/// words in Electron (row 2).
pub const LABEL: TextStyle = TextStyle::new(12.0, 400.0).lh(12.0 * LEADING);

/// A name or heading inside a card: 13 px at weight 600, as the toolkit has it.
pub const TITLE: TextStyle = TextStyle::new(13.0, 600.0);

/// A fact's label beside its value, as the approval card's receipt sets it: 12 px at weight 600, sentence case
/// (shell.css 0.862's receipt labels; approval_card.rs says where).
pub const FACT_LABEL: TextStyle = TextStyle::new(12.0, 600.0);

#[cfg(test)]
#[path = "type_tests.rs"]
mod type_tests;
