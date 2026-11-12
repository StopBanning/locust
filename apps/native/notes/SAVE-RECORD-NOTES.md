# Save the record in the Rust window

Notes on wiring **Save the record** end-to-end in `apps/native/` to match Electron (`apps/desktop/src/main/index.ts`, `apps/desktop/src/main/mission-export.ts`, and `apps/desktop/src/renderer/src/components/SaveRecordDialog.tsx`).

## What was built

- **Chain walking & disk ledger recovery (`apps/native/core/src/mission_export.rs`)**:
  - `record_turns`: walks `metadata.continuesFrom.missionId` from the chosen turn backwards, oldest first, stops on cycles, and reports `missing_parent` when an older parent mission id is absent from the ledger.
  - `recover_mission_from_path` / `recover_mission`: reads a `.jsonl` ledger file from either native's ledger directory or Electron's ledger directory (read-only via `native::find_ledger`) into `packages/mission-store`'s `RecoveredMission`-shaped JSON (`metadata`, `events`, `hostFailures`, `checkpoints`, `peerLinks`, `editChecks`, `approvals`, `schemaVersion`, `phase`, `lastUpdatedAt`, `ledgerSequence`, `issues`), including `truncated-tail` when a file lacks a trailing newline and `invalid-record` on malformed lines.
  - `prepare_record_for_mission` and `write_record_files`: builds Markdown and raw JSON strictly from disk, checks destination paths against `Store::check` so Electron's and Codex's folders can never be written inside, writes the Markdown file, and creates the optional `.json` sidecar exclusively (`create_new`, falling back through `(2).json`..`(99).json`).
- **Save Record dialog (`apps/native/src/ui/save_record.rs`)**:
  - Modal dialog matching `SaveRecordDialog.tsx`: header `"Save the record"`, `RECORD_CLAIM` warning, `"Include the raw record (JSON, not scrubbed)"` checkbox (unticked by default), status line, `"Close"` button, and `"Save the record…"` primary button invoking Rune's `tx.save_file` with `"Markdown"` (`md`) filter, `"Save the record"` title, and `record_file_name`.
- **Entry points (`apps/native/src/ui/sidebar.rs`, `apps/native/src/ui/command_palette.rs`, `apps/native/src/main.rs`)**:
  - Added `"Save the record…"` to each conversation's `...` actions menu (`org_menu`) and to the `Cmd+K` Command Palette (`"This conversation"` group) when a non-pending conversation is on screen (`active_record_mission_id`).
- **Headless window tests (`apps/native/src/ui/window_drive_tests.rs`)**:
  - `save_the_record_writes_markdown_matching_ledger_and_optional_raw_json` opens a synthetic two-turn ledger conversation via the sidebar, invokes `"Save the record…"` from the `...` menu, resolves the Rune save dialog, and verifies byte-for-byte equality with `mission_record_markdown` plus non-overwriting/unsanitized raw JSON sidecar export.
  - `save_the_record_cancel_writes_nothing_and_command_palette_opens_it` opens via `Cmd+K` Command Palette, cancels the save dialog, and verifies nothing is written and dialog state matches Electron.

## Judgement calls

1. **Separate `RecoveredMission` JSON loader in `mission_export.rs` rather than extending `ledger::Tail`**:
   `ledger::Tail::decode` strips evidence/raw payloads and ignores `mission.host_failure`, `mission.peer`, `mission.edit_check`, and `mission.approval` records because the live thread pane only needs rendered chat items. A dedicated `recover_mission_from_path` keeps `Tail` fast and puts the exact `mission-store` JSON shape right beside `mission_record_markdown` and `raw_record_json`, so saved records reflect everything on disk.
2. **Tip `mission_id` vs `root_id` in `sidebar::apply_org_action`**:
   Sidebar pin/snooze/rename operations normalize `mission_id` to `stable_id` (`root_id`), whereas `beside` keeps the leaf/tip `mission_id`. `"save-record"` handles its action early with the leaf `mission_id` (before `stable_id` substitution and before requiring `s.data`), so `record_turns` walks backwards through the full conversation chain rather than stopping at turn 1.
3. **Teammate and folder resolution**:
   Following `apps/desktop/src/main/index.ts`, `resolve_owner_and_folder` scans `chain.missions` newest-first against `people::owner_names(store)` and `native::roster_in(store)` (both read Electron files read-only merged with native overrides). If the owner has a `folder`, it is used; otherwise `native::open_folder()` is used only when `people::workspace_id(&open)` matches `first.metadata.workspaceId`. If neither matches, `folder` stays `None` rather than guessing an unrelated workspace folder.
4. **Cancel status behaviour (`said = None`)**:
   In `apps/desktop/src/renderer/src/components/SaveRecordDialog.tsx` line 42, cancelling the system save dialog calls `setSaid(undefined)` while keeping the dialog open and writing nothing. `save_record.rs` matches this literally (`said = None`, `error = false`), clearing any earlier status message instead of inventing extra text.
5. **Default directory in `FileDialogOptions`**:
   Electron sets `defaultPath` to `join(app.getPath('downloads'), recordFileName(...))`. `save_record.rs` resolves `$HOME/Downloads` when it exists as a directory and passes `record_file_name` as `file_name`.
6. **Electron/Codex folder refusal**:
   `write_record_files` runs `Store::check` before writing either `.md` or `.json`, and surfaces the refusal cleanly (`"<why>. Nothing was written."`) rather than allowing any path traversal into protected folders.

## Tests carried over from TypeScript

Earlier work in `EXPORT-NOTES.md` had already brought over 34 tests from `apps/desktop/src/main/a-saved-record-*.test.ts` (including two `mission_record_markdown` smoke checks for missing parents and empty chains). This task adds:

- All five `recordTurns` tests from `a-saved-record-says-only-what-the-ledger-holds.test.ts` (`brings every turn before it, oldest first`, `from the middle turn, brings its parent and not the turn that came after`, `names a parent the ledger no longer holds instead of skipping it`, `says there is nothing to record rather than writing an empty report, when no turn is found`, and `stops at a cycle instead of spinning`) under `record_turns_*` in `locust-core`.
- `recover_mission_rebuilds_events_approvals_and_truncated_tail_from_jsonl` (testing real `.jsonl` parsing from disk including `truncated-tail` detection, approvals, edit checks, and peer links).
- `write_record_files_writes_non_overwriting_raw_json_and_refuses_electron_dir` (porting the non-overwriting `(2).json` test and asserting Electron's folder is refused).
- Command Palette action-gating test (`palette_includes_save_record_only_when_conversation_is_open`) and the two end-to-end headless window tests in `window_drive_tests.rs`.

## What was left out

- **Archived `.jsonl.gz` mission ledgers**: `recover_mission` reads `.jsonl` files via `native::find_ledger` across native and Electron ledger directories; gzipped mission archives (`.jsonl.gz`) are not decompressed yet.
- **No Electron IPC handler**: Unlike `apps/desktop/src/main/index.ts`, the native window calls `mission_export` directly inside the Save dialog handler rather than across an IPC bridge.
