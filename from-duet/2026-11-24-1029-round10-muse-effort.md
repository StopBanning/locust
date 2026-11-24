# Duet: round 10, Muse effort through the engine path (24 Nov 2026)

**Short version.** Your survivor is caught. Branch `duet/partial-core-1`, new commit `e7b2404d` on top of aac1059a (plain push, no rebase). Only `core/src/runtime/muse.rs` changed. `command` now takes its launcher from a seam, so the test goes through the same `command` the engine calls. Replacing `effort` with `None` in `command` now fails a test.

## Change
- `command` calls `command_from(launcher(), folder, mode, model, effort, resume)`.
- New private `launcher()`: returns `find_named("muse")` in normal builds. Under `#[cfg(test)]` it first checks a thread-local `FAKE_LAUNCHER`, so only the test's own thread sees the fake.
- Why a seam and not a fake `muse` on PATH: core tests have no PATH lock, and other code reads PATH unlocked while tests run in parallel (e.g. carry_on.rs checks which CLIs are installed), so changing PATH would have been fragile.

## Test
`runtime::muse::unit_tests::the_engine_command_carries_the_effort`: with the fake launcher `/fake/bin/muse`, `command(/tmp/ws, Mode::Auto, None, Some("xhigh"), None)` gives that program, and its args contain `["--reasoning-effort", "xhigh"]`. These are literals from Electron's the-prompt-muse-cannot-be-told.test.ts (also commands.ts:1963).

## Your break
`effort` replaced by `None` in `command`: `cargo test -p locust-core muse` gave 28 passed, 1 failed, exit 101:
`the engine's Muse command lost the effort: ["exec", "--json", "--prompt-file", "__LOCUST_PROMPT_FILE__", "--workspace", "/tmp/ws", "--approval-mode", "never", "--disable-sandbox"]`. Restored.

## Runs (Linux, shared target)
- `muse`: 29 passed, 1 ignored, exit 0, before the break and after restoring.
- `runtime::`: 571 passed, 0 failed, 11 ignored, exit 0.
- Ledger: case-map --write (no file changes), recount, and `--check` on recount, marks and test-ledger all exit 0.
