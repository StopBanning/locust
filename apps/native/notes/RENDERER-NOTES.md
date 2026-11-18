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
- Screen/tooltip section committed as `8bce5aa5`. Routine section is carried
  after all six named cases passed in the final restored workspace run.
- This continuation adds **3 files / 17 Electron cases**. Across the recorded
  rounds, **12 of 45 requested files** are carried; **33 remain waiting**.
  The list is not finished. No other file is claimed carried or excluded.
- Global inventory after recount: **189 carried, 62 does-not-apply,
  185 partial, 873 waiting**, 1309 total; recount and --check both exit 0.
- Continuation point remains the teammate list, starting with applicable inline
  new-page preview and pets. The prior notes record further roster-selection,
  signed-out/discovery state, model editor, Home layout/board and work-ownership
  gaps. The glyph-eye file also includes animated geometry/visors and recorder
  assertions; existing pure geometry alone does not carry the entire file.
- No Electron/JavaScript gate, live window, real agent or packaged app was run.
  Release binary build follows. Original HANDOFF files stay untracked and intact.

### Final build and handoff

- Routine section committed as `43650097` after screen/tooltips `8bce5aa5`.
- `renderer-4-release-build.log`: release binary build exit 0, finished in
  1m 01s, 12 existing warnings in drawing/conductor code. No new warning in
  changed production logic. This build excludes test-only features.
- Final requested-subset count checked directly from tests.tsv: **33 waiting**.
  Continue teammate files before conversation-only files as the brief directs;
  the first waiting teammate file remains the inline new-page preview.
- Final `git diff --check` and recount --check pass. Both implementation
  sections are committed; tracked tree clean before this verification note.
  No mutation is applied. No HANDOFF file was changed or committed.
- The requested list remains unfinished. The final verification above covers
  these 17 cases only; no claim is made for the 33 waiting files, real agents,
  a live window, a packaged build or the Electron/JavaScript gate.

## Renderer round 2 — removed selection continuation

Starting tracked tree clean at a9f5fc14. Of 45 files, 33 remained waiting.
Inline preview, pet integration, runtime discovery/status, model editor,
folder question, Home urgency sections and handoff glances need actual
surface integration; unused core helpers alone do not carry these files.
The removed-teammate file now has three separate `renderer_address_tests`
with Electron's exact Ash/Wren ids. They exercise roster refresh and the
actual send target resolver. Refresh no longer falls back to index zero
after removal. Existing initial default selection is preserved. Verification
pending; row waiting. An edit attempted from the wrong working directory
failed without changing files; its baseline run was interrupted before the
actual changes. All edits and test artifacts stay inside the workspace.

Next section prepared while the first run executes: teamBoard's four exact
cases in `team_board::tests`. Home and Team now use the urgency grouping,
with record-based owner ids for pending approvals and unseen finishes.
Checked missing/sign-in outcomes block, unknown discovery stays quiet.
This section is not yet verified and its row remains waiting.

## Renderer continuation — verification audit

- Retained the inherited selection and team-board sections as verifiable work;
  no stash or discard. The handoff's extra untracked `src/team_board.rs` belongs
  to that section too. No external scratch file is read or changed.
- `renderer-6-workspace.log`: 2079 core passed, 20 ignored; window 477 passed,
  5 failed. All three removed-selection and four board cases passed by name.
  Four window fixtures replaced the selected roster with a different identity
  and implicitly relied on the unsafe fallback. Their setups now explicitly
  pick the new fixture teammate; product fallback is not restored. The fifth
  failure was the relay hop-cap test's 60-second live-run deadline.
- Corrected quiet-board fixture to supply only Wren's idle status, exactly as
  Electron does, and added a headless Home/Team surface regression. Pending
  verification of those additions; no rows marked carried on this failed run.
- Further teammate files read, still waiting: working-face rotation has pure
  rig helpers but the native stage does not use its working beats; idle world
  needs the per-teammate empty-state surface (Home role starters alone do not
  suffice); month/limit needs real editor and card rows; plain ghost needs
  reply/live-row faces; every-bot includes editor shuffle and rendering hooks.
  These are applicable feature gaps, not React-only exclusions.
- Conversation files inspected, still waiting: Alt-arrow needs actual window
  navigation in drawn sidebar order, including folded groups; long-thread newest
  turns needs bounded drawing plus Show earlier; phone list needs the phone
  projection, not merely existing sidebar grouping. No unused-helper tests or
  does-not-apply rows were substituted for these missing features.

### Case mapping for these inherited sections

- `a-removed-teammate-is-not-replaced.test.ts` → `renderer_address_tests`:
  `is_the_teammate_picked`,
  `is_nobody_once_that_teammate_is_removed_never_whoever_is_first`,
  `is_nobody_when_nobody_was_picked`. Inputs retain `tm_ash`/Ash and
  `tm_wren`/Wren; the first additionally exercises roster reordering, and
  the second checks another refresh cannot silently reselect a teammate.
- `teamBoard.test.ts` → `team_board::tests`:
  `puts_each_teammate_where_what_they_need_from_you_says_most_urgent_first`,
  `draws_a_quiet_team_as_the_plain_grid_no_board_of_empty_columns`,
  `leaves_out_a_section_with_nobody_in_it`,
  `an_unseen_finish_outranks_idle_never_a_live_run_or_a_wait`.
  Electron's Wren/Juno/Sable/Quill/Pip ids, exact section titles and member
  order are preserved. An additional native surface test checks both actual
  Home and Team views, rather than only an uncalled pure helper.
- Board classification uses host-recorded owner ids for approval/unseen sets,
  not display-name matching. Initial selection behavior is retained; a removed
  selection remains empty until an explicit pick. No Electron plumbing exclusion
  is needed for either section.

### Selection negative control

- `renderer-6-selection-control.log` / `renderer-control-removed-selection.log`:
  control runner exit 0. Reintroducing the index-zero fallback failed exactly
  `renderer_address_tests::is_nobody_once_that_teammate_is_removed_never_whoever_is_first`.
  Core 2079 passed (20 ignored); window 482 passed, one intentional failure.
  Source restored byte for byte. The added Home/Team headless test passed by
  name in this run, and the earlier relay deadline did not recur.
- A restored full workspace run is in `renderer-6-restored-workspace.log`.
  The board-working control is provided but not yet run.
- Two requested inventory paths are absent from this checkout:
  `a-waiting-conversation-is-first-in-the-sidebar.test.tsx` and
  `an-app-started-conversation-says-so-in-the-sidebar.test.tsx`. Their rows
  remain waiting; no invented Electron fixtures or exclusion is claimed.

### Restored verification (renderer-6)

- Whole release workspace exit 0: 2079 core + 483 window = 2562 passed,
  20 ignored real-agent tests, doc tests 0. All seven Electron cases and the
  extra Home/Team surface test observed passing by name in
  `renderer-6-restored-workspace.log`. No mutation remains.
- Selection row now carried; board row will be carried with its separate
  section commit. Four explicitly selected window fixtures all passed, as
  did the previously timed-out relay test. No unrelated product fix.

- Selection committed as `000c5cb8`. Board row now carried after the same
  restored green run; four Electron cases plus one native drawing regression.
  This continuation carries two more files, leaving 31 of 45 waiting.

## Ownerless model mark — next section

- Read both cases and Sidebar.tsx's actual runtime/own-model branch. Native's
  conversation rows had no mark. They now draw the named runtime mark for a
  known route with no recorded owner, or the empty place for an own model or
  unknown route. Uses the existing native vector marks, no DOM recreation.
- Two separate tests in `sidebar::renderer_model_mark_tests`, with Electron's
  mission ids, `yo`, date, `claude-opus-5-5`, and
  `own-0a1b2c3d/llama-local`. The tests draw the real sidebar and check the
  actual keyed mark/place plus the accessible Claude Code name.
- Row still waiting pending named results. The own-model control is prepared.

- `renderer-7-workspace.log`: exit 0, 2079 core + 485 window passed,
  20 ignored real-agent cases, doc tests 0. Both new model-mark cases observed
  passing by name. While this binary compiled, tightened the own-model identity
  to Electron's eight-lowercase-hex id and nonempty model rule, and resolve a
  recorded owner against today's roster before withholding the fallback mark.
  Those refinements are included in the control and final restored run below.
- Exact mapping to `sidebar::renderer_model_mark_tests`:
  `wears_its_models_mark_where_the_face_goes_named` and
  `keeps_the_empty_place_for_a_model_of_your_own_or_a_runtime_not_yet_known`.
  Neither case is excluded as React plumbing.

- Model-mark control caught exactly the own-model placeholder test; core 2079
  passed, window 484 passed plus one intentional failure, 20 ignored. The
  runner restored sidebar.rs byte for byte (`renderer-7-model-control.log`).
- Final review found an incidental grouping regression: Home's Scout invitation
  disappeared when no teammate was Ready. Keep that action after the urgent
  sections in that case; the existing Scout invitation test now draws actual
  Home with its sole teammate busy and requires the invitation to remain.
  This correction is included in the final restored workspace run.

### Final restored model-mark verification

- `renderer-7-restored-workspace.log`: exit 0, **2079 core + 485 window =
  2564 passed**, **20 ignored** real-agent tests, doc tests 0. All nine
  Electron cases across the three carried files, the Home/Team surface
  regression, and the strengthened Scout invitation test passed by name.
- The final source includes the strict own-model id check, current-roster
  ownership check and Scout invitation correction. Both intentional mutations
  were caught at their named assertion and restored byte for byte. No break
  remains. The optional board-working control was not run.
- Three files carried in this continuation, none excluded. Together with the
  earlier recorded rounds, **15 of 45** requested files are carried and
  **30 remain waiting**. The requested list is **unfinished**. Applicable
  gaps and the two absent inventory paths remain documented above.
- No Electron/JavaScript gate, real agent, live window or packaged app was
  run. Whole release workspace runs used gentle-cargo only, one at a time,
  serial tests, optional Git read locks disabled, and the existing in-workspace
  TypeScript reader. Release binary build follows.

### Final build and continuation receipt

- Release binary build exit 0 in 29.87s (`renderer-7-release-build.log`),
  twelve existing drawing/conductor warnings. No new warning in the changed
  production logic. No package, signing or live-window verification.
- `scripts/recount.py` and `--check` exit 0: 192 carried, 62 does-not-apply,
  185 partial, 870 waiting, 1309 total. Direct requested-subset check: 30 waiting.
  `git diff --check` and mutation-script Python syntax passed.
- Commits: `000c5cb8` removed selection; `ff9d1ff5` urgency board;
  `77051685` ownerless model mark; `f42ff722` preserve Scout invitation.
  Final validation notes committed separately. All source changes are committed;
  no mutation remains applied. The original untracked HANDOFF files are intact
  and not committed. Nothing was stashed or discarded.
- Continue the waiting teammate list before remaining conversations, with the
  applicable inline page preview first. Prior notes explain which surface
  integrations are still missing. The full requested list is not complete.

## Renderer-8 continuation — eye geometry and motion (verification pending)

- Initial tracked tree was clean at `5133db08`; the round-3 leftovers described
  in the handoff were already committed by earlier continuations. Nothing to stash.
- Read the remaining small teammate tests and their native surfaces. Added fifteen
  separate exact-value cases from `a-teammates-eyes-are-drawn-not-typed.test.ts`
  in `core/src/renderer_eye_tests.rs`. Geometry, glyph weights, blinking, hue,
  front-plane projection, all motion loops and visor fitting use the same values
  as Electron. Most are already used by the native painter/rig; front_plane and the CSS-string same_hue function are pure compatibility contracts only. Native draws through its own vector transform and uses same_hue_rgba, so these two tests alone do not verify the actual screen projection or color path.
- The inventory row stays waiting: the four drawing cases (round strokes,
  screen fill/glow ordering, rig front-transform interception, mouth suppression)
  still need native painter assertions. Canvas interception itself is Electron
  plumbing; the resulting drawing behavior is applicable. Native currently draws
  ordinary eyes on plastic rather than the glyph pair, so this is a real gap.
- Other inspected teammate files remain waiting: model/effort editor is native
  text fields rather than the shared route picker; folder editor lacks the full
  folder/connector presentation; signed-out and pending-discovery files need one
  status projection shared by all surfaces (Home currently classifies only
  missing/sign-in probes); work-face cycle needs real stage/conductor coverage;
  idle teammate starters need conversation-screen integration. None excluded.
- Whole release workspace run: `renderer-8-eyes-workspace.log`, gentle-cargo only,
  in-workspace target/TypeScript reader, serial tests and optional Git read locks
  disabled. Named results and controls will be recorded after completion.
- Initial command from the repository root bypassed `.cargo/config.toml`'s
  isolation runner: all fifteen new tests passed by name, but existing guarded
  tests failed and core aborted during cleanup. This is not a suite pass.
  Restarted from `apps/native` using its configured isolated runner; no guard
  was weakened or product code changed to bypass isolation.

## Renderer-8 — conversation keys (prepared, not yet carried)

- After recording the inspected teammate gaps above, prepared the five cases
  from `alt-and-an-arrow-moves-between-conversations.test.ts` as the next small
  conversation section. Four exact-value pure cases in
  `conversation_list::renderer_step_tests`; the fifth in
  `sidebar::renderer_step_wiring_tests` draws the actual keyed rows and calls
  the window key handler to open the selected conversation.
- Native lacked this keyboard path. Alt+Up/Down now traverses the visible
  sidebar rows; Shift restricts the search to rows with a waiting card.
  Pins, groups, folded groups, search, settled filtering and one-level relay
  nesting determine the order. Drawing and navigation share `nested_order`;
  the existing folded-group drawing test also checks navigation's row list.
- No React source-regex assertion is rebuilt: the wiring case exercises the
  native handler and actual sidebar instead. No case excluded. Row stays
  waiting until all five named cases pass.
- Isolated `renderer-8-workspace.log`: core **2094 passed, 20 ignored**;
  window **489 passed, 1 failed**. All fifteen new eye cases passed by name;
  the four pure navigation cases passed. The new wiring case caught a real
  defect in the prepared code: using `active_record_mission_id` loses the
  selection in demo mode. It now uses the same conversation selection as the
  sidebar itself. This correction and folded-group assertions await the next run.
  The eye-only test section is verified independently of that window failure;
  its file remains waiting for the four missing drawing cases and for painter-level projection/color verification.

