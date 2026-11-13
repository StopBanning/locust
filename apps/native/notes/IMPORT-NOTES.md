# Import a conversation in the Rust window

Notes on porting **Import a conversation** end-to-end into `apps/native/` to match Electron (`apps/desktop/src/main/session-import.ts`, `apps/desktop/src/main/agent-session-import.ts`, `apps/desktop/src/main/index.ts`, and `apps/desktop/src/renderer/src/components/ImportDialog.tsx`).

## What was built

- **Engine session listing and importing (`apps/native/core/src/session_import.rs`)**:
  - `list_importable_sessions`: scans synthetic or configured `SessionPlaces` (`claude_home`, `codex_home`, `cursor_home`) for sessions from the last 30 days (`DEFAULT_DAYS = 30`), reading only `HEAD_BYTES` (64 KiB) and `TAIL_BYTES` (256 KiB) of Claude Code and Codex `.jsonl` files, excluding Locust's own runs (`sdk-cli`, `originator` starting with `locust` or containing `exec`, `source` containing `exec` or `subagent`, and `locust-cloud-*` worktrees) and sidechain subagent files, marking `open_now` (`< 2m`) and `folder_missing`, and sorting newest first.
  - `session_exchanges`: streams Claude Code and Codex `.jsonl` transcripts line-by-line into slimmed records (`MAX_PROMPT = 8000`, `MAX_ANSWER = 16000`) and extracts finished exchanges via `runtime_terminal::exchanges`.
  - Cursor Agent (`list_cursor_sessions`, `cursor_exchanges`, `cursor_message_ids`): reads `meta.json` (`< 64 KiB`, `schemaVersion == 1`) and `store.db` read-only via `sqlite3 -readonly` (never opening `cli-config.json` or writing to Cursor's files), decodes the root protobuf blob index, fetches message blobs bounded by `MAX_CURSOR_BLOB` (16 MiB), and extracts typed `<user_query>` prompts and finished assistant replies.
  - OpenCode (`open_code_sessions`, `open_code_exchanges`, `OpenCodeSessionReader`): parses `opencode session list --format json` (excluding child sessions with `parentID`) and `opencode export <id>` (validating `^ses_[A-Za-z0-9]+$`, excluding synthetic/ignored parts, tool/reasoning parts, foreign `sessionID`s, and unfinished exchanges).
  - `import_session` / `import_session_with_id`: refuses missing folders and already-imported sessions (`terminal-imports.json` and `held_session_ids`), keeps the newest `MAX_IMPORTED_EXCHANGES` (200) turns, learns the resolved folder in `folders.json`, writes schema-27 `.jsonl` ledgers atomically in native's data folder only on `model: "account-default"` with `continuesFrom` and `startedBy: { kind: "terminal", exchange }`, and records the `terminal-imports.json` watermark.
- **Import dialog (`apps/native/src/ui/import_dialog.rs`)**:
  - Modal dialog matching `ImportDialog.tsx`: header `"Import a conversation"` (`"the last 30 days"`), `"Search by title or folder"` filter, problem/warning banner, session rows with runtime display name, folder name, `age_of` (`"3m"`, `"5h"`, `"2d"`), missing-folder / open-now indicators, `"Import"` / `"Importing…"` action button, and footer note.
  - Imports in the background via `Latest`, names the root mission in `org` (`org::rename_mission`), closes the dialog, refreshes recent conversations, and opens the imported conversation once `recents` updates.
- **Entry points (`apps/native/src/ui/sidebar.rs`, `apps/native/src/ui/command_palette.rs`, `apps/native/src/main.rs`)**:
  - Sidebar `+` button opens an Add menu with `"New conversation"` and `"Import a conversation…"` (matching Electron's `openAddMenu` on the sidebar `+`), and `Cmd+K` Command Palette also offers `"Import a conversation…"`.
  - Real `SessionPlaces::from_env()` and `opencode_enabled` are wired only in live non-headless launches (`!HEADLESS`); tests inject synthetic `SessionPlaces` and never touch real home directories.

## Judgement calls

1. **Read-only Cursor SQLite via `sqlite3 -readonly`**:
   `locust-core` has no SQLite crate dependency and builds offline. Invoking `/usr/bin/sqlite3 -readonly -noheader -batch` keeps `locust-core` free of new dependencies and `unsafe` FFI while opening `store.db` strictly read-only.
2. **Preserving block `type` in `slim_codex`**:
   `runtime_terminal::exchanges` filters Codex message content blocks by `type` (`"text"`, `"input_text"`, or `"output_text"`). `slim_codex` preserves each block's `type` (defaulting to `"text"`) so `runtime_terminal::exchanges` reads all text blocks without duplicating the Codex parser.
3. **Sidebar `+` Add menu**:
   In Electron, the sidebar `+` button opens an Add menu containing `"Import a conversation…"`. `sidebar.rs` now toggles `add_menu_open` from `+` to offer `"New conversation"` and `"Import a conversation…"`, while also exposing `"Import a conversation…"` in the `Cmd+K` Command Palette.

## Tests carried over from TypeScript

- From `apps/desktop/src/main/a-conversation-can-be-imported.test.ts`:
  - `lists_claude_and_codex_sessions_newest_first_excluding_locust_subagent_and_old`
  - `imports_claude_session_into_resumable_turns_and_refuses_duplicate`
- From `apps/desktop/src/main/past-opencode-and-cursor-chats-can-be-imported.test.ts`:
  - `reads_opencode_list_and_export_without_tools_reasoning_foreign_or_unfinished`
  - `opencode_reader_uses_documented_commands_and_refuses_failures_and_invalid_ids`
  - `reads_cursor_metadata_and_ordered_messages_without_changing_store_or_opening_config`
  - `lists_huge_cursor_stores_from_metadata_and_keeps_missing_folders_visible`
  - `imports_cursor_and_opencode_as_resumable_on_account_default_and_refuses_duplicates` (including verifying `cursor::print_args` emits `--resume <id>` without `--model`)
- Headless window test (`apps/native/src/ui/window_drive_tests.rs`):
  - `import_dialog_lists_synthetic_session_imports_it_and_opens_the_conversation`

## What was left out

- **Per-account `claudeHome` / `codexHome` overrides**: `SessionPlaces::from_env()` reads `HOME`, `CLAUDE_CONFIG_DIR`, and `CODEX_HOME`, matching the single-home defaults; multi-account home scanning is not wired yet.
- **Pre-importing `other_sessions` / OpenCode in headless window tests**: the headless window test exercises the Claude Code synthetic session end-to-end through the Import dialog, while OpenCode and Cursor listing/importing are covered by the unit test suite.
