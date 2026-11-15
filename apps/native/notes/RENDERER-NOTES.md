# Renderer test port — round 1

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
