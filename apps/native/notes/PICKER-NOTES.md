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

### Part 3, fourth: managed controls say who set them (done, 18 Nov)
- `src/ui/settings_panel.rs`: `org_policy_section` draws `"Managed by your organization"` (or `role="alert"` on error) with `policy.path` and `org_policy::setting_words`; `door_section` disables `otherApps` with `"Set by your organization."`.
- `src/ui/phone_section.rs`: `managed` adds `"Set by your organization."`.
- `src/ui/thread_view.rs`: `mode_menu` filters modes with `org_policy::mode_allowed`.
- `src/ui/people_panel.rs`: `edit_form` shows `"Set by your organization"` and `"Maximum {cap:.2} USD. You can set a lower amount."`.
- Tests passed: `managed_controls_tests::*` (3).
- Breaks (`part2b-ui.breaks`): all 5 killed by their named tests.

### Part 4: the rest of the thread's waiting rows (done, 18 Nov)
- `core/src/route_rows.rs`: `row_detail_repeats_heading`, `quiet_minutes` (`QUIET_MINUTES_BEFORE_SAYING_SO = 5`), and `place_menu` (`placeMenu`).
- `core/src/effort_levels.rs`: `swarm_effort_for` (`swarmEffortFor`, all 8 Electron cases).
- `src/ui/thread_view.rs`: `model_picker` omits detail lines where `row_detail_repeats_heading` is true (keeping hover tooltips) and omits `READY` tags; restored queue strip + `queue_error` already present and tested.
- `src/ui/free_offer.rs`: `view_at` draws `"no word for {mins} min"` + `Stop` (`quiet-turn-row`, `quiet-turn-stop`) when a turn has been quiet >= 5 minutes without a running tool.
- Partial rows: `a-context-menu-stays-in-the-window.test.tsx` (`place_menu` carried, no right-click `ContextMenu` in Rust), `swarm.test.ts` (`swarm_effort_for` carried, no swarm toggle in Rust composer), `main/three-placements.test.ts` (`peer_thread` already says `"{from} sent"`, mission strip / left-ruled note register not in Rust).
- Tests passed: `route_rows::tests::*` (30), `effort_levels::tests::*` (19), `route_picker_tests::*` (16), `free_offer::tests::*` (4).
- Breaks (`part3-core.breaks`, `part3-ui.breaks`, `part3-native.breaks`): `quiet_minutes`, `place_menu`, `swarm_effort_for`, `row_detail_repeats_heading`, and `free_offer` `quiet-turn-row` killed by their named tests.

## Round 3 (18 Nov 2026)

### Part 1: the right-click menu and three placements (done, 18 Nov)
- `src/ui/context_menu.rs` (new): the right-click menu (`ContextMenu.tsx`). Own surface, not rune-kit's `Menu`: the kit
  has no accessible name for the menu and draws shortcuts as given. Placed by `route_rows::place_menu` from its measured
  size (`cx.layout("row-menu")`, an estimate before the first layout) inside `app-body`. No title bar (the title is the
  `role="menu"` label), upper-case key at each row's end that chooses the row, hairlines where `divider_above` and
  index > 0, Delete red and arming "Delete for good?" before acting, lists behind `›` opened on hover or press, refused
  rows greyed with the reason drawn under the label (no `›`). Escape (modal) or a press on the scrim closes it.
- `src/ui/sidebar.rs`: `mission_row` opens `context_menu::conversation_menu` on `on_secondary_press` at the pointer:
  Open, Open beside, Rename, Pin/Unpin, Settle/Bring back, Snooze ›, Move to project ›, Save the record…, Copy its
  record id, Delete (refused while running). The DOTS inline `org_menu` stays as it was (its tests use its keys).
  Not in the Rust menu: Assign to and Save as routine (no such actions in Rust yet). Not wired: rail teammates, rooms,
  Changes panel (git lane), file viewer, activity cards (reply lane).
- `core/src/mission_strip.rs` (new): `mission_strip(runtime, model, phase, cost_tail)`; `thread_view.rs` draws
  `thread-header-route` as runtime · strip · "runs in Locust for now" (without calling `agent_line`).