## Renderer-8 — prompt rail (prepared)

- The four cases from `a-long-conversation-has-a-rail-of-your-messages.test.tsx`
  already have one native test each in `prompt_rail::tests`, with Electron's
  exact thresholds, geometry, keys and hover strings. Strengthened
  `takes_hover_line_from_message` to draw five real native marks from Electron's
  five prompts and check each keyed control's accessible label, not only the
  pure string helper. Existing whole-thread headless test also loads and draws
  five linked turns and checks scrolling/jumping and room gating.
- Mapping in source order: `gives_rail_to_four_messages_when_scrollable_and_room_suffices`,
  `keeps_marks_in_order_and_current_is_nearest_top`,
  `jumps_with_ctrl_alt_up_and_down_only`, `takes_hover_line_from_message`.
  The DOM data-attribute and React source-regex checks become native drawing
  and handler checks; no product behavior is excluded. Still waiting until the
  strengthened case passes by name. The newest-turn rendering file remains
  waiting: it needs its actual 400-row budget and expansion behavior verified.

### Verified controls and navigation section

- `renderer-control-glyph-weight.log`: all fifteen eye cases except the
  deliberately weakened weight rule passed; `is_as_heavy_as_the_eye_it_stands_for`
  failed as intended. Core 2093 passed + one intentional failure, 20 ignored;
  window 490 passed. The control runner exited 0 and restored bot_spec.rs byte
  for byte (`renderer-8-glyph-control-result.log`).
- All five navigation cases passed by name in that run, including the corrected
  actual key-handler case and the strengthened folded-group test. Navigation row
  now carried. No React-only exclusion. Native selection is read from the same
  conversation the sidebar highlights, including a just-opened ledger whose tip
  has not loaded yet. Final restored verification and its own control follow.
- Eye test section committed as `1c64d157`. Its row remains waiting for drawing
  coverage; no false full-file claim. Navigation is the next verified section.

### Verified prompt-rail section and step control

- `renderer-control-conversation-step.log`: core 2094 passed, 20 ignored;
  window 487 passed and exactly three intentional navigation failures
  (`move_nowhere_when_there_is_nowhere_to_go`,
  `with_shift_pass_over_every_conversation_that_is_not_waiting_for_you`,
  and the actual key-handler test). The runner caught the named Shift-filter
  assertion, exited 0, and restored conversation_list.rs byte for byte.
- All four prompt-rail cases passed by name in that run, including the newly
  strengthened exact five-prompt drawing case. The existing whole-thread
  scrolling/jumping and folded-group regressions passed too. Rail row now
  carried; its threshold control and final restored suite follow.
- Navigation committed as `6a54f3e0`. This continuation carries two full files
  (nine Electron cases), plus fifteen pure eye cases in an incomplete file.
  **17 of 45 requested files carried; 28 waiting.** No does-not-apply additions.
- Prompt-rail threshold control caught exactly
  `gives_rail_to_four_messages_when_scrollable_and_room_suffices` and restored
  prompt_rail.rs byte for byte. Core 2094 passed, 20 ignored; window 489 passed
  plus one intentional failure. Runner exited 0 (`renderer-8-rail-control-result.log`).
  All three new controls are caught and restored; no break remains applied.
- Prompt-rail section committed as `8fe5d864`. Final restored full release
  workspace and release binary build now follow; no filtered runs were queued.

## Renderer-8 final restored verification

- `renderer-8-restored-workspace.log`: exit 0; **2094 core + 490 window =
  2584 passed**, **20 ignored** real-agent cases, doc tests 0. All 24 relevant
  Electron cases observed passing by name (15 pure eye cases, 5 conversation
  key cases, 4 prompt-rail cases). The existing full-thread rail/jump and
  folded-group drawing regressions also passed. This verifies the final
  committed source with every mutation restored.
- All three new controls caught their named assertion; each restored its source
  byte for byte. No break remains applied. All Cargo tests were whole release
  workspace runs through gentle-cargo, one at a time, without filtered runs.
- The requested handoff remains **unfinished**: **17 of 45 files carried,
  28 waiting**. The eye file is still waiting: four drawing cases plus actual
  painter projection/color behavior remain unverified, despite the 15 pure
  cases passing. Earlier notes list the applicable teammate integration gaps;
  continue the waiting teammate list before remaining conversation work,
  starting with the inline new-page preview. No does-not-apply rows added.
- No Electron/JavaScript gate, live window, real-agent turn, packaged app,
  signing or release was run. Initial failed runs are recorded above; only
  this restored run is the final green suite. Release build follows below.
- Release binary build: exit 0, 1m 19s (`renderer-8-release-build.log`),
  twelve existing drawing/conductor warnings. No live/packaged verification.
- `scripts/recount.py` and `--check` passed: 194 carried, 62 does-not-apply,
  185 partial, 868 waiting, 1309 total; requested subset has 28 waiting.
  Mutation-script Python syntax and `git diff --check` passed.
- Commits: `1c64d157` pure eye cases; `6a54f3e0` conversation keys;
  `8fe5d864` exact prompt-rail labels. Final validation notes committed separately.
  No source mutation, stash or discarded work remains; original untracked
  HANDOFF files are unchanged and are not committed.

## Renderer-9 — queue isolation (prepared)

- Initial tracked tree was clean; round-3 leftovers are already committed by
  intervening sessions. No stash needed. Original HANDOFF files untouched.
- Read steering.ts and the actual enqueue, drain, Edit/Discard and persistence
  callers. Added ten separate exact-value cases from
  `a-queued-message-stays-in-its-conversation.test.ts` in
  `composer::queue::renderer_conversation_tests` (source order, first ten cases).
  Native's existing keyed queue suffices for these pure isolation rules.
- The five remaining cases require busy-other-conversation routing (`waitFor`)
  and its actual enqueue/display/drain wiring. The row stays waiting. No
  does-not-apply claim: the React source check pins applicable product behavior.
- First attempted append used a repository-relative path from apps/native and
  failed before writing; the concurrent full run is an unchanged baseline,
  not evidence for the new cases. The next run must include the actual tests.

## Renderer-9 — context receipt (prepared)

- Read cost.ts, ContextRing's native drawing, normalized receipt folding and
  subscription money classification. Prepared four exact cases from
  `the-ring-reads-what-the-conversation-holds.test.ts`, each a separate test in
  `context_usage::renderer_ring_tests`: last-call reading, missing reading,
  subscription dollars omitted, and ordinary dollars retained. The two
  reading cases go through the real transcript fold used by the window.
- Remaining six cases are not claimed: subscription receipt wording (native
  benchmark::cost_line currently puts token counts before `plan`), ring cost
  exclusion/mixed costs, aggregate totals, and exact pie geometry. The native
  ring currently uses a polygon approximation rather than Electron's arc.
  Row remains waiting; none of those product behaviors is Electron plumbing.
- Added a fifth ring-file case, `says_on_its_receipt_that_the_plan_covered_it`.
  It tests both the shared cost formatter and the actual inspector receipt
  projection. Native now says `Cost` / `in your plan` for subscription-only
  receipts, before token usage; benchmark cells use the same priority.
  This supersedes the prepared note's subscription-wording gap. Five other
  ring-file cases remain unported; the row is still waiting.
- Added queue-isolation and plan-receipt controls to the existing in-workspace
  runner. Verification is pending; no mutation is currently applied.
- `renderer-9-cases-workspace.log`: all ten queue cases observed passing by
  name; four prepared context cases also passed by name. This compilation
  predates the fifth case and subscription-wording changes. Core: 2098 passed,
  20 ignored. Window result follows; no green final-source claim yet.
- First cases run completed exit 0: **2098 core + 500 window = 2598 passed**,
  20 real-agent cases ignored, doc tests 0. All ten queue cases passed by name.
  Committing this verified queue section before running its negative control.

### Renderer-9 case mapping and limits

Queue module: `composer::queue::renderer_conversation_tests`.
In Electron source order, its first ten cases map to:
1. `is_its_own_rows_in_the_order_they_were_typed`
2. `never_folds_into_another_conversations_message`
3. `still_folds_lines_typed_during_one_run_into_one_message`
4. `takes_that_conversations_rows_folded_and_leaves_every_other_row_where_it_was`
5. `takes_nothing_from_a_conversation_with_nothing_queued`
6. `goes_by_the_row_itself_two_rows_can_be_given_the_same_id`
7. `empty_that_conversations_queue_and_nobody_elses`
8. `the_reviewers_case_gems_line_edited_pips_untouched_each_sent_to_its_own`
9. `sends_each_its_own_as_each_run_ends_in_either_order_the_others_staying_pending`
10. `sends_nothing_the_words_are_back_in_the_box_not_the_queue_and_leaves_the_other_conversation_queued`

Context module: `context_usage::renderer_ring_tests`.
Electron cases 1, 2, 3, 4 and 7 map respectively to:
- `reads_what_the_conversation_held_after_the_last_call_not_the_run_added_up`
- `draws_nothing_for_a_receipt_that_cannot_say_what_the_conversation_held`
- `carries_no_dollar_figure_nobody_paid_it`
- `says_on_its_receipt_that_the_plan_covered_it`
- `keeps_the_dollars_on_a_run_with_no_subscription_behind_it`

No full-file status advances this section: queue has five remaining cases;
ring has five remaining cases. Other waiting files remain as recorded above.
No live window, real agent, packaged app or Electron/JavaScript gate is run.

### Renderer-9 queue control and billing section verification

- Queue control caught exactly
  `never_folds_into_another_conversations_message` and restored queue.rs byte
  for byte; `git diff -- src/composer/queue.rs` is empty. Runner exit 0,
  `renderer-9-queue-control-result.log`. Whole mutant suite: 2099 core passed,
  20 ignored; 499 window passed, one intentional failure; doc tests 0.
- All five final context/billing cases observed passing by name in that run.
  The fifth proves both shared formatter and actual inspector receipt words.
  Billing section committed on these named passes; its independent negative
  controls and final restored run follow. No aggregate-cost or pie claim.
- Warnings match the unchanged baseline: 3 core-test and 11 window-test
  warnings in existing buddy/drawing/conductor code. None in changed files.

### Renderer-9 billing negative controls

- `plan-receipt`: exactly the named plan-coverage case failed when inspector's
  subscription branch was disabled. 2098 core passed + one intentional failure,
  500 window passed, 20 ignored, doc tests 0. Runner exit 0, inspector.rs
  restored byte for byte. Logs: `renderer-control-plan-receipt.log` and
  `renderer-9-plan-receipt-result.log`.
- `plan-format`: disabling the shared plan formatter failed the named case
  and existing `benchmark::tests::compare_record_rows_calculates_middle_time_and_cost`;
  both expected `in your plan`. 2097 core passed + two intentional failures,
  500 window passed, 20 ignored, doc tests 0. Runner exit 0, benchmark.rs
  restored byte for byte. Logs: `renderer-control-plan-format.log` and
  `renderer-9-plan-format-result.log`.
- All three controls caught; tracked source diff is empty after restoration.
  Final restored whole-workspace release suite and binary build follow.


## Renderer-10 — continuation and context ring (prepared)

- Started from 934e7106. The round-3 code leftovers listed in the handoff are
  already committed. Preserved the existing uncommitted renderer-9 billing
  control notes; no stash or source cleanup was needed.
- Running a restored whole release workspace suite before committing those
  verification notes (`renderer-10-baseline-workspace.log`). This compilation
  predates this section's new ring tests and changes.
- Read ContextRing.tsx, cost.ts, the remaining ring cases, native ledger's
  conversation cost accumulation and native drawing. The ring now uses the
  actual snapshot's conversation cost: subscription-only totals say only the
  context; paid usage beside a plan remains priced. Added separate exact-value
  cases `gives_the_ring_nothing_to_say_about_cost_only_the_context` and
  `still_prices_what_was_priced_beside_it` in `context_usage::renderer_ring_tests`.
- Added `fills_a_slice_from_twelve_oclock_the_size_of_the_share_used`, preserving
  Electron's exact SVG path values for 0/25/75/100 percent, centre 7, radius 4.5.
  Native uses its existing SVG outline tessellator to draw that path, including
  a full disc. This replaces the ring's private one-percent polygon loop.
- Added both list-of-runs cases: `states_no_total_for_runs_a_subscription_covered`
  and `counts_as_priced_only_the_runs_that_were`. The shared cost-total formatter
  is also used by the actual ring; it suppresses plan-only totals and counts
  only the displayed unit. Token formatting now uses the existing precise
  compact count (2.4k, not 2k), matching Electron's exact aggregate values.
- Added a headless drawn-ring regression for the snapshot-to-control wiring,
  testing the exact subscription and mixed paid sentences.
- All ten Electron cases are now implemented (five pre-existing, five new),
  plus the additional headless regression. Row stays waiting until verified
  by name. No Electron-plumbing exclusions. Ring-plan control prepared;
  all new-source verification is pending. No mutation applied.

### Restored renderer-9 verification

- `renderer-10-baseline-workspace.log`: exit 0, 2099 core + 500 window
  passed, 20 real-agent tests ignored, doc tests 0. This is the restored
  verification for 934e7106 and the inherited billing control notes, not
  evidence for renderer-10 source. No prior source changes needed stashing.

- Reviewed the next queue cases while the ring control compiles: enqueue uses
  only the current queue key and drain judges only the current screen's run.
  Carrying the remaining five M32 cases needs waitFor persisted and used in
  enqueue/display/drain; adding queueHome alone would be insufficient. Queue
  row remains waiting. Idle-world and teammate-work also retain applicable
  screen/ownership gaps described in earlier notes; not excluded as plumbing.
- Ring control is currently running as one whole release workspace suite;
  benchmark.rs is temporarily mutated only under the runner's restore guard.

### Renderer-10 negative control

- `ring-plan` runner exit 0: caught the intended
  `gives_the_ring_nothing_to_say_about_cost_only_the_context` assertion,
  `states_no_total_for_runs_a_subscription_covered`, and the additional actual
  drawn-ring test. Exactly those three failures: 2102 core passed / 2 failed,
  500 window passed / 1 failed, 20 ignored, doc tests 0. The actual failure
  adds `This conversation: in your plan`, which the person should not see.
