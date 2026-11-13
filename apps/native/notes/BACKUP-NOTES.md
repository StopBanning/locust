# Profile backup, restore, and retention in Rust

Notes on porting PARITY row 115 ("Retention and manifest-verified local profile backup/restore") into `apps/native/` to match:

- `apps/desktop/src/main/profile-backup.ts`
- `apps/desktop/src/main/retention.ts`
- `apps/desktop/src/renderer/src/backupWords.ts`
- `apps/desktop/src/renderer/src/components/Screens.tsx` (`RetentionControl`, `BackupControl`)
- `apps/desktop/src/main/index.ts` (`applyPendingRestore`, `takeLastRestore`)
- `apps/desktop/src/main/a-profile-backs-up-and-restores.test.ts`
- `apps/desktop/src/main/retention.test.ts`
- `apps/desktop/src/renderer/src/backupWords.test.ts`
- `packages/mission-store/test/retention.test.ts`

## What was built

- **Profile backup & restore (`apps/native/core/src/profile_backup.rs`)**:
  - `write_backup`: copies only explicitly classified carried files/folders from the native data folder into `<parent>/Locust backup YYYY-MM-DD HHMM/profile/`, computes each copy's byte size and SHA-256 hash by reading back from the copy, counts profile items (`count_profile`), and writes `locust-backup.json` last. Refuses if `parent` is missing or the destination folder already exists.
  - `read_backup`: validates `locust-backup.json` (`app == "Locust"`, `schema <= BACKUP_SCHEMA`, `compare_versions(manifest.appVersion, appVersion) <= Equal`), verifies every listed path is carried and has no `..`/empty segments, and checks each file's size and SHA-256. Used by both preview and restore.
  - `request_restore`: refuses if `live_runs > 0`, validates via `read_backup`, and writes `pending-restore.json`.
  - `apply_pending_restore` / `apply_pending_restore_with_copy`: called in `main.rs` at startup before `App::new` / any store is read (when `!headless`). Removes `pending-restore.json` first, re-validates the backup, moves existing carried items aside into `before-restore-YYYYMMDD-HHMMSS`, copies the backup in, and rolls back on any partial copy failure. Writes `last-restore.json` (`take_last_restore` reads and removes it once).
  - Wording helpers: `counts_line`, `folder_name`, `backed_up_line`, `restore_notice_line`, `format_bytes`.