- `src/ui/peer_thread_tests.rs` (new): the four attribution cases on `cards_for_mission`.
- three-placements stays partial (7 of 9): the note-vs-marker register is drawn in `src/ui/messages.rs` (reply lane).
- Tests passed: `context_menu::tests::*` (9), `peer_thread::tests::*` (4), `mission_strip::tests::*` (3),
  `route_picker_tests::the_conversations_header_names_the_model_and_says_the_phase_only_when_it_is_news`.
- Breaks (`~/work/scratch/picker3/part1-*.breaks`, `c1`, `c2`): all 13 killed by their named tests.
- Gates: nextest 4518 passed; `test --release --workspace` 3520 + 998 passed; `build --release` ok.

### Part 2: Settings headings held to the index (19 Nov, picked up from the lane by the Mac chat)
- Every Settings section title is a heading (`role("heading")`, level 2): `section_heading` in `settings_panel.rs`
  and the titles in agents, backup, chat apps, phone, scout, trash, feedback and quit-guard.
- `src/ui/settings_pages_real_tests.rs` (new, Electron `settings-pages-are-real.test.ts`, 3 of 3 + one of ours): every
  page drawn headless; its level-2 headings equal its index entries; nothing indexed goes undrawn; each page says its
  own name once besides the nav.
- Found by it: Project folder drew no "Project folder" heading, only the native-only "Native data folder" block as one.
  Now the heading opens the page (Electron's h2); the data-folder block is a level-3 sub-heading.
- Five pages whose name is also a heading (Between teammates, Your own models, Connectors, Cloud computers, Project
  folder) said the name twice: once as the plain page title and once as the heading. The plain title is now skipped.
- `core/src/org_policy.rs`: `use_policy_on_this_thread`. The page test needs a policy for "Managed by your
  organization", and the process-wide one made the door tests fail under `cargo test` (one process). Tested in
  `org_policy_tests::a_policy_on_one_thread_is_not_seen_on_another_and_leaves_when_cleared`.
- Breaks (`~/work/scratch/pick3-breaks*.txt`): 5 of 5 killed by the page tests.
- `src/settings_search_words_tests.rs` (new, `settings-search-knows-the-words.test.ts`, 9 of 9): the index already
  matched Electron's; breaks 3 of 4 killed, the survivor (dropping "worktree") is equal since "worktrees" contains it.
- Auto mode (Teammates) was a "runs in Locust for now" placeholder. Now `auto_mode_section`: heading, "Set by your
  organization." when `mode_allowed(.., "auto")` is false, the on/off sentence, and a switch named "Auto mode" that
  flips `auto_mode` and saves (drawn but deaf when held off). Electron's "How it works" fold is not drawn: Rust has no
  fold yet (settings-read-like-claudes).
- `src/ui/settings_switches_tests.rs` (new, `every-settings-switch-says-what-it-is.test.ts`, 3 of 3 + 2 of ours);
  breaks 6 of 6 killed.
- Project settings dialog (`group-settings-dialog.test.tsx`, 5 of 5 by name in `group_settings_dialog.rs`). Two
  differences fixed: the route was "Runtime / Model · Mode · level" from the catalog's names, and is now Electron's
  `routeLabel` (`core/src/teammate_tooltip.rs` `route_chrome`, new, with Electron's values from four test files in
  `route_name_tests`); with a route picked, the foot said the picked route, and now names the composer's.
  Breaks 9 of 9 killed (two only by the core test).
- Stay awake (Startup): Rust said its own subtitle ("Prevents idle sleep on AC power...") and never Electron's note.
  Now Electron's one sentence names the row and the switch, and "Only the sleep that comes when nobody uses it..."
  shows while it is on. `stay_awake_tests` holds Electron's case by name; breaks 2 of 2 killed.
- Remote Control (`a-remote-control-switch-shows-claudes-own-words.test.tsx`): the card already matched; 6 of 6 cases in
  `src/ui/remote_control_card_tests.rs` (new); breaks 5 of 5 killed.
