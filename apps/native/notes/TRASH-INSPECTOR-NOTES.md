# The Trash, the after-restore notice, and the reply inspector in Rust

Notes on porting:
- **The Trash** on Settings › Privacy & data (`apps/desktop/src/renderer/src/components/Screens.tsx` `TrashControl`, `apps/desktop/src/main/index.ts` lines 9912–10003, `packages/mission-store/src/index.ts` `removeMissionFile`, `listTrashedMissions`, `restoreMission`, `emptyTrash`)
- **The after-restore notice** (`apps/desktop/src/renderer/src/App.tsx` `lastRestore` / `restoreNoticeLine`, `locust_core::profile_backup::take_last_restore`)
- **The reply inspector** (PARITY row 26, `apps/desktop/src/renderer/src/components/Inspector.tsx`, `apps/desktop/src/shared/may-do.ts`, `apps/desktop/src/renderer/src/App.tsx` `inspectorOpen`)

## What was built

- **Core trash (`apps/native/core/src/trash.rs`, `people.rs`, `references.rs`)**:
  - `delete_mission`: validates `is_safe_id` (`[A-Za-z0-9_-]{1,128}`), refuses if `mission_id` is in `engine_api::api().running()`, moves `mission-ledger/<id>.jsonl` into `mission-ledger/.trash/<id>.jsonl` (mode `0700` on Unix), and restamps `mtime` to now. Keeps `missionOwners` in `teammates.json` and `references.json` intact so a restore brings the conversation back whole.
  - `list_trashed_missions`: lists `mission-ledger/.trash/*.jsonl` newest deletion first, reading only the first 16 KiB of each file for `metadata.prompt` (bounded to 200 chars) and `metadata.createdAt`. Unreadable or malformed files are still listed by `mission_id` alone.
  - `restore_mission` / `restore_mission_or_err`: validates `is_safe_id`, refuses if a live `mission-ledger/<id>.jsonl` already exists (`"That conversation is not in the trash, or one with the same id is live again."`), and moves the file back from `.trash/`.
  - `empty_trash` / `empty_trash_dir`: deletes trashed `.jsonl` files (continuing past individual remove errors), then calls `references::forget` and `people::unassign_missions` to drop references and owners from `teammates.json` only then.
- **Trash UI & Delete action (`apps/native/src/ui/trash_section.rs`, `backup_section.rs`, `settings_panel.rs`, `sidebar.rs`)**:
  - Added `"Delete"` to the conversation menu in `sidebar.rs`, moving the native ledger file to `.trash/` and reloading the sidebar/settings.
  - Replaced `deferred("Trash", p)` in `settings_panel.rs` with `trash_section::section(s, p)`, matching Electron's `TrashControl` wording, `YYYY-MM-DD HH:MM` deletion timestamps, one-click `"Put back"`, and two-step `"Empty the trash"` → `"Delete N for good?"`.
- **After-restore notice (`apps/native/src/ui/app_state.rs`, `backup_section.rs`, `main.rs`)**:
  - Reads `profile_backup::take_last_restore(store.root())` once on startup after `load_composer_store` (when `!s.demo`) and renders a dismissible `restore-notice` banner (`OK` button) with `profile_backup::restore_notice_line`.
- **Reply inspector (`apps/native/core/src/inspector.rs`, `ledger.rs`, `apps/native/src/ui/inspector_panel.rs`, `thread_view.rs`, `main.rs`)**:
  - Ported `what_it_may_do` (`may-do.ts` for all 9 runtimes), `sandbox_phrase`, `account_phrase`, `short_mission_id`, `checkpoint_label`, `ledger_verification_label`, `no_artifacts_line`, `build_signal_rail`, `produced_files`, and `cost_label_and_line`.
  - Extended `ledger::Metadata` and `ledger::Session::inspector_inputs` to expose every turn in the active conversation (`continuesFrom` chain) with route, provenance, checkpoints, events, and recovery state.
  - Added the `Activity` icon button (`inspector-open`) in `thread_view.rs` and the `"About this reply"` side panel (`inspector-panel`) with `Activity`, `Details`, `Artifacts`, and `Receipt` tabs plus a turn switcher (`Turn 1`, `Turn 2`, …) for multi-turn conversations.