- Runner restored benchmark.rs byte for byte, retaining the intended changes.
  Logs: `renderer-control-ring-plan.log`, `renderer-10-ring-plan-result.log`.
  Restored whole release workspace run is in progress; no mutation remains.
- Manifest audit also found two waiting paths absent from this checkout (and
  no path history in the current branch):
  `a-waiting-conversation-is-first-in-the-sidebar.test.tsx` and
  `an-app-started-conversation-says-so-in-the-sidebar.test.tsx`. Neither can
  be carried with exact cases from the supplied workspace yet. Rows unchanged.


## Renderer-10 next section — teammate work age (prepared)

- Read teammateWork.test.ts and the native people screen's actual ago_label
  callers. The existing behavior matches its four age cases. Factored only
  the clock argument so those exact Electron timestamps can be tested.
- Four separate cases in `people_panel::renderer_work_age_tests`:
  `reads_in_the_largest_unit_that_fits`, `singularises_honestly`,
  `calls_anything_under_a_minute_just_now_including_a_clock_that_ran_ahead`,
  `says_nothing_for_a_timestamp_it_cannot_read`. Verification pending in the
  ongoing restored run (if its window compilation includes this edit).
- The six teammateWork ownership/usage/recent-list cases still need the proper
  id-based ownership projection on the actual screen. Row remains waiting.

- All ten ring cases observed passing by name in
  `renderer-10-restored-workspace.log`; ring row now carried. Whole run still
  finishing. Recount and --check pass: 195 carried, 62 does-not-apply,
  185 partial, 867 waiting, 1309 total. Requested subset: 27 waiting.

### First restored run (not green)

- `renderer-10-restored-workspace.log`: exit 101. All ten ring cases, the
  drawn-ring regression, and all four work-age cases observed passing by name.
  Core: 2103 passed, one failed, 20 ignored. Window: 505 passed, none failed.
- The failure is `native::tests::native_claude_denies_then_allows_one_file_write`:
  its fake Claude process closed stdout before requesting approval (zero
  records). It passed in both this turn's baseline and mutant suite. No
  unrelated approval or runtime source is changed. One unchanged-source full
  confirmation follows in `renderer-10-confirm-workspace.log`.

### Final restored confirmation — verified

- `renderer-10-confirm-workspace.log`: exit 0, **2104 core + 505 window =
  2609 passed**, **20 ignored** real-agent cases, doc tests 0. All ten ring
  cases, the added drawn-ring check and four work-age cases observed passing
  by name. The fake Claude approval case passed; the earlier failure is
  retained above and is not counted as a pass.
- One whole workspace run at a time via gentle-cargo, release, serial tests,
  optional Git read locks disabled, target and TypeScript reader inside this
  workspace. No filtered runs. Ring-plan control caught and fully restored.
- Ring file is carried (18 of 45 requested files now carried cumulatively,
  27 still waiting). No exclusions added. The requested list is unfinished.
- Recount and --check passed; `git diff --check` and mutation-runner Python
  syntax check passed. Release binary build is running separately.

### Teammate work age section — verified

- All four `people_panel::renderer_work_age_tests` passed by name in both
  restored runs, including the final green 2609-test confirmation. Committed
  separately after the completed ring section. Only the clock injection was
  factored; production time labels retain the same behavior.
- teammateWork.test.ts stays waiting for its six ownership/usage cases.
  No full-file parity claim is made for those four verified age cases.

### Renderer-10 final handoff

- Release binary build: exit 0, `renderer-10-release-build.log`, 1m 32s.
  Twelve existing native warnings in drawing/conductor code; none added in
  this section's changed code. No live window, packaged app, real agent,
  Electron/JavaScript gate, or platform build beyond this Mac was run.
- Source commits: `03fb3ffb` context-ring parity; `1232b8ae` work-age cases.
  `f27b9c4f` preserves and verifies the prior billing-control notes. No handoff
  file changed or committed. No mutation, stash or discarded work remains.
- Tracked tree is committed clean after this note. The only untracked files
  are the original HANDOFF files; the Python syntax check's bytecode cache
  was removed. Recount, --check and diff whitespace checks passed.
- Unfinished: 27 of the 45 requested files remain waiting. Continue the
  teammate list before conversation-only files, retaining the gap notes.
  First applicable gap is still inline new-page preview. The queue needs
  persisted waitFor wiring; teammateWork needs six ownership/usage cases.
  Two missing Electron source paths are recorded above. No further file is
  claimed carried, and no full-task completion is claimed.


## Renderer-11 — working-face rule cases (prepared)

- Started from f7ff9ceb with a clean tracked tree. The round-3 leftovers in
  the handoff are already committed; nothing needed stashing or discarding.
- Read a-teammate-at-work-goes-round-its-faces.test.ts, faceLife.ts,
  faceState.ts, TeammateBot.tsx's rules and native bot_rig/buddy_moves/bot_stage.
  Twelve separate Rust cases in renderer_work_faces_tests preserve Electron's
  activities, seeds, counts, timings and twelve-step Antigravity tool sequence.
  The typed LiveRuntimeEvent inputs retain exactly the fields these rules read;
  transport ids, timestamps and text are not inputs to this function.
- Verification pending in renderer-11-workspace.log. First invocation was made
  from the repository root and exited 101 before finding Cargo.toml; corrected
  by running from apps/native. No test pass is claimed for that invocation.
- Important integration gap: bot_stage does not call worn_activity,
  everyday_face or working_beat_for. Home, roster and sidebar mostly draw
  static faces. Passing the pure rule tests therefore cannot carry this file.
  Its manifest row stays waiting, with this limitation recorded explicitly.
- Other inspected gaps remain applicable: plain conversations lack reply/live
  avatars; phone listing has no native phone module; the teammate folder row
  needs connector/permission behavior; new-page preview and queued waitFor
  remain as described above. None is excluded as Electron plumbing.
- A working-cycle negative control has been added; not yet run. All runs use
  gentle-cargo, one whole release workspace at a time, with target and the
  existing TypeScript reader inside this workspace, serial test execution,
  and GIT_OPTIONAL_LOCKS=0. No outside-workspace mutation script is accessed.


### Renderer-11 next section — newest turns (prepared)

- Read a-long-conversation-opens-on-its-newest-turns.test.tsx and Thread.tsx's
  budget policy. Native previously constructed every item. Added a whole-turn
  budget underneath message_column, with per-conversation expansion state and
  a real Show earlier turns button. The current turn is outside the budget;
  the newest earlier turn always stays. Prompt-rail jumps reveal all turns
  before resolving their target, retaining access to old messages.
- Three exact index/budget cases in thread_view::renderer_newest_tests and
  two actual headless message-column cases in messages::renderer_newest_tests.
  Added a real-pointer interaction check for expansion and conversation isolation.
  All pending verification; manifest row stays waiting until named passes.
- Native budgets its own ledger items, including prompt rows, instead of
  constructing Electron's intermediate React row tree. Exact 20-by-60 and
  3-by-5 fixtures preserve the pinned visible range and words.

- renderer-11-workspace.log completed exit 0: 2116 core + 505 window tests
  passed, 20 real-agent cases ignored, doc tests 0. All twelve work-face cases
  observed passing by name. This run predates the newest-turn changes; those
  are not claimed verified. Commit the rule tests as a bounded partial section.


### Renderer-11 next section — routine runner (prepared)

- Read nobodys-conversation-can-be-a-routine.test.tsx and RoutineDialog.tsx.
  Native routine editing now asks Who runs it when the entry has no owner,
  starts on Pick a teammate, and never defaults to the first roster member.
  Existing owned entries state who runs them without asking. An empty roster
  explains that there are no teammates yet. Save is disabled and guarded at
  the press boundary until there is an existing teammate to run it.
- An overlong step says Shorten step 2 to save this routine for Electron's
  exact Fine./8001-x fixture and disables Save. Counts UTF-16 units as JS does;
  host validation remains in routines.rs. No route is invented or changed.
- Six separate actual headless editor cases in
  routines_panel::renderer_unowned_tests; pending verification. Manifest row
  remains waiting. A routine-owner negative control is prepared, not yet run.


### Newest-turn negative control

- newest-budget exited 0 after observing the intended named assertion fail;
  the runner restored thread_view.rs byte for byte. Core 2116 passed and
  three intended budget assertions failed, 20 ignored. Window 506 passed
  and the long-window and missing-button interaction cases failed. Short
  conversations and the twelve work-face cases passed by name. No unrelated
  failures. Logs: renderer-control-newest-budget.log and
  renderer-11-control-result.log. No mutation remains applied.
- Final restored full workspace run started in renderer-11-restored-workspace.log,
  also including the newly prepared routine editor cases and pointer choice.

- Follow-up to the prepared pagination section: the older-edge layout hook
  requests another budget within 600 points after scrolling up, retaining the
  previous first item as the scroll anchor. A headless scroll test is prepared.
  Opening a different ledger clears expansion state, matching Thread's identity
  reset. Rail jumps still reveal their target before resolving its key.
  These edits postdate the first restored compilation and require the next run.


### First restored run — pagination passes, routine dispatch correction

- renderer-11-restored-workspace.log: core 2119 passed, 20 ignored; window
  512 passed, three failed. The five Electron newest-turn cases and the
  pointer expansion case all passed by name. This binary predates the scroll
  hook/test and identity reset. No final green claim is made for that source.
- All three failures were disabled routine Save pointer cases. Rune still
  dispatched the attached handler after disabled(true), and the guard displayed
  a refusal. The screen now attaches no press handler while saving is refused,
  in addition to styling it disabled and revalidating an enabled press.
  Runner-choice, default-nobody, inherited-owner and selection-pointer cases
  passed by name. The correction must be verified by the next full run.


### Pagination section — verified and carried

- renderer-control-routine-owner.log: 2119 core passed, 20 ignored; 513 window
  passed and only three deliberately broken runner assertions failed. All five
  exact pagination cases, pointer expansion, and the new older-edge scroll
  case passed by name. The runner caught its named owner assertion and restored
  routines_panel.rs byte for byte (renderer-11-owner-control-result.log).
- Pagination is now carried. Its own newest-budget control was caught and
  restored earlier. The scroll hook and identity reset are included in this
  run; no scroll performance benchmark or live window is claimed.
- Final green restored workspace verification of both sections follows. The
  routine row remains waiting until all six cases pass after restoration.


## Renderer-11 final restored verification

- renderer-11-final-workspace.log: exit 0, **2119 core + 516 window = 2635
  passed**, **20 ignored** real-agent cases, doc tests 0. All 26 added tests
  were checked by name: 12 work-face rules, 5 exact newest-turn cases plus
  pointer and scroll regressions, and 6 exact unowned-routine cases plus the
  real-pointer runner-choice regression. The disabled-Save failures are fixed.
- Both newest-budget and routine-owner negative controls failed their intended
  named assertions and restored their source byte for byte. working-cycle is
  supplied but was not run. No break remains applied.
- Routine row is now carried; work-face row retains waiting with its exact
  missing integration called out in the manifest. No Electron-plumbing
  exclusions added. This continuation carries **two more full files**, plus
  twelve rule cases in a still-waiting file. Cumulatively **20 of the original
  45 requested files are carried, 25 remain waiting**. The brief is unfinished.
- Recount and --check passed: 197 carried, 62 does-not-apply, 185 partial,
  865 waiting (1309 total). git diff --check passed. All cargo runs were whole
  release workspace runs via gentle-cargo, one at a time, with the in-workspace
  target/TypeScript reader and serial optional-lock configuration stated above.
- Source commits: 224fb389 work-face rule cases; 4f0b45a9 newest-turn paging;
  routine section committed separately after this final green run. Release
  binary build follows; no build pass is claimed before its result.
- Not run: live/packaged window, real agent, Electron/JavaScript gate, other
  platform builds, or a performance benchmark. No handoff file was edited,
  committed or stashed. No unrelated change was discarded.


### Renderer-11 release build and handoff

- Release binary build exited 0 in 1m 32s (renderer-11-release-build.log).
  Twelve existing native warnings, matching the previous section's build.
- Source commits: 224fb389 rule tests, 4f0b45a9 pagination, 636b66a1 routine
  runner/step guard. The final full release suite is green, both mutation
  targets have their production expressions, and no mutation is left applied.
- Final recount/--check, Python control syntax, and diff whitespace checks pass.
  Tracked tree committed clean; only the original untracked HANDOFF files remain.
- **Unfinished:** 25 requested files remain waiting. Continue the teammate
  list before conversation-only files. New-page preview remains the first
  applicable gap. Working-face rules now have twelve verified exact cases;
  carrying that file still requires actual animated stage/screen integration.
  Queued waitFor, id-based teammate work, pets and the other previously recorded
  gaps remain. No additional file is claimed carried or excluded. No live or
  packaged app, real agent, Electron gate, other OS build or benchmark was run.

## Renderer-12 — idle teammate section (in progress)

- Started at b239b517 with a clean tracked tree; the round-3 leftovers in the
  brief were already committed. No stash or discard needed.
- Read the idle-teammate file and IdleTeammate.tsx, its role descriptions,
  mode sentences and App caller. Native's fresh thread now shows its teammate's
  own template starters or role defaults and role description; a Custom role
  uses its saved title. Starter presses dispatch through the existing send gate.
  Existing transcript loading/empty-ledger states remain separate.
- Five exact Electron cases live separately under idle_teammate::tests, drawing
  the actual thread headlessly. The first full run observed all five passing by
  name (2119 core passed, 20 ignored; window still running). That binary predates
  correcting the mode sentence to the composer's effective mode and wiring the
  press to send, as Electron's App does. Final verification remains pending.
- Empty-folder starters use the existing native invented-folder signal and
  template fresh-starter mapping; no filesystem access during drawing.
- Still waiting: inline new-page preview, pet animations, model/effort editor,
  signed-out/pending discovery labels, queued busy-elsewhere dispatch, month/limit
  rows and handoff glances need the surface/host work noted in prior sections.
  Further reads this turn confirmed those applicable gaps, not exclusions.
