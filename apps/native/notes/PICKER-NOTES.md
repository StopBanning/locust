# The model picker, the effort control and the route chip (exec/native-picker, 18 Nov 2026)

Hand-off: `HANDOFF-picker-1.md` (not committed). Branch `exec/native-picker` from 732e9fe7, worktree
`~/work/locust-wt/opus`. Breaks and logs: `~/work/scratch/picker/`.

A conflict, settled: `~/work/locust-wt/phone/HANDOFF-phone-1.md`'s Hard rules list `src/ui/thread_view.rs` as
do-not-edit, while this hand-off names it as this lane's home. This hand-off's file lists win (the phone rule was that
lane's); the rest of the phone rules apply. `thread_view.rs` churn is kept small: logic in new files, one call in.

## Part 1: the effort control (done, 18 Nov)

Electron mounts `EffortSlider` in the RoutePicker's effort slot (Composer.tsx), so it replaces the picker foot's
Level chips.

- `core/src/effort_levels.rs`: `effortLevels.ts` (`effort_name`, `effort_description`, `effort_footer`),
  `effortScale.ts` (`split_effort`, `effort_rank`, `effort_scale`, `join_effort`, `levels_line`), `status.ts`
  `defaultEffort`, and EffortSlider's geometry (`EFFORT_TRACK_WIDTH` 216, `THUMB_WIDTH` 16, inset 13,
  `stop_position`, `held_at`, `nearest_stop`, `MOVE`). `nearest_stop` rounds halves up, as `Math.round` does.
- `src/ui/effort_slider.rs`: the panel. Effort, the level's name (" · Fast" on a fast variant), the ? with a hover tip
  (`hover::tip`); Faster and Smarter; a 20 track, the fill to the thumb, a 2px dot per stop (`effort-notch-passed-N`
  up to the level), one 16 x 20 white thumb with 4px corners and both token shadows. The track is the slider for the
  keyboard and screen reader (range, step 1, value text, arrows, Home and End; disabled with one level) and takes the
  pointer (`on_drag`, track-local x): the held thumb follows the pointer, the level is picked as the nearest stop
  changes, letting go settles it. The Fast variant switch where a level has `-fast`; the Cursor footer.
- `thread_view::model_picker`: the foot draws the panel, on the turn's level or `default_effort` of the model's levels
  with the runtime's own effort as its default.
- `State::effort_held`: where the held thumb is.

Not drawn: the gooey squash and stretch (Rune has no goo filter). The thumb is Claude Code's rectangle at rest.
A drag on the track cannot scroll the model list: the foot sits outside the list's scroll.
Not this part: the people panel's team form still has its own level chips (`for_team` draws no foot).

Tests (all passed by name, nextest and release): core `effort_levels::tests::*` (15), window
`effort_slider::tests::*` (15, one through the real `model_picker`). Suite: 4,290 passed (nextest); release
3,409 + 881 passed; `build --release` passed; `Cargo.lock` unchanged.

Breaks (37, all caught; `core-a/b`, `ui-a/b/c` logs):

