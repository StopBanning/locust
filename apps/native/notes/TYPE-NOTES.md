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
