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

## Left (parts 2 to 4)

Not started: the picker (7 files), the chip and header (4), the rest of the thread's waiting rows (6).