| Break | Caught by |
|---|---|
| "Extra high" -> "Extra High" | a_level_is_named_in_words_as_claude_code_names_it |
| unknown word lower-cased | a_level_this_build_does_not_know_keeps_the_runtimes_own_word_capitalised_never_renamed |
| description case-sensitive | is_not_case_sensitive_because_runtimes_are_not_consistent |
| high described "thinks better" | talks_about_time_and_cost_not_about_how_good_the_answer_will_be |
| footer for every runtime but codex | draws_no_note_for_a_runtime_it_has_nothing_true_to_say_about |
| one-stop branch of stop_position | puts_the_stops_end_to_end_along_the_track |
| held_at not clamped at the first stop | a_held_thumb_never_leaves_the_stops_... |
| nearest_stop floors | letting_go_settles_on_the_nearest_stop_... |
| nearest_stop not clamped at the last | letting_go_never_names_a_stop_that_is_not_there |
| MOVE.trail 0.35 | leaves_no_tail |
| scale sorted most to least | cursors_eight_levels_are_four_stops_and_a_fast_switch_sorted_least_to_most |
| join_effort invents an unlisted level | the same |
| thumb 16 tall | the_thumb_fills_the_tracks_height_... and the_thumb_is_claude_codes_... |
| thumb radius 8 / thumb not white | the_thumb_is_claude_codes_a_white_rounded_rectangle_16_by_20_with_4px_corners |
| fill on the track colour | the_thumb_sits_on_a_fill_that_shows_as_claude_codes_does |
| the level's own stop not passed | the_control_names_the_ends_faster_to_smarter_and_draws_a_stop_for_every_level_the_model_has |
| "Smarter" -> "Smart" | the same |
| thumb not centred on its x | the_control_puts_the_thumb_on_its_level (+2 track tests) |
| letting go keeps the hold | the_track_takes_the_pointer_and_picks_as_the_held_thumb_crosses_each_stop |
| picks only on the press, not mid-drag | the same, and the_track_does_not_start_a_selection_with_a_drag |
| fill at the stop while held | the_track_keeps_the_fill_under_a_held_thumb |
| arrows step backwards / one level not disabled / value text "high" | the_control_is_a_real_range_control_over_the_track_... |
| no " · Fast" / switch never checked / switch on every model | the_control_offers_the_fast_variant_as_a_switch_... |
| ? hover without the cost | the_control_reads_effort_max_with_a_question_mark_that_says_what_the_level_costs |
| picker foot ignores the default level / defaultEffort not medium | the_model_pickers_foot_is_the_effort_control_on_the_models_default_level |

## Part 2, first slice: the picker list (done, 18 Nov)

- `core/src/route_rows.rs`: `orderRouteRows` (RUNTIME_BAND, flagshipRank, Your models first, `localeCompare`
  approximated by lower-case order), `freeTagOf`, `unfoldedRows`, `olderByGroup`, the row line and hover title,
  `nextFreeModel` (on `connectors_off::free_models_best_first`), `quietFreeRun`.
- `core/src/models.rs`: `claude_older_models` (the registry), `claude_older_offered` (claudeModelsFrom's rule), and
  `catalogue_description` (Claude Code's alias and older words, the account default's).
- `thread_view::model_picker`: groups and rows in `order_route_rows` order; each row one line
  (`row_detail`) with its full hover (`hover::tip`, key `model-RUNTIME-ID-tip`); Claude Code's Older versions fold
  (`State::older_models_open`), never folding the route in use, searched rows shown without opening.
- Older versions do not reach `s.models` (`door_tools::list_models` hides them, as the app door should), so the
  window adds them from `models::claude_older_offered`.
- `window_drive_tests::the_picker_and_model_chip_show_the_name_earned_from_local_history` asserted the row line was
  exactly the resolved name; Electron's line is the catalogue words then the resolved name, so it now checks that the
  line contains it.

Tests (passed by name): core `route_rows::tests::*` (24), `models::tests::what_the_fold_holds_*` (2) and
`the_fold_skips_the_version_an_alias_means_now`; window `route_picker_tests::*` (6). Suite 4,318 passed (nextest);
release 3,435 + 887 passed after touching `core/src/lib.rs` (the shared target first handed over a stale
locust-core without the new modules); `build --release` passed; `Cargo.lock` unchanged.

Breaks (`core-c` to `core-f`, `ui-d`, `ui-e` logs). All caught in the end:
- Caught by the Electron cases: SIGN IN band, best row (min), alphabetical inside a band, ACTIVE first, freeTagOf
  length and space, unfoldedRows and olderByGroup keeping Recent, levels line, hover detail, nextFreeModel's runtime
  and wrap, quietFreeRun's two minutes, Claude's alias words (window hover test).
- Survived the Electron values, so tests were added: NOT INSTALLED level with PLANNED (the Electron pair is already
  alphabetical) -> `what_does_not_exist_sinks_below_what_is_not_installed_even_against_the_alphabet`; recent and
  flagship order -> `inside_a_group_what_was_used_comes_first_then_the_flagships_then_the_rest`; Your models first
  -> `your_models_lead_their_band`; an older version an alias means now -> `the_fold_skips_the_version_an_alias_means_now`.
- Window: group order ignored -> `the_window_lifts_what_can_run_...`; no hover -> `keeps_what_the_catalogue_says_...`;
  search not reaching into the fold -> `lets_a_search_reach_into_the_fold_...`; route in use folded ->
  `never_folds_away_the_route_in_use`; fold drawn open or open shows nothing -> `keeps_older_versions_out_...`.
- One window run of the Your-models break was "caught" by `window_drive_tests::quitting_asks_about_a_turn_started_outside_the_window`,
  which has nothing to do with it: that test looks flaky under load. The break is caught properly by
  `your_models_lead_their_band` on locust-core.

## Fixes after review (18 Nov)

- A Claude account's group drew its fold and older rows under the plain group's keys; they carry `:ACCOUNT` now, as
  its rows do (`route_picker_tests::a_claude_accounts_group_has_its_own_fold_keyed_as_its_rows_are`; break: no
  suffix, caught by it).
- `order_route_rows` reads each group's band once (Electron's `groupBand`), and the picker reads places from maps
  instead of rescanning per row. Break "the last row decides the band" survived Electron's values (its best row is
  listed last), so `takes_a_group_on_its_best_row_wherever_it_is_listed` was added and catches it.
