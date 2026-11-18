# Disk observation, conversation folders, and long turns (18 Nov 2026)

## Part 1: A conversation runs in its own folder
- `core/src/native.rs`: added `known_folder_of` and `resolve_run_folder` so continuing turns (follow-up, route switch, side question, rewind) look up the earlier mission's `workspaceId` in native's `folders.json` (falling back read-only to Electron's `folders.json`) when it differs from the current folder, and refuse with Electron's exact words if unknown or missing. Also records used folders via `folders::use_folder` and hashes the project root (not the worktree) for `turn_workspace_id`.
- `core/src/native/codex_mission_tests.rs`: updated `earlier()` to use `workspace_id_for(&work)` instead of `"ws_test"`, and added 5 tests matching `main/a-conversation-runs-in-its-own-folder.test.ts` (68/68 `codex_mission` tests passed).
- Break check (`mutsafe.py`): skipping `earlier_ws != workspace_id_for(&current)` killed by `runs_in_its_conversations_folder_not_the_windows_and_is_recorded_there`, `continues_a_restored_windows_conversation_in_the_chosen_mac_folder`, `is_refused_not_run_here_when_its_folder_is_not_known`, and `is_refused_not_run_here_when_its_folder_is_gone`.

## Part 2: Edits made by commands are seen (finished by the Mac chat, 18 Nov)
- `core/src/disk_observation.rs` (new): Electron's `disk-observation.ts` with its exact bounds. Git snapshots carry up to 5,000 untracked files (400 texts, 256 KB each, 8 MB in all); plain folders are walked to depth 8 skipping program folders; a mtime check with 2 s slack keeps files that only moved into view from looking like changes.
- `core/src/native.rs` `record_disk_observation`: snapshots before and after each turn and appends `observed_edit` started/completed pairs with patches (`ledger_write::append_events`). Paths the runtime already reported with an edit are dropped. The Changes panel and Undo read the patch. `checkpoints.rs` gained the file-at-checkpoint lookup Electron calls `fileAt`.
- This port's own rule, beyond Electron: a changed file that is a symlink is never read (`read_text_up_to` refuses links); Electron's `readFile` follows them.
- `resolve_run_folder` skips the `"native"` placeholder workspace id, so older records never refuse.
- The fakes (`m1-app-server.py`, `commands-app-server.py`) write their test logs beside the work folder, not in it, so they don't show up as observed edits. Test helpers read the logs from there. `kinds()` and `check-m1-ledger.mjs` skip observed_edit rows written after the terminal event, as Electron writes them.
- `window_drive_tests::a_failing_check_shows_its_card_and_sends_back_to_fix` now reads the thread from its top as well: turn 1's edit card is now seen on disk, so the thread runs past one window.
- Breaks, each caught by a named test:
  - whole/partial comparison, the by-helper skip, folder depth, the link rule: caught by the tests named in the Part 2 run.
  - `written < taken_at.saturating_sub(WRITTEN_SLACK_MS)` → `written < taken_at`: caught by `a_file_written_a_moment_before_the_first_look_is_still_a_change`.
  - `if holds_secrets(&path)` → `if false`: caught by `never_reads_a_new_file_that_may_hold_secrets_and_still_sees_it_change`.
  - `looked_at >= MAX_UNTRACKED_LOOKED_AT` → `>`: caught by `carries_no_more_than_five_thousand_new_files_and_leaves_the_rest_bare`.
- Ledger: 5 Electron files set to carried. Part 1's row had lost its round column; fixed.

## Part 3: long turns keep their start — NOT DONE
`a-long-turn-keeps-what-it-said` and `a-trimmed-live-turn-says-so-on-screen` are still waiting.
