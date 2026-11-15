# Renderer test port — rounds 1 and 2

Branch: `exec/native-thread`. Started 2026-11-15 from a clean tracked tree.
The requested scope is the waiting renderer files containing `team` (first), then
`conversation`. No handoff file is part of the change.

## First section — carried (11 cases)

- `a-decision-without-a-teammate-names-nobody.test.tsx`: two separate Rust tests
  in `src/ui/decision_card.rs`, with the exact anonymous and Wren title strings.
  The existing headless decision-card drive also exercises the assembled card.
- `a-new-teammate-suggests-a-free-name.test.ts`: three name-default cases in
  `core/src/people.rs`, the absent route wording case and Share window wording
  case in `src/ui/people_panel.rs` (five cases total).
- `a-new-teammate-wears-a-new-colour.test.tsx`: one pure colour-order test and
  three headless editor cases, preserving Electron's values (four cases total).
- Native changes: suggest the first unused example name as the name placeholder;
  start a new editor on its first unused hue (cycling after all eleven), keep
  an edited hue, and offer hue selection. Show a duplicate-name hint and disable
  both save actions immediately; retain core validation as the persistence guard.
  Say how to add the shared team image in the Share window.
- Native uses its existing typed editor and host validation, with pure defaults
  in core, rather than recreating React state/DOM checks. No case excluded yet.

## Verification

First full release run (`notes/renderer-1-workspace.log`): all eleven new tests
passed by name. Core: 2061 passed, 1 failed, 20 ignored; window: 447 passed,
1 failed. Relay roundtrip exceeded its six-second wait. The existing own-copy
window test did not persist the switch; the added colour row changed that control's
position. Moving the picker below existing controls restored the test; it passed
in subsequent complete workspace runs.
The three rows are now carried after the named cases passed in the full green run.

## Remaining

All requested files after these first three remain. Recount passed; the release
build status is recorded below.
The outside-workspace `~/work/scratch/mut6.py` from the brief is not read or edited;
this turn is restricted to this workspace. Controls live under native's
`scripts/fixtures/`, restoring each changed byte before they exit.

## Named case mapping (first section)

| Electron case | Rust test (module suffix) |
| --- | --- |
| says a decision is needed, and no word for the machinery, when nobody asked by name | decision_card::renderer_tests::says_a_decision_is_needed_and_no_word_for_the_machinery_when_nobody_asked_by_name |
| names the teammate who asked | decision_card::renderer_tests::names_the_teammate_who_asked |
| is Wren on an empty team | people::renderer_defaults_tests::is_wren_on_an_empty_team |
| skips every name already on the team, whatever its case | people::renderer_defaults_tests::skips_every_name_already_on_the_team_whatever_its_case |
| falls back to Wren once every example is taken | people::renderer_defaults_tests::falls_back_to_wren_once_every_example_is_taken |
| is said the same way on Home and the Team screen | people_panel::renderer_defaults_tests::is_said_the_same_way_on_home_and_the_team_screen |
| names the way in, not a drop that does not exist | people_panel::renderer_defaults_tests::names_the_way_in_not_a_drop_that_does_not_exist |
| is the first one nobody wears, then round again | people::renderer_defaults_tests::is_the_first_one_nobody_wears_then_round_again |
| is where the dialog opens, and an edit keeps the colour it has | people_panel::renderer_defaults_tests::is_where_the_dialog_opens_and_an_edit_keeps_the_colour_it_has |
| says when it is taken, and will not save it | people_panel::renderer_defaults_tests::says_when_it_is_taken_and_will_not_save_it |
| says nothing about a name nobody else has | people_panel::renderer_defaults_tests::says_nothing_about_a_name_nobody_else_has |

### Intermediate verification record

- Colour-order negative control: `scripts/fixtures/renderer-mutation.py fresh-colour`
  exited 0, observed the named assertion fail, restored `core/src/people.rs` byte for
  byte. The mutant run had exactly that one failure (2061 core passed, 1 failed,
  20 ignored; all 448 window tests passed). Own-copy and relay roundtrip passed.
  The anonymous-title and free-name controls are provided but have not been run.
