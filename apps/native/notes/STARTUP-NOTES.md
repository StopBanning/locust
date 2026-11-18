# Startup, window bounds, slow-agent probes, and runtime start errors (18 Nov 2026)

Ported the 6 startup/window/workspace gaps from `apps/native/notes/TRIAGE-2026-11-18.md`:

1. **Slow agent deadline (`core/src/agents.rs`, `core/src/slow_agent_tests.rs`)**:
   - Added `FIRST_SCREEN_DEADLINE` (2 s), `pending_found`, `SweepCheck`, `Settled`, `CheckHandle`, `settle_within`, `keep_what_was_answered`, `sweep_has_pending_agent`, and `OutstandingChecks`.
   - `agents::discover()` now runs via `discover_with_deadline(FIRST_SCREEN_DEADLINE)`, signalling `located` right after `find_named` so slow probes do not stall the first screen.
2. **Window size and bounds (`core/src/window_bounds.rs`, `core/src/window_bounds_tests.rs`, `src/main.rs`)**:
   - Added `minimum_size` (1120x720 floor, clamped to smaller work areas), `opening_size` (0.82 x 0.86 capped at 1215x800), `read_saved_window`, `opening_placement` (160x48 minimum overlap on attached displays, off-screen drop, maximized restore), and `window-bounds.json` persistence.
   - Wired into `src/main.rs` (`AppSpec` initial size/min_size, `.maximized(...)`, `.on_window(...)` position, and `on_close`/`on_quit` save).
3. **Runtime start failure sentences (`core/src/failure_words.rs`, `core/src/failure_words_tests.rs`, `core/src/ledger.rs`)**:
   - Added `runtime_display_name`, `transport_sentence`, and `failure_message_in_cwd` handling missing folder, `ENOENT`/`os error 2`, `EACCES`/`EPERM`/`os error 13`/`os error 1`, and unexpected stop (`stream closed`).
   - `ledger::Metadata` now reads `command.cwd` and passes it to `failure_message_in_cwd` on `run.failed` (without editing `core/src/native.rs`).
4. **Workspace launch resolution (`core/src/workspace_launch.rs`, `core/src/workspace_launch_tests.rs`, `src/main.rs`)**:
   - Added `resolve_workspace_path` (`--workspace=<abs>` > launch folder excluding install/home/agent-settings/Windows folders via `thread_view::folder_caution` > remembered `workspace.json` > `~/Documents/Locust`), `is_inside_directory`, `workspace_id_for`, and `resolve_startup_workspace` wired in `src/main.rs`.
