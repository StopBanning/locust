# A reply and its steps read as Electron's (18 Nov 2026)

Lane: `~/work/locust-wt/opus2`, branch `exec/native-reply`, from 732e9fe7. The job is the 34 waiting Electron test
files about how a turn reads: the reply's text and Markdown, its activity drawers, and the live line.

## Part 1a: Markdown in a reply (five small files)

**How it is checked.** Electron tests its own parser (`agentText.ts` `parseAgentText`). The window does not have that
parser: it hands the reply to rune-kit's Markdown (pulldown-cmark, CommonMark). So the Rust tests put Electron's text
through what the window really draws with:
- `markdown_prep::prepare_reply` (new, `src/ui/markdown_prep.rs`), which holds the steps `reply_markdown` ran inline
  before (diagram, undrawn markers, math), plus the new rules below;
- then `rune_kit::markdown::parse`;
- and, for what a person sees (list markers, quotes, no `](`, no TeX), `reply_markdown` drawn headless through
  `rune::Runtime`.

`reply_actions.rs` changed in two lines only: it calls `prepare_reply`, and it sets `link_help` to `link_host`.

**What CommonMark already did as Electron does** (tests only):
- a wrapped bullet stays one bullet;
- nesting by 2, 4 or tab indents, and back out;
- blank-line numbered steps are one list, drawn 1. 2. 3.;
- numbering resumes at 3 after a code block (rune-kit draws `start + k`);
- a link inside emphasis, a code span, and `snake_case`.

**Where it differed, and the fix:**
- **Prose at the left margin straight after a bullet** went INTO the bullet: CommonMark's lazy continuation.
  Electron ends the list there. `end_lists_at_margin_prose` puts a blank line before such a line, outside code
  fences.
- **A displayed equation over several lines** (`\[` on a line of its own, closed lines later; Codex writes these in
  quotes) was never drawn. In a quote, its `=` line even turned the line above into a heading.
  `math_tex::take_display_block` gathers it the way `agentText.ts`'s math block does, taking the quote markers off
  each line.