- First restored serial workspace run: all 11 new cases and all 448 window cases
  passed. Core: 2060 passed, 2 failed, 20 ignored. Failures:
  `native::tests::relay_prose_return_posts_unanswered_prose_back_to_asker`
  (six-second deadline) and
  `runtime::process::tests::end_terminal_tree_kills_descendants_and_reaps_root`
  (`descendant ... still alive`). Both had passed in the prior mutation run.
  A confirmation run on unchanged source is in `notes/renderer-1-confirm-workspace.log`.
- Unchanged-source confirmation: core 2062 passed, 20 ignored; window 447 passed,
  1 failed. The only failure was
  `window_drive_tests::review_changes_shows_a_teammates_branch_turn_by_turn_and_lands_it_as_one_commit`:
  Git refused to create the temporary repository's existing `index.lock`.
  Next full run (`notes/renderer-1-verified-workspace.log`) adds the process-local
  `GIT_OPTIONAL_LOCKS=0` to avoid optional read-side index refresh locks; required
  write locks remain. This is verification configuration, not a production fix.


## Final section verification

- Full release workspace: exit 0, **2062 core + 448 window = 2510 passed**,
  **20 ignored** real-agent cases; doc tests 0. All eleven cases above observed
  passing by name in `notes/renderer-1-verified-workspace.log`.
- Configuration: `RUST_TEST_THREADS=1`, `GIT_OPTIONAL_LOCKS=0`, and
  `LOCUST_TEST_TS_READER` pointing to `dist/thread-7-ts-reader/index.js` in this
  workspace. Earlier full-run failures are retained above, not counted as passes.
- Colour mutation caught and restored; the full restored run is green.
- Release binary build: exit 0 (`notes/renderer-1-release-build.log`), 12 warnings
  in unchanged drawing/conductor code, none in this section's changed files.
- `scripts/recount.py` and `scripts/recount.py --check` passed: 180 carried,
  62 does-not-apply, 185 partial, 882 waiting (1309 total).
- `git diff --check` passed. No mutation remains applied.
- No React/DOM-only exclusion was needed for this section. Native uses its own
  headless editor rather than constructing or asserting HTML markup.
- No live window, real agent, packaged app, or Electron/JavaScript gate was run.

## Continuation point

Only **3 of the 45 unique requested files** are carried in this section. The
remaining **25 teammate files and 17 additional conversation files** are still
waiting; next is `a-page-a-teammate-made-runs-in-its-turn.test.tsx`. Its inline
new-page preview behaviour is applicable and is not marked does-not-apply.
No work on those remaining files is claimed complete.

## Round 2 — handed files (section 1)

- `a-teammate-can-hand-you-a-file.test.ts`: all five cases carried separately
  in `messages::tests`, using Electron's reply, date, item and path values.
  The native tests apply a message event to the ledger and draw the real headless
  thread, including streaming and block-only replies. Existing native behavior
  sufficed; no production change in this section.
- Named mapping: `draws_the_file_as_its_own_item_with_the_path_and_the_note`,
  `shows_the_person_the_answer_and_not_the_protocol`,
  `still_draws_the_file_when_the_block_was_the_whole_reply`,
  `waits_for_the_message_to_finish_before_drawing_a_button`,
  `leaves_an_ordinary_reply_with_no_file_item`.
- `renderer-2-files-workspace.log`: full release workspace passed; all five
  observed passing by name. Uses the same serial, optional-lock and in-workspace
  TypeScript-reader configuration as round 1. Mutation verification follows.
- Deferred, still waiting: new-page-in-turn needs inline new-file preview wiring
  (native currently offers a separate comparison preview); pet sheet animation
  needs reconciling with native's buddy rig. These are applicable product behavior,
  not Electron-only exclusions. Other unported files remain waiting. Per the
  round-2 instruction to prefer small files, continue with smaller applicable cases.

## Round 2 — tagged message and long conversation (section 2, carried)

- `a-tagged-conversation-shows-the-message.test.ts`: title case in
  `conversation_turns::tests::a_tagged_conversation_is_titled_by_the_message`;
  bubble case in `messages::tests::draws_the_tagged_message_as_the_bubble_without_the_context_after_it`.
  Both use `Can you double-check the tax rate?`, Wren and `I changed TAX_RATE.`.
  Native now cuts the tag context for ordinary prompt bubbles and ordinary/tag
  prompt title projection. Other starter kinds retain their own briefing rules.
