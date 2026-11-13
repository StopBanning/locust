# Rooms, Targeted Mentions, Task Board & Queue Drain — Notes

## What was built

- **Core Rooms Store, Task Board & Host (`apps/native/core/src/`)**:
  - `room_task.rs`: `<locust-task>` block parser/stripper/defanger (`claim`, `done`, `handoff`, `new`, max 8 ops per reply, max 200 UTF-16 task text, `task_key` normalization), `board_lines`, `task_section`, `fitted_task_section` (sheds `done` rows first, then trailing open rows, then shortens/drops under tight budgets so a 8,000-char room post never fails to start), and `row_named_by_post` / `member_named_by_post` / `row_to_claim_at_start`.
  - `room_history.rs` & `briefing.rs` & `people.rs`: `room_history_section` (`ROOM_HISTORY_POSTS = 4`, `ROOM_HISTORY_BUDGET = 4000`, `ROOM_HISTORY_FLOOR = 400`, quoted defanged prior answers, newest-fitting priority so waiting peer messages are never pushed out), wired through `Origin.room_history` into `briefing::assemble_with_room_history` and `people::turn_brief_with_room`.
  - `room_store.rs`: `rooms.json` fail-closed store (`UNREADABLE`, 4 MB cap), `Store::create_new_file_in` atomic writes serialized by mutex, CRUD (`list`, `get`, `create`, `rename`, `remove`, `add_post`), `MAX_ROOM_TEAMMATES == native::MAX_LIVE_TURNS` (8), `start_queued` / `refuse_queued` (omitting empty `"queued"` key), `apply_task_ops` (`new`, `claim`, `done`, `handoff` with no-bounce guard & `handedWithoutNote`), `update_task`, `recipients_of`, `exchange_of_room_post`, and `remove_teammate` hooked from `people::remove_teammate`.
  - `room_tasks.rs` & `rooms.rs`: Run-end handler `on_run_ended` (`NativeRoomHost`) wired from `native.rs` to apply `<locust-task>` moves and drain queued room posts (`MAX_DRAIN_PER_RUN = 8`); `post_to_room` / `start_room_member`; `EngineApi::room_notices` (`InProcess` & `Client` + `"room-notices"` IPC op).

- **Rooms UI & Window Integration (`apps/native/src/`)**:
  - `ui/room_screen.rs`: Rooms list & always-visible sidebar entry (`+ New room`), Create room form with `room_full_note` live cap warning, Room view with inline rename, confirmable remove, Side-by-side column toggle (`min_w(320.0)` horizontal scroll), Sticky post head with `To <names> · <time> · <headline>` + face pips (`not asked` dim state), whole Markdown answers (`ANSWERS_BEFORE_A_LIST = 6`, never folded/clamped), `Reply` / `Build on` / `Merge` / `Open` buttons (Build on / Merge populate draft without auto-sending), Waiting-for-slot strip, grouped `absent_line` & `refusal_notice`, Task board (`DONE of TOTAL done`, single `Actions` menu per task + `+ Add a task`), `@mention` autocomplete/tiles (`mention_at`, `mention_choices`, `without_mention`, `mention_completed`, `names_said`).
  - All `rooms.json` / transcript reads and room mutations run off the UI thread via `Latest` jobs (`load_job`, `action_job`).
  - `ui/sidebar.rs` & `ui/rail.rs`: Always-visible `Rooms` section (`+ New room` + clickable room cards with `room_card_meta`) and compact rail `Rooms` button (`rail-rooms`).
  - `scripts/fixtures/m1-app-server.py` & `ui/window_drive_tests.rs`: Added `room-answers`, `room-targeted`, and `room-queue` fake app-server scenarios + 3 headless window tests.

## Judgement calls

- **Sidebar conversation groups left out**: Per row 45 scope, `GroupSettingsDialog.tsx` and `main/group-store.ts` belong to the separate sidebar groups work; `room_screen.rs` covers rooms, `@` targeting, project instructions/room context, task board, and queue draining, so `parity/rows.tsv` row 45 moves to `partial`.
- **No-exchange Grid/List layout**: `RoomExchange` sequence view (`renderer/src/room-exchange.test.ts` when a room post turns into an automatic peer relay) was not added in this round; room posts show parallel answers, waiting queue, and absent/refused lines directly.
- **No disk/git I/O on UI thread**: Every `room_store::*`, `rooms::post_to_room`, and `ledger::read_transcript` call executes on background threads via `Latest::request` inside `room_screen.rs`, refreshed at startup, on `seen_live` transitions, on `room_notices`, and every 1s while any displayed post is running or queued.
- **Rooms list link on sidebar + rail**: Added clickable `Rooms` header button in `sidebar.rs` and `MESSAGE` icon in `rail.rs` so compact windows (`<= 1199px`) can also reach `RoomView::List` and `New room`.

## Which TypeScript tests came over

