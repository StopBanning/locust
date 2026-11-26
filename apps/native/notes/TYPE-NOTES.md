# TYPE-NOTES: the Rust Locust's type matches Electron's (lane type-1, branch exec/native-type-1)

Colin, 26 Nov 2026: "the font looks off". Cause list: docs/REVIEW-design-rust-2026-11-26.md section 1. Pictures: the
headless run, `locust-native --scene <name> --snap <png> --size 1200x800 --scale 2` (HOME pointed at an empty folder,
so the real app data is never read), by `~/work/scratch/type-1/snaps.sh`. Scenes: home, settings, thread-rich (a
conversation with a plan), team (which draws a conversation with Codec, not a team board), kit; dark and light each.
`before/` is the base, 2339aa12 without this lane. One row per commit. Expected numbers in the tests are Electron's
(`shell.css`, `tokens.css`), read at build time, never the Rust constants under test.

## Row 1: smoothing

Electron: `body { -webkit-font-smoothing: antialiased }` (shell.css:30). Rune's name for it is `FontSmoothing::Off`
(vendor/rune/crates/rune/src/text/coretext_match.rs:17-25). The app set none, so it got Rune's Mac default `On`, which
thickens every glyph.

- `look::type_::SMOOTHING = FontSmoothing::Off`, taken by the main window (`main.rs`, the `App` chain after
  `.window_options`) and Quick Ask (`quick.rs`, the `App` chain before `.chrome`).
- Tests (`look/type_tests.rs`): shell.css's body rule says antialiased and `SMOOTHING` is `Off`; `main.rs` and
  `quick.rs` each carry exactly one chain line that takes it. `rune::App` has no getter for its smoothing, so the
  second is a scan of the source, which fails when the line is removed or points at something else.
- Pictures: `~/work/scratch/type-1/row1/`.
- Breaks (`~/work/scratch/evidence/type-1/row1.entries`, run with mut6.py): 5 of 5 KILLED. The line removed in main.rs and
  in quick.rs, and the line pointed at `FontSmoothing::On` in each, by
  `the_main_window_and_quick_ask_are_built_with_that_smoothing`; the constant flipped to `On` by
  `electron_smooths_the_body_with_antialiased_and_that_is_runes_off`.
- Call sites of `SMOOTHING` outside tests: `src/main.rs` (the `App::new(AppSpec{..})` chain, after `.window_options(window_glass::…)`)
  and `src/quick.rs` (the Quick Ask `App::new(scenes())` chain, before `.chrome(..)`).

## Row 2: Locust's own BODY, LABEL and TITLE (weight only)

The 712 + 143 + 78 uses of `type_::LABEL`, `BODY` and `TITLE` came from `rune::tok::type_`, the toolkit's sample scale
(vendor/rune/crates/rune/src/tok.rs:137-143). `look::type_` now has the same three names and the screens reach them
through the one import line in `main.rs` (`use crate::look::type_;`, line 362) and the one in `quick.rs` (line 10).

- `BODY` 13 px / 400 and `TITLE` 13 px / 600: the toolkit's own numbers, unchanged.
- `LABEL` 12 px, weight 500 to **400**. Electron's body sets no weight, so text is 400 unless a rule asks (shell.css sets
  `font-weight` on 119 rules, 500 on 50 and 600 on 45). Sizes and line heights are exactly the toolkit's, so text only
  gets narrower and nothing can wrap that did not.
- Tests (`look/type_tests.rs`), expected values from Electron's CSS: body rule has no `font-weight`, so BODY and LABEL are
  400; sizes equal `--lc-text-description` (13) and `--lc-text-ask` (12); TITLE's 600 is a weight 40+ shell.css rules use;
  `crate::type_::{LABEL,BODY,TITLE}` (what every screen imports) equal this module's, and both import lines are scanned.
- Breaks (`~/work/scratch/evidence/type-1/row2.entries`): 5 of 5 KILLED. The import line in main.rs and in quick.rs
  (`main_and_quick_take_their_type_from_this_module_not_the_toolkits`); LABEL back to 500, BODY to 12 px, TITLE to 500
  (one test each, named in the log `row2.out`).
- Call sites outside tests: every screen's `type_::LABEL / BODY / TITLE` (933 uses, e.g. src/ui/sidebar.rs:384 `type_::LABEL`,
  src/main.rs:2885 `.body_font(type_::BODY)`); the two import lines above.
- Pictures: `~/work/scratch/type-1/row2/`.