- `a-long-conversation-is-still-one-conversation.test.ts`: five separate cases
  in `sidebar::review_tests`: `finds_the_real_root_36_turns_back`,
  `collapses_every_turn_of_it_into_one_sidebar_row`,
  `does_not_care_how_long_the_conversation_gets`,
  `still_stops_on_a_cycle_rather_than_spinning`, and
  `stops_at_a_turn_that_is_missing_rather_than_guessing_past_it`.
  Uses mission_000 ids, Antigravity, Electron's dates and prompts, 36/33/65/200
  lengths and the missing mission_004 ancestor. Tests native's real sidebar
  grouping rather than reconstructing Electron's intermediate row map.
- All seven tests observed passing by name in `renderer-control-streaming-file.log`.
  That full run passed 2063 core tests (20 ignored) and failed only the deliberately
  broken streaming-file case in the window suite. The control caught that named
  assertion and restored messages.rs byte for byte. Restored verification follows.
- No exclusions. Both rows are carried after these named passes.

### Round 2 controls and continuation

- Streaming-file control: caught exactly
  `messages::tests::waits_for_the_message_to_finish_before_drawing_a_button`;
  2063 core passed, 458 window passed, one intentional window failure, 20 ignored.
  Source restored. Log: `renderer-control-streaming-file.log`.
- Tag-context control: caught the named title regression and its bubble regression;
  source restored. Log: `renderer-control-tag-context.log`. Final counts below.
- Strengthened the handed-file tests to count the actual keyed file controls,
  independently of prose mentioning the path. This closes a weak negative assertion
  for an ordinary reply; final verification below applies to these assertions too.
- `two-teammates-look-at-each-other.test.ts` remains waiting: core has
  `glances_among`, but the real screen does not call it. Testing the unused helper
  alone would not establish the product behavior. No does-not-apply classification.
- This round carries 3 additional files (12 cases); combined with round 1,
  **6 of 45** requested files are carried and **39 remain waiting**.
  The continuation is still the waiting teammate list before conversation-only
  files, starting with the applicable new-page preview. No other file is claimed
  carried. No handoff file was changed or committed.
- Tag control totals: 2062 core passed + one intentional title failure,
  458 window passed + one intentional bubble failure, 20 ignored. No other failure.
- First long-root control attempt was rejected by the control runner: a compile
  error in the strengthened test helper (capturing closure where Rune requires a
  function pointer), not a caught mutation. Fixed by carrying transcript items
  in the runtime's tuple state, as the existing headless helper does; rerun below.
- Long-root control: adding the old 32-hop cap failed the named
  `finds_the_real_root_36_turns_back` assertion and two other chain cases;
  sidebar.rs restored byte for byte. The run also exposed three helper-count
  failures: the `handed-file-` prefix included its Copy and Save controls too.
  The helper now excludes those auxiliary controls. This mixed run is not a
  green baseline; the subsequent restored run verifies the correction.


## Round 2 final restored verification

- `notes/renderer-2-restored-workspace.log`: exit 0; **2063 core + 459 window =
  2522 passed**, **20 ignored** real-agent cases; doc tests 0. All twelve new
  cases observed by name, with the final file-control counts. No mutation remains.
- `scripts/recount.py` and `scripts/recount.py --check` passed: **183 carried,
  62 does-not-apply, 185 partial, 879 waiting**, 1309 total. Three newly carried
  files this round, no exclusions.
- `git diff --check` passed. The native-only handoff workflow was used; no
  Electron/JavaScript gate, real agent, live window or packaged app was run.
- Commits so far: `df8bbccd` handed files; `6cb4e847` tagged and long conversations.
  Final verification/stronger assertions are committed separately.
- Release binary build: exit 0, `notes/renderer-2-release-build.log`, completed
  in 59.87s. Twelve existing warnings in drawing/conductor code; none in changed
  production files. All mutations restored; final tracked tree committed clean.

## Round 2 continuation — runtime gap (carried)

