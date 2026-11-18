# Called Routine Links and Remote App Pairing — Notes

## Section 1: Private routine link hook (`core/src/routine_hooks.rs`, `routine_runner.rs`, `routine_host.rs`, `routines.rs`, `src/ui/routines_panel.rs`)

### What was built
- **Encrypted hook store and sync (`core/src/routine_hooks.rs`)**:
  - `RoutineHooks` keeps link passwords and pending webhook batches in `routine-hooks.json` (0600 temp-file protection before writing ciphertext), sealed via `SecretBox` (`v: 2`) and migrating legacy `v: 1` plaintext records before displaying or registering.
  - Refuses link creation without encryption (`CANNOT_ENCRYPT`) and fails closed when stored links cannot be decrypted (`CANNOT_READ`), closing unusable links at the relay via `close_unusable` without touching disk.
  - Keeps retired passwords for `WEEK_MS` (7 days) on link replacement or schedule removal so calls already accepted by the relay still have their passwords redacted (`[link password removed]`).
  - `keeps_key()` checks `routine-hooks.json` without touching `SecretBox` so `sync()` does not prompt the keychain when no hook is kept or scheduled.
  - `hook_note(batch, step)` fences untrusted `contentType` and `body` inside backticks longer than any backtick run in either, bounded to `HOOK_BODY_CHARS` (4,000) and `STEP_BUDGET`.
  - Every network call sits behind the `HookRelay` trait (`room`, `connected`, `hook_open`, `hook_close`, `hook_letters`, `hook_collected`).
- **Runner, store, desk, and UI integration**:
  - `core/src/routines.rs`: `record_hook_loss` records expired or >200-overflowed hook calls into routine `history` (`hookCalls`).
  - `core/src/routines/progress.rs`: validates optional `hook: true` on `parsed_execution`.
  - `core/src/routine_runner.rs`: `Arrival::Hook(HookBatch)` strips `inCopy` / `untilCheck` on hook dispatches without modifying the saved routine, forces `"ask"` mode on step 1 and all handed-off steps, notifies `hook_fired` after persisting dispatch intent, and restores the batch via `hook_restore` on step-1 preflight refusals.
  - `core/src/routine_host.rs`: wires `RoutineHooks` into `LiveHost` (`StubHookRelay` by default, injectable via `LiveHost::with_hook_relay`).
  - `core/src/desk.rs`: adds `routine:hook-read` and `routine:hook-new` to `WINDOW_ONLY`.
  - `src/ui/routines_panel.rs`: adds `"When a link is called"` to the schedule picker, `hook_link_view` (URL with `Copy` / `Copied`, `New link`, `Make link`, `HOOK_WARNING`, `HOOK_ASK`), async state/sync wiring, and `hookCalls` loss notices on routine cards.

### Stubs
- **`StubHookRelay` (`core/src/routine_hooks.rs`)**: `LiveHost::new` installs `StubHookRelay`, which reports `room() -> None`, `connected() -> false`, and `Err(...)` on network calls. The real TLS relay transport will plug into `HookRelay` once Apple's `NSURLSession` transport lands from the chat-apps lane (since `ureq` has no TLS).

### Tests and Break Checks (Section 1)
- `routine_hooks::tests`: 22 tests covering all cases from `a-called-link-starts-its-routine.test.ts` (12 cases), `a-called-link-is-closed-when-its-record-cannot-be-used.test.ts` (2 cases), `a-called-link-review.test.ts` (4 non-relay cases), `the-keychain-is-asked-only-for-a-key-and-said-first.test.ts` (3 routine-hooks cases), plus `a_hook_cannot_run_a_folder_check_or_make_a_writable_copy_and_preserves_saved_routine`.
- `routines_panel::routine_hook_ui_tests`: 2 headless Rune UI tests (`a_called_link_in_the_editor_offers_the_kind_privacy_and_ask_words_with_save_first_for_a_new_routine`, `a_saved_hook_shows_url_copy_new_link_and_hook_loss_on_the_card`).
- Break checks (`~/work/scratch/mut6.py` against `locust-core`, 7/7 killed by named test):
  1. Skip `secrets.available()` check in `state_inner` -> killed by `refuses_a_link_without_encryption_writes_no_plaintext_and_sends_no_registration_to_the_relay`.
  2. Hardcode 3-backtick fence in `hook_note` -> killed by `saves_a_hook_schedule_keeps_its_password_out_of_routines_exports_dispatches_and_notices_and_fences_the_data_in_ask`.
  3. Skip retired password redaction in `clean` -> killed by `redacts_the_password_a_call_already_sent_once_the_link_has_been_replaced`.
  4. Skip keychain-avoidance guard in `sync` -> killed by `a_called_links_sync_with_no_link_kept_and_no_routine_called_by_one_asks_nothing`.
  5. Disable 7-day expiry check in `ready` -> killed by `records_relay_expiry_in_the_routine_history_and_refuses_a_collected_call_once_it_becomes_a_week_old`.
  6. Use saved route mode instead of forcing `"ask"` for hook steps in `routine_runner.rs` -> killed by `keeps_handed_on_steps_in_ask_even_if_the_teammate_and_saved_routine_both_write`.
  7. Skip `hook_restore` on step-1 preflight refusal in `routine_runner.rs` -> killed by `keeps_the_body_after_a_monthly_limit_preflight_refusal_and_retries_only_after_the_normal_hold_off`.
