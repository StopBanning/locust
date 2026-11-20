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

# Round 2

## Part 1a: the live line says the start's phase (partial)
`thread_view::live_line_starting(.., starting_label)` puts the host's phase ("Reading the folder") in the place of
"Starting"; `live_line` is unchanged and calls it with `None`. Three `live_line_tests` carry the three Electron cases
(only the swept word, the label is `sweep_text`'d by the window). NOT done: nothing yet passes a phase from the host
into the ledger snapshot (`ledger.rs:1356` still calls `live_line`), and the "title for hover" / one-time-drawn markup
case is Electron's. Parity row left `waiting`; counts not yet recounted. Stopped at 93% of the 5-hour window.
Next: part 1b opens-the-plan, then orb, frame-pass.

## Part 1b-d (round 2)
- opens-the-plan: `LiveLine::plan` (steps `done`/`running`/`pending`, read like `readPlan`) is attached only when the
  line's detail is the plan's "step N of M". Only 1 of 5 cases named: the button, compact card, Escape and no-plan
  markup are Electron drawing.
- an-orb: `LiveLine::orb` (`tool_orb`, `register_orb`; `orbStateFor`) with all 7 cases named; the Plan-mode `solving`
  floor is in `register_orb` but `live_line` is not yet told the turn is in Plan mode.
- the-frame-pass: new `core/src/runtime_order.rs` (`runtime_list_order`, `keep_what_was_known`) plus the A1 counter
  cases; localeCompare is approximated by lower-cased comparison; nothing draws from it yet (partial).
- starts-phase: see Part 1a; row now partial 3 of 3 named.

## Part 2 (round 2, started)
- `core/src/thread_words.rs`: `thought_headline`, `plain_summary`, `helper_reported_back` (all cases of
  a-helpers-summary named; the drawer's helper row does not call them yet).
- a-long-call: the live line's label (the call's description) and clock are named in `live_line_tests`.
- NOT done: the steps fold (a new line per thought that says something, a wordless thought folding into its line;
  `buildThread`/`stepsLine`) does not exist in Rust yet, so a-wordless-thought and most of a-long-silent-stretch wait
  on it; needs a design in the `ledger.rs` item builder.

## Part 2 steps fold (round 2)
- `core/src/steps_fold.rs` `fold_steps(items, running)`: a message/user/handoff breaks the turn, a thought with words
  starts a line, a wordless thought and calls fold in; running calls are left out while running. Both cases of
  a-wordless-thought are named in `steps_fold_tests` (3 pass). NOT done: the drawer/thread does not call it yet, and
  `stepsLine`'s phrase counting (a-long-silent-stretch) is not ported.
- a-long-silent-stretch: `steps_fold::group_headline` (first thought headline is the whole line) and its two
  buildThread cases are named in `steps_fold_tests`; the headline case is in `thread_words_tests`. The rest of
  `stepsLine`'s counts phrases is still not ported.

## Part 3 (round 2, started)
- a-plan-shows-what-happened-to-it: all 4 cases named in `steps_fold_tests` (`touched_nothing`, `plan_done_count`,
  `read_plan` for Cursor's TODO_STATUS_ spelling). The drawer does not yet call `touched_nothing` to pick the plain list.

# Round 3

## Part 1 (partial): plan mode and the start's phase reach the live line
- `thread_view::live_line_in(.., starting_label, plan_mode)`; `live_line_starting` and `live_line` call it. The orb's
  `solving` floor now holds when `Metadata.mode == "plan"` (test `a_turn_in_plan_mode_holds_the_solving_orb_with_no_call_open`).
- `Transcript::starting_label` + `set_starting_label` re-draw the snapshot's live line (test
  `the_hosts_phase_reaches_the_snapshots_live_line`). NOT done: nothing in the host calls `set_starting_label` yet, so the
  starts-phase row stays `partial`.
- NOT started: steps fold wiring, `stepsLine` phrase counting, frame-pass drawing, a-long-call's last case, parts 2 and 3.
- Not run: the full gates, recount, case-map and break tests.

## Part 1 (partial): the steps fold is wired into the thread
- `messages.rs` splits each run of thoughts and calls through `steps_fold::fold_steps(run, turn_running)` and draws one
  `activity_group` per line; a group's headline is `group_headline` (first thought's wins), not the last as before.
- Test: `messages::tests::the_thread_draws_one_steps_line_per_thought_that_says_something`. Break (one group per run) →
  that test fails; restored.