- Read the waiting runtime-gap test and Electron's AgentText paragraph renderer.
  Native lacked the note. Standalone `<truncated N bytes>` lines now draw a
  muted `note` saying `N bytes the runtime did not keep`; reply words on either
  side remain selectable Markdown. Inline mentions and fenced samples stay intact.
- Three separate Electron cases with exact text in `messages::tests`:
  `draws_the_marker_as_a_note`, `keeps_every_other_word_of_the_reply`,
  `leaves_a_sentence_that_merely_mentions_truncation_alone`. An extra regression
  covers fenced code and a marker between single newlines, matching Electron's
  actual line-splitting behavior. No row marked carried before named results.
- Next small section prepared: two eye-state cases through bot stage's real
  `shown_face_for`, including screen-only gating. No production eye changes.
- Additional files inspected but not carried: removed teammate uses index-based
  native selection and apply_team falls back to index zero; a correct port must
  reconcile selection across roster refresh and the plain-conversation path.
  Queued-message file includes the missing busy-other-conversation waitFor rule;
  existing native queue isolation alone does not carry that file.
- All commands and new control sources stay within this workspace. Cargo uses
  `apps/native/target` and the existing in-workspace TypeScript reader. The first
  command used a wrong edit path and started an unchanged baseline; it was stopped,
  no result claimed. The current full run includes the actual gap changes.
- Further small section prepared: all nine mood cases from
  `a-teammate-wears-how-it-feels.test.ts` in `bot_rig::renderer_mood_tests`,
  preserving Electron's pose, seed, side, weight and tick inputs. Native already
  has the applicable rig behavior. JS null-rig handling and library property-key
  reflection are replaced by typed owned-rig and JumpSpec assertions, not rebuilt.
  Verification pending; row still waiting.

- `notes/renderer-3-gap-workspace.log`: exit 0, 2063 core and 465 window
  tests passed, 20 real-agent tests ignored, doc tests 0. All three gap cases,
  the extra fenced/single-newline regression, and the two prepared eye cases
  were observed passing by name. This binary predates the nine mood tests.
  Runtime-gap row now carried. Eye/mood rows stay waiting until their section.

### Continuation controls

- Runtime-gap control caught `messages::tests::draws_the_marker_as_a_note` and
  the extra fenced/single-newline regression, then restored reply_actions.rs
  byte for byte. Full mutant run: 2072 core passed, 20 ignored; 463 window
  passed, two intentional failures only. All nine mood cases passed by name;
  both eye cases passed too. Logs: `renderer-control-runtime-gap.log` and
  `renderer-3-gap-control-result.log`. This is not a restored green run.
- Further inspected, still waiting: teammate work uses names on native's
  cards, while Electron pins host-recorded id ownership and reports aggregate
  usage. Routine handoff requires chain words and prompt projection in addition
  to the existing native controls. Screen choice has a known parity gap:
  Electron keeps Prompt's screen with Terminal faces off; native's wears_screen
  currently returns false for every shape when that switch is off. No row is
  excluded as Electron plumbing for any of these applicable product behaviors.

### Exact mood case mapping

The nine Electron cases map, in file order, to these tests under
`bot_rig::renderer_mood_tests`:

1. `is_worn_for_what_a_teammate_is_doing_curious_waiting_perked_receiving_glad_done`
2. `tips_a_curious_head_to_its_own_side_by_the_tilt_in_proportion_to_the_mood`
3. `tips_each_teammate_the_same_way_every_time_and_not_every_teammate_the_same_way`
4. `lifts_a_perked_head_up_a_little`
5. `smiles_with_the_rigs_own_laugh_on_a_face_of_plastic_lending_working_weight_for_eyes_alone`
6. `leaves_a_screens_eyes_to_its_glyphs_carets_say_glad_there`
7. `wears_nothing_when_it_feels_nothing_and_never_changes_the_rigs_own_pose`
8. `blinks_the_real_rigs_eyes_at_once`
9. `lands_with_the_librarys_own_bounce_and_no_turn_and_can_put_every_part_back`