- One initial compile failed on a String/&str mismatch and ghost_button's &str
  argument; fixed. A path typo in a test-helper edit did not change files. No
  compiler error is counted as a passed test or mutation.
- Idle-own-starters control exited 0 and restored idle_teammate.rs byte for
  byte. The full mutant run passed 2119 core tests (20 ignored) and 520 window
  tests, failing only offers_a_template_teammates_own_first_messages. Log:
  renderer-control-idle-own-starters.log. No break remains applied.

### Next small section — discovery reach (prepared)

- Seven separate cases from a-teammate-is-not-stuck-while-agents-are-still-being-found
  live under teammate_reach::tests, with Electron's ids, version and exact labels.
  Native uses absence of a blocked label to preserve its existing activity,
  instead of introducing another parallel activity/status state machine.
- Shared core reach rules now feed Home/Team urgency and labels, reading only
  already-loaded discovery. The boot sweep remains the fallback before the richer
  answer arrives. No disk reads, probes or process launches were added to drawing.
- The complete restored run includes both sections; neither row is marked carried
  until named results. The 9-case signed-out file remains waiting (runtime choice
  and presence projection are additional cases, not covered by this section).
- Inventory audit found two waiting paths absent from this checkout:
  a-waiting-conversation-is-first-in-the-sidebar.test.tsx and
  an-app-started-conversation-says-so-in-the-sidebar.test.tsx. They remain waiting;
  absence is not an Electron-plumbing exclusion. No external reference was read.
- renderer-12-restored-workspace.log exited 0: 2126 core + 521 window passed,
  20 ignored real-agent tests, doc tests 0. All twelve Electron cases passed by
  name on restored source. The added Home/Team surface regression and effective
  composer-mode regression were written after those test binaries compiled;
  their first verification is the following control run, not this baseline.
- Both new modules are rustfmt-formatted. The pending-reach control temporarily
  treats a partial unusable sweep as a final negative verdict. Final results and
  restored verification follow below; no compile failure counts as a catch.
- Integration review found the full teammate JSON was loaded only when opening
  Team: startup's roster projection omitted template starters/custom titles.
  The existing roster-refresh team read now also caches that JSON (no extra I/O).
  A regression creates a native roster under this workspace's target directory,
  refreshes it without opening Team, and draws Quill's actual idle thread.
  This correction and its regression require the final restored run below.
- Pending-reach control exited 0, restored teammate_reach.rs byte for byte.
  Intentional failures: three core reach cases (2123 core passed, 3 failed,
  20 ignored) and the new Home/Team regression (522 window passed, 1 failed).
  No unrelated assertion failed. The six idle cases then compiled, including
  effective composer mode, all passed. Startup-cache regression was added after
  that compilation and is first exercised by idle-roster-cache below.
- Startup-cache control exited 0, restored main.rs byte for byte. Whole mutant
  workspace: 2126 core passed, 20 ignored; 523 window passed and only the named
  startup-cache regression failed. All seven discovery cases and its Home/Team
  surface regression passed on restored discovery logic. Log:
  renderer-control-idle-roster-cache.log. No mutation remains applied.
- The final idle buttons wrap long starter text within the thread's width,
  retain native focus rings and omit press handlers while blocked/busy. Final
  verification is renderer-12-final-workspace.log, followed by the release build.

### Startup-control correction

- The initial idle-roster-cache run above is REJECTED as mutation evidence:
  its named test failed at the test isolation guard before exercising the cache.
  The following restored run exposed the same fixture-path failure. Neither is
  a green baseline or a valid catch, despite the old control runner's exit 0.
- Corrected the fixture to use the configured runner's disposable TMPDIR,
  as the other native store tests do. No isolation bypass or runner change.
  The renderer control runner now rejects isolation-guard failures explicitly.
  The startup-cache control must be rerun, then source restored and verified.

### Discovery section verified for commit

- All seven exact cases passed by name in the green restored workspace run
  (2126 core, 521 window, 20 ignored). The additional Home/Team regression
  passed on restored reach logic in the subsequent startup-control run. Its
  failure under pending-reach was the intended surface assertion, not isolation.
- The discovery row is carried. Its production code has not changed since that
  green run. Startup-fixture validation is separate, still pending for idle.
- Native keeps activity in its existing state machinery: unknown discovery is
  absence of a proven blocked label, not a second parallel status machine.
  No cases were excluded as Electron plumbing. Signed-out/runtime-choice and
  presence cases in the other file remain waiting.
- Corrected startup-cache control rerun exited 0 and is a VALID catch:
  the named test failed at idle_teammate.rs's prompt assertion. The captured
  headless thread showed Quill with Custom's default prompts instead of her
  three writer prompts. No isolation error. Main.rs restored byte for byte.
  Totals: 2126 core passed (20 ignored), 523 window passed, one intentional
  window failure only. Result: renderer-12-cache-control-rerun-result.log;
  control detail: renderer-control-idle-roster-cache.log.
- Final source verification is now renderer-12-verified-workspace.log. The
  prior renderer-12-final-workspace.log retains the rejected fixture failure.

## Renderer-12 final restored verification

- renderer-12-verified-workspace.log exited 0: **2126 core + 524 window =
  2650 passed**, **20 ignored** real-agent tests, doc tests 0. All twelve
  Electron cases and three additional regressions observed passing by name,
  also checked mechanically against the fifteen new test declarations.
- Idle's five exact case suffixes, in Electron order: offers_a_template_teammates_own_first_messages;
  never_offers_code_to_the_money_or_the_writing_team;
  never_asks_a_non_coder_for_a_page_the_app_would_only_show_as_code;
  says_what_the_role_is_for_not_that_every_role_reads_the_workspace;
  offers_a_hand_made_teammate_its_roles_and_speaks_of_code_only_for_the_roles_that_are_code.
  All live under idle_teammate::tests. Two additional cases cover effective mode
  and startup's actual roster refresh; team_board::surface_tests covers both
  Home and Team with pending and checked missing agents.
- Both rows now carried. No exclusions. **22 of the original 45 requested
  files carried, 23 still waiting. The full brief remains unfinished.**
- All runs use gentle-cargo, one whole release workspace at a time, serial tests,
  optional Git read locks disabled, the in-workspace Cargo target and TS reader.
  No mutation remains. Three valid controls caught their intended regressions;
  the invalid isolation attempt is explicitly rejected above.
- The final suite has the same existing warnings as the baseline: 3 core-test
  warnings and 11 window-test warnings, none in the new modules. Python control
  syntax and git diff --check pass. Release binary build follows.
- Not run: Electron/JavaScript gate, live or packaged window, real agent,
  another OS build, or a performance benchmark. Starter dispatch uses the
  existing send gate; a live starter-button/model turn was not attempted.

### Renderer-12 release build and continuation

- Release binary build exited 0 in 1m 09s (renderer-12-release-build.log),
  with the same 12 existing native warnings and none in this section's files.
- Source commits: **949813d4** pending discovery; **d5994318** idle starters
  and mutation controls. Final verification is committed separately. No push,
  version, changelog, handoff-file or external-reference change.
- Final recount/--check: **199 carried, 62 does-not-apply, 185 partial,
  863 waiting** (1309 total). The requested subset has **23 waiting**;
  22 of its original 45 are carried. No new exclusion or partial classification.
- The task remains incomplete. Continue the waiting teammate list before the
  conversation-only list. Inline new-page preview is still the first applicable
  gap; pets, real face rotation, queued busy-elsewhere dispatch, route/effort
  editor, monthly spending, id-based work accounting and the other prior gaps
  remain. Two inventory paths are absent, as listed above; they were not excluded.
- The three valid mutations are restored. Tracked tree clean after commits;
  only the original untracked HANDOFF files remain. Native-only validation:
  no Electron gate, live/packaged window, real agent, other OS, or benchmark.


## Renderer-13 — inherited ghost section and runtime selection (in progress)

- Read both inherited diffs. They implement all three cases of
  `a-plain-conversation-wears-the-ghost.test.tsx`: tm_atlas/droid/blue at 24,
  anonymous pearl ghost, and the Thinking live row at the epoch. Native's
  actual transcript draws the owner face beside replies and the live row.
  No break was left applied; the plain-ghost control is prepared, not yet run.
- Baseline full release workspace: renderer-13-ghost-workspace.log exited 0: 2126 core + 527 window passed, 20 real-agent cases ignored, doc tests 0. All three ghost tests observed passing by name. This binary predates the runtime-selection changes.
- Revisited inline new-page preview first. It still needs a per-file embedded
  isolated browser frame: activity_drawers renders diffs, while reply_page_ui
  and compare_view only navigate the separate browser panel. A button opening
  that panel would not carry the four Electron inline-preview cases. Row stays
  waiting; no exclusion. Following the brief's small-section preference.
- Next section: first three exact runtime-selection cases from
  `a-signed-out-teammate-is-not-idle.test.ts` in
  home_panel::renderer_runtime_tests. Home and Team now select a recorded
  running runtime, then the newest recorded runtime, then the saved route.
  Ownership uses the cached missionOwners ids, never a display name, and no
  new disk access during drawing. Two extra tests check owner separation and
  the actual Home/Team labels with a ready past Codex mission and signed-out
  Claude saved route. Verification pending; the row stays waiting: six status,
  tone and presence cases are not claimed carried by these selection tests.

- Ghost section committed as **82008602** after the three named baseline passes
  and the valid plain-ghost control. Control exited 0, caught all three intended
  assertions, restored messages.rs byte for byte: 2126 core passed, 529 window
  passed and exactly three intentional failures, 20 ignored. No unrelated failure.
- The five runtime-selection tests all passed by name in that same run (the
  three Electron cases and two extra regressions). Recorded-runtime control is
  now running. Ghost inventory row is carried; runtime row remains waiting.

### Renderer-13 next section — work accounting (prepared, not yet carried)

- Re-read teammateWork.test.ts and teammateWork.ts. The four exact age cases
  were already present in people_panel::renderer_work_age_tests. Added six
  separate exact work cases in core teammate_work::tests, plus a cycle/group
  regression. Values remain m1/m2/m3/m4, tm_atlas/tm_bramble/tm_nobody,
  the September 2026 timestamps, and 100/10 + 200/20 excluding 999/99.
- The Team card now calls this projection, counts by cached host owner ids,
  sums only reported usage (absence stays absent), and lists three newest
  conversations, named from the root's displayed title. No new disk read in
  drawing. The extra headless Team-card test gives unrelated work the same
  teammate display name and requires that its title and usage stay absent.
- Work-owner control prepared. Inventory row stays waiting until all ten
  exact cases and real card regression have passed by name. The summary's
  existing ledger cost projection is used rather than reparsing events in UI.


### Runtime section verified

- **602a5143** commits the runtime selection and its Home/Team integration.
  Five tests passed by name in renderer-control-plain-ghost.log. Recorded-runtime
  control then caught exactly the selection case and its real-surface test,
  restored home_panel.rs byte for byte, and exited 0. Totals: 2133 core passed
  (20 ignored); 531 window passed, two intentional failures. No unrelated failure.
- All ten exact teammateWork cases (six core work tests and four existing age
  tests), its extra grouping/cycle regression, and the new Team-card regression
  passed by name in renderer-control-recorded-runtime.log. Work-owner control
  follows. Neither mutant run is a restored green baseline.


### Work section verified and committed

- **c3fc1654** core projection and exact work tests; **886332c4** real Team-card
  integration and its surface regression. All twelve work/age/group/surface
  checks were mechanically verified by full name in the recorded-runtime log.
- Work-owner control exited 0 and restored teammate_work.rs byte for byte.
  Six intended core assertions failed (2127 other core passed, 20 ignored),
  plus the intended real Team-card assertion (532 other window passed).
  No unrelated failure or compile failure counted as a catch. Log:
  renderer-control-work-owner.log; receipt: renderer-13-work-control-result.log.
- teammateWork row now carried. The restored whole release workspace run is
  renderer-13-restored-workspace.log, running. Release build follows it, serially.
- This continuation carries two files (ghost, work); **24 of the original 45
  carried, 21 still waiting**. Signed-out stays waiting despite its first three
  exact cases being carried. No new does-not-apply or partial classification.

### Renderer-13 named checks

The final restored log will be checked against these 20 full test names:

- `messages::tests::beside_a_reply_the_ghost_when_the_conversation_is_nobodys_the_teammates_own_bot_when_it_has_one`
- `messages::tests::wears_the_ghosts_own_white_as_the_covers_ghost_does_on_the_reply_and_the_live_row_alike`
- `messages::tests::on_the_live_row_the_same_ghost_while_nobodys_conversation_thinks`
- `home_panel::renderer_runtime_tests::is_judged_by_the_route_it_is_set_to_use`
- `home_panel::renderer_runtime_tests::still_prefers_a_running_mission_then_a_past_one_over_the_route`
- `home_panel::renderer_runtime_tests::is_judged_by_nothing_when_it_has_neither_missions_nor_a_route`
- `home_panel::renderer_runtime_tests::home_and_team_judge_the_recorded_runtime_before_the_saved_route`
- `home_panel::renderer_runtime_tests::runtime_ownership_uses_ids_even_when_another_teammate_has_the_same_name`
- `teammate_work::tests::counts_only_the_missions_the_host_recorded_as_theirs`
- `teammate_work::tests::takes_the_last_run_from_the_newest_mission_not_the_first_in_the_list`
- `teammate_work::tests::lists_the_newest_few_newest_first`
- `teammate_work::tests::says_nothing_about_a_teammate_who_has_never_run_rather_than_zeroes`
- `teammate_work::tests::adds_up_what_their_missions_reported_and_only_theirs`
- `teammate_work::tests::uses_the_callers_idea_of_a_title_so_a_briefing_is_not_shown_as_the_words_a_person_typed`
- `teammate_work::tests::recent_work_groups_follow_ups_and_never_spins_on_a_cycle`
- `people_panel::renderer_work_age_tests::reads_in_the_largest_unit_that_fits`
- `people_panel::renderer_work_age_tests::singularises_honestly`
- `people_panel::renderer_work_age_tests::calls_anything_under_a_minute_just_now_including_a_clock_that_ran_ahead`
- `people_panel::renderer_work_age_tests::says_nothing_for_a_timestamp_it_cannot_read`
- `people_panel::renderer_work_surface_tests::team_cards_use_recorded_ids_and_report_only_their_own_usage`

