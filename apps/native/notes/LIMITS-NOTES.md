# "AT LIMIT" and the rules that lift it (17 Nov 2026)

Lane: `~/work/locust-wt/argon`, branch `exec/argon-limits` (from `bc51322e`).

## What was built

- **`core/src/limits.rs` and `core/src/limits_tests.rs`**:
  - `reading_has_quota_left` (`apps/desktop/src/shared/limit-lifts.ts` `readingHasQuotaLeft`): checks that an
    Antigravity usage reading names at least one `(\d+)% left` window and that every window it names has `> 0%` left.
  - `reset_millis` and `limit_has_reset` (`apps/desktop/src/shared/limit-lifts.ts` `limitHasReset`): parses the first
    `resets (\S+)` token as an RFC 3339 timestamp and returns `true` once `at <= now_ms`; a missing or unparseable
    reset time never passes.
  - `limited_runtimes_from_at`, `limited_runtimes_from`, `limited_runtimes_at`, `limited_runtimes`, and
    `limited_runtimes_in_dir` (`apps/desktop/src/main/mission-history.ts` `limitedRuntimesFrom`): reads both the
    native and Electron `mission-ledger` folders (`store.root()` and `store.electron()`), keys each event by
    `agent_accounts::usage_key(adapter, account_from_route(resolved_route_id))`, takes the latest
    `route.limit_detected` (`quota-exhausted`) or `run.completed` event in time across all missions, and drops any
    limit whose named reset has passed at `now_ms`.
  - `plan_out_sentence_at` and `plan_out_sentence` (`apps/desktop/src/main/mission-history.ts` `planOutSentence`):
    returns `"their <Runtime> plan is used up until <weekday, month day, time>."` (or `"their <Runtime> plan is used
    up."` when the limit named no readable reset) while `limited_runtimes_from_at` holds that key at its limit, and
    `None` once a run completes or the reset time passes.
  - `row_tag` (`apps/desktop/src/renderer/src/status.ts` `routeRowTag`): `AT LIMIT` outranks `ACTIVE`, and only a
    selectable row may be `ACTIVE`.
- **`core/src/usage_ledger.rs`**:
  - `limited_runtimes_from` and `limited_runtimes` now delegate to `crate::limits`, so `core/src/native.rs`
    (`out_of_plan`) and `core/src/mission_history_tests.rs` share the reset-lifting rule without duplicating ledger
    scans.
- **Passing `limited` to `agents::row_status` and the model picker**:
  - `src/main.rs` (`--agents`): reads `locust_core::limits::limited_runtimes(&store)` and passes each runtime's limit
    to `agents::row_status(f, agents::integration_of(&f.id), false, limited.get(&f.id).map(String::as_str))`.
  - `src/ui/agents_section.rs`: `request` loads `s.limited_runtimes` from `locust_core::limits::limited_runtimes` and
    retains only limits whose reset has not passed; `ordered(found, &s.limited_runtimes)` passes each agent's limit to
    `agents::row_status`, placing `AT LIMIT` rows in band 2 and drawing the limit sentence under the agent's name.
  - `src/main.rs` (`open_models`) and `src/ui/composer_wiring.rs` (`load_composer_store`): populate
    `s.limited_runtimes` via `locust_core::limits::limited_runtimes`, which the model picker rows in
    `src/ui/thread_view.rs` read for both default and added accounts.
- **`parity/tests.tsv`**:
  - Added `apps/desktop/src/main/a-limit-lifts-when-the-account-says-so.test.ts` as `carried` (`LIMITS`) and
    updated `apps/desktop/src/main/a-reply-waits-for-a-plan-that-is-out.test.ts` to `carried` (`LIMITS`).

## Break checks (mutfilter / mut6)

- `apps/native/core/src/limits.rs`: 14 breaks introduced and verified killed by named tests in
  `limits::tests::*` (covering `reading_has_quota_left`, `limit_has_reset`, `limited_runtimes_from_at`,
  `plan_out_sentence_at`, and `row_tag`).
- `apps/native/src/ui/agents_section.rs`: 2 breaks introduced and verified killed by
  `agents_section::tests::ordered_marks_a_limited_agent_at_limit_with_its_words_and_places_it_in_band_two` and
  `agents_section::tests::section_draws_at_limit_from_the_ledger_and_lifts_it_once_the_reset_passes`.

## Verification

- `python3 scripts/gentle-cargo.py nextest run --workspace --no-fail-fast`: `3771 passed, 22 skipped` (including
  all 20 `limits::tests::*`, all 17 `mission_history_tests::*`, both `agents_section::tests::*`, and
  `tests::an_added_account_is_named_in_the_picker_and_respects_policy_and_limits`).
- `python3 scripts/recount.py --check`, `python3 scripts/marks.py --check`, and
  `python3 scripts/test-ledger.py --check`: exited 0.

## What was not built

- **Antigravity's live CLI usage reading (`apps/desktop/src/main/antigravity-usage.ts` `readAntigravityUsage` and
  `antigravityUsageText`)**: `grep -rn usage core/src/runtime/antigravity*` confirmed the Rust app has no `/usage`
  probe for Antigravity (only step token usage fields in `antigravity_tests.rs`). As instructed by the handoff brief,
  `reading_has_quota_left` is ported and tested in `core/src/limits.rs`, while live limit lifting in the Rust app uses
  the reset-time rule (`limit_has_reset`) and completed runs (`run.completed`).
- **`core/src/native.rs` (`out_of_plan`)**: skipped editing `core/src/native.rs` (owned by another lane); its limit
  lookup delegates through `core/src/usage_ledger.rs` so reset-time lifting still applies.