- Full `nextest run --workspace`: 4,500 passed; `permission_bridge::…refuses_after_end` failed under load (60 s) and
  passed alone in 0.13 s, the same engine flake as round 1.
- NOT done: `stepsLine` phrase counting, so a line's words are still the old summary; the drawers' wordless-thought rows
  are not yet checked against Electron's detail list; the rows `a-wordless-thought…` and `a-long-silent-stretch…` stay
  `partial`. Frame-pass drawing, a-long-call's last case, parts 2 and 3, release gates, recount: not done.

## Part 1: `stepsLine` phrase counting (`core/src/steps_line.rs`)
- `steps_line(items, finished, workspace, open)` ports `stepsLine` whole: phrases by kind in first-seen order, the
  headline as the whole line, a lone thought's preview, "Failed to …" for a lone titled failure, the amber segments
  (failed, sandbox, working, did not report, refused, declined), `shorterForms` and the hover title. Its own
  `command_head` skips `cd x &&` as Electron's does (`thread_view::command_head` does not; left alone).
- Reading Electron's detail off a ledger item: the ledger keeps no separate title (`summary` is the title when the
  runtime sent one, else the command), so a shell call has a title only when its summary is not its command.
- `activity_group` now draws the line's sentence (closed or open), the file counts after it, and the other segments
  under it in amber; the old `activity_summary` + `commands_run_text` line is gone from the drawer. Two old tests that
  asserted the old words now assert Electron's (`activity_group_stays_folded…`, `a_step_that_read_a_picture…`).
- Tests: 13 in `steps_line_tests` (one per case of a-turns-steps-read-like-claude-code), 4 `from_events` tests (events →
  ledger → fold → line), `a_codex_thought_carries_the_summary…` (the Codex adapter), and the window test
  `messages::tests::the_thread_draws_a_wordless_stretch_as_one_line_of_its_moves`.
- Breaks, all caught: headline dropped → `from_events::a_silent_stretch_starts_a_line…`; mixed kinds named →
  7 tests incl. `counts_reads_and_commands…`; `cd` hop removed → `reads_a_command_that_only_looks…`; drawer back on
  `activity_summary` → `activity_group_stays_folded…` and three headline tests.
- Rows: a-long-silent-stretch and a-wordless-thought carried; a-turns-steps-read-like-claude-code partial (13 of 13
  named; the window does not yet pick a shorter form for a narrow row, nor draw the hover title).
- `nextest run --workspace --no-fail-fast`: 4,521 passed.

## Part 1: the start's phase reaches the window
- The Rust host has no phase messages: `native::send` does Electron's looking/briefing/reading-folder work before it
  returns, while the window has no ledger yet, and returns once the ledger exists and the runtime is being launched.
  So the phase from then until the runtime's first word is `starting-runtime` (what `carriedStartPhase` keeps). The
  ledger snapshot sets it on the tip's transcript (`thread_view::launched_phase_label`: "Starting Codex", None for a
  runtime Locust does not name). No engine file was changed. First-setup (OpenCode) and cloud phases are not sent.
- The live line's label carries its headline as its hover (`.help(words)`), as `lc-livestep__label`'s title does.
- Tests: `ledger::tests::a_launched_run_that_has_said_nothing_yet_reads_as_starting_its_runtime`,
  `messages::tests::the_live_line_leads_with_the_starts_phase_and_holds_it_as_its_hover_title`. Breaks: no
  `set_starting_label` in the snapshot → the ledger test; empty hover → the messages test. Row carried.
- `nextest run --workspace --no-fail-fast`: 4,523 passed.

## Part 1: the last long-call case, and the frame pass (not drawn: another lane's file)
- `live_line_tests::a_restated_call_is_one_row_now_carrying_the_command_and_what_it_is_for`: Claude Code's restated
  `tool.started` folds into one unsettled row with `npm test` and "Run the test suite", and the steps line says the
  title. Break (the ledger ignores a later title) → that test fails (and `ledger::tests::tool_keeps_title…`). Row carried.