### Renderer-13 continuation inventory

The requested list is unfinished. Waiting teammate files have priority over
conversation-only files; inline new-page preview is still the first applicable gap.
The following 21 rows remain waiting (no exclusions added):

- `a-page-a-teammate-made-runs-in-its-turn.test.tsx`
- `a-pet-plays-what-its-teammate-is-doing.test.ts`
- `a-signed-out-teammate-is-not-idle.test.ts`
- `a-teammate-at-work-goes-round-its-faces.test.ts`
- `a-teammate-can-be-a-pet.test.tsx`
- `a-teammate-has-a-model-of-its-own.test.tsx`
- `a-teammates-eyes-are-drawn-not-typed.test.ts`
- `a-teammates-month-and-limit.test.tsx`
- `every-teammate-is-a-bot.test.tsx`
- `home-fits-two-rows-of-teammates.test.ts`
- `teammate-folder-row.test.tsx`
- `the-folder-chip-says-a-folder-is-no-place-for-teammates.test.tsx`
- `two-teammates-look-at-each-other.test.ts`
- `a-call-shows-on-its-conversation-row.test.tsx`
- `a-phone-lists-conversations.test.ts`
- `a-queued-message-stays-in-its-conversation.test.ts`
- `a-waiting-conversation-is-first-in-the-sidebar.test.tsx`
- `an-app-started-conversation-says-so-in-the-sidebar.test.tsx`
- `an-older-conversation-stays-read.test.ts`
- `review-sees-the-conversation.test.ts`
- `savable-conversations.test.ts`


### Renderer-13 final restored verification

- renderer-13-restored-workspace.log exited 0: **2133 core + 533 window =
  2666 passed**, **20 ignored** real-agent cases, doc tests 0, no filtered tests.
  All 20 full names listed above were mechanically matched to `... ok` in
  this final log, with no FAILED lines. This is the restored green baseline.
- All three controls are restored: plain-ghost, recorded-runtime, work-owner.
  A source audit found each original anchor exactly once and no mutant anchor.
  Source changed only in this workspace's allowed files. The shared target/lock
  is used through gentle-cargo; no filtered run or parallel workspace run.
- Warnings remain the baseline's 3 core-test and 11 window-test warnings.
  No new warning in the added modules. No Electron/JavaScript gate, live or
  packaged window, real agent/model call, another OS, or benchmark was run.
  The new cost tests consume the existing ledger-summary projection; they
  do not claim a new live runtime-usage ingestion check.
- Recount and --check: **201 carried, 62 does-not-apply, 185 partial,
  861 waiting**, 1309 total. Requested subset: **24 carried, 21 waiting**.
  git diff --check and renderer-control Python syntax passed.
- Release binary build exited 0: renderer-13-release-build.log, optimized release finished in 1m 21s, with the same 12 existing native binary warnings. No packaged app or live window was launched.


### Renderer-13 final handoff

- Source commits: **82008602** ghost; **602a5143** runtime selection;
  **c3fc1654** work projection; **886332c4** Team cards.
  Supporting records: **0d92a429**, **624c3314**, **79077d16**; this final
  verification note is a separate commit. No push, merge, version, changelog,
  protected-lane source, handoff-file, or external-reference change.
- No break left applied. All final validation commands have finished.
  Tracked source is committed; the original untracked HANDOFF files remain.
- The full requested port remains unfinished: **21 waiting rows** above,
  with inline new-page preview first. Continue teammate files before the
  conversation-only files. The signed-out file's remaining six status/tone/
  presence cases are not established by the three carried selection cases.

### Renderer-14 continuation — signed-out status (verification pending)

- Initial tracked diff empty: round-3 leftovers already finished in renderer-13,
  including ghost commit 82008602 and the restored named verification above.
  No stash necessary. No break was applied on arrival.
- Revisited inline new-page preview: still lacks a per-file isolated embedded
  frame, so all four cases remain waiting. A separate browser-panel button is
  insufficient. No does-not-apply classification.
- Added the six remaining signed-out status cases in core teammate_reach,
  retaining claude/codex, Docs & QA, Builder, Code & Migrations, pending
  approvals 1, probe-failed/offline and auth-required inputs. The three runtime
  selection cases already exist in home_panel::renderer_runtime_tests.
- Shared status projection now supplies Home/Team labels, tone and presence,
  including distinct amber not-answering and red sign-in states. Board grouping
  consumes the same projection. Cached missionOwners ids replace roster drawing's
  owner disk lookups. Protected look/rig/pet files untouched.
- Extra headless surface regression and signed-out-status control prepared.
  Row stays waiting until every case passes by name.


### Renderer-14 signed-out section verified

- **2e8cb70a** core status projection, six exact cases and break check;
  **31eac1df** real Home/Team integration and surface regressions.
- renderer-14-workspace.log exited 0: 2139 core + 534 window passed,
  20 real-agent cases ignored, doc tests 0. All nine Electron cases and
  home_and_team_draw_the_signed_out_warning_and_presence_from_the_same_status
  were matched to their full named passing lines.
- signed-out-status control exited 0, restored teammate_reach.rs byte for byte.
  Exactly four expected core assertions failed: reads_as_blocked_not_idle_when_its_runtime_is_signed_out,
  is_blocked_when_nothing_at_all_is_signed_in_whatever_it_is_set_to,
  is_not_told_to_sign_in, still_is_when_the_runtime_says_it_is_signed_out.
  Two surface assertions failed: home_and_team_draw_the_signed_out_warning_and_presence_from_the_same_status
  and home_and_team_keep_pending_agents_quiet_and_name_a_checked_missing_agent.
  2135 other core + 533 other window passed; no compiler failure.
- Row now carried: requested subset 25 carried, 20 waiting. No exclusion added.
- The extra cached-owner approval regression and hiding the presence dot for
  idle teammates are included in the final restored run, pending below.
- Remaining teammate gaps examined: the model dialog uses raw Runtime/Model/
  Effort fields instead of the shared picker and reported effort stops;
  the folder row lacks the picker, explanatory text and connector cases;
  folder caution has no native composer question yet; month/limit presentation
  remains absent. These are behavior gaps, not Electron-plumbing exclusions.
  Existing eye/work-face pure tests do not establish all painting/pet cases.
  Respect the protected look, rig and pet files. Conversation rows remain
  waiting behind the teammate files. Inline isolated page preview is still
  the first applicable gap. No unsupported carried claim for these files.
- No push, merge, version, changelog, handoff-file or protected-lane change.
  No Electron/JavaScript gate, live window, packaged app, real-agent/model call
  or other operating system was run.

### Renderer-14 final restored verification

- renderer-14-restored-workspace.log exited **101**, not green: 2139 core
  passed, 20 ignored; 534 window passed, one failed; doc tests 0.
- The nine exact Electron cases and both added surface/owner regressions all
  matched their full named `... ok` lines in this final restored run.
- The failing existing test was
  `window_drive_tests::review_changes_shows_a_teammates_branch_turn_by_turn_and_lands_it_as_one_commit`.
  Its landing commit was refused because its isolated temporary repository
  already held `.git/index.lock` (window_drive_tests.rs:1603). No review-landing
  code was changed. The first baseline passed it; the final failure is kept
  as evidence, not hidden by a filtered retry. No restored whole-suite green
  result is claimed.
- All 32 original renderer-control anchors are present. The signed-out mutant
  is absent; only parity and notes differ after the source commits.
- Recount and --check passed: 202 carried, 62 does-not-apply, 185 partial,
  860 waiting, 1309 total. Requested subset: 25 carried, 20 waiting.
- Python mutation script syntax and git diff --check passed. Release build
  is running; its result follows.

Named checks in the final restored run:
- `teammate_reach::tests::reads_as_blocked_not_idle_when_its_runtime_is_signed_out`
- `teammate_reach::tests::still_reads_as_idle_when_its_own_runtime_is_fine`
- `teammate_reach::tests::keeps_a_mission_waiting_on_an_approval_as_its_own_state_not_blocked`
- `teammate_reach::tests::is_blocked_when_nothing_at_all_is_signed_in_whatever_it_is_set_to`
- `teammate_reach::tests::is_not_told_to_sign_in`
- `teammate_reach::tests::still_is_when_the_runtime_says_it_is_signed_out`
- `home_panel::renderer_runtime_tests::is_judged_by_the_route_it_is_set_to_use`
- `home_panel::renderer_runtime_tests::still_prefers_a_running_mission_then_a_past_one_over_the_route`
- `home_panel::renderer_runtime_tests::is_judged_by_nothing_when_it_has_neither_missions_nor_a_route`
- `home_panel::renderer_runtime_tests::home_and_team_draw_the_signed_out_warning_and_presence_from_the_same_status`
- `home_panel::renderer_runtime_tests::approval_presence_uses_cached_owner_ids_and_stays_distinct_from_a_sign_in_wall`

- Final release binary build exited 0: renderer-14-release-build.log,
  optimized release finished in 2m 09s, with the same 12 native binary warnings.
  No live or packaged window launched.
- Stopping state: no break applied; source, inventory and notes committed;
  only original untracked HANDOFF files remain. The full requested port is
  unfinished: 20 waiting rows in the renderer-13 inventory above, excluding
  the now-carried signed-out row. Continue inline page preview and remaining
  teammate files before conversation-only files. The final full workspace
  suite's review-landing Git-lock failure remains unresolved.


### Renderer-15 continuation — monthly card and limit (verification pending)

- Arrival: tracked diff empty. The round-3 leftovers had already been committed
  by renderer-13; no stash needed and no break applied.
- Inline preview still requires isolated frames embedded in each turn. The
  existing singleton browser panel is insufficient. All four cases stay waiting.
- Continuing the next feasible teammate section: calendar-month receipts and
  monthly limit form. Protected pet/look/rig files untouched. The host usage
  ledger is read through a latest-result worker, on Team open and every 30s
  while open, independently of the window's recent mission rows. Owner ids and
  local calendar months filter receipts. Missing prices on own-model runs stay
  separate from dollars. A failed read reports an error instead of zero.
- Eight pure Electron cases, the filled-dialog case and the summary-only cost
  case have separate Rust tests, plus owner/month, invalid save and read-failure
  surface regressions. Row remains waiting until observed by name.
- The first workspace run overlapped writing the next section; window compilation
  saw the core module before spend_for_month was added and reported E0425. This
  is not a test pass or a mutation catch. A completed-source run follows.
- The shared spend.ts receipt rule additionally requires used tokens and no
  premium-request receipt before counting an unpriced own-model run. The host
  usage projection exposes tokens only from run.completed. Carry that condition
  too; a pending own-model turn is not an unpriced receipt.


### Renderer-15 round-4 recovery

- Applied stash `sol run3 14:04 monthly card + limit (unverified, suite interrupted)`
  with `git stash apply`; retained it. Resolved the main poll and identity-layout
  conflicts by keeping both the upstream behavior and monthly integration. The
  untracked teammate_month.rs on arrival is the stash's accompanying module.
- Debug nextest baseline: renderer-15-month-debug.log, exit 0, 3022 passed,
  20 skipped. All 15 monthly tests below passed by their full names. This
  baseline precedes the added worker wakeup hook and pending-read guard; final
  restored validation must cover those additions.
- Removed the mutation runner's stale TypeScript-reader override, per round 4;
  gentle-cargo supplies its normal default. No protected lane source changed.

Monthly named checks (the first eight pure cases, filled dialog and summary-only
receipt carry the ten Electron cases; the other five are native regressions):
- `teammate_month::tests::is_money_or_no_row_and_this_month_not_a_total_of_whatever_the_window_held`
- `teammate_month::tests::says_it_against_the_limit_whenever_there_is_one_even_at_nothing_spent`
- `teammate_month::tests::says_runs_on_a_model_of_your_own_beside_the_dollars_with_why_unpriced_not_zero`
- `teammate_month::tests::says_premium_requests_beside_the_dollars_never_toward_them`
- `teammate_month::tests::reads_amounts_as_a_person_types_them_and_nothing_as_no_limit`
- `teammate_month::tests::says_a_limit_on_an_edit_only_when_it_changed`
- `teammate_month::tests::refuses_what_is_not_an_amount_rather_than_saving_a_limit_nobody_meant`
- `teammate_month::tests::says_how_the_limit_works_and_what_has_been_spent_when_editing`
- `teammate_month::ledger_tests::counts_only_the_calendar_month_and_owner_without_pricing_a_plan_or_free_model`
- `people_panel::renderer_month_tests::is_in_the_dialog_filled_with_the_limit_already_set`
- `people_panel::renderer_month_tests::month_cards_use_the_calendar_month_and_recorded_owner_and_show_unpriced_separately`
- `people_panel::renderer_month_tests::month_card_names_this_month_when_it_has_money_and_omits_an_empty_month`
- `people_panel::renderer_month_tests::invalid_limits_cannot_save_and_clearing_removes_only_the_limit`
- `people_panel::renderer_month_tests::still_says_what_it_cost_from_the_money_the_host_read_without_events`
- `people_panel::renderer_month_tests::unreadable_months_are_reported_instead_of_claiming_zero_spent`

- Month-limit control exited 0, caught the exact core reached-state and real
  Team-card assertions, and restored teammate_month.rs byte for byte. Core:
  2414 passed, one intentional failure, 20 ignored; window: 606 passed, one
  intentional failure. No unrelated assertion or compilation failure counted.
  Logs: renderer-control-month-limit.log, renderer-15-month-limit-receipt.log.


Further waiting-file inspection (no additional carried claim):
- `an-older-conversation-stays-read.test.ts` is specifically the Electron
  skinny-history IPC merge (`eventsKept`, digest, eventCount and JavaScript
  object identity). Native's `LedgerPane` owns a `ledger::Session` and full
  snapshot independently of the recent-summary map. `begin_ledger` preserves
  that Session for the same conversation; the recents result updates rows and
  summaries, never the open pane. The two IPC merge cases need no native
  implementation. Classification is recorded separately after this section.
