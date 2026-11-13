# Edit checks and fix-until-check-passes (12 Nov 2026)

A workspace folder can configure a shell command to run automatically after any turn that edits files. When the check fails, the Rust app records a check card in the ledger and sends the teammate back to fix it up to the configured try limit.

## What was built

- **Edit check & check-fix core** (`core/src/edit_check.rs`, from `main/edit-check.ts`, `main/check-fix.ts`, and `shared/check-fix.ts`):
  - `EditCheck`, `run_check_command`, `without_folder`, and `comparable_lines` run the configured check command in the workspace folder (with `CHECK_TIMEOUT_MS = 180_000`, `CHECK_OUTPUT_BYTES = 256 * 1024`, `MAX_NEW_LINES = 30`, `MAX_LINE = 300`), strip leading workspace paths, and drop noisy duration/header lines when comparing failures across turns.
  - `CheckFix`, `parsed_check_command`, `parsed_check_fix_tries`, `parsed_check_fixes`, `check_fix_prompt`, `edit_check_follow_up`, and `stopped_reason` manage per-conversation retry chains (`DEFAULT_CHECK_FIXES = 2`, `MAX_CHECK_FIXES = 5`), stop when the same failure repeats or the limit is reached, and clear chains on manual user turns or stop requests.
- **Checkpoint edit detection & ledger integration** (`core/src/checkpoints.rs`, `core/src/ledger.rs`, `core/src/ledger_write.rs`):
  - `Checkpoint::finish_reporting_edits` compares `before.files != after.files` so post-edit checks only run when a turn actually changed files.
  - `append_edit_check` writes `mission.edit_check` events (`edit_check_record_json`), and `append_note_with_level` writes `host.check_fix` diagnostics.
  - `Transcript::apply` decodes `mission.edit_check`, `startedBy.kind == "check-fix"` (`Metadata.check_fix`, `Item::CheckFix`), and `host.check_fix` notes.
- **Engine orchestration** (`core/src/engine_api.rs`, `core/src/engine_process.rs`, `core/src/native.rs`):
  - Added `EngineApi::running_checks()` across `InProcess`, `Client`, and the engine process server.
  - After a turn edits files, `native.rs` runs `EDIT_CHECK.after(...)` and appends `mission.edit_check` before dropping `memory_completion`, then triggers `run_check_fix` via `engine_api::api().send(...)` after dropping `memory_completion`.
- **UI cards & Workspace settings** (`src/ui/edit_check_card.rs`, `src/ui/messages.rs`, `src/ui/app_state.rs`, `src/ui/settings_panel.rs`):
  - `running_card`, `card`, and `check_fix_line` match Electron's `EditCheckCard.tsx` and `CheckFixLine.tsx` wording (passed, failed with exit code / signal / timed out, truncated output, and fix attempt count or stopped reason).
  - `SettingsPageId::Workspace` adds `"CHECK AFTER EDITS"` (`check_commands` per workspace) and `"When it fails"` (`check_fixes` tries `0..=5`, default `2`).
- **Headless window test & fixture** (`scripts/fixtures/m1-app-server.py`, `src/ui/window_drive_tests.rs`):
  - Added `cards-edit-check` scenario and `a_failing_check_shows_its_card_and_sends_back_to_fix`.

## Judgement calls

- **ASCII-only `\W` in `comparable_lines`**: Rust's `regex` crate treats Unicode symbols such as `ℹ` (`U+2139`) as word characters under `\W`, whereas JavaScript's `\W` is `[^A-Za-z0-9_]`. `comparable_lines` uses `^[^A-Za-z0-9_]*duration_ms\b` so Node test-runner duration lines match Electron's stripping behaviour.
- **Turn-end ordering around `memory_completion`**: `edit_check().after(...)` and `append_edit_check(...)` run before `drop(memory_completion)` so callers polling turn status do not see `"Ready"` before the check finishes and writes its ledger record. `run_check_fix` calls `engine_api::api().send(...)` after `drop(memory_completion)` so the automatic fix turn does not contend with the finishing turn's completion lock.
- **`ledger_tick` polling for engine-owned check-fix turns**: `pollable` in `ledger_tick` includes `engine_owns` so the UI continues polling while a check is running or a check-fix turn is in flight.
- **`host.check_fix` diagnostic visibility**: `ledger.rs` normally ignores `"info"` diagnostics in `adapter.diagnostic`, so `code == "host.check_fix"` is handled explicitly to ensure both `"info"` and `"warning"` check-fix notes render in the thread.

## Which TypeScript tests came over

- `edit-check.test.ts` -> `core/src/edit_check_tests.rs`
- `a-check-command-is-one-line.test.ts` -> `core/src/edit_check_tests.rs`
- `a-failing-check-goes-back-to-fix.test.ts` -> `core/src/edit_check_tests.rs`
- `a-reopened-conversation-keeps-its-check.test.ts` -> `core/src/edit_check_tests.rs` (including TypeScript `mission-store` reader verification via `LOCUST_TEST_TS_READER`)

## What was left out

- Electron's IPC/session bridge for check-fix is not needed; the native engine dispatches fix turns directly through `engine_api::api().send(...)`.
