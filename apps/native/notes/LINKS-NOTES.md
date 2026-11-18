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

## Section 2: Remote MCP app pairing (`core/src/remote_mcp.rs`, `src/ui/remote_mcp_card.rs`)

Electron: `main/remote-mcp.ts`, `shared/remote-mcp.ts`, `RemoteMcpSettings.tsx`, `RemoteConnectCard.tsx`, `main/an-app-uses-the-team-from-anywhere.test.ts`.

### What was built
- **`RemoteMcp` (core)**: the computer's side of an app using the team through the relay. `answer(RemoteMcpAsk)` takes the three kinds the relay passes on:
  - `Connect`: a card the person answers. It blocks the relay's held request until Allow, Deny or `ASKING_MS` (5 min), then makes a 43-character code (good once, `CODE_MS` 5 min) or says `403 {"error":"denied"}`. It makes no card at all for a sign-in without client id, redirect, a 43-character challenge or six digits, when off, or when a rule (`refusal`) forbids it.
  - `Token`: RFC 6749 with PKCE. A code for keys, once, right or wrong, only with the verifier the app started with; or a refresh key for new keys, the old ones gone (form or JSON body). Electron's error words are kept exactly.
  - `Call`: JSON-RPC, one request at a time: `initialize` (protocol chosen from 2025-06-18, 2025-03-26, 2024-11-05), `ping`, `tools/list`, `tools/call` (only the twelve listed tools), notifications answered 202, an answer over `MAX_ANSWER_CHARS` said to be too long.
- **Keys**: only SHA-256 (base64url) hashes are written to `remote-mcp.json` (0600, temp file then rename); an access key lasts a day, a refresh key ninety days unused and is replaced on each use; ten apps, the one used longest ago goes. The key itself is never stored, so no `SecretBox` is used here (the handoff's secret-box rule is about secrets kept; these are not kept). A key is never in the state the window is told, in an error, or in the file. An app's last use is written at most once a minute (`tick()`).
- **Tools**: `remote_mcp_tools()` is the twelve tools of Electron's `REMOTE_MCP_TOOLS`, word for word, and a test reads `apps/desktop/resources/locust-mcp-bridge.mjs` and checks each description, the read-only marks and the order against it.
- **Window**: `src/ui/remote_mcp_card.rs`. Settings › General › Your phone gets the "Use from anywhere" section (switch, address with Copy, each app with its line and Remove, the message); the sign-in card is shown over every screen under the title bar (name, host, the six digits as `482 915`, Allow and Deny). `poll` reads the state every turn of the window's loop so the card appears on any screen.

### Stubs and what differs from Electron
- **`StubMcpRelay`** (core): no address, never reached. The real relay transport comes with Apple's NSURLSession (`ureq` has no TLS). Until then From anywhere can never be on, so the switch says "Turn on From anywhere under Your phone first…". The relay's own side (register, the sign-in page, held requests, room-core) is not ported: it lives in `apps/relay/`.
- **`NoToolsYet`** (window): the tools an app calls are not yet wired to the door's engine; a call returns Locust's "could not complete" words. `McpTools` is the trait to implement.
- **The tool list is Electron's `remote-mcp.ts` list (12 tools), not `door::tool_list()` (13, with `answer_question`)**: the two already differ in this checkout, in tool count and in several descriptions. The bridge file in this tree matches `remote-mcp.ts`, so that is what the test checks. Whoever moves the bridge forward moves both.
- Not built: Electron's "keep it running" lines in the settings (they need the tray and login-item switches); the window notification when an app asks (`asked` is a hook, unused); keys that cannot be written are not given (Electron would reject the request, here it is a 500 `server_error`).
- Allow after Off: `answer_sign_in` passes `allow && enabled`, as Electron does, but turning off already ends every card first, so removing the `&& enabled` is an equivalent change (no test can tell); `connect` rechecks `enabled` after the answer, and that check has its own break.

### Tests and Break Checks (Section 2)
- `remote_mcp::tests`: 24 tests in `core/src/remote_mcp_tests.rs`, one per Electron case plus the rules around them (fake relay, fake tools, fake clock, temp folder; no real relay, keychain or token).
- `remote_mcp_card::tests` (window): 3 tests (`on_it_shows_the_address_with_copy_and_each_app_with_its_line_and_remove` clicks `remote-connect-allow` via Rune).
- Break checks (`~/work/scratch/maxlane/mutsafe.py` and `~/work/scratch/mut6.py`, 22/22 non-equivalent breaks killed by named tests; 1 equivalent change `allow && enabled -> allow` explained above):
  - `remote_mcp.rs`: 21 non-equivalent breaks (`!is_key(&challenge)`, `!same_hash` PKCE proof, `found.until <= now`, `access_until > now`, `refresh_until > now`, `.take(MAX_APPS - 1)`, `inner.grants.clear()` on off, `self.options.relay.open(false)` on off, `grant.access = sha256_base64url(&access)`, `one.id != grant.id` on refresh, `self.persist()` on remove, `if !allowed || !self.lock().enabled`, `relay.address().is_none()`, `utf16_len <= MAX_ANSWER_CHARS`, `client_id` mismatch on refresh, `list_background_runs` description, `http` redirect host, notification 202 `id.is_none()`, 80-char app name truncation, `.filter(|a| a.until > now)` card expiry, `NEEDS_FROM_ANYWHERE` message) -> all 21 killed (`~/work/scratch/codec/section2_breaks.log`, `section2_breaks_bc.log`, `section2_breaks_b_ui.log`).
  - `remote_mcp_card.rs`: `decide(s, &allow_request, true) -> false` on `remote-connect-allow` -> killed by `remote_mcp_card::tests::on_it_shows_the_address_with_copy_and_each_app_with_its_line_and_remove` (`~/work/scratch/codec/section2_breaks_b_ui.log`).
