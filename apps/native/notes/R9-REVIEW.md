# Round 9 Review

## `list_models` live payload check (`src/engine.rs`)

Checked `parse_models` against the two saved live replies (`list_models-teammate.json` and `list_models-all.json`):

- **Finding 1 (`src/engine.rs`) — `Models` omitted the `teammate` field and allowed rows without a `runtime`**:
  - **What was wrong**: `list_models-teammate.json` is a single object `{teammate, runtime, model, effort, models[{id, name, efforts}]}` while `list_models-all.json` is an array of `{runtime, models[{id, name}]}` with no `efforts`. `Models` did not read `teammate`, and `parse_models` defaulted a missing `runtime` to `""` instead of rejecting malformed rows that lack `runtime`.
  - **Fix**: Added `pub teammate: Option<String>` to `Models`, populated `teammate: optional(row, "teammate")`, required a non-empty `runtime` on each agent row, filtered empty effort strings, and added `parses_live_teammate_and_all_runtime_list_models_shapes` using trimmed copies of both live JSON shapes.

## Findings and fixes

- **Finding 2 (`src/main.rs`) — Stop could appear on an external turn the window did not start**:
  - **What was wrong**:
    1. When a send or carry-on was accepted, `s.own_turn` was always initialized with `tip: None` (even on `start_conversation` or when `carry_on` returned a new mission ID, where `before` was `None`). If our turn finished before the first ledger poll observed `snapshot.running == true` (or if `after_tip` timed out), `s.own_turn` remained `Some(OwnTurn { tip: None, .. })` after `pane.after_tip` was cleared. When an external turn later started in the same conversation, `ledger_tick` bound `own.tip` to that external turn and exposed **Stop**.
    2. `ledger_tick` never cleared `s.own_turn` when a turn finished (`!active`), when `after_tip` timed out, or when a newer external tip replaced `own.tip`.
    3. `detach`, `open_recent`, `fresh_conversation`, `send`, and `carry` did not clear stale/unconfirmed ownership, so in non-ledger fallback mode (`s.ledger.is_none()`) reopening a conversation from Recent while an external turn was working matched `own.conversation` and exposed **Stop**.
    4. `s.stopping.take()` cleared `s.own_turn` unconditionally even if `s.own_turn` had moved to another conversation.
  - **Fix**: Bound `own.tip = Some(started.conversation_id)` immediately when starting a new conversation ID; required `after.is_some()` to track ownership on same-conversation ledger follow-ups; restricted `own.tip` binding in `ledger_tick` to while `pane.after_tip` is active and cleared `s.own_turn` whenever the turn finishes, times out, or changes tip; cleared `s.own_turn` on `detach` (when unconfirmed or non-ledger), `open_recent`, `fresh_conversation`, `send`, and `carry`; and scoped `stopping` completion to the matching conversation ID.

- **Finding 3 (`src/main.rs`) — One-turn model pick could leak across Recent conversations, runtime changes, or after carry-on**:
  - **What was wrong**:
    1. All Recent conversations opened from local ledger summaries share `header.id == "@ledger"` (`LEDGER_TARGET`), and `leave_ledger`, `open_recent`, and `fresh_conversation` never removed `s.picks[LEDGER_TARGET]`. A one-turn pick chosen on one Recent conversation leaked into the next Recent conversation opened (even for a different teammate and runtime).
    2. `carry` did not clear `s.picks` for the active conversation, so an unconsumed composer pick stayed armed and leaked into a later send after the carry-on turn finished.
    3. `apply_team` kept cached `s.picks` and `s.models` even when a teammate was removed or its `runtime` changed in Locust.
    4. In `view`, `current` fell back to `.or_else(|| agents.first())` even for `LEDGER_TARGET` (where `s.models` holds all runtimes), which could display another runtime's model metadata when the ledger's runtime was absent, and `open_recent` wrapped empty `summary.runtime` / `summary.model` strings as `Some("")` instead of `None`.
  - **Fix**: Cleared `LEDGER_TARGET` picks on `leave_ledger`, `open_recent`, `fresh_conversation`, and `carry`; cleared the teammate's pending pick on `carry`, `fresh_conversation`, and `open_recent`; invalidated `s.picks` and `s.models` in `apply_team` when a teammate leaves or changes runtime; guarded the `current` fallback in `view`; and converted empty `summary.runtime` / `summary.model` strings to `None`.

