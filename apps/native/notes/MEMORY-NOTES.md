# Memory screen & hover labels (18 Nov 2026)

## What was built
- `src/ui/memory_card.rs`:
  - `memories_of_conversation`, `memory_card_summary`, `memory_changed_notice` (matching Electron `conversationMemories.ts` / `MemoryCard.tsx` for proposed rewrites, forgets, merges, tidy counts, and refusals).
  - `keep_every` (sequential `Keep all N` with deduplicated refusal reasons).
  - `when_short` (local `%b %-d` formatting for memory dates).
- `src/ui/people_panel.rs` (`memory_screen`):
  - Separate `"Waiting for you"` card at the top of Memory when `status == "proposed"` rows exist, with `"Keep all N"` + `"Anything you keep can be put back."` when 2 or more are waiting.
  - Proposed rows wear the `SPARK` suggestion mark instead of the kept on/off `switch`, and show:
    - Merge: `"Wants to merge these into one: "` + each `"Now: <text>"` + `"Merge them"` / `"Keep them apart"`.
    - Forget / retire: `"Wants to forget this: "` + `"Why: <readable_reason>"` (using `locust_core::memory_tidy::readable_reason`, stripping cited model ids) + `"Forget it"` / `"Keep it"`.
    - Rewrite: `"Wants to change this to: "` + `"Now: <text>"` + `"Keep the change"` / `"Keep the old one"`.
    - New: `"Keep"` / `"Forget"`.
  - Tidy pass offer under `"This folder"` when `mode != "off"` and `here.len() >= 2`: `"Tidy up…"` + `"suggests merges and retirements; nothing changes until you keep it"` (prefixed with `"{N} memories here. "` past `TIDY_NUDGE_AT`).
  - Rewritten kept memories show `"Was: <previousText>"`, `"Put it back"` (calls `people::change_memory(..., "edit", Some(previous))`), and `"changed <Mon Day>"` on the meta line (using `updatedBy ?? by`).
- `src/ui/hover.rs`:
  - Shared `HoverState` + `hover::tip(s, &l, key, label, trigger)` using `rune_kit::Tooltip` with `HOVER_DELAY = 500ms` (`tx.wake_after(HOVER_DELAY)` and `rune::time::Instant`).
  - Wired on the composer route chip in `src/ui/thread_view.rs` via `compare_view::asking_label(s)` for `a-comparison-shows-its-models-on-hover.test.tsx` (both non-blind `"… vs …"` and blind `"Model A vs Model B vs Model C"`).

## Other waiting rows unblocked by `src/ui/hover.rs`
Searching `apps/native/parity/tests.tsv` for `"hover"`:
- `renderer/src/a-model-is-one-line.test.tsx` (effort levels on hover in the model picker)
- `renderer/src/a-pull-request-chip-says-its-state.test.tsx` (full PR hover line)
- `renderer/src/a-routine-names-its-folder.test.tsx` (full folder path on hover)
- `renderer/src/the-header-is-a-row-of-icons.test.tsx` (header icon hover titles)
(Note: `renderer/src/a-comparison-shows-its-models-on-hover.test.tsx` is carried in this batch.)

## Tests & break checks (18 Nov 2026)
- `memory_card::tests` (4/4): `proposed_change_is_named_for_what_it_asks_not_as_something_to_remember`, `notice_names_each_change_for_what_it_is_without_double_period`, `tidy_pass_in_conversation_and_on_screen_notice`, `keep_every_keeps_each_in_order_and_says_every_refusal_once`.
- `people_panel::memory_screen_tests` (8/8): `a_proposed_rewrite_shows_new_words_old_words_and_keep_the_change_or_old_one`, `a_proposed_forget_says_so_with_forget_it_and_keep_it`, `a_new_proposed_memory_uses_keep_and_forget`, `a_merge_suggestion_lists_every_memory_it_replaces_with_merge_them_and_keep_them_apart`, `a_retirement_suggestion_says_why_without_citing_model_ids`, `offers_a_tidy_pass_when_two_or_more_and_mode_is_on_and_nudges_past_limit`, `keep_all_is_offered_for_two_or_more_waiting_and_suggestions_wear_mark_not_switch`, `a_rewritten_memory_shows_what_it_said_before_when_it_changed_and_put_it_back`.
- `hover::tests` (2/2): `resting_the_pointer_shows_the_tooltip_only_after_the_delay_and_hides_when_leaving`, `moving_away_before_the_delay_cancels_the_tooltip`.
- `compare_window_tests::a_comparison_shows_its_models_on_hover` (1/1).
- `mutsafe.py`: 8/8 killed, 0 survived, 0 build/timeout.