- Frame pass: not drawn at the time (another lane's file); drawn 19 Nov, see "The frame pass, drawn" below.

## Part 2: plans in the thread (Mac chat took over, 18 Nov night)
- `activity_drawers::plan_answer`: a plan in a turn that touched nothing (`steps_fold::touched_nothing` over the turn's
  items) draws as an ordered list beside the face, numbered, no PLAN label, no counter, the ordinal gutter widened past
  nine steps; nothing for no steps. A plan in a turn that did work keeps `plan_card`. The list sits one level inside a
  wrapper because the thread keys every item it draws.
- Tests: `messages::tests::a_turn_that_only_planned_draws_its_plan_as_a_numbered_answer_with_no_header`,
  `a_planned_answer_widens_its_gutter_past_nine_steps`, `a_plan_with_no_steps_draws_nothing_answer_or_card`,
  `a_plan_in_a_turn_that_did_work_keeps_its_label_counter_and_markers`.
- Breaks: gutter at ten -> the gutter test; every plan an answer -> the did-work test; the empty guard survives
  (an empty list draws nothing either way, an equivalent change). Rows plan-answer and a-plan-shows carried.

### Part 2: a large diff says the right counts (19 Nov, picked up by the Mac chat)
- A file row said the cut excerpt's counts (+0 -747 for a 1,700-line rewrite) and then "runtime reported +a -d".
  Now Electron's `fileRowCounts` (`core/src/activity_entries.rs` `FileEntry::row_counts`, new `truncated` field):
  the tool's record, else none when cut, else the drawn lines. The card's header adds up the rows' own counts, or
  says none when any row has none (`header_counts`, ActivityCard.tsx). Counts read as `changes::line_counts`
  (Electron's LineCounts: no zero side, U+2212 minus), and "large" is lowercase, as on Electron's file row.
- Tests: `activity_drawers::tests::never_says_0_with_a_deletion_count_when_lines_were_added_in_a_recorded_large_rewrite`,
  `shows_no_counts_and_lets_large_say_it_when_true_counts_cannot_be_known`, `leaves_a_small_diffs_counts_unchanged`,
  and `activity_entries::tests::a_cut_patch_of_several_files_says_no_counts_and_an_uncut_one_says_each_files`; three
  older drawer tests now read the U+2212 minus. Breaks 5 of 5 killed.
- a-step-keeps-its-own-change: Rust already matched; Electron's events replayed in `disk_observation_tests`
  (`keep_each_step_its_own_diff_after_the_turn_has_ended_too`, `list_each_file_once_in_the_turns_files_as_its_net_change`).
  Breaks on the ledger's no-diff guard and on `net_entries` both killed.
- a-new-document-reads-as-one: Rust drew a new Markdown file as a green diff of raw Markdown. Now
  `core/src/changes.rs` `new_document` (Markdown, Table, Page, as DocPreview's isNewDocument) and `document_text_of`,
  and `activity_drawers::doc_preview` draws a new Markdown file through the reply renderer, with "Open {name}" when
  the conversation has a folder, "Show the change" and back, and "Only its start was recorded here." when cut. A new
  table or page still opens as its diff (no sheet or page frame in the drawer yet). Breaks 5 of 5 killed.

### The turn foot (19 Nov)
- Rust drew no foot: a finished turn ended in its step groups and a totals line. Now `core/src/turn_foot.rs` (Thread.tsx
  `TurnFoot`, `filesHeading`; missionView `pathInWorkspace`): the turn's edits that did not fail, only those in the
  folder unless every one is outside, one row a file (net entries, then files named with no diff, one row each, a
  file the host could not read folded into its step's row), counted as edited (runtime named it) or seen ("observed
  on disk"). `activity_drawers::turn_foot_card` draws it above the totals, open by default; the totals drop their
  files segment under it; a turn still going has none (`messages.rs` Trace arm).
- Tests: `turn_foot_tests` (core, 8), `messages::turn_foot_tests` (window, 3). `window_drive_tests::
  a_failing_check_shows_its_card_and_sends_back_to_fix` now reads the thread a screen at a time (the open feet made it
  taller than top + bottom).
- Breaks: 7 of 7 killed (folder filter, observed status, the 0.672 fold, relative paths, no foot while going, totals drop the files segment, open by default).

### The command row (19 Nov)
- Rust drew a shell row as one muted line (`ls · exit 0`) that opened onto the command it already showed, even when
  nothing was behind it. Now `activity_drawers::shell_heading` (ActivityCard.tsx `lc-filerow is-shell`,
  `shellResultClass`): the command first in the code face, then the verdict in a badge (green tint for a clean run,
  red for a failure, amber for stalled or refused, quiet while running), then the quiet words ("no output", the time).
  The row opens only onto output, or onto the command when the row led with a description or showed only the first
  of several lines (`shell_command_folded`; that last case is Rust's, since its row draws one line). "no output" still
  needs the runtime to have reported output.
- Tests: the 5 Electron cases plus `a_described_or_multi_line_command_opens_onto_the_command`. Breaks 7 of 7 killed.
- One left edge: a command that opens and a file row both start their words 20 pt in, after the chevron; a command
  with nothing behind it now keeps a 12 pt space where the chevron would be, so it starts there too. Tests
  `puts_the_command_before_its_badge_and_the_path_before_done`, `keeps_a_failures_wording_where_the_row_ends`;
  breaks 3 of 3 killed.

### Verbose (19 Nov)
- Rust had no Normal/Verbose. Now `WorkspaceSettings.transcript_view` ("transcriptView", `normal` unless `verbose`),
  ⌘O flips and saves it (`settings_panel::toggle_transcript_view`; the shortcut row is now native), and
  `activity_drawers::verbose` makes every step group, row, file and step change start open unless the person toggled
  it. Missing: Electron's Verbose/Normal item in the thread header's … menu (Rust's header has no … menu).
- Tests: `verbose_opens_every_step_of_a_long_transcript` (220 steps), `verbose_switches_for_this_moment_when_there_is_no_store`,
  `window_drive_tests::command_o_switches_normal_and_verbose_steps_and_remembers_the_choice`, and core's
  `reads_normal_until_verbose_was_chosen_and_ignores_anything_else`, `stays_usable_when_storage_refuses`. Breaks 4 of 4 killed.

## 19 Nov (later): Undo on the turn foot, the header ⋯

- **The turn foot's Undo** (`src/ui/turn_undo.rs`, TurnUndo.tsx): under the files card, "Undo these changes", asked first in Electron's words, run off the window's thread through `changes_panel::undo_turn_in`, then "Undone: N files put back…" with each file left alone and why. `checkpoints::undo_state` answers in Electron's order (undone, shared, ready, none); the latest turn is asked again every 750 ms up to 20 times while its record is not written yet. The heading adds "· undone" only when Undo put a file back.
- **A resumed file card** 5 of 5: a stopped Write says "not confirmed", never "did not report".
- **The header ⋯** (`src/ui/header_menu.rs`): Verbose/Normal, Send feedback (the turns as feedback.ts writes them; a failure's words from the turn's error note; a stopped turn's "(the turn was stopped)" is not yet said, since the window's items do not keep the run's ending), Delete conversation once nothing runs. Electron's other entries (Open in terminal, Ask on the side, Cloud tasks, Save as routine, the review ask) stay where Rust draws them, as header icons, or are not built yet.

## 19 Nov: the frame pass, drawn (`src/ui/agents_section.rs`, freed for this lane)
- Settings › AI agents now draws its rows through core's `runtime_list_order` (Screens.tsx), with the tag of each row
  from `agents::row_status`; the section's own copy of the band rule is gone. Nothing installed still puts OpenCode
  first, as Electron does.
- `poll` stores each discovery answer through `keep_what_was_known` against the list already held, an empty one before
  the first answer (App.tsx `keepWhatWasKnown(held, response.runtimes)`). A runtime whose answer is silence
  (`probe-failed`, `offline`; discovery's "still checking" placeholder `agents::pending_found` is `probe-failed`) keeps
  its earlier `Found` whole; an answer without a version keeps the version printed before; every definite answer wins.
  `Probe` is read as PublicRuntimeStatus's status word by `status_word`.
- Rust tags Antigravity READY when it goes through its own CLI, so the order test uses it through the app
  (EXPERIMENTAL), the band the Electron case is about.
- Tests (`agents_section::tests`): `the_window_draws_what_can_be_used_first_and_what_does_not_exist_last`,
  `the_window_draws_one_band_by_name_whatever_order_discovery_answered_in`,
  `a_recheck_still_out_keeps_the_row_it_had_and_a_version_not_printed_again`,
  `a_recheck_draws_every_definite_answer_at_once_and_rows_it_never_saw`. Each reads the drawn rows (top to bottom, and
  each row's own words); a re-check lands through the real `agents_job` and `poll`.
- Breaks 6 of 6 killed: no keep (`let kept = found`), silence not held, version not kept, discovery order, every tag
  READY, every row held.
- localeCompare is still approximated by lower-cased comparison. Row carried.

## Page round 1 (19 Nov, `exec/native-page-1`)
Groundwork for the design plan: a page a teammate makes runs beside the conversation, and the person can point at a part
of it. Codex started this round and left nothing behind; it was done again from the start.

**A page that runs** (44851679)
- The turn's files card opens a new web page wherever it sits, and only it (`activity_entries::new_page_entry`,
  ActivityCard's `newPageEntry`). Before, each row went through `default_open` as a one-row list, so every small file's
  diff opened; a changed page and any other new file now stay folded, as in Electron.
- An opened new page is `page_preview` in `src/ui/activity_drawers.rs`: "index.html, running", what it cannot reach, its
  Page (runs it) and "Show the change". The file viewer gives an .html file Page and Source. Electron frames the page
  inside the card and the viewer; this window has no frame inside a pane, so Page runs it in the browser panel at its own
  `locust-page` address, and the panel opens on it.
- Found and fixed: `Host::navigate` refuses the `locust-page` scheme (it takes what a person types), and both the
  reply page's "Open the page" and a compare column's page passed it one, so neither ever loaded. `Host::open_page`
  serves the page through `page_preview` and loads it; both now use it.
- **Still broken, not fixed here:** `page_preview::read` serves a page only while the open conversation's folder holds
  it. A reply page is kept under the app's data folder, so once loaded it is answered 403. The fix is a decision about
  that guard: let a root granted for Locust's own pages folder be served whatever conversation is open.
- The page draws twice in a turn that wrote it: in the step's own card (open by default for a new page) and in the
  files card. Electron's step group is folded unless verbose; why Rust's is open was not looked into.
- The pages a browser runs are whole: the phone page both ways and its service worker parse with node, and
  `deviceName` names the six browsers. The desk page (the way into Locust in a browser) is not built, so the row is
  partial.
- The comparison-cell case is not named: the compare view draws no turn files card, so there is nothing to fold.

**What a browser last saw** (not built)
- `a-browser-opens-on-what-it-last-saw` is `desk-snapshot.ts`, the sealed cache inside the Locust-in-a-browser desk
  page, not the browser panel reopening on its last page, as the hand-off read it. Rust has no desk page or DeskHost, so
  a port would have nowhere to run. The row stays waiting, with that note.

**Pointing** (83403adb)
- `core/src/browser_point.rs` carries shared/browser-point.ts and the decisions in pointInBrowser. Two promises, each
  with its own named test:
  - *Form values never come along.* `BrowserPoint`'s fields are private, and the only way to make one is
    `read_browser_point`, which scrubs (`omit_form_values`). In Electron a point is a plain object, so its own chip test
    builds one holding `value="s3cret-pass"`, and that value would reach the block. Here it cannot.
    Test: `form_values_never_appear_in_a_pick`.
  - *A password or card field is never pictured.* `picture_rect` gives no rect for a part with a picture note.
    `point_answer` asks the camera only where it gives one, and the WKWebView host photographs only that rect.
    Tests: `a_password_field_is_not_pictured_with_its_contents`, `a_card_or_one_time_code_field_is_never_pictured_either`.
- The picker is Electron's script (`core/src/browser_point.js`, template escapes undone). `browser_mac::point` runs the
  probe, then the picker, through `callAsyncJavaScript`, which awaits the picker's promise (`Command::Evaluate`'s
  `evaluateJavaScript` would not). They run in their own content world, `locust-native-point`. Then it takes a
  `takeSnapshotWithConfiguration` of the rect.
  - **Not run on a live page:** no test opens a WKWebView. The snapshot rect assumes page zoom 1.
  - In that world React and Vue expandos are not seen, so a part's source comes only from `data-*` attributes.
- `src/ui/page_points_ui.rs`:
  - The point button sits beside Go in the browser panel. Its name is what it would do, or why it cannot. It tells
    its shortcut, Control+Shift+C; ⌘⇧C and Ctrl+Shift+C both work here.
  - A page that will not run the picker is remembered, and its button says so. A pick left 5 minutes is let go.
  - Chips sit in the chat box: place, source, styles, picture note, a note field and ×. Removing a chip deletes its
    picture.
  - Send puts the blocks after the person's words and attaches the pictures. Pictures are kept under the data folder's
    `page-parts`.
  - The chip shows "Picture", not a thumbnail.
- Rows: the chat-box test carried; the browser-pointing test partial, its 10 cases named, with the live pick noted;
  the viewer-frame test does-not-apply with a true note; PARITY area 8's row partial.

**Breaks** (`~/work/scratch/page1/b01..b19`, one per run; all killed in the end)
- b01 new_page_entry takes any new file: `opens_the_page_even_when_another_file_came_first_and_only_the_page` (core).
  It first survived the core-only run, so the core test was added.
- b02 open_page loads nothing: `opens_a_page_locust_runs_at_its_own_address_which_an_address_typed_in_may_not_name`.
- b03 Edge named Chrome: `names_a_computers_browser_by_its_name_and_system_as_settings_lists_it`.
- b04 the service worker does not parse: `parses_as_the_browser_receives_it_…`.
- b05 a new page falls to its diff: `opens_a_new_web_page_so_it_runs` and two more.
- b06 every row opens again: `leaves_a_changed_page_and_any_other_new_file_folded_as_before`, plus one more. It
  survived at first: diffs are drawn token by token, so checking for "export const x" proved nothing. The tests now
  read the diff's state line.
- b07 the viewer gives a page no Page: `runs_a_page_in_a_sandboxed_frame_at_its_own_address_with_page_and_source`.
- b08 the panel stays shut: `runs_the_new_page_beside_the_conversation_…`. It survived at first, because adding a tab
  opens the panel; the test now starts with a tab and the panel shut.
- b09 and b12 values and textarea words kept: `form_values_never_appear_in_a_pick`.
- b10 the guard in picture_rect gone, and b13 the camera asked for any part: the password and card tests.
- b11 CVV no longer a card field: `a_card_or_one_time_code_field_is_never_pictured_either`.
- b14 a blocked page still pointable: `takes_ctrl_or_cmd_with_shift_and_c_and_says_so_on_the_button`.
- b15 pictures not attached: `sends_each_part_after_the_words_…`.
- b16 shortcut not told: `a_blocked_page_says_it_does_not_allow_pointing`.
- b17 removed picture kept: `removing_a_part_takes_its_picture_off_the_message_and_the_disk`.
- b18 picture not written: `points_at_the_open_page_when_pressed_…`.
- b19 Send leaves the parts out: `a_pointed_part_goes_with_the_next_message_after_the_words_and_leaves_the_box`
  (window_drive_tests). It survived at first, and that test was added for it. In that run
  `compare_of_two_models_shows_two_columns_each_answering` also failed; it passed in the full gate.

**Gates:** nextest workspace 4771 of 4771 passed; `test --release --workspace` 1115 + 3656 passed; `build --release`
finished.

**After the usage limit lifted (same day)**
- A message queued behind a running turn now carries the parts pointed at (`page_points_ui::with_points`, then `sent`
  once queued). Before, Enter with only parts queued an empty message and left them behind. A second message refused
  by a full queue keeps its parts. Test: `a_part_waits_with_a_message_queued_behind_a_running_turn_…`.
- `capture_rect_of` is tested with Electron's numbers (`is_the_part_inside_the_frame_in_the_windows_pixels_at_its_zoom`,
  `is_cut_to_what_the_frame_shows_and_is_none_when_nothing_of_it_is`), so the viewer-frame row is partial, 2 of 8.
- The press test now presses the files card's Page, not the step card's (the page draws in both).
- The chat-box row is partial, not carried: the chip says "Picture" where Electron draws the picture, and the live
  pick, which is what finds a blocked page, has not been run.
- The thread beside the browser (row 557), drawn at 418px against full width, 3 of 6:
  - the live line was one text, cut through "step 2 of"; its words now give way and the step and time stay whole;
  - the turn's closing line never wrapped; it now wraps by whole parts;
  - the handed file's pill now gives way inside the reply, Copy and Save a copy stay whole, and the next file wraps.
    The path keeps 96px (`PATH_KEPT`) while the note is cut. Rune has no shrink weight for Electron's
    `flex: 0 1000 auto`, and the note is not dropped in a narrow reply.
  - Not named: the file row (Rust's has no open and copy buttons beside it), the workroom header and Home's account
    marks (not found in the Rust window).
  - The group key is `handed-group-…`, because the handed-file tests count keys starting `handed-file-`.
- Breaks, all killed:
  - b20 the queue drops the parts: `a_part_waits_with_a_message_queued_behind_…`;
  - b21 zoom dropped: `is_the_part_inside_the_frame_in_the_windows_pixels_at_its_zoom`;
  - b22 the pill cannot shrink, and b23 the path has no floor: `fits_a_handed_files_pill_to_the_reply_…`;
  - b24 the live words cannot give way: `keeps_the_plan_pill_of_the_running_line_whole_…`;
  - b25 the closing line never wraps: `wraps_a_turns_closing_line_by_whole_parts_…`.

**Left:**
- the reply-page read guard above;
- a live run of pointing on a real page, and the chip's picture thumbnail;
- the desk page and its snapshot;
- the page drawn twice in one turn;
- row 557's three unnamed cases.