- **Finding 4 (`src/ledger.rs`, `src/main.rs`) — Carry-on card stayed visible on turns that already finished or were carried on, and `carry` raced with in-flight ledger polls**:
  - **What was wrong**:
    1. In `ledger.rs`, `Session::poll` concatenated `reader.transcript.items` from every file in the continuation chain (`root -> child_1 -> ... -> tip`), including `Item::CarryOn` from ancestor turns (`!is_tip`) whose `.jsonl` files ended with a limit event before being carried on into a child mission file.
    2. In `ledger.rs`, `Transcript::apply` allowed a trailing `route.limit_detected` event after `run.completed` or `run.cancelled` (with no intervening `run.started`) to re-enable `carry = true` on a finished turn, pushed empty warning notes when `route.limit_detected` had no `message`, and `try_list_recent` did not mark `route.limit_detected` turns as `running = false`.
    3. In `main.rs`, `message_column` rendered `Item::CarryOn` even while a turn was actively in flight or waiting for its continuation child (`s.flight.is_some()`, `s.sending.is_pending()`, `pane.snapshot.running`, or `pane.after_tip.is_some()`), and `carry` did not cancel `s.ledger_job` (allowing a landing poll from the stopped turn to see `active == false`, clear `s.flight = None`, and drop the `carry_on` reply).
    4. In `main.rs`, the expanded carry-on model list (333 models across runtimes in `list_models-all.json`) was rendered inline without a bounded scroll container, and the composer's `model-menu` scroll container lacked a `.key(...)`.
  - **Fix**: Filtered `Item::CarryOn` out of non-tip readers in `Session::poll`; tracked `finished` in `Transcript::apply` so completed/cancelled runs ignore trailing `route.limit_detected` triggers; skipped empty `route.limit_detected` warning notes; handled `route.limit_detected` in `try_list_recent`; hid `Item::CarryOn` in `message_column` and `begin_ledger` while a turn or continuation is active; cancelled `s.ledger_job` and filtered empty `after_tip` in `carry`; and placed the carry-on model choices inside a keyed `max_h(180.0)` scroll container alongside a keyed `model-menu`.

- **Finding 5 (`src/ledger.rs`, `src/main.rs`) — Potential panics on malformed JSON and background `list_models` error clobbering**:
  - **What was wrong**:
    1. In `src/ledger.rs`, `Transcript::apply` indexed `&event["payload"]`, which panics in `serde_json` if `Transcript::apply` is passed a record whose `event` field is not a JSON object.
    2. In `src/main.rs`, `replay_scene` indexed `saved["team"]`, `saved["teammate"]`, `saved["message"]`, and `saved["answer"]`, which panics if the replay file contains valid non-object JSON.
    3. In `src/main.rs`, `poll` auto-prefetched `list_models` even for teammates with `runtime: None` or while `load_error` was set, and any background prefetch failure overwrote `s.status` with a red error and cached an empty `Vec` that prevented retrying.
  - **Fix**: Replaced string indexing on `serde_json::Value` with `.get(...)` in `src/ledger.rs` and `src/main.rs`; restricted automatic `list_models` prefetch to teammates with a configured runtime when `load_error.is_none()`; only surfaced `model_job` errors in `s.status` when the user has that menu open; and cleared empty cached model lists when reopening the menu.

## Verification

Ran inside `native/`:

- `CARGO_TARGET_DIR=$HOME/work/scratch/native-target-r9 cargo test --release`: **53 passed, 0 failed, 2 ignored** (55 total tests).