- `an-app-started-conversation-says-so-in-the-sidebar.test.tsx` and
  `a-waiting-conversation-is-first-in-the-sidebar.test.tsx` are named by the
  inventory but absent from this checkout (also no local git path history).
  Leave them waiting; exact cases cannot be inferred from file titles.
- `review-sees-the-conversation.test.ts` needs a review-brief projection which
  labels this turn separately from earlier changed paths. No matching native
  brief or those headings found. `savable-conversations.test.ts` similarly
  needs finished-conversation offers and the header's Save as routine gate;
  neither has a native savable projection. Both remain applicable and waiting.

- Month-owner control exited 0 and restored teammate_month.rs byte for byte.
  Exactly the owner/month core regression and real Team-card regression failed;
  all other core and window assertions passed. Log: renderer-control-month-owner.log;
  receipt: renderer-15-month-owner-receipt.log. Both deliberate breaks are gone.
- Monthly inventory row carried only after all ten Electron cases passed by
  full name and both controls were caught. Final release verification follows.


### Renderer-15 final restored monthly verification

- Full release workspace, renderer-15-month-release.log: exit 0, 2415 core
  and 607 window tests passed (3022 total), 20 real-agent cases ignored,
  doc tests 0. Every one of the 15 full monthly names above mechanically
  matched to `... ok`; no FAILED lines. This covers the worker wakeup and
  pending-read guard added after the first debug baseline.
- The shared lock wait was 114 seconds. Release compile took 2m 57s.
  No filtered run, reader override, parallel workspace suite or break remains.
- Both controls failed only their two intended assertions (one core, one
  surface), restored byte for byte, and exited 0. All other tests passed in
  each control run. Release build result follows before committing.
- Native-only brief checks used; no Electron/JavaScript gate, live or packaged
  window, real agent/model call, other OS or benchmark was run. These tests
  use existing host usage projections; they do not claim live usage ingestion.

- Release binary build exited 0, renderer-15-month-build.log: optimized build
  finished in 2m 34s after a 92-second shared-lock wait. Existing warnings in
  drawing/conductor code remain; no new warning in the monthly module or UI.
- Monthly source committed in small groups: core projection/module, Team UI
  and worker integration, then two mutation controls. The stash is retained.
- Recount and --check passed: 220 carried, 63 does-not-apply, 205 partial,
  821 waiting (1309 total), before the separate history-IPC classification.


### Renderer-15 older-history IPC classification

- `an-older-conversation-stays-read.test.ts`: does-not-apply, with a reason in
  parity/tests.tsv. Read both tests, historyMerge.ts and native's begin_ledger,
  LedgerPane, ledger_tick and recents-result path. The first case selects m1
  from IPC rows missing events; the second preserves a JS object while its
  eventCount is unchanged, replacing it when it grows. Both are the window's
  compensation for Electron's partial-history IPC, which native does not use.
- Native opens a local ledger Session that drains its conversation chain and
  keeps its snapshot independently of the recents summary map. No feature was
  removed and no synthetic Rust version of the IPC cache was built. Existing
  Session behavior was read as architecture evidence, not claimed as two new
  exact-value case passes. No additional carried row or new test claimed.
- Source remains exactly the source checked by the restored release suite and
  release build. This metadata-only section needs recount/whitespace validation.

### Renderer-15 remaining work

The full requested renderer port remains incomplete. The current inventory has
14 waiting renderer rows whose filenames contain team or conversation. Applicable
teammate behavior remains first; page isolation, pet selection/painting, model and
folder pickers and short-Home layout still need work. Two sidebar test sources
are missing from this checkout, as recorded above. No unsupported carried claim.

- `a-page-a-teammate-made-runs-in-its-turn.test.tsx`
- `a-teammate-can-be-a-pet.test.tsx`
- `a-teammate-has-a-model-of-its-own.test.tsx`
- `a-teammates-eyes-are-drawn-not-typed.test.ts`
- `home-fits-two-rows-of-teammates.test.ts`
- `teammate-folder-row.test.tsx`
- `the-folder-chip-says-a-folder-is-no-place-for-teammates.test.tsx`
- `a-call-shows-on-its-conversation-row.test.tsx`
- `a-phone-lists-conversations.test.ts`
- `a-queued-message-stays-in-its-conversation.test.ts`
- `a-waiting-conversation-is-first-in-the-sidebar.test.tsx`
- `an-app-started-conversation-says-so-in-the-sidebar.test.tsx`
- `review-sees-the-conversation.test.ts`
- `savable-conversations.test.ts`


### Renderer-15 stopping receipt

- Commits: `2f5ed7db` core month projection and exact cases; `8e072b73`
  Team cards/editor and worker wiring; `5d4b7769` mutation controls;
  `828bb9be` verified monthly inventory/evidence. History-IPC classification
  and this final receipt are committed separately.
- Final recount and --check passed: 220 carried, 64 does-not-apply,
  205 partial, 820 waiting (1309 total). One new carried row, one IPC-only
  exclusion; 14 matching renderer rows still waiting, listed above.
- Final mutation-script AST syntax and git diff --check passed. Both month
  anchors audited present once and mutant text absent. No break remains.
  Original stash retained, no push/merge/version/changelog, protected-lane,
  HANDOFF-file or external-reference changes. All required commands finished.
- Stopping state: tracked changes committed; only the original untracked
  HANDOFF files remain. No live agent, Electron gate, live/packaged window,
  cross-platform or performance verification claimed. Continue the 14 waiting
  rows; do not treat this receipt as completion of the full port.

### Renderer-16 — teammate model dialog (verification pending)

- Arrival tracked tree clean at 25243882. The monthly recovery requested by
  round 4 is already committed and documented above; its stash remains retained.
- Read the six Electron model/effort cases and NewTeammateDialog's route, pick
  and default-effort rules. Reused the real composer picker, including catalog,
  account groups, policy filtering, search, recent routes and limit labels.
  Dialog picks now update the form only. A different model clears the old effort;
  saving an unchanged route-less teammate retains its inherited composer route.
- Native offers the reported effort stops as selectable native chips, with the
  current stop labelled for accessibility; it does not reconstruct an HTML
  range input. Medium is the default when offered, otherwise the middle stop.
- Six separate exact-input cases and one real picker/save isolation regression
  added in people_panel::renderer_model_tests. Inventory stays waiting until
  observed passing by name. Two negative controls prepared.
- First compile attempts hit the shared target's stale core artifact (existing
  teammate_month reported missing), not assertion failures. Applied the brief's
  touch-all-Rust workaround, then restarted the complete debug nextest suite.
- First completed debug run: 3028 passed, one new assertion failed because
  the dialog used the long runtime label `Codex CLI` rather than Electron's
  `Codex`. Corrected the model row to use native's existing short runtime
  labels and catalog model name. Six other new tests passed by name. This
  run is not a green baseline; subsequent controls and restored runs follow.
- Opening the dialog picker clears the composer's popup state immediately;
  cancelling an edit cannot leave that popup open on the conversation.
- `team-model-target` control caught exactly the picker/save isolation
  assertion: 2415 core passed (20 ignored), 614 window passed, one deliberate
  failure. Source restored byte for byte. All six Electron cases passed by
  name in this run; the extra model-switch effort reset case also passed.
  The initial control attempt's stale-core compile error is not a catch.
- First effort-control run was rejected: the shared target ran a different
  worktree's 601-test window binary; none of this section's named tests were
  present. No catch claimed. Added renderer-cargo.py, a thin entry to the
  unchanged gentle-cargo wrapper that performs the brief's source-touch fix
  after its shared lock is acquired, before Cargo starts. Touching before a
  lock wait is insufficient when another tree builds during that wait.
- `team-effort-default` control caught exactly the default-effort assertion:
  2415 core passed (20 ignored), 614 window passed, one deliberate failure.
  Source restored byte for byte. The in-lock touch workaround compiled this
  worktree's 615-test window binary and observed the required named failure.
- Added an inherited-route account regression: choosing an account adopts the
  displayed composer route in the form and leaves the active chat unchanged.
  Account choices use the displayed runtime, rather than stale hidden fields.

Electron case -> Rust suffix in people_panel::renderer_model_tests:
- names the model / own picker -> names_the_model_the_teammate_runs_on_and_offers_the_chats_own_picker
- none of its own / chat box -> says_a_teammate_with_none_of_its_own_takes_the_chat_boxs_until_one_is_picked
- no picker / nothing to press -> offers_nothing_to_press_when_there_is_no_picker_to_open
- own effort / this model's four stops / selected High at index 2 -> states_the_teammates_own_level_with_this_models_stops
- default Medium -> says_the_models_default_when_the_teammate_has_no_level_of_its_own
- no reported levels -> is_not_offered_for_a_model_that_reports_no_levels

Additional regressions:
- picking_a_model_and_effort_updates_only_the_form_and_saves_the_exact_route
- a_different_model_clears_the_previous_level_and_the_same_model_keeps_it
- choosing_an_account_on_an_inherited_route_saves_that_route_without_changing_the_chat

### Renderer-16 restored verification

- Restored debug nextest: renderer-16-model-restored-debug.log exited 0;
  3031 tests passed, 20 real-agent tests skipped. All nine names above were
  observed passing. No filtered suite. Both controls are restored.
- Model row carried after the six exact Electron cases passed by name.
  Recount and --check passed: 221 carried, 64 does-not-apply, 205 partial,
  819 waiting (1309 total). Thirteen matching requested rows remain waiting.
- Release workspace suite and release binary build still pending below.
- Final exact-input audit: the model-name case now uses Electron's empty
  picker catalog, and the inherited-route case creates a new teammate with
  no initial record. Added a readable GPT-ID fallback before discovery lands;
  the model's reported catalog name takes precedence. The earlier release
  run predates this audit; final verification is rerun below.


### Renderer-16 round-4 verification recovery (in progress)

- Read the round-4 restart, round-2 workflow, thread-5 case rules, current
  inventory and all existing picker changes. Left the monthly stash alone.
- Fresh full debug nextest, renderer-16-recovery-debug.log: exit 0,
  3031 passed, 20 skipped. Observed all nine renderer_model_tests by name.
- team-model-target rerun caught its named picker/save isolation assertion
  and restored thread_view.rs byte for byte. Full workspace control, debug;
  receipt renderer-16-recovery-target-receipt.log. Release checks pending.
- Found and fixed an additional applicable picker gap: a new team with no
  composer route could press Change but saw no picker. The catalog now opens
  using Electron's claude/account-default display fallback, without saving
  a route until a real selection. Added the surface regression
  a_new_team_without_a_composer_route_can_still_pick_its_first_model and
  team-first-model negative control. Fresh full validation follows.
- Revisited waiting teammate files first. Inline page needs per-turn isolated
  embedding (the existing singleton preview is not equivalent); pet and glyph
  paint behavior lives in this round's protected look/rig/pet lane; Home needs
  an actual fitted layout rather than an unused copy of homeIsShort.
- Folder-row connector cases cannot be satisfied by UI alone: native.rs
  claude_args explicitly keeps account connectors off because none can be
  ticked yet. A checkbox without runtime enforcement would falsely promise
  access. This main-process implementation is outside this round's owned
  core files. Folder caution also lacks its once-per-folder send gate.
- Conversation inspection: native has no live-call state in the sidebar or
  phone conversation projection; queue.rs carries ten exact isolation cases
  but explicitly lacks busy-elsewhere waitFor wiring. Review brief and savable
  offers remain missing as recorded above. The two missing sidebar sources
  remain waiting. No additional carried/excluded row is claimed.


### Renderer-16 recovery controls and restored debug

- renderer-16-recovery-fallback-debug.log: exit 0, 3032 passed, 20 skipped;
  all ten renderer_model_tests mechanically matched to full named PASS lines.
  This includes pressing Change with no composer route before selecting.
- team-model-target: 2415 core passed, 20 ignored; window 615 passed and
  exactly the intended picker/save isolation assertion failed (before the
  additional route-less case). Receipt: renderer-16-recovery-target-receipt.log.
- team-effort-default and team-first-model: each 2415 core passed, 20 ignored;
  window 616 passed and exactly its intended assertion failed. Both restored
  people_panel.rs byte for byte. Receipts: renderer-16-recovery-effort.log and
  renderer-16-recovery-first-model.log. No unrelated failure counted as a catch.
- Recount and --check passed: 221 carried, 64 does-not-apply, 205 partial,
  819 waiting (1309 total). Thirteen requested renderer rows still waiting.
- Python AST syntax for both fixture scripts and git diff --check passed.
  Final restored release suite and build results follow before any commit.


### Renderer-16 recovery restored release verification

- renderer-16-recovery-release.log: exit 0; 2415 core + 617 window =
  3032 passed, 20 real-agent tests ignored; doc tests 0. All ten full picker
  names mechanically matched to `test people_panel::renderer_model_tests::...
  ... ok`; no named FAILED line. This verifies the final fallback and its
  real Change-button regression as well as the six Electron cases.
- Release build is pending. The shared lock and default TypeScript reader
  were used throughout; no filtered run or reader override.


### Renderer-16 recovery final build and stopping receipt

- Release build exited 0: renderer-16-recovery-build.log, optimized build
  finished in 1m 39s after a 72-second shared-lock wait. Eleven native binary
  warnings remain. No live or packaged window was launched.
- Audited all three new original mutation anchors present exactly once and
  the mutated team-target branch absent. No break remains. A first audit
  incorrectly rejected the normal chat branch's identical assignment; checking
  the team branch and original anchors verifies restoration without that false
  positive. Both fixture scripts parse; diff whitespace and recount passed.
- The full requested port is unfinished: thirteen renderer rows still wait,
  as the renderer-15 list above minus the now-carried model-dialog row. The
  concrete next gaps and protected-lane boundaries are recorded in this recovery.
- No Electron/JavaScript gate, live window, packaged app, real agent/model,
  other OS or performance benchmark was run. The 20 ignored tests are not
  claimed as passes. The monthly stash and HANDOFF files were left alone.
- All required verification commands completed before committing. No push,
  merge, version or changelog change; only this section's owned files changed.

