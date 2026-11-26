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

## Row 3: Electron's line height for BODY and LABEL

Electron: `body { font-size: 13.5px; line-height: var(--lc-leading-prose) }` (shell.css:25-31), `--lc-leading-prose: 1.55`
(tokens.css:556). A unitless line height is inherited as the factor, so every rule that sets a size and no line height
of its own has 1.55: counted over shell.css, 266 of the 291 rules at `--lc-text-description` (13 px) write none; the 25
that do write 1.35 to 1.6 (1.5 the commonest, 12 of them). The toolkit's default, which `type_::LABEL` and `BODY` had, is
1.25 times the size.

- `look::type_::LEADING = 1.55`; `BODY` has 20.15 px lines (was 16.25) and `LABEL` 18.6 px (was 15). `TITLE` keeps 1.25
  (a heading; the brief names BODY and LABEL). No screen gets the old height back: none looked clearly worse.
- Tests: body's `line-height: var(--lc-leading-prose)` is in shell.css, the token is 1.55, and `line_px()` of BODY and LABEL
  is size times it; TITLE is still 1.25; `crate::type_` (the screens') has the same heights.
- Breaks (`~/work/scratch/evidence/type-1/row3.entries`): 5 of 5 KILLED. LEADING to 1.25, the `.lh(..)` taken off BODY
  and off LABEL, TITLE given a line height, the import line pointed back at the toolkit.
- Pictures: `~/work/scratch/type-1/row3/` against `row2/`. All ten read. Nothing wraps, clips or overlaps. Heights are in
  CSS px (the pictures are at scale 2 and were viewed at 1.667 per CSS px, so figures are +-1 px):
  - settings, settings-light: the settings page grows. Every row in the left list +3 px (53 to 58 viewed px at ~1.67);
    the search field +4; the Dark/Light/Match chips and Standard/Large/Largest +4; each setting block +2 to +4 (Reply text
    size block 183 to 190 viewed, Code font 124 to 128, Terminal faces 123 to 130). By the bottom of the window the page
    is 29 px lower (Plush at 1120 to 1168 viewed): the Sidebar row now falls below the 800 px window where it was visible.
  - team, team-light (a conversation with Codec): each reply paragraph +2 to +3 px a line (a reply at 431 to 435 viewed,
    the second at 667 to 677); the copy icon, which follows the last reply, +8 px lower in all. User bubbles are the same.
  - thread-rich, thread-rich-light: the status line under the thread ("Working...") is 2 px taller and sits 2 px higher.
  - home, home-light, kit, kit-light: every box is within 2 px of row 2's (no change in the positions I measured).