- **Retention & storage report (`apps/native/core/src/retention.rs`)**:
  - `retention_cutoff`: validates `1..=3650` finite days against the host clock.
  - `read_storage_report`: measures native `mission-ledger/*.jsonl` file count, total bytes, unreadable count, and oldest `occurredAt` record date.
  - `prune_missions` / `prune_missions_with_move`: scans native `mission-ledger/*.jsonl` only (never Electron's ledger), aborts if any `.jsonl` entry cannot be opened or is not a regular file, protects `protect_mission_ids` (from `engine_api::api().running()`) and all `continuesFrom` ancestor chains behind kept/running conversations, bounds confirmation by `only` (the IDs shown in the preview), and moves pruned ledgers into `mission-ledger/.trash/`.
  - `prune_preview_summary`: matches Electron's exact preview wording.
- **Privacy & data UI (`apps/native/src/ui/backup_section.rs`, `apps/native/src/ui/settings_panel.rs`, `apps/native/src/main.rs`)**:
  - Wired `SettingsPageId::Privacy` in `settings_panel.rs` to render `"Privacy & local data"` (storage report + `30 days` / `90 days` / `1 year` chips + `Review` + `Delete them for good`) and `"Back up and restore"` (`Back up…` + `Restore from a backup…` using `tx.open_folder`, restore preview card with `Restore and restart` + `Cancel`).

## Every file in the native data folder and whether it is carried

### Carried (`BACKED_UP_FILES` and `BACKED_UP_FOLDERS`)

- **Files**: `teammates.json`, `routines.json`, `memories.json`, `approval-rules.json`, `groups.json`, `rooms.json`, `compares.json`, `folders.json`, `pets-removed.json`, `calls.json`, `scout.json`, `scout-looks.json`, `issue-origins.json`, `panel-runs.json`, `appearance.json`, `folder-cautions.json`, `tune-lessons.json`.
- **Folders**: `mission-ledger` (excluding `.writable` and `checkpoints`), `workroom`, `pets`, `skill-library`, `teammate-skills`.

### Not carried (`LEFT_OUT` and `NEVER_COPIED`)

- **Machine-local / runtime / IPC state**: `workspace.json`, `window.json`, `away.json`, `background.json`, `awake.json`, `seen-version.json`, `keychain-opened.json`, `update-lane.json`, `queued-messages.json`, `locust-errors.log`, `engine.lock`, `engine.sock`, `engine.log`, `guard`, `panel-runs`, `claude-skills`, `run-skills`, `claude-context-launches.json`, `claude-app.json`, `claude-background.json`, `runtime-commands.json`, `runtime-facts.json`, `runtime-updates.json`, `cursor-connector-holds.json`, `cursor-default-model.json`, `terminal-imports.json`, `usage-readings.json`, `brief-sessions.json`, `kilo-conversations`, `acp-agents.json`, `watched_pull_requests.json`, `skills.json`, `pending-restore.json`, `last-restore.json`.
- **Machine-bound credentials, keys, and pairings**: `own-models.json`, `teammate-keys.json`, `voice-settings.json`, `locust-mcp-connection.json`, `locust-mcp.json`, `remote-mcp.json`, `routine-hooks.json`, `phone.json`, `phone-relay.json`, `voice-call-mcp`, `voice-call.json`, `discord.json`, `slack.json`, `telegram.json`, `agent-accounts`, `account-sync.json`, `profile-sync.json`, `account-copies`, `scout-away.json`, `claude-cloud.json`, `cloud-tasks.json`, `cloud-turns.json`.
- **Machine-local references, caches, and attachments**: `conversation-references.json`, `reference-repositories`, `reference-trash`, `memory-recall-cache.json`, `pets-cache`, `voice`, `voice-previews`, `mac-updates`, `dictionaries`, `.locust`, `checkpoints` (both at root and `mission-ledger/checkpoints` via `NEVER_COPIED`).

## Judgement calls

1. **Relaunch gating for tests and `--snap`**:
   - `relaunch_for_restore` in `src/ui/backup_section.rs` is gated behind `#[cfg(not(test))]` and `!crate::HEADLESS.load(Ordering::Relaxed)`, so unit tests, headless window tests, and `--snap`/`--scene`/`--video`/`--script` invocations never spawn a new process or call `std::process::exit(0)`. Similarly, `apply_pending_restore` in `main.rs` runs only when `!headless`.
2. **Skipping `mission-ledger/checkpoints` in `NEVER_COPIED`**:
   - Native undo checkpoints live at `mission-ledger/checkpoints/` (whereas Electron places them at `<profile>/checkpoints/`). Adding `"checkpoints"` to `NEVER_COPIED` ensures backups never carry machine-local workspace snapshots (which can contain `.env` or secrets).
3. **Prune deletes by moving to `mission-ledger/.trash/`**:
   - Matching Electron's `packages/mission-store/src/index.ts` (`removeMissionFile`), `prune_missions` moves pruned `.jsonl` files into `mission-ledger/.trash/` rather than unlinking them permanently.
4. **Native-only data files classified in `LEFT_OUT`**:
   - Native's `guard`, `engine.lock`, `engine.sock`, `engine.log`, `.locust`, `panel-runs`, `watched_pull_requests.json`, and `skills.json` are explicitly classified in `LEFT_OUT` and verified by `every_file_the_native_app_keeps_in_its_profile_is_classified`.

## Which TypeScript tests came over

- `apps/desktop/src/main/a-profile-backs-up-and-restores.test.ts` -> `locust_core::profile_backup::tests` (all 8 test cases, including manifest hashes, key exclusion, duplicate minute refusal, tampered file/schema/version/path validation, live run refusal, next-start restore + `before-restore-*` aside + rollback on partial copy failure, and the profile-file classification scan).
- `apps/desktop/src/renderer/src/backupWords.test.ts` -> `locust_core::profile_backup::tests::backup_words_match_electron`.
- `apps/desktop/src/main/retention.test.ts` & `packages/mission-store/test/retention.test.ts` -> `locust_core::retention::tests` (cutoff bounds, old vs recent, continuity chains, preview vs confirm `only` bounding, running mission protection, unreadable/malformed/newer-schema files, move failure reporting, storage report, and preview summary wording).
- Headless window tests in `apps/native/src/ui/window_drive_tests.rs`:
  - `privacy_page_backs_up_previews_and_applies_restore_at_next_start`
  - `privacy_page_previews_and_confirms_retention_keeping_running_and_electron`

## What was left out

- **Trash UI (`TrashControl`)**: The `"Trash"` section on `SettingsPageId::Privacy` remains `deferred("Trash", p)`, as Trash listing/restoring/emptying is a separate parity item.
- **Toast/banner for `take_last_restore`**: `apply_pending_restore` writes `last-restore.json` and `take_last_restore` is implemented and tested in `locust_core::profile_backup`, but `main.rs` does not yet display a dedicated post-restart dismissible banner in `App` (Electron's `App.tsx` toast).