- Recovery source commit: `907540b4` (shared picker, six exact Electron cases,
  four native save/account/first-model regressions). Control/lock-workaround
  commit: `fe1b65a8`. Parity and the complete verification receipt are committed
  separately. Final tracked tree is clean; original untracked HANDOFF files stay.


### Renderer-17 — teammate folder row (in progress)

- Arrival was clean at b32d4ee5; renderer-16 recovery was already committed,
  including named picker checks, controls and release suite/build receipts.
  Left its stash and all HANDOFF files alone.
- Read all thirteen remaining sources available in this checkout; two sidebar
  sources are still absent. Teammate-first inspection confirms protected-lane
  pet/glyph rendering, per-turn page embedding and fitted Home remain gaps.
- The folder dialog had a generic Folder text input. Replaced it with Works in,
  a path/default sentence, native Choose folder/Change and Use the project
  folder. Choices belong to the form until Save; the open workspace is untouched.
  Added Electron's mode summary words, including Auto's whole-machine scope.
- Five separate exact-value folder cases plus a native reset/save isolation
  regression are in people_panel::renderer_folder_tests. The four connector
  cases in the same Electron file are not implemented: this requires native
  runtime enforcement in native.rs, outside the round's owned core files.
  The row cannot be called carried. Verification and controls follow.


Renderer-17 named folder mapping (people_panel::renderer_folder_tests):
- is named in the dialog / project folder -> is_named_in_the_dialog_and_reads_as_the_project_folder_when_it_has_none
- pointed-at folder / way back -> shows_the_folder_it_was_pointed_at_and_the_way_back
- reason in the person's terms -> says_why_the_folder_is_worth_setting_in_the_persons_terms
- permission matches next mission -> does_not_promise_a_permission_the_next_mission_will_not_have
- absent on creation / no chooser -> is_absent_while_a_teammate_is_being_created_or_without_a_host_chooser
- native regression -> resetting_the_folder_changes_only_the_form_until_saved

- Full debug nextest, renderer-17-folder-debug.log: exit 0, 3038 passed,
  20 skipped. All six full folder names observed on PASS lines.
- Conversation sources inspected after teammates: live-call sidebar state and
  phone projection remain absent; review brief has no native author/reviewer
  projection; Routines lacks finished-conversation offers/header action.
  Queue's ten exact isolation cases already exist in src/composer/queue.rs,
  but four busy-elsewhere cases still need waitFor persistence and end-run
  routing. The final Electron source check is window wiring only. No code
  or carried claim was fabricated for these missing product behaviors.


- team-folder-reset control: exit 0, caught exactly
  people_panel::renderer_folder_tests::resetting_the_folder_changes_only_the_form_until_saved;
  2415 core passed, 20 ignored; 622 window passed and one deliberate failure.
  people_panel.rs restored byte for byte. Receipt renderer-17-reset-receipt.log.
- Final permission audit found a route-less form must describe its inherited
  composer mode, not its unused default field. Summary now uses shown_route;
  five mode cases set the composer's mode, with a seventh regression proving
  the inherited mode wins until the teammate has its own saved route:
  permissions_use_the_inherited_composer_mode_then_the_teammates_own_mode.
  The earlier debug baseline predates this correction. Restored release must
  verify the final seven names; the following control is against final code.

- team-folder-mode control: exit 0, caught the five-mode case and the new
  inherited-route regression; no unrelated assertion failed. Source restored
  byte for byte. Receipt renderer-17-mode-receipt.log. Final restored release
  suite is now running. Both original mutation anchors audited present once.

- During the release run an unrelated edit appeared in scripts/gentle-cargo.py
  (shared-target rebuild bookkeeping). This turn did not make that edit;
  preserve it and exclude it from this section's commits. The release run had
  already loaded the prior wrapper, waited 80 seconds, and is compiling this
  worktree. Final stopping status must name the preserved unrelated change.


### Renderer-17 restored release verification

- renderer-17-folder-release.log: exit 0, 2415 core + 624 window = 3039
  passed, 20 real-agent tests ignored, doc tests 0. Mechanically matched all
  seven full folder names and all ten existing picker names to `... ok`.
  This is the final inherited-mode code; no named FAILED line.
- team-folder-mode: 2415 core passed, 20 ignored; window 622 passed and
  exactly two intentional failures (five-mode case and inherited-route
  regression). Both controls exited 0 and restored the source byte for byte.
- Folder row is partial, not carried: five Electron folder cases are covered,
  four connector cases remain. No exclusion claimed. Release build pending.
- Native-only workflow: no Electron/JavaScript gate, real native folder-dialog
  interaction, live or packaged window, real agent/model, other OS or benchmark
  was run. Headless UI verifies row contents, reset interaction and entry
  construction, not the operating system's chooser itself.

- Release binary build: exit 0, renderer-17-folder-build.log; optimized build
  finished in 1m 28s after a 288-second shared-lock wait. Eleven pre-existing
  binary warnings remain. The unrelated wrapper edit was used by this build;
  it is preserved separately and is not part of this section's patch.
- Recount and --check passed: 221 carried, 64 does-not-apply, 206 partial,
  818 waiting (1309 total). Twelve requested rows wait; folder row is partial.
- Both fixture scripts parse; git diff --check passed; both original mutation
  anchors present exactly once. No break remains. All commands completed.


### Renderer-17 — existing conversation queue coverage classification

- Re-read steering.ts and all fifteen cases in the requested Electron queue
  file, plus src/composer/queue.rs and the window's queue send/drain path.
  Ten exact Rust isolation cases already existed. All ten were mechanically
  matched by full name to `... ok` in the final green release workspace run.
  Marked the row partial to reflect those verified cases, not carried.
- No queue source or new behavior changed in this metadata-only section.
  Missing: three queueHome cases and one waitFor verdict case, with persistence
  and real end-run routing. The fifteenth case is Electron App source wiring;
  it is not a substitute for native integration. Existing tests prove only
  the ten isolation rules below, not the missing busy-elsewhere behavior.

Named passes in composer::queue::renderer_conversation_tests:
- is_its_own_rows_in_the_order_they_were_typed
- never_folds_into_another_conversations_message
- still_folds_lines_typed_during_one_run_into_one_message
- takes_that_conversations_rows_folded_and_leaves_every_other_row_where_it_was
- takes_nothing_from_a_conversation_with_nothing_queued
- goes_by_the_row_itself_two_rows_can_be_given_the_same_id
- empty_that_conversations_queue_and_nobody_elses
- the_reviewers_case_gems_line_edited_pips_untouched_each_sent_to_its_own
- sends_each_its_own_as_each_run_ends_in_either_order_the_others_staying_pending
- sends_nothing_the_words_are_back_in_the_box_not_the_queue_and_leaves_the_other_conversation_queued


### Renderer-17 stopping receipt and remaining boundary

- Code/control commit: 283ba1b8. Folder inventory/evidence: 449c4106.
  This metadata-only queue classification is committed separately.
- Final recount and --check: 221 carried, 64 does-not-apply, 207 partial,
  817 waiting (1309 total). Of the requested matching files, eleven wait and
  two are partial (folder and queue); no new carried/excluded row this run.
- All seven folder cases passed by full name in the final release suite;
  all ten picker and ten existing queue names also mechanically matched.
  Both deliberate controls caught and restored; release suite and binary
  build exited 0. Whitespace and fixture-script AST checks passed.
- No source change after the green release suite/build. All this turn's work
  is committed. Preserve the unrelated scripts/gentle-cargo.py edit and all
  original untracked HANDOFF files. No stash operation, push, merge, version,
  changelog, protected-lane or outside-workspace file changes by this turn.
- The handoff remains unfinished. Completing connector enforcement requires
  runtime code the brief reserves for the max lane; pet/glyph cases require
  its explicitly protected pet/face/look files. Do not quietly cross those
  ownership boundaries. Two sidebar test sources are still missing. Other
  owned gaps (per-turn page embedding, fitted Home and folder caution) remain
  applicable work, not exclusions. Conversation live-call/phone, reviewer
  brief, savable offers and busy-elsewhere queue integration are still absent.
- No Electron gate, actual OS folder dialog, live/packaged window, real agent,
  other OS or benchmark was verified. The 20 ignored tests are not passes.

Requested rows still waiting:
- a-call-shows-on-its-conversation-row.test.tsx
- a-page-a-teammate-made-runs-in-its-turn.test.tsx
- a-phone-lists-conversations.test.ts
- a-teammate-can-be-a-pet.test.tsx
- a-teammates-eyes-are-drawn-not-typed.test.ts
- a-waiting-conversation-is-first-in-the-sidebar.test.tsx
- an-app-started-conversation-says-so-in-the-sidebar.test.tsx
- home-fits-two-rows-of-teammates.test.ts
- review-sees-the-conversation.test.ts
- savable-conversations.test.ts
- the-folder-chip-says-a-folder-is-no-place-for-teammates.test.tsx

### Renderer-18 — folder-chip hover (in progress)

- Arrival: d651c7a8, renderer-16 recovery and renderer-17 folder work already
  committed. Preserved the unrelated gentle-cargo.py edit and HANDOFF files.
- Read the remaining folder-caution renderer cases, Composer's hover copy and
  workspace.ts's lexical classification. Native's chip had neither the warning
  nor the ordinary project-path hover; Windows paths also displayed whole.
- The chip now uses the existing native help surface with Electron's words,
  retaining its ordinary composer-chip treatment. Folder names split on both
  platform separators. Classification is lexical only, with explicit platform,
  home and optional system paths; no folder contents or credentials are read.
- Two separate exact-input renderer cases exercise the real native help surface;
  extra path-boundary checks cover case, ancestors, descendants and dot segments.
- The third Electron case (first-send question, Choose another folder before
  Start here) is NOT implemented. Correct integration requires the common send
  boundary and durable once-per-folder confirmation; rendering a disconnected
  card in thread_view would not enforce it. Those shared send files are outside
  this round's owned file list. Row remains waiting until observed named passes,
  then partial, never carried. No confirmation/safety gate is claimed.
- Initial nextest command ran the unchanged 3039-test baseline after a local
  script path error; it is not evidence for these changes. Fresh debug run is
  renderer-18-folder-final-debug.log; controls and release verification pending.
- First changed-source full debug run: 3040 passed, three new tooltip tests
  failed, 20 skipped. Path classification passed by name. Runtime::elements()
  exposes the chip text but not native help text; this is not a passing hover
  receipt. Tests now inspect native accessibility/help metadata instead, matching
  Electron's static title check. The final surface result is still pending.
- One test-harness compile error (capturing closure where Rune needs a function
  pointer) was corrected by keeping the path in runtime state. Neither that
  compile attempt nor the earlier path-error baseline counts as a new pass.

Renderer-18 named mapping (`thread_view::renderer_folder_chip_tests`):
- says why in its hover for a folder like .claude, in the chip's own colour ->
  says_why_in_its_hover_for_a_folder_like_claude_in_the_chips_own_colour
- stays as it was for a project folder (control) ->
  stays_as_it_was_for_a_project_folder_control
- Additional actual-thread integration ->
  the_thread_attaches_the_warning_to_its_effective_folder_chip
- Core regression (`thread_view::folder_help_tests`) ->
  cautions_are_lexical_bounded_and_follow_the_paths_platform

- renderer-18-folder-help-debug.log: exit 0, 3043 passed, 20 skipped.
  All four full names observed on PASS lines. Native help is present in the
  accessibility tree, rather than Runtime::elements()'s painted text list.
  This matches the Electron tests' static title assertions. The production
  chip retains the shared neutral composer-chip style without a caution branch;
  no visual/pixel colour assertion or live tooltip interaction is claimed.
- Inventory marked partial only after named passes; the third renderer case
  and once-per-folder enforcement are still absent. No exclusion claimed.
- folder-chip-help control: exit 0, caught the caution-hover assertion plus
  the project-path and actual-thread integration assertions. No unrelated
  failure: 2416 core passed, 20 ignored; 624 window passed, exactly three
  deliberate failures. Source restored byte for byte. Receipt:
  renderer-18-folder-control-receipt.log; full log renderer-control-folder-chip-help.log.
- Recount and --check passed: 221 carried, 64 does-not-apply, 208 partial,
  816 waiting (1309 total). Requested scope now has ten waiting and three
  partial rows (folder dialog, queue, folder caution). Restored release suite
  and release binary build pending before commit.

### Renderer-18 restored release verification

- renderer-18-folder-release.log: exit 0; 2416 core + 627 window = 3043
  passed, 20 real-agent cases ignored; doc tests 0. Mechanically matched all
  four full names above to `test ... ok` lines; no named failure. No source
  change after this restored run. Release binary build follows.
- Native-only handoff workflow: no Electron/JavaScript gate, real agent,
  live/packaged window, actual tooltip visual/pixel comparison, other OS or
  benchmark run. Explicit Windows strings are tested lexically on this Mac;
  that is not a Windows execution. No first-send confirmation is implemented.

### Remaining scope and ownership decision

- The common first-send boundary lives in src/main.rs (send) and
  src/ui/composer_wiring.rs (submit_composer), neither in the round-4 owned
  list. A correct confirmation needs to preserve the pending draft, bind the
  answer to the still-selected folder and save only that folder's answer;
  a renderer-only card would not enforce this. Ask whether to extend ownership
  to those send files or leave this row partial and continue other owned gaps.
- Requested waiting rows: a-call-shows-on-its-conversation-row.test.tsx,
  a-page-a-teammate-made-runs-in-its-turn.test.tsx,
  a-phone-lists-conversations.test.ts, a-teammate-can-be-a-pet.test.tsx,
  a-teammates-eyes-are-drawn-not-typed.test.ts,
  a-waiting-conversation-is-first-in-the-sidebar.test.tsx,
  an-app-started-conversation-says-so-in-the-sidebar.test.tsx,
  home-fits-two-rows-of-teammates.test.ts, review-sees-the-conversation.test.ts,
  savable-conversations.test.ts. The two sidebar sources are still absent.
- Partial rows remain: teammate folder dialog (connectors need protected runtime
  enforcement), conversation queue (busy-elsewhere routing/persistence), and
  folder caution (first-send card and durable confirmation). Existing pet/face/
  look ownership exclusions remain; no carried/excluded row added this section.

