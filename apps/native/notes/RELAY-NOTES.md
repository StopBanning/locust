# Teammate Relay, Peer UI, and Handoff Omissions — Notes

## What was built

- **Automatic teammate relay engine (`core/src/relay.rs`, `core/src/native.rs`)**:
  - Ported Electron `main/relay.ts` into `core/src/relay.rs`, wired into `core/src/native.rs` (`RelayHost` backed by `native` runtime starter, workroom store, workspace settings `relay` / `relayHopCap` / `interrupt`, and ledger `Host`).
  - Covers direct shares (`PeerExchange`), busy deferral/coalescing (`held_also` / `also_waiting` + urgent steer/interrupt), meeting barriers (`Meeting` when a run starts >=2 replies), prose answer return (`return_answer`), read-only prompt suffix (`ask`/`plan`) & Cursor widening note, borrowed conversation routes, reply-budget counting/release across hops/closings, diagnostic ledger notes (`RelayNoticeLevel` -> `ledger_write::append_note_with_level`), MCP-run skip, and live relay status (`relay_live_state` / `ExchangeOverview`).
  - Added fake runtime relay scenarios (`relay-roundtrip`, `relay-prose-return`, `relay-busy`, `relay-cap`, `relay-stop`) in `scripts/fixtures/m1-app-server.py`.

- **UI & Settings (`src/ui/`, `src/main.rs`)**:
  - `ui/peer_thread.rs` & `ui/messages.rs`: Inline `PeerThread` cards for teammate shares in conversations.
  - `ui/exchange_strip.rs`: Live exchange header strip with participants, hop count/cap, summed cost, and Stop action (`exchange-stop`) calling `engine_api::api().stop(run_id)`.
  - `ui/handoff_divider.rs` & composer wiring (`ui/thread_view.rs`, `src/main.rs`): Model/runtime handoff divider, carried/reason line, `Left out` checkboxes synced to `left_out_by_you` / `ledger_write::append_edit_check`, respected on next turn via `carry_on::left_out_in_words`.
  - `ui/settings_panel.rs`: Workspace **Relay** settings page persisting `relay`, `relayHopCap`, and `interrupt` via `workspace_settings::set`.
  - `ui/sidebar.rs`: 1-level origin nesting under originating conversation.

## Judgement calls

- **`wake_ledger_pane` on live/relay state transitions (`src/main.rs`)**: `native.rs` clears active teammate/running tracking right before `relay().on_shared()` appends cap-stop notes to the ledger, so `main.rs` resets `pane.due` to `now` when `seen_live` or `relay_live` changes and includes `pane.due.is_finite()` in `pollable` so cap/stop notes load immediately.
- **Stop waits on engine/ledger sync**: Pressing `exchange-stop` issues `engine_api::api().stop(run_id)` without forging optimistic completion in `seen_live`; UI clears the button once `running()` empties and `seen_live` / `ledger_summaries` refresh.
- **Settings persistence**: Reused `workspace_settings::{load, set}` for `relay`, `relayHopCap`, and `interrupt` switches instead of inventing a separate relay config store.
- **1-level sidebar nesting**: Relayed conversations nest at most one hop deep under their `origin` rather than deeper trees, matching Electron's flat-with-origin-grouping layout (`the-flat-sidebar`).
- **Handoff omission state**: Stored checkbox toggles locally in `State.left_out_by_you` (keyed by tip) and persisted as edit checks when sending, keeping `engine_calls.rs` direct-engine-use counts untouched.

## Which TypeScript tests came over

- **Carried (every `it(...)` / `it.each(...)` case covered — 5 files):**
  - `apps/desktop/src/main/a-held-reply-that-cannot-start-tells-every-thread.test.ts` -> `core/src/relay_tests.rs`
  - `apps/desktop/src/main/a-teammate-cut-off-is-not-said-to-have-finished.test.ts` -> `core/src/relay_tests.rs`
  - `apps/desktop/src/main/an-answer-not-written-back-is-brought-back.test.ts` -> `core/src/relay_tests.rs`, `core/src/native/tests.rs`
  - `apps/desktop/src/main/an-fyi-need-not-page-a-model.test.ts` -> `core/src/peer_share.rs`, `core/src/relay_tests.rs`
  - `apps/desktop/src/renderer/src/a-handoff-names-the-models-either-side.test.tsx` -> `src/ui/handoff_divider.rs`