## Electron's special cases

1. **Emptying the Trash drops each conversation's owner then, and only then**:
   - `delete_mission` leaves `teammates.json` untouched; `empty_trash` calls `people::unassign_missions` and `references::forget` only after emptying `.trash/`.
2. **Emptying removes a comparison whose every answer is gone (`comparesGoneWith`)**:
   - The Rust app has no comparisons (`compares` store/UI) yet, so there are no comparison records or copies to remove.
3. **A restore is refused when a live conversation has the same id**:
   - `restore_mission` checks `fs::symlink_metadata(&to).is_ok()` and `restore_mission_or_err` returns `"That conversation is not in the trash, or one with the same id is live again."`.
4. **Exact Trash error words**:
   - `ERR_READ`: `"The trash could not be read. Nothing in it has been deleted for good."`
   - `ERR_RESTORE`: `"That conversation could not be put back. It is still in the trash."`
   - `ERR_EMPTY`: `"The trash was not emptied. Everything in it is still there."`
5. **Restore works from a browser; emptying for good is done on the computer**:
   - In Electron (`Screens.tsx` line 1069), `onTheComputer(MISSION_TRASH_EMPTY_CHANNEL)` disables emptying when viewed in a remote browser. The Rust native window runs on the computer itself, so both Restore and Empty are allowed here.
6. **`listTrashedMissions`, `restoreMission`, `emptyTrash` edge cases**:
   - `is_safe_id` enforces `/^[A-Za-z0-9_-]{1,128}$/`.
   - `header_of` reads at most 16 KiB and lists unreadable/malformed `.jsonl` files by `mission_id` alone.
   - `empty_trash_dir` continues removing remaining `.jsonl` files if removing one fails.
   - `delete_mission` removes any stale `.trash/<id>.jsonl` before renaming so a delete → restore → delete cycle succeeds.
7. **Native-only realities**:
   - `delete_mission`, `list_trashed_missions`, `restore_mission`, and `empty_trash` touch only the native `mission-ledger/` (never Electron's `also_read` folder).
   - Running turns are queried via `engine_api::api().running()`.

## Which TypeScript tests came over

- `packages/mission-store/src/deleting-is-reversible.test.ts` → `locust_core::trash::tests` (7 tests covering reversible delete, storage report exclusion, byte-for-byte restore, live-id collision refusal, missing mission, empty + owner/reference drop, delete → restore → delete cycle, unreadable header, and unsafe ID refusal).
- `apps/desktop/src/renderer/src/permissions-panel-is-true.test.tsx` → `locust_core::inspector::tests` (all 9 tests covering plain-word phrasing, `no_artifacts_line`, `what_it_may_do` for Auto/workspace-write/read-only/approve-each/OpenCode/Antigravity, `sandbox_phrase`, and multi-turn `build_conversation_reports`).
- Headless window tests in `apps/native/src/ui/window_drive_tests.rs`:
  - `privacy_page_trash_lists_restores_and_keeps_owner`
  - `privacy_page_empties_trash_and_drops_owner_from_teammates_json`
  - `after_restore_notice_shows_once_and_dismisses`
  - `inspector_opens_on_two_turn_conversation_and_shows_route_provenance_and_recovery`

## What was left out

- **Comparisons cleanup on Empty (`comparesGoneWith`)**: Rust has no comparisons feature yet.
- **Artifact "Show in file manager" button**: Electron's `Artifacts` tab has a per-row reveal button (`revealFile`); the Rust `Artifacts` tab lists the changed files and their status without a per-row reveal button.
