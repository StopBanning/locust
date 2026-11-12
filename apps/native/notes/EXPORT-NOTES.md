# Save a conversation as Markdown or JSON (12 Nov 2026)

Ported Electron's conversation record exporter (`apps/desktop/src/main/mission-export.ts`) into `locust-core` as `core/src/mission_export.rs`, registered in `core/src/lib.rs`.

## What was ported

- **`core/src/mission_export.rs`** (from `apps/desktop/src/main/mission-export.ts`):
  - `SAVED_RULE_DENIAL`
  - `RecordSource<'a>` (taking `&[serde_json::Value]` missions in `mission-store`'s `RecoveredMission` JSON shape, rather than introducing a duplicate struct)
  - `mission_record_markdown(source)`: full Markdown record of a conversation, including header, `## Who answered`, and per-turn sections (`Asked` / `The person said`, `<teammate> said`, `Approvals and refusals` for pre-v20 and v20+ ledgers, `Commands`, `File changes`, `Other tool calls`, `Hand-offs`, `How it ended`)
  - `collapse_outside_fences(text)`
  - `raw_record_json(missions, locust_version, saved_at)` (preserving `warning` as the first key via `serde_json`'s `preserve_order` feature)
  - `record_file_name(teammate, now)`
  - Pure tool-kind helpers from `apps/desktop/src/shared/tool-kinds.ts` (`is_shell_tool`, `is_edit_command`, `edit_tool_name`, `reports_a_change`, `by_helper`), which did not yet exist in `locust-core`.
  - Reuses `crate::carry_on::display_name` for runtime display names (`Codex CLI`, `Claude Code`, etc.).
- **`core/src/secrets.rs`** (smallest addition needed from `apps/desktop/src/shared/secrets.ts`):
  - `SECRET_SHAPED_TEXT_REMOVED` (`"[secret-shaped text removed]"`) and `scrub_secrets(text) -> (String, usize)`, reusing `looks_like_a_password` and `looks_like_a_card` already in `secrets.rs`. Existing `redact_secrets` (which uses `[redacted]` and does not return a replacement count) was left untouched.

## Which TypeScript tests were carried over

All 34 tests in `mission_export::tests` come from the three Electron test files:

- `a-saved-record-says-only-what-the-ledger-holds.test.ts` (including `golden_file_matches_byte_for_byte` against `apps/desktop/src/main/a-saved-record-says-only-what-the-ledger-holds.golden.md` loaded via `include_str!`, determinism, missing parent, empty conversation, pre-v20 and v20+ approvals, unwritten approval notes, commands/diffs/truncation/fences, hand-offs/edits/endings, self-check/header, raw JSON, and file naming).
- `a-saved-record-keeps-keys-out.test.ts` (5 tests: command output + diff scrubbing and plural/singular count, raw JSON keeping originals with `warning` first, clean turn reporting `none found`, card answer + edit check scrubbing).
- `a-saved-record-keeps-output-as-recorded.test.ts` (2 tests: collapsing blank lines outside fences while preserving blanks inside fences, and longer fences).

## What was left out, and why

- `recordTurns` (walks `continuesFrom` chains via async ledger reads) and the window's Save dialog / IPC handler (`missions:save-record`): left out by instruction for a later round. Accordingly, the Electron tests that exercise `recordTurns` chain walking, IPC channel guards, `SaveRecordDialog.tsx` rendering, and importing `renderer/src/status.ts` were not carried over.

## How it was checked

- `cargo test --release -p locust-core mission_export`: 34 passed, 0 failed.
- `cargo test --release --workspace`: `locust-core` 507 passed, 0 failed, 20 ignored (existing live CLI tests); `locust-native` 208 passed, 0 failed, 0 ignored.
- `cargo build --release`: passed with no warnings.