- **Carried (17 files):**
  - `apps/desktop/src/main/qa-0365-rooms.test.ts` -> `core/src/room_task.rs` (`4` tests)
  - `apps/desktop/src/main/a-room-member-is-told-the-room-so-far.test.ts` -> `core/src/room_history.rs` (`4` of `6` tests)
  - `apps/desktop/src/main/room-store.test.ts` -> `core/src/room_store.rs` (`12` tests)
  - `apps/desktop/src/main/a-room-task-is-never-bounced-back.test.ts` -> `core/src/room_task.rs` + `core/src/room_store.rs` (`7` cases)
  - `apps/desktop/src/main/a-room-post-asks-who-was-named.test.ts` -> `core/src/room_store.rs` + `core/src/room_history.rs` (`7` cases)
  - `apps/desktop/src/main/room-and-mission-caps-agree.test.ts` -> `core/src/room_store.rs` (`2` tests)
  - `apps/desktop/src/main/room-tasks.test.ts` -> `core/src/room_tasks.rs` (`6` tests)
  - `apps/desktop/src/main/room-queue-drains.test.ts` -> `core/src/room_tasks.rs` (`8` tests)
  - `apps/desktop/src/main/room-answer-is-whole.test.ts` -> `src/ui/room_screen.rs` (`answer_has_no_fold_clamp_or_newline_splitting`)
  - `apps/desktop/src/main/room-answers-side-by-side.test.ts` -> `src/ui/room_screen.rs` (`answers_side_by_side_lay_out_in_a_horizontal_scrolling_row_with_wide_columns`)
  - `apps/desktop/src/main/rooms-are-reachable.test.ts` -> `src/ui/room_screen.rs` (`sidebar_rooms_section_is_present_and_never_gated_behind_nonempty_rooms`)
  - `apps/desktop/src/renderer/src/a-room-names-who-it-asked.test.tsx` -> `src/ui/room_screen.rs` (`targeted_post_draws_answers_reply_to_time_not_asked_faces_and_no_absent_report_for_untargeted`)
  - `apps/desktop/src/renderer/src/a-room-post-can-ask-one-teammate.test.ts` -> `src/ui/room_screen.rs` (`at_mentions_find_offer_strip_autocomplete_and_format_names`)
  - `apps/desktop/src/renderer/src/room-absent-is-a-line.test.tsx` -> `src/ui/room_screen.rs` (`absent_line_refusals_grid_switch_whole_answers_and_task_board`)
  - `apps/desktop/src/renderer/src/room-card-names-who.test.tsx` -> `src/ui/room_screen.rs` (`room_card_meta_names_members_counts_posts_and_dates`)
  - `apps/desktop/src/renderer/src/room-round-two.test.tsx` -> `src/ui/room_screen.rs` (`round_two_offers_build_on_and_merge_without_auto_sending`)
  - `apps/desktop/src/renderer/src/room-too-big-to-make.test.tsx` -> `src/ui/room_screen.rs` (`room_full_note_warns_above_max_and_stays_quiet_at_or_under`)

- **Partial (3 shared files):**
  - `apps/desktop/src/shared/room-task.test.ts` -> `core/src/room_task.rs` (parse/strip/sanitize/`task_key`/`task_section` carried; exhaustive edge cases omitted)
  - `apps/desktop/src/shared/a-room-remembers-what-was-said.test.ts` -> `core/src/room_history.rs` (older-first rendering, defanging & empty-after-strip carried; full truncation/budget sweep omitted)
  - `apps/desktop/src/shared/a-long-room-post-still-starts.test.ts` -> `core/src/room_task.rs` (`fitted_task_section_sheds_done_then_trailing_rows_to_fit_budget` carries core fit/shed behavior; tiny-budget fallback strings left untested directly)

- **Headless window tests (`src/ui/window_drive_tests.rs`):**
  - `making_a_room_and_posting_starts_one_run_per_member_and_shows_answers`
  - `posting_with_target_mention_in_a_room_asks_only_that_member`
  - `posting_past_running_cap_queues_and_drains_as_runs_end`

## Special cases & invariants mutation-verified (7 / 7)

| Check | Location (`core/src/room_store.rs`) | Observed failing Rust test when broken |
|---|---|---|
| `special-cases.py`: `op.kind === 'new'` duplicate guard | `TaskOp::New` (`existing_idx.is_some()`) | `store_apply_task_ops_refuses_invalid_ops_by_name_without_dropping_rest` |
| `special-cases.py`: `op.kind === 'claim'` done guard | `TaskOp::Claim` (`tasks[idx].state == "done"`) | `store_apply_task_ops_refuses_invalid_ops_by_name_without_dropping_rest` |
| `special-cases.py`: `op.kind === 'done'` missing guard | `TaskOp::Done` missing `refused.push(...)` | `store_apply_task_ops_refuses_invalid_ops_by_name_without_dropping_rest` |
| Key invariant: omit empty `"queued"` key | `post_to_json` (`!p.queued.is_empty()`) | `store_leaves_no_queue_key_in_file_once_last_waiting_member_started` |
| Key invariant: no-bounce handoff guard | `TaskOp::Handoff` sender check | `bounce_cannot_hand_task_straight_back_to_sender`, `bounce_can_go_to_third_member_and_rule_follows_latest_hand`, `bounce_handed_by_persists_across_reread_from_disk` |
| Key invariant: `recipients_of` normalized `to: None` | `recipients_of` (`asked.len() == teammate_ids.len()`) | `recipients_leaves_out_non_member_and_is_everyone_when_all_named` |
| Key invariant: room cap agrees with live cap | `MAX_ROOM_TEAMMATES = native::MAX_LIVE_TURNS` | `room_limit_and_live_mission_cap_agree` |

## Review (13 Nov)

- Scripted review: mechanical checks pass (1482 tests, release build 0 warnings, window tests 3/3 three times).
- The room screen keeps every `rooms.json`, transcript and post call in `Latest` jobs; a live room refreshes once a
  second only while it is on screen. `room_notices` is an in-memory drain.
- Four special cases had no test that failed when broken. Added one for each, and each now fails when its line is
  broken: only the last four posts told (`tells_only_the_last_four_posts_of_the_room`), at most eight started per run
  end (`queue_starts_no_more_than_eight_on_one_run_ending`), a member busy elsewhere is Busy not refused
  (`a_member_busy_in_another_run_is_busy_not_refused`), and a removed teammate leaves its rooms
  (`a_teammate_removed_from_the_team_leaves_its_rooms`).
