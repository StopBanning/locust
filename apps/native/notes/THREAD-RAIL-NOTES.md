# The prompt rail, the way down, and time markers in the Rust thread

Notes on porting Electron's three thread-navigation features into `apps/native/` to match:

- `apps/desktop/src/renderer/src/promptRail.ts` and `apps/desktop/src/renderer/src/components/PromptRail.tsx`
- `apps/desktop/src/renderer/src/components/JumpToBottom.tsx`
- `apps/desktop/src/renderer/src/components/TimeMarker.tsx` and `apps/desktop/src/renderer/src/missionView.ts`

## What was built

- **Time markers & rail clock (`apps/native/core/src/time_marker.rs`, `apps/native/core/src/ledger.rs`, `apps/native/src/ui/time_marker.rs`)**:
  - `time_marker.rs` in `locust-core` implements `QUIET_GAP_MINUTES` (`2`), `clock_of`, `clock_time`, `elapsed_in_label`, `minutes_between`, and `thread_markers`.
  - `ledger::Snapshot` now records `timings: Vec<TurnTiming>` in `Session::poll()` (each turn's `before_item` offset in `Snapshot.items` and its first/last event `occurredAt` timestamps).
  - `messages.rs` inserts `time_marker::view` (`role="separator"`, hairlines on both sides, centered mono `{day} · {HH:MM} · {elapsed} · {note}`) above turns with a gap of at least 2 minutes.
- **The prompt rail (`apps/native/src/ui/prompt_rail.rs`)**:
  - Pure logic ported from `promptRail.ts`: `PROMPT_RAIL_MIN` (`4`), `PROMPT_RAIL_ROOM` (`18.0`), `PROMPT_RAIL_KEYS` (`"Ctrl+Alt+Up and Ctrl+Alt+Down"`), `prompt_rail_line`, `prompt_rail_scrollable`, `prompt_rail_room`, `prompt_rail_shows`, `prompt_mark_place`, `current_prompt_mark`, `prompt_rail_step`, and `prompt_rail_motion`.
  - `PromptRailState` tracks the `"messages"` viewport rect, `message_column` rect, relative user-bubble `tops`, current mark, hover index, and 2.4s jump highlight key (`hit_key` / `hit_until`).
  - `prompt_rail::view` pins the 14pt track to the right edge of the transcript when `prompt_rail_shows` holds, draws 6pt/8pt dots, opens an anchored hover tooltip with the stripped first line and `keys_on(PROMPT_RAIL_KEYS, platform, true)` (`Ctrl+Option+Up and Ctrl+Option+Down` on macOS), and jumps with `tx.scroll_key_to(..., 0.0)`.
  - `main.rs` routes `Ctrl+Alt+Up` / `Ctrl+Alt+Down` via `prompt_rail::handle_key` before the general `event.mods.alt` early return.
- **The way down (`apps/native/src/ui/jump_to_bottom.rs`)**:
  - `thread_view.rs` attaches `.on_at_bottom(|s, at| s.thread_away = !at)` to the `"messages"` scroll viewport and pins `jump_to_bottom::view` at the bottom center (`30x30` circle).
  - When `turn_running(s)` is true and not hovered, it shows the 3 cycling thinking dots and accessible label `"Still working. Go to the newest message"`. When hovered or idle, it shows `CHEVRON_DOWN` (`"Go to the newest message"` when idle). Pressing it scrolls to the bottom (`tx.scroll_to_bottom("messages")`).
- **Headless window tests (`apps/native/src/ui/window_drive_tests.rs`)**:
  - `long_conversation_shows_prompt_rail_time_markers_and_pressing_mark_jumps`
  - `scrolling_up_shows_way_down_working_says_so_and_pressing_returns_to_bottom`

## Judgement calls

1. **Zero-rerun scroll measurement with `on_layout_tx` + `tx.live_only()`**:
   - Instead of re-running `view()` on every scroll frame, `messages` viewport, `message_column`, and each `Item::User` bubble use `on_layout_tx` and call `tx.live_only()` whenever sizes and `current_prompt_mark` have not changed. `view()` only re-runs when the active mark changes or the thread resizes.
2. **Turn timings on `ledger::Snapshot` rather than adding timestamps to `ledger::Item`**:
   - Keeping `TurnTiming` as a parallel vector of `(before_item, first_at, last_at)` on `Snapshot` avoids changing every `Item::User` / `Item::Assistant` pattern match across the codebase while giving `messages.rs` exact turn boundaries.
3. **Scroll-bar width passed as `0.0` in `prompt_rail_room`**:
   - Rune's `ScrollSpec` draws overlay scrollbars inside the viewport without shrinking `clientWidth` relative to `offsetWidth`, so `prompt_rail_room(thread_right, 0.0, 0.0, column_right)` measures the actual clear margin between `message_column` and the viewport's right edge.
4. **Pure logic in `locust-core::time_marker` and `ui::prompt_rail`**:
   - `time_marker.rs` lives in `locust-core` (no Rune dependency) so `ledger::Session` and unit tests can share it cleanly; `prompt_rail.rs` keeps all threshold/formatting/key-matching functions pure and unit-tested alongside the Rune view.

## Which TypeScript tests came over

- `a-long-conversation-has-a-rail-of-your-messages.test.tsx`: all 4 test cases ported as unit tests in `ui::prompt_rail::tests` (`gives_rail_to_four_messages_when_scrollable_and_room_suffices`, `keeps_marks_in_order_and_current_is_nearest_top`, `jumps_with_ctrl_alt_up_and_down_only`, `takes_hover_line_from_message`) plus the headless window test.
- `the-way-down-says-a-run-is-still-working.test.tsx`: all 3 test cases ported as unit tests in `ui::jump_to_bottom::tests` (`wears_thinking_dots_while_working_and_says_so`, `plain_chevron_when_idle_and_absent_at_bottom`, `brings_chevron_back_on_hover`) plus the headless window test.
- `959-minutes-is-not-a-duration.test.ts` & `the-rail-clock-is-made-once.test.ts`: ported to `locust_core::time_marker::tests` (`elapsed_in_reads_minutes_hours_and_days`, `names_the_day_when_a_turn_has_crossed_one_and_not_before`, `says_how_long_the_silence_was_and_skips_short_gaps`, `rail_clock_formats_valid_iso_and_rejects_invalid`) plus `ui::time_marker::tests`.
- `a-mac-says-cmd.test.tsx` (prompt-rail parts): `jumps_with_ctrl_alt_up_and_down_only` verifies `keys_on(PROMPT_RAIL_KEYS, "darwin", true) == "Ctrl+Option+Up and Ctrl+Option+Down"` while `prompt_rail_step` listens for `Ctrl` alone.

## What was left out

- **Reduced-motion runtime sync on `PromptRailState`**: Rune's `tx.scroll_key_to` already respects `Runtime::reduced_motion` natively, so the jump itself does not animate when reduced motion is enabled; `s.prompt_rail.reduced_motion` defaults to `false` for the 2.4s bubble border flash.
- **Compare view**: `apps/native` has no side-by-side compare column yet, so `compare` is passed as `false` to `prompt_rail_shows`.