### Renderer-18 final build and stopping receipt

- renderer-18-folder-build.log: exit 0, optimized release build finished in
  1m 12s after a 97-second shared-lock wait. Eleven existing binary warnings.
- Source: 3bdd7cfe; control: 8db51834. Inventory and evidence are committed
  separately. Final recount --check, fixture AST syntax, restoration-anchor
  audit and git diff --check passed. No mutation remains applied.
- All commands completed before committing. No source changed after the green
  restored release suite/build. No stash operation, push, merge, version,
  changelog, protected-lane or outside-workspace file change by this turn.
- This section is verified; the full handoff remains unfinished. Paused at the
  concrete ownership fork above, rather than editing shared-send files without
  an expanded assignment. Preserve the unrelated scripts/gentle-cargo.py edit
  and the original untracked HANDOFF files. No other turn-owned change is left
  uncommitted. This is not a claim that the whole tracked tree is clean.

### Renderer-19 — durable first-send confirmation (in progress)

- Arrival clean at a68f9337, with only untracked HANDOFF files. Prior picker,
  folder-row and hover sections are already committed; no stash operation.
- Round-4 explicitly extends ownership to main.rs and composer_wiring.rs.
  Added a common send guard plus a pre-consumption composer guard; the latter
  preserves attachments, tags and history while the question waits. The card
  binds to the selected teammate and exact open workspace folder. Answers use the
  native store's atomic JSON save before re-entering submission. Descendants
  require separate answers; existing conversations do not ask again.
- The lexical caution is shared with the hover; no credential contents read.
  The native state field and new folder_confirmation module are necessary
  integration for the authorized send-boundary change.
- Initial command used an incorrect relative write path, so the first debug
  attempt failed compilation for the absent module. Second run passed the
  pre-test baseline (3279 tests); it does not verify the six newly added tests.
  Final changed-test run: renderer-19-final-debug.log, pending.
- Changed-test debug attempt ran all 3285 cases but the six new cases were
  refused by the test-isolation guard: fixtures must use LOCUST_TEST_ROOT,
  not notes/. Corrected the fixture to the standard runner-owned disposable
  root; this is a test-harness error, not a product assertion failure.
- Added headless button checks for Start here saving before readiness is
  rechecked, and Choose another folder granting nothing while preserving
  the draft. No actual native chooser or successful agent turn is claimed.

- Electron main's a-conversation-in-a-settings-folder-is-asked-about-once.test.ts
  pins the guard to workspacePath. Native uses the open workspace too; choosing
  another folder therefore uses the existing workspace chooser without silently
  changing a teammate's saved Works in setting. The selected teammate is also
  bound so an old card cannot grant a new selection's pending send.
- renderer-19-eight-debug.log: exit 0, 3287 passed, 20 skipped. All eight
  folder_confirmation::tests names observed on PASS lines. This predates only
  the final workspace-only target adjustment; restored release verifies it.

Named mapping for the third Electron case:
- names the folder and why, with Choose another folder first and Start here ->
  folder_confirmation::tests::names_the_folder_and_why_with_choose_another_folder_first_and_start_here

Additional native cases (folder_confirmation::tests):
- the_first_submit_keeps_the_entire_draft_and_direct_dispatch_also_waits
- an_answer_is_durable_for_exactly_that_folder_and_not_its_children
- changing_folder_or_teammate_invalidates_the_question_without_saving
- a_failed_save_keeps_the_question_and_sends_nothing
- an_existing_conversation_does_not_ask_again
- the_start_button_saves_the_answer_before_submission_rechecks_readiness
- choosing_another_folder_keeps_the_draft_and_does_not_confirm_the_old_one

- The two sidebar sources are recoverable from this repository's history:
  `git show 9718f2fb:apps/desktop/src/renderer/src/a-waiting-conversation-is-first-in-the-sidebar.test.tsx`
  has both nested Needs you cases; `git show 5e108a31:apps/desktop/src/renderer/src/an-app-started-conversation-says-so-in-the-sidebar.test.tsx`
  has four app-origin/filter cases, including the newer accessible plug mark.
  Read both during this run. They are no longer blocked by missing evidence;
  their behavior still needs native integration and named tests. No desktop
  file was restored or changed, and neither row is claimed carried.

- folder-answer-target control: exit 0, caught the named stale-folder/teammate
  assertion and restored folder_confirmation.rs byte for byte. Full log:
  renderer-control-folder-answer-target.log; receipt renderer-19-target-receipt.log.
- The first folder-answer-save attempt did not compile (ambiguous Ok error
  type); runner rejected it and restored source. This is not a caught control.
  Corrected only the mutation's Ok::<(), String> type; rerun pending.
- Re-read the remaining pet and drawn-eye sources. Their production rendering
  paths are src/look/faces.rs, pet_faces.rs and the face/pet core modules,
  which the handoff explicitly reserves. This run has not edited them.
  No row is carried on the strength of helper names or an inspection alone.

- Current-inventory correction: the pets-7 lane has already carried all eight
  a-teammate-can-be-a-pet cases in pet_picks_tests.rs/pet_faces_tests.rs. The
  earlier notes' pet waiting claim is stale. Leave that fuller row intact.
  The drawn-glyph-eye row still waits and targets protected rendering code.
- Corrected folder-answer-save control: exit 0, caught exactly the durable
  reopening and Start-button persistence assertions; 2613 core passed,
  672 window passed with two deliberate failures, 20 ignored. Source restored
  byte for byte. Target control: 2613 core passed, 673 window passed and one
  deliberate stale-answer failure, 20 ignored. No unrelated failure in either.
- Restored release suite in renderer-19-release.log is running against the
  final workspace-only boundary and all eight new tests. No break remains.

### Renderer-19 restored release verification

- renderer-19-release.log: exit 0, 2613 core + 674 window = 3287 passed;
  20 real-agent tests ignored, doc tests 0. Mechanically matched all eight
  folder_confirmation::tests full names above, the three existing
  thread_view::renderer_folder_chip_tests names, and core's
  thread_view::folder_help_tests::cautions_are_lexical_bounded_and_follow_the_paths_platform
  to exact `test ... ok` lines. No named failure. This is the final code.
- Both controls caught named assertions and restored byte for byte; a final
  audit found each original anchor exactly once and neither break present.
  Fixture AST parsing, git diff --check, recount and recount --check passed.
- The folder-caution renderer row is now carried, with all three Electron
  cases observed passing by name. Native uses the shared lexical classifier,
  a typed in-app question and Store::save_json's atomic persistence; it does
  not recreate React/IPC plumbing. No hidden reasoning or credentials kept.
- Release binary build: renderer-19-build.log, pending. No source changed
  after the restored release suite. Commit only once the build finishes.
- Not verified: actual native folder picker result/cancellation, live tooltip
  or visual/pixel colour comparison, successful real agent dispatch, live or
  packaged window, Electron/JavaScript gate, other OS, performance benchmark.
  Headless Choose another folder verifies the request and no permission grant;
  headless Start here verifies persistence and readiness recheck with sending
  disabled. This is not a real model turn or a real operating-system dialog.

### Renderer-19 continuation boundary

- Full handoff remains unfinished. Nine of the original waiting sources
  remain: call row, inline new page, phone conversations, drawn glyph eyes,
  waiting-conversation sidebar, app-origin sidebar, two-row Home sizing,
  review conversation brief, and savable conversations. The teammate folder
  row (connectors) and queued-message row (busy-elsewhere integration) remain
  partial. Other lanes' fuller existing rows are preserved.
- Source absence is resolved for the two sidebar files using the historical
  commits above. No behavior or named pass is inferred from their recovery.
- The remaining drawn-eye cases need the face rendering helpers under
  core/src/face_* and src/look/*, which round 4 expressly forbids this lane
  from editing. Two-row Home is still a real layout gap, and inline pages
  still need per-turn web-view embedding. None is classified as plumbing.
- Ownership decision for the person: keep protected rendering work with its
  current lane and continue only owned gaps, or transfer that work here.
  This turn leaves protected files untouched while that boundary is decided.

### Renderer-19 final build and stopping receipt

- Release binary build: exit 0, renderer-19-build.log; optimized profile
  finished in 1m 01s with 11 existing binary warnings. All required commands
  finished before committing. No source changed after the green release run.
- Source commits, applied together for this section: eb41063b (classifier and
  durable question model), 1bb7fbf0 (state/common send boundary), 7ce3e8e7
  (composer pre-consumption boundary and question surface). Controls: d5ef0f67.
  Inventory and this final evidence receipt are committed separately.
- Final recount/--check: 242 carried, 65 does-not-apply, 213 partial,
  789 waiting (1309 total); one newly carried row this turn, no exclusions.
- Remaining original waiting files:
  a-call-shows-on-its-conversation-row.test.tsx;
  a-page-a-teammate-made-runs-in-its-turn.test.tsx;
  a-phone-lists-conversations.test.ts;
  a-teammates-eyes-are-drawn-not-typed.test.ts;
  a-waiting-conversation-is-first-in-the-sidebar.test.tsx;
  an-app-started-conversation-says-so-in-the-sidebar.test.tsx;
  home-fits-two-rows-of-teammates.test.ts;
  review-sees-the-conversation.test.ts; savable-conversations.test.ts.
  Partial: teammate-folder-row.test.tsx and
  a-queued-message-stays-in-its-conversation.test.ts.
- No break remains, no background command is pending. No stash, push, merge,
  version, changelog, protected renderer/runtime file or HANDOFF change.
  Final tracked tree is committed; the original untracked HANDOFF files stay.
  This is a completed confirmation section, not a completed renderer handoff.

### Renderer-20 — drawn glyph eyes (in progress)

- Arrival tracked tree clean; picker and first-send confirmation sections already
  committed. Round 4 now grants core/src/face_* and src/look/*, except puppets.
- Read the Electron test and Bot.tsx plus native bot_rig/bot_spec/painter.
  Added one Rust test per each of the 19 Electron cases in glyph_eyes_tests.rs.
  Tests use the native vector output in place of Canvas interception.
- First write used a root-relative path while the command ran in apps/native;
  it changed no files, but launched an old 3287-case baseline. A second run
  queued before that was noticed and shared its log; this log is not accepted
  as evidence. Subsequent runs must use distinct logs and wait sequentially.
- Native draw_screen_light currently ignores GlyphMotion.shown, sx and sy.
  The new cursor test exercises the painter as well as the motion helper;
  wait for a named assertion before fixing it. No carried row yet.
- Initial new-case run observed 18 named PASS lines and the intended
  at_work_the_cursor_blinks_and_the_prompt_stays_lit assertion failure.
  No other new case failed. Source now honors visibility and per-eye stretch
  in both normal and power-on drawing; an additional geometric stretch test
  checks that the painter consumes the motion, beyond testing the formula.
- Fixed debug run is waiting on native-engine's release suite. Final source
  formatting and the extra stretch assertion postdate that run's start; the
  controls and restored release suite must verify the final source.
- Next ownership boundary: the waiting call/waiting/app-origin sidebar rows
  need src/ui/sidebar.rs (and likely shared conversation-list/state support).
  Round 4 lists neither sidebar.rs nor conversation_list.rs as owned files.
  No edits made there. Ask for that scope after committing the glyph section.
- renderer-20-fixed-debug.log: exit 0, 3307 passed, 20 real-agent tests
  skipped. All 19 carried-case candidates plus the painter stretch test
  observed on exact PASS lines. The final transform test was then strengthened
  to inspect actual draw_bot_shape output at yaw 0.3 and pitch 0.2; release
  must verify this final test. No production change after that green debug run.
- Both controls exited 0 and restored bots.rs byte for byte. Cursor control:
  2613 core passed, 693 window passed, exactly the cursor assertion failed.
  Stretch control: 2613 core passed, 693 window passed, exactly the additional
  stretch assertion failed. Logs: renderer-control-glyph-{cursor,stretch}.log;
  receipts: renderer-20-{cursor,stretch}-receipt.log. No unrelated failure.
- Post-control AST parsing and anchor audit passed: each original anchor once,
  neither break present. Restored release: renderer-20-release.log, running.
- Native rendering uses a single typed body-space mapper for the front body,
  visor and glyphs, not Electron's intercepted Canvas clip state. The original
  front_plane numeric contract is also tested with Electron's exact values.
- Limits: headless vector output/logic only. No live or packaged window,
  visual/pixel comparison, real agent call, phone session, Electron/JS gate,
  other OS, performance claim, or puppet change. Release suite/build pending.

# Hydrogen: teammate and conversation window tests

## Batch 1 (17 Nov 2026)
- Carried:
  - `an-unknown-record-stays-out-of-the-thread.test.ts` (3 cases): `core/src/unknown_record_tests.rs` on `Transcript::apply` and `signal_rail`.
  - `an-earlier-turn-that-only-messaged-is-not-silent.test.tsx` (2 cases): `core/src/earlier_peer_only_tests.rs` on `Transcript::apply`.
- Partial:
  - `a-finished-message-stops-typing.test.ts` (3 of 4 cases): `core/src/finished_message_tests.rs` on `Transcript::apply` + `thread_view::streaming`. Case 4 (`stepsLine` `schedule` -> `"waited"`) is not built in `activity_summary`.
- Does-not-apply:
  - `a-shorter-window-keeps-the-newest-reply.test.ts`: source-inspection test of `useFollowBottom.ts?raw` for browser DOM `ResizeObserver`.
- Waiting:
  - `a-call-is-said-in-the-thread.test.tsx`: no voice calls in Rust.
- Bugs fixed in `core/src/ledger.rs`:
  - `Transcript::apply` now marks earlier open `Item::Assistant` items as `r#final = true` when a `tool.started` arrives or when a `message.delta` with a different `itemId` arrives (watched 2 tests fail first, then pass).
  - `Transcript::apply`'s empty-turn check now strips `<locust-share>` blocks via `peer_share::strip_share_blocks` before checking whether `Item::Assistant` wrote anything, so a turn whose only text was `<locust-share>` without a `mission.peer` link still emits `"This turn ended without a reply"` (watched test fail first, then pass).
- Breaks: 8 of 8 killed by name (`~/work/scratch/hydrogen/b1-breaks.log`).