The eye file maps to `look::bot_stage::renderer_eye_tests`:
`is_a_prompt_and_a_block_cursor_at_work_round_eyes_in_thought_carets_when_done_and_eyes_on_you`
and `is_drawn_on_a_screen_only_a_face_with_no_screen_keeps_its_own_eyes`.
Both exercise `shown_face_for`, which the real stage calls, rather than a
new unused compatibility helper. Mood cases exercise the existing core rig
called by that stage. No product behavior changed in these two files.
- Screen-eyes control caught exactly
  `look::bot_stage::renderer_eye_tests::is_drawn_on_a_screen_only_a_face_with_no_screen_keeps_its_own_eyes`,
  then restored bot_stage.rs byte for byte. Full mutant run: 2072 core passed,
  20 ignored; 464 window passed, one intentional failure. Logs:
  `renderer-control-screen-eyes.log`, `renderer-3-eyes-control-result.log`.
  The optional `glad-screen` control is provided but has not been run.
  No mutation is left applied. Final restored full run follows.


## Continuation final restored verification

- `notes/renderer-3-restored-workspace.log`: exit 0; **2072 core + 465 window =
  2537 passed**, **20 ignored** real-agent tests; doc tests 0. All fourteen
  Electron cases plus the extra fenced/single-newline gap regression passed
  by name in this final restored run. Eye and mood rows are now carried.
- Two deliberate regressions caught their intended named assertions, and both
  files were restored byte for byte. No break remains. All runs were whole
  release workspace runs, serial, with optional Git read locks disabled and
  the existing in-workspace TypeScript ledger reader. No filtered runs.
- This continuation carries three more files (14 Electron cases, one additional
  regression). Together with the prior sections **9 of 45** requested files
  are carried; **36 remain waiting**. No does-not-apply classifications added.
- The requested list is **not complete**. The next waiting teammate file is
  still `a-page-a-teammate-made-runs-in-its-turn.test.tsx`; applicable inline
  preview and pet work remain, along with the other waiting rows. The earlier
  sections document the specific gaps discovered while preferring small files.
- No Electron/JavaScript gate, real-agent test, live window, packaged app or
  release was run. Release binary build and final recount follow below.

- Release binary build: exit 0, finished in 1m 07s with 12 existing warnings;
  `notes/renderer-3-release-build.log`. No warnings in this continuation's
  changed production code. The build excludes test-only features.
- `scripts/recount.py` and `scripts/recount.py --check`: exit 0; **186 carried,
  62 does-not-apply, 185 partial, 876 waiting**, 1309 total. These are global
  inventory counts; the requested subset still has 36 waiting files.
- Final `git diff --check` passed. Tracked tree clean before this final notes
  update; original untracked HANDOFF files preserved and never committed.
- Verified sections committed as `7226958a` (runtime gaps) and `4fc525fe`
  (eyes and moods). The verification record is committed separately.

## Renderer round 2 continuation — screen choice (in progress)