- **Partial (11 files; exact gaps noted per row in `parity/tests.tsv`):**
  - `apps/desktop/src/main/relay.test.ts` -> `core/src/relay_tests.rs` (20 tests), `core/src/native/tests.rs` (4 tests)
  - `apps/desktop/src/main/a-relayed-conversation-sits-under-its-origin.test.ts` -> `core/src/relay_tests.rs`, `src/ui/sidebar.rs` (4/5 `nestedUnder` cases + UI review test)
  - `apps/desktop/src/main/a-reply-outside-an-exchange-lands-in-the-hub.test.ts` -> `core/src/relay_tests.rs`, `core/src/native/tests.rs`
  - `apps/desktop/src/main/an-automatic-reply-stays-on-the-model-picked-in-its-conversation.test.ts` -> `core/src/relay_tests.rs`
  - `apps/desktop/src/main/relay-note-is-writable.test.ts` -> `core/src/ledger_write.rs`, `core/src/native/tests.rs`, `src/ui/window_drive_tests.rs`
  - `apps/desktop/src/main/the-reply-budget-is-counted-right.test.ts` -> `core/src/relay_tests.rs`
  - `apps/desktop/src/renderer/src/a-brief-part-can-be-left-out.test.tsx` -> `src/ui/handoff_divider.rs`, `core/src/native/tests.rs`
  - `apps/desktop/src/renderer/src/a-carried-on-turn-is-said-as-locusts.test.tsx` -> `src/ui/handoff_divider.rs`
  - `apps/desktop/src/renderer/src/a-runtime-switch-shows-where-it-happened.test.tsx` -> `src/ui/handoff_divider.rs`
  - `apps/desktop/src/renderer/src/exchange.test.ts` -> `src/ui/exchange_strip.rs`
  - `apps/desktop/src/renderer/src/the-flat-sidebar.test.ts` -> `src/ui/sidebar.rs`

- **Still `waiting` (0 `it(...)` carried):**
  - `apps/desktop/src/main/peer-exchange-conversation.test.ts`, `apps/desktop/src/main/a-run-end-remembers-first-and-keeps-going.test.ts`, `apps/desktop/src/main/only-the-face-you-talk-to-hops.test.ts`, `apps/desktop/src/main/an-antigravity-relay-reply-says-who-started-it.test.ts`, `apps/desktop/src/renderer/src/only-the-face-you-talk-to-hops.test.tsx`

## What was left out / partial

- **Rooms / Group exchanges**: `RoomExchange` (`renderer/src/room-exchange.test.ts`), room post-key store integration in `the-reply-budget-is-counted-right.test.ts`, and group standing instructions (`main/peer-exchange-conversation.test.ts`) remain unported because Rooms do not exist in `apps/native` yet.
- **Run-end ordering edge cases & host `Starting` updates**: `main/a-run-end-remembers-first-and-keeps-going.test.ts` and `relay.test.ts` host `RelayUpdate::Starting`/`StartSettled` assertions + JS throw harness tests were not ported (`relay.rs` emits those updates, untested in `relay_tests.rs`).
- **Self-loop / empty edge cases**: `nestedUnder` self-loop case (`a-relayed-conversation-sits-under-its-origin.test.ts`), empty-roots `exchange.test.ts` edge cases, and `routineDraft` runtime-switch extraction (`a-runtime-switch-shows-where-it-happened.test.tsx`) were left out.

## Special cases ported from `main/relay.ts` (21 / 21 mutation-verified)

Each case from `scripts/special-cases.py apps/desktop/src/main/relay.ts` was ported into `core/src/relay.rs` and broken individually against `cargo test -p locust-core --lib relay::tests` (restoring `relay.rs` afterward):