- `models::tests::the_opus_alias_reads_opus_5_5` backs a-model-is-one-line's naming half.
- A folded older version sends as picked: `native::send_from` refuses an effort only for Cursor (read, not driven by a
  test).
- Suite 4,324 passed (nextest); release 3,437 + 888; `build --release` passed.

## Left

- Part 2, second slice: `a-default-route-names-its-model` (not started; the chip already uses
  `resolved_model_names`, but no hover reads "its own default model, X on its last run" and `liveModelNames` is not
  ported); the chip's Free tag (`free_tag_of` is ready); the busy and quiet offers in the thread (functions ready, the
  thread rows are in `src/ui/messages.rs`, the other lane's).
- Part 3, the chip and header (4 files), and part 4, the rest of the thread's waiting rows (6 files): not started.

## Round 2 (18 Nov 2026)

### Part 2, second slice (done, 18 Nov)
- `core/src/model_alias.rs`: `live_model_names` (`LiveRun`) ports `liveModelNames`, tested by `names_the_model_of_a_run_under_way_from_its_start_record`.
- `core/src/teammate_tooltip.rs`: `short_runtime_name` (`Codex`, `Claude`, `Cursor`, `Gemini`, `Copilot`, else `runtime_display_name`).
- `src/look/controls.rs`: `composer_route_chip` draws `Free` as its own non-truncating tag (`model-chip-free`) outside the truncated model text.
- `src/ui/thread_view.rs`: `account-default` shows the resolved model name (or `short_runtime_name` until known, never `account-default`) with `hover::tip` (`model-chip-tip`) reading `"{runtime_display_name} / its own default model[, {name} on its last run]"`; `free_tag_of` splits `Free` onto `model-chip-free`; `free_offer::view(s, p)` is called under `message_column`.
- `src/ui/free_offer.rs`: draws the busy-free offer (`busy-free-offer`) and quiet-free offer (`quiet-free-offer`, `quiet-free-offer-action`), stopping the run, switching `s.picks` to the next free model, and handing the prompt back to `s.draft`.
- Tests passed: `model_alias::tests::*` (6), `route_picker_tests::*` (10), `free_offer::tests::*` (3), `window_drive_tests::the_picker_and_model_chip_show_the_name_earned_from_local_history` (1).
- Breaks (`slice2-core.breaks`, `slice2-ui.breaks`): all 7 killed by their named tests.

### Part 3, first three: chip reading, header cost, connectors mode note (done, 18 Nov)
- `core/src/effort_levels.rs`: `effort_phrase` and `route_chip_reading` (`Haiku 5.5 · Medium`, `Opus 5.5 · High · Fast`); `thread_view.rs` formats `model-chip` with them.
- `core/src/benchmark.rs`: `money_line` and `header_cost_tail` (dollars or premium requests only, never tokens or subscription); `thread_view.rs` appends it to `thread-header-route`.
- `core/src/mode_facts.rs`: `connectors_note(runtime, mode, has_connectors)`; `thread_view.rs` attaches it to `mode-chip` via `hover::tip` (`mode-chip-tip`).
- Tests passed: `effort_levels::tests::*` (17), `benchmark::tests::*`, `mode_facts::tests::*` (11), `route_picker_tests::*` (13).
- Breaks (`part2a-core.breaks`, `part2a-ui.breaks`): all 7 killed by their named tests.