- Starting tracked tree was clean; only the original HANDOFF files were untracked.
- Read Electron's six screen-choice cases, Bot/TeammateBot and avatar validation.
  Native now preserves a shape's intrinsic screen before applying the Terminal
  faces preference (Prompt's screen remains even with an explicit false choice).
  The disk look parser rejects a non-boolean screen field instead of silently
  treating it as an absent choice. Internally the owned spec uses Option<bool>.
- Six separate cases are in `look::faces::renderer_screen_tests`, preserving
  Electron's shape lists, droid/star/flower/Prompt choices and 40-point drawing.
  Verification pending; inventory row remains waiting. A prompt-screen control
  is prepared in the in-workspace renderer-mutation.py.
- The first edit command used a root-relative path from apps/native and failed
  before changing anything. Its unchanged baseline run was stopped (exit 130).
  The actual screen workspace run is `renderer-4-screen-workspace.log`.
- Next section prepared while that run compiles: five exact tooltip cases in
  core's `teammate_tooltip`, used as help on both full sidebar and compact rail
  teammate controls. This section has not yet been compiled or carried.
- More files read, still waiting: the model editor needs parity for model-specific
  effort stops and absent-picker handling; folder-row includes connector-choice
  rendering; teamBoard needs urgency sections (Home currently draws a plain grid);
  idle teammate needs role/template-specific prompts in the conversation itself.
  Every-teammate includes editor controls and display semantics beyond the pure
  bot spec tests. Home-fit needs actual layout work rather than porting Electron's
  measurement helper unused. These are applicable behavior, not exclusions.
- Follow-through: static face callers previously passed Terminal faces=true
  unconditionally. The preference now reaches static faces in Home, sidebar,
  compact rail/flyout, thread header, teammate cards, rooms and Scout cards.
  The animated renderer already read the setting. The first run predates this
  propagation; the second full workspace run includes it and the tooltip section.
- First screen run: exit 0, 2072 core + 471 window passed, 20 ignored, docs 0.
  All six screen tests passed by name, but the row remains waiting for verification
  of the final screen call sites.
- Routine handoff section prepared during the second run: chain label projection
  in core and on actual routine cards; step-choice accessibility labels and checker
  explanation in the real editor; routine-only step-word projection shared by
  titles and ledger user items. Six Electron cases are separate tests (four under
  routines_panel::renderer_handoff_tests, two under conversation_turns::tests).
  No carried claim before a run containing them.
- `renderer-4-tooltip-workspace.log`: exit 0; 2077 core + 471 window passed,
  20 ignored real-agent tests, doc tests 0. The five tooltip cases and six
  screen cases all passed by name. Later test strengthening adds the absent
  star screen at the disk boundary; the next run must verify that version.

### New case mapping

- Screen file, in Electron order, under `look::faces::renderer_screen_tests`:
  `suits_the_made_boxy_shapes_and_not_the_grown_bumpy_ones`,
  `names_only_shapes_that_exist`,
  `keeps_the_choice_and_refuses_anything_but_true_or_false`,
  `is_a_screen_where_the_shape_suits_one_and_its_own_eyes_where_it_does_not_until_chosen`,
  `follows_the_teammates_choice_either_way`,
  `is_never_a_screen_with_terminal_faces_off_but_prompts_whose_face_it_is`.
- Tooltip file, in order, under `teammate_tooltip::tests`:
  `gives_the_name_the_role_and_the_model`,
  `names_the_model_as_the_chip_does_never_by_its_id`,
  `stops_after_the_role_when_the_teammate_has_never_run`,
  `still_leads_with_the_action_so_a_click_is_predictable`,
  `names_the_runtime_the_way_the_rest_of_the_app_does`.
- Routine file, in order: `routines_panel::renderer_handoff_tests::`
  `asks_who_takes_each_step_and_whether_it_checks_when_there_is_a_team`,
  `draws_no_choice_for_a_teammate_on_their_own`,
  `names_the_chain_in_step_order_each_teammate_once_in_a_row_the_checker_marked`,
  `says_nothing_new_for_a_routine_one_teammate_runs_throughout`;
  `conversation_turns::tests::`
  `is_shown_as_the_steps_own_words_not_the_answer_and_rule_it_was_given`,
  `are_the_step_not_the_answer_quoted_above_it`.
  The chain case also draws the real routine card, and the latter prompt case
  applies a mission-created record and checks the actual ledger user item.

### Screen negative control

- `renderer-4-screen-control-result.log`: prompt-screen control exited 0,
  caught exactly
  `look::faces::renderer_screen_tests::is_never_a_screen_with_terminal_faces_off_but_prompts_whose_face_it_is`,
  and restored faces.rs byte for byte. Underlying full workspace totals:
  **2079 core passed**, 20 ignored; **474 window passed, one intentional failure**;
  doc tests 0. No unrelated failure. All five tooltip cases, all six routine
  cases, and the five unbroken screen cases passed by name in that run.
- Final restored run is `renderer-4-restored-workspace.log`; its result is
  pending. The tooltip-default and routine-step-words controls are provided but
  have not been run. No mutation remains applied.

### Final restored verification (renderer-4)

- `renderer-4-restored-workspace.log`: exit 0, **2079 core + 475 window =
  2554 passed**, **20 ignored** real-agent tests, doc tests 0. All seventeen
  cases listed above observed passing by name, including the final absent-screen
  boundary assertion, real routine cards and ledger user item.
- Screen choice and tooltip rows now carried. No Electron plumbing exclusion.
  Routine row is updated with the next verified section commit.
- No mutation remains. All runs used the whole release workspace, serial tests,
  GIT_OPTIONAL_LOCKS=0 and the existing in-workspace TypeScript reader. No
  filtered Cargo test run. Mutation script syntax and git diff --check passed.