| # | Special case in `relay.ts` | Port in `core/src/relay.rs` | Observed failing Rust test when broken |
|---:|---|---|---|
| 1 | `returnAnswer` preserves `relayedTo` | `return_answer` sets `relayed_to: exchange_origin.relayed_to.clone()` | `return_answer_and_close_meeting_preserve_relayed_to_release_budget_and_defer_when_busy` |
| 2 | `returnAnswer` releases budget on `!started` | `if !matches!(result, StartOutcome::Started { .. }) { st.release(...) }` | `return_answer_and_close_meeting_preserve_relayed_to_release_budget_and_defer_when_busy` |
| 3 | `returnAnswer` defers on `busy` | `if let StartOutcome::Busy { pool } = result { self.defer(...) }` | `return_answer_and_close_meeting_preserve_relayed_to_release_budget_and_defer_when_busy` |
| 4 | `returnAnswer` `refused` message wording | `StartOutcome::Refused` emits `"to read on their next run."` | `return_answer_and_close_meeting_preserve_relayed_to_release_budget_and_defer_when_busy` |
| 5 | `startFor` widens `cursor` runtime | `lent.runtime == "cursor"` | `cursor_widens_only_when_machine_cannot_hold_read_only_and_appends_read_only_sentence` |
| 6 | `startFor` widens only `ask` / `plan` | `(lent.mode == "ask" \|\| lent.mode == "plan")` | `cursor_widens_only_when_machine_cannot_hold_read_only_and_appends_read_only_sentence` |
| 7 | Borrow note edit vs read-only suffix | `suffix = if route.mode == "accept-edits" { " and may edit this folder" } else { ", read-only" }` | `conversation_model_is_kept_when_replied_to_and_borrows_when_none` |
| 8 | Read-only reply sentence on `ask`/`plan` | appends `read_only_reply_sentence()` when `route.mode` is `"ask"` or `"plan"` | `cursor_widens_only_when_machine_cannot_hold_read_only_and_appends_read_only_sentence` |
| 9 | `deliverDeferred` releases budget on `!started` | `if !matches!(outcome, StartOutcome::Started { .. }) { st.release(root.as_deref()); }` | `deliver_deferred_requeues_restores_exchange_releases_budget_and_notifies_also_waiting` |
| 10 | `deliverDeferred` re-queues on `busy` | `StartOutcome::Busy { .. } => { st.deferred.insert(...) }` | `deliver_deferred_requeues_restores_exchange_releases_budget_and_notifies_also_waiting` |
| 11 | `deliverDeferred` restores `exchanges` on `started` | `if let Some(ex) = entry.exchange { st.exchanges.insert(...) }` | `deliver_deferred_requeues_restores_exchange_releases_budget_and_notifies_also_waiting` |
| 12 | `deliverDeferred` moves `held_also` -> `also_waiting` | `if let Some(others) = st.held_also.remove(teammate_id) { st.also_waiting.insert(...) }` | `deliver_deferred_requeues_restores_exchange_releases_budget_and_notifies_also_waiting` |
| 13 | `deliverDeferred` tells every held thread on `refused` | `StartOutcome::Refused` notifies all `held_also` askers | `busy_teammate_defers_steers_or_interrupts_and_tells_every_held_thread_on_refusal` |
| 14 | `closeMeeting` preserves `relayedTo` | `relayed_to: meeting.origin.relayed_to.clone()` | `return_answer_and_close_meeting_preserve_relayed_to_release_budget_and_defer_when_busy` |
| 15 | `closeMeeting` releases budget on `!started` | `if !matches!(outcome, StartOutcome::Started { .. }) { st.release(...) }` | `return_answer_and_close_meeting_preserve_relayed_to_release_budget_and_defer_when_busy` |
| 16 | `closeMeeting` defers on `busy` | `if let StartOutcome::Busy { pool } = outcome { self.defer(...) }` | `return_answer_and_close_meeting_preserve_relayed_to_release_budget_and_defer_when_busy` |
| 17 | `onShared` skips MCP-started runs | `if matches!(mission.started_by, Some(MissionStarter::Mcp))` | `relay_skips_mcp_and_deduplicates_recipients_and_tracks_hub` |
| 18 | `onShared` releases budget when `!started` | `if !matches!(result, StartOutcome::Started { .. }) { st.release(Some(&root)); }` | `budget_counts_across_branches_room_posts_and_held_replies` |
| 19 | `onShared` defers on `busy` + takes urgent | `StartOutcome::Busy { pool } => { self.defer(...); if message.urgent { ... } }` | `busy_teammate_defers_steers_or_interrupts_and_tells_every_held_thread_on_refusal` |
| 20 | `onShared` tracks started replies / meetings | `StartOutcome::Started { mission_id } => { started.push(...) }` | `silent_question_brings_answer_back_unless_cut_off_or_fyi`, `meeting_waits_for_all_recipients_and_closes_once`, `return_answer_and_close_meeting_preserve_relayed_to_release_budget_and_defer_when_busy` |
| 21 | `Meeting.origin` preserves `relayedTo` | `relayed_to: earlier_relayed` | `return_answer_and_close_meeting_preserve_relayed_to_release_budget_and_defer_when_busy` |
