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

## Row 4: sizes, a proposal (nothing applied)

Counted over shell.css's 391 `font-size` rules (the numbers below are rule counts, not uses on screen):

| Electron rule | Rules | Size |
| --- | --- | --- |
| `var(--lc-text-description)` | 293 | 13 px (tokens.css:531) |
| `var(--lc-text-mono-caption)` | 150 | 11.5 px mono |
| `var(--lc-text-mono-label)` | 84 | 10.5 px mono |
| `var(--lc-text-control)` | 55 | 14 px |
| `var(--lc-text-body)` | 35 | 13.5 px; also what `body` sets (shell.css:28), so anything with no size of its own |
| a literal `12px` | 33 | 12 px |

What each of the three should be:

- **BODY: stay 13 px.** It is `--lc-text-description`, the size on 293 rules. 13.5 is only the inherited base.
- **LABEL: 12 px to 13 px.** The 712 texts set in LABEL are what Electron writes at the description size (the sidebar
  rows, settings descriptions, chips, card lines); 12 px is the literal on 33 rules and the token `--lc-text-ask`
  (an ask card's own text), a minority. That makes the Rust app's commonest text one pixel smaller than Electron's,
  the finding in the design review (1b).
- **TITLE: 13 px to 13.5 px**, still weight 600: a card's name sets no size of its own in Electron and takes the
  body's 13.5 px (shell.css:28; `.lc-nextstep__title` sets only weight, 500).

Where 12 to 13 px would wrap: these are estimates from the row 3 pictures (13/12 is 8% wider), not a trial. In
`settings` and `settings-light`, "A teammate's face says what it is doing, and its eyes say it too. This one shows each
in turn; click it for the next." now ends at 1776 of 2000 viewed px and the face beside it begins about 1888: at +8% it
ends near 1856, which leaves almost no gap and wraps at 1100 px or narrower. Every other line in `settings`, `home`
and the conversation scenes ends 100 px or more from its neighbour, so I expect them to hold; the sidebar rows already
truncate with "…" at 12 px ("Review the native pr…") and would cut a character or two earlier. `kit`'s "Everything Codec does
happens in ~/work/locust." already wraps and still would. Try it before applying, on the scenes above at 1200 and 1000 px.

## Row 5: a suggested task's words

Electron (`NextSteps.tsx`, shell.css:6961-7030): `.lc-nextstep__brief` is plain sans in `--lc-text-secondary`, `line-height: 1.45`,
`display: -webkit-box; -webkit-line-clamp: 2` (cut at two lines); `.lc-nextsteps__count` is `color: var(--lc-text-secondary)` and
nothing else, over the same words, "N suggested tasks" / "1 suggested task".

- `src/ui/next_steps.rs`: the brief was `l.ty.label` (Geist Mono 10.5 px, letters spread 0.14 em) in a wrapping text. It is now
  `BRIEF = type_::BODY.lh(13.0 * 1.45)` in the secondary colour with `.max_lines(2)` ("..." ends the second line).
  The count was the same spaced mono label in the muted colour. It is now `type_::BODY` in the secondary colour. The words
  "N suggested tasks" are Electron's already; what was off was the type, not the string, so that is what changed.
- Tests (`next_steps::tests`): shell.css's brief rule carries the secondary colour, 1.45 and a clamp of 2 and none of
  font-family, letter-spacing or uppercase; the drawn brief is `secondary` in colour, 13 x 1.45 px tall for one line, two
  lines tall at most for a 4-line brief; the count is in the secondary colour, on 13 x 1.55 px (the body's lines), and its
  rule in shell.css has no size, family or spacing of its own.
- Breaks (`row5.entries`): 7 of 7 KILLED. BRIEF back to the mono label, the clamp taken off, the clamp at 3, the line height
  at 1.25, the count back to the mono label, the brief's colour at muted, the count's colour at muted.
- Pictures: none. No scene draws a suggested task (the cards need a reply with a `<locust-next>` block and `--scene` has none
  with one; adding a scene is outside this lane's files), so the tests read the drawn elements instead.
- Call sites: `BRIEF` and `BRIEF_LINES` are used in `view` in the same file (next_steps.rs, the brief's `text(..)`), which
  `messages.rs:517` calls for every reply that carries suggested tasks.