- **An equation inside a quote or a list item** was drawn with blank lines around it, which ended the quote or the
  item. On a prefixed line (`> ` or an indent), the picture now stays on its line (`push_math`'s `pad`).
- **Words that look like HTML were dropped.** rune-kit draws nothing for HTML: `<!-- big -->` (changelog 0.845.0),
  `Switch to <model>` (0.479.0), a `<div>` line. Electron draws a reply as text, never as markup.
  `keep_html_as_words` escapes each tag-opening `<` outside code. Autolinks stay links.
- **`linkHost`** is new (`markdown_prep::link_host`), and reply links show it as their tooltip (`link_help`).

**Breaks, each caught by a named test** (run through `breaks_core.run` on `locust-native`, filtered to
`markdown_reply_tests`):

| Break | Caught by |
|---|---|
| margin prose not split from the list | `still_ends_the_list_at_unindented_prose_which_a_model_usually_means_as_prose`, `a_nested_list_drops_nothing_at_any_depth` |
| tag `<` not escaped | `a_line_that_opens_like_markup_is_drawn_as_words_not_dropped`, `drops_no_words_from_any_entry_in_the_changelog` |
| multi-line display block never gathered | `a_displayed_equation_inside_a_quote_is_drawn_and_none_of_its_tex_is_left_showing` |
| equation always padded with blank lines | `an_equation_written_in_the_middle_of_a_line_is_drawn_too`, `a_displayed_equation_inside_a_quote_…` |
| `www.` kept in the link host | `names_where_a_link_actually_goes_so_a_label_cannot_lie_about_it` |
| `prepare_reply` skips both new rules | four tests, listed in the commit |

**Left:**
- The link tooltip itself is not seen headless, so `links-go-somewhere` stays partial. `isOutboundLink` is Electron's
  host allowlist for its own IPC and does not apply.

Suite after part 1a: `nextest run --workspace` passed 4,294 tests, 22 skipped.

## Part 1b: `agentText.test.ts` (34 cases)

Most of it was already CommonMark's behaviour (fences, CRLF, tables, rules, quotes, emphasis rules, `mcp__` names).
Each case has its own test anyway. Where it differed:
- **`1.` then `2)`** made two lists. `markdown_prep::electron_line_shapes` writes `N)` as `N.`, so it is one list.
- **`#####`** was drawn at rune-kit's fourth, muted size. Electron draws anything deeper than three as the third.
- **A lone `#`** was an empty heading; now it is the word `#`.
- **`$\rightarrow$`** was drawn as a math picture. Electron writes a lone macro from `TEX_SYMBOLS` as its character
  ("README.md → docs/README.md"). `math_tex::TEX_SYMBOLS` holds the same 77 entries, read from `agentText.ts`.
- **Math inside a code span** (`` `$\to$` ``) was rewritten into an image inside the code. `rewrite_inline` now
  skips code spans.

Breaks: `)` kept → `reads_a_numbered_list_as_ordered`; deep heading kept →
`lifts_a_hash_heading_out_of_the_prose_at_its_level`; lone `#` kept →
`does_not_mistake_a_hashtag_or_a_shell_comment_for_a_heading`; lone macro never matched →
`a_lone_tex_macro_is_drawn_as_its_symbol_in_the_words_around_it`; code spans not skipped →
`leaves_money_and_code_as_they_were_written_and_a_whole_formula_is_math` (and the changelog coverage test).

Suite after 1b: 4,328 passed, 22 skipped.

## Part 2a: tool calls, status chips, colour swatches

All three are drawn inside the reply's Markdown, where rune-kit gives one hook per link: `link_style`. So the reply's
words are rewritten into links on two private schemes, and `reply_link_look` paints them:
- **`locust-status:<tone>`**: a table's Status/State/Stage/Progress cell whose value `statusToneOf` knows. It is drawn
  in the tone's `-text` and `-tint`, the colours `.lc-status--*` uses.
- **`locust-swatch:<rrggbb>`**: a `■` in that colour, set before the colour code.
- `reply_actions`'s `on_link` ignores both schemes, so neither is ever opened.
- **`<tool_call>…</tool_call>`** is fenced as ```` ```tool call ```` first, before anything else reads the reply.

Breaks: tool calls not fenced → both tool-call tests; status never marked →
`statuses_draw_as_chips_in_a_reply_and_other_columns_and_unknown_values_are_left_alone`; inline-code swatch never
added → `a_colour_code_shows_its_colour_beside_it`, `a_colour_shows_inside_bold_and_italic_…`; chip painted in its ink
instead of its tint → the chips test; `#rrggbb` accepting longer → `accepts_rrggbb_only_…`.

**Left:**
- The 0.733 chip is a button whose menu drafts `statusAsk`'s words into the message box. Here a chip is drawn, not
  pressable; the box is the composer lane's.
- A swatch is a `■` character, so it is copied with the text.

Suite after 2a: 4,338 passed, 22 skipped.

## Fix: HTML escaping ran before the math (found in review)

`keep_html_as_words` ran before `prepare_math_markdown`, so `$0<x<1$` became `$0\<x\<1$` and was no longer drawn.
It runs last now, once math has been read. Test: `an_equation_with_a_less_than_sign_is_still_drawn`. A break that
puts the old order back is caught by it.

## Gates (18 Nov, after 2a and this fix)
- `nextest run --workspace`: passed (4,338, 22 skipped).
- `test --release --workspace`: the first run failed one ENGINE test,
  `permission_host_tests::leaves_an_answered_card_alone_when_its_connection_closes_after_the_answer` ("no card was
  raised"). It passed alone in 0.12 s, and this lane changes nothing in `core/`, so it looks like a timing flake under
  load. A rerun with `--no-fail-fast` passed: 3,394 engine tests and 945 window tests.
- `build --release`: passed. No new warnings from this lane's files (the unused `reply_actions::reply` was already
  there).
- The thread draws a reply through `messages.rs` → `reply_actions::reply_with_copy` → `reply_markdown` →
  `markdown_prep::prepare_reply`, so these rules are what the window draws.

## Part 2b: the rest of the reply, triaged (no code)

| File | Row | Why |
|---|---|---|
| settled-text | partial | A live reply is drawn by rune-kit's `Markdown::streaming` (`stream.rs`), which parses the tail as what it will become, so no block changes kind. Not Electron's `splitSettled`, and not yet tested over Electron's stages. |
| the-live-word-is-visibly-live | waiting | The sweep over the live word and the larger orb are Electron CSS; nothing like them is drawn. |
| cold-start-is-not-an-alert | waiting | `native.rs` knows a turn started cold, but the window is never told. It needs a flag on the turn and the note under its time marker. |
| an-other-app-origin-survives-a-restart | partial | `startedBy {kind: mcp}` is written and the backup exclusions hold. Missing: `via`, schema-22 refusal (outside this lane's `ledger.rs` scope), and the "Started from another app · …" line. |
| a-lead-shows-what-it-handed-out | waiting | There is no Handed-out card. |

## Part 3a: the live line names the plan step and the call, in words

`core/src/thread_view.rs`:
- `live_line` follows `plan.updated` now. Its `detail` is "step 2 of 3" when no connector names the call.
- New field `LiveLine::action` holds Electron's `live-step` action. It is the open call in words (`live_action`, a
  partial `liveActionLine`), or else the plan's step under way, with its own full stop dropped.
- `sweep_text` is Electron's live word: capitalised, one ellipsis.

`messages.rs` draws the action when there is one ("Running npm test · step 2 of 3 · 15s"). Otherwise it draws the
swept word ("Starting…"), or "Using a tool · …" as before.

`live_action` carries only the `liveActionLine` branches these cases name: a title, a shell command that reads,
searches, lists or runs, and writing tools. Electron's helper, skill, reading-tool, web and edit-entry branches are
not built; for those the line falls back to the call as before.

Breaks: step number off by one → three `live_line_tests`; full stop kept → `drops_the_steps_own_full_stop_…`;
"Reading" changed → `leads_with_what_the_open_call_is_doing_in_words`; ellipsis dropped → the three `the_live_word_…`
tests; the window drawing the label instead of the action →
`messages::tests::the_live_line_draws_what_the_call_is_doing_and_which_plan_step`.

Suite: 4,349 passed, 22 skipped.

Final gates at 9cdb2375: `nextest run --workspace` passed (4,349, 22 skipped); `test --release --workspace` passed
(3,403 engine, 946 window); `build --release` passed.

## Not started (left `waiting`, each with its triage note)
- Part 3: the-live-line-says-the-starts-phase, the-live-line-opens-the-plan, an-orb-needs-a-runtime-that-streams,
  the-frame-pass.
- Part 4 (all 14): a-long-call-is-named-while-it-runs, a-long-silent-stretch-reads-as-its-moves,
  a-helpers-summary-reads-as-words, a-wordless-thought-does-not-start-a-line, a-plan-shows-what-happened-to-it,
  plan-answer, a-step-keeps-its-own-change, a-large-diff-says-the-right-counts, a-new-document-reads-as-one,
  a-resumed-file-card-matches-the-folder, command-output, commands-and-paths-share-one-left-edge,
  a-verbose-transcript-opens-every-step, transcriptView.
- Hint for the next lane: `a-long-call-is-named-while-it-runs` and `a-long-silent-stretch-reads-as-its-moves` need
  more of `liveActionLine` / `stepsLine` (missionView.ts 2137-2300). `thread_view::live_action`, `command_looks_at`,
  `command_target` and `command_pattern` are the start of that port.

## Not mine
`src/ui/routine_recovery_tests.rs` was untracked in this worktree before this lane started (17 Nov 23:58), and no
`mod` names it. It is left as found.
