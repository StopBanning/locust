# CLEAN1 cleanup notes

This copy only: `/Users/locust/Documents/Locust/native-review/clean1/native`.
No dependencies were added. No real agent turn was run. An accidental application launch during harness discovery is documented below; interactive behavior was not tested.

## Measurements

| Measure | Before | After |
| --- | --- | --- |
| Release tests | 283 passed, 13 ignored (recorded baseline) | 285 passed, 13 ignored, three consecutive runs |
| Compiler warnings, production build | 37 (request's measurement) | 0 (verified release build) |
| Compiler warnings, test build | 16 (recorded baseline) | 0 |
| Default release Clippy findings | approximately 120 (request's measurement) | 4 |
| Textual `.unwrap()` + `.expect()` occurrences, all Rust source including tests | 994 | 848 |
| Production `.unwrap()` calls | Not separately recorded on untouched source | 0 (compiler-backed Clippy audit) |
| Production `.expect()` calls | Not separately recorded on untouched source | 37 documented assertion sites |
| Files implementing Unix group signaling | 8 | 1 (`runtime/process.rs`) |
| `kill_process` implementations | 6 | 1 |
| `reader_check` implementations | 6 | 1 (test support) |
| Repeated controlled watchdog implementations | 6 | 1 (`Control::watch`) |
| `main.rs` lines | 4,806 | 1688 |

The baseline test count was recorded before edits. The original production warning and Clippy counts above are attributed to the request, rather than claimed as independently reproduced. The all-source assertion count is a textual measurement, including test assertions; the production counts come from Clippy with `unwrap_used` and `expect_used` enabled. Default Clippy does not enable those audit-only lints. Full file sizes are listed below; rustfmt expanded formerly compressed adapter code, so line counts are not directly a code-volume comparison.

## Shared process mechanics

`src/runtime/process.rs` owns process-group setup and spawn pipe configuration, group-only and group-plus-PID signals, bounded line reads, stderr collection policies, the controlled failure/cancel watchdog, ACP's delayed group watchdog, kill/reap, and bounded exit confirmation. Every runtime uses it directly or through its existing ACP/serve delegate. `opencode_process.rs` was replaced, not retained as a second implementation.

The bound is 8 MiB plus one sentinel byte per record, with existing 256-record queues. Existing driver parsing, error messages, terminal normalization, approvals, cancellation budgets, and protocol selection remain in the drivers. OpenCode and Kilo still decide how to report an unconfirmed exit; the shared confirmation helper returns false instead of manufacturing success. The shared primitives do not introduce one universal terminal-event policy.

Two regression tests were added: unterminated output cannot force unbounded record allocation; queue overflow kills the owned producer without event polling. Existing cancellation, blocked stdin, output flooding, approval lifecycle, continuation, and TypeScript ledger-reader tests still pass.

### Runtime-specific policies retained

These are the port's existing distinctions, identified from this copy's code and TypeScript references in its comments. External TypeScript sources were not attached or independently re-audited.

| Runtime | Retained distinction |
| --- | --- |
| Codex | JSON-RPC handshake, strict malformed record/early EOF errors, turn interrupt owed until IDs arrive, five-second watchdog, stderr discarded. |
| Claude | Stream-json, blank-line handling, stdin held until result, approval release on result, SIGINT then five-second watchdog, stderr discarded. |
| Cursor | Its oversized-record stand-in and edit metadata handling stay in its custom reader; cancellation remains SIGINT then five seconds. |
| Copilot | Print mode uses no stdin and keeps the beginning of stderr for policy/model refusal explanations. Approve-each still delegates to ACP instead of print's allow-all-tools route. |
| Antigravity | agy user record, stdin close after the prompt, stream normalization, SIGINT then five seconds; no native Approve-each. Transcript/project parsers remain fixture-only. |
| Muse | Prompt-file substitution and cleanup, no stdin, CRLF and BOM handling, SIGINT then five seconds; no native Approve-each. |
| ACP | Ignores non-object/non-JSON noise, observes prompt reply IDs, sends session/cancel, then kills only the process group after three seconds under its reap lock. Stderr retains its existing redaction/16 KiB display policy. |
| Kilo | Continues to use ACP with its existing profile, permission behavior, and exit confirmation; no independent process transport was introduced. |
| OpenCode | Version/protocol selection, ACP admission, serve startup/retry/SSE handling, no-resend behavior, one-second stop confirmation, and termination-unconfirmed reporting remain distinct. |

Copilot's old stderr collector used an unbounded `read_line(String)`, even though it stopped retaining new lines after its 64 KiB prefix threshold. It now uses the same bounded reader; it retains its line-boundary prefix policy and rejects invalid UTF-8 as before. The pathological unterminated-line allocation is therefore bounded (at most one 8 MiB + 1 byte record beyond the prefix threshold), as requested. Normal refusal capture is unchanged; this is the explicit oversized-input enforcement boundary introduced by the cleanup.

## Warnings and Clippy

No unused production item was wired into a new feature. Uncalled composer labels/accessors, `Decision::as_str`, ACP startup failure helper, Cursor's unused output-limit mutator, Copilot's unused scrubber/opaque list, unused request metadata fields, unused slash variants, the unconstructed queue origin, unused retryability metadata, and an unused project lookup helper were removed. Existing fixture-only Antigravity parsers and test helpers are compiled only for tests, preserving all existing tests.

Clippy fixes include equivalent conditional simplifications, derived implementations, large enum payload indirection, boxed large worker-error results, approval lifecycle type aliases, moving the agy refusal regex out of a loop, sorting by a key, drain loops, prefix handling, and formatting compressed code. No lint was suppressed.

Four argument-count lints remain:

- `src/approval.rs`: `Channel::ask_claude` (8 including self). Keeping the explicit protocol fields and cancellation callback keeps the wire operation readable.
- `src/ledger_write.rs`: `LedgerWriter::create_runtime` (8). The explicit mission, executable/argv, and continuation inputs describe the ledger boundary directly.
- `src/native.rs`: `command_for_turn` (8). These are the adapter's explicit command options; adding a second turn/options model solely to satisfy the count would obscure the port mapping.
- `src/native.rs`: `run_turn` (9). The explicit ownership handoff includes the cancellation registration, approval bridge lifetime, and store. Packaging unrelated lifetime owners into a lint-only bag would hide their purpose.

All remaining findings are this API-shape lint; no default correctness, complexity-of-types, large-variant, or performance findings remain. The argument-count findings are retained as a clarity tradeoff rather than hidden with allow attributes.

## UI and assertions

`src/ui/` separates app state/palette, sidebar, thread view, message rendering, activity drawers, approval card, composer wiring, scenes, and existing UI test modules. The root retains application orchestration and the entry point. Moving declarations and rendering expressions did not add screen behavior. Tool details and large runtime alternatives now use boxes without changing their contents.

Fallible filesystem parents, temporary-path clock reads, permission bridge timestamp/UTF-8 path conversion, missing transport state, and unavailable approval hosts now propagate errors through the existing status/ledger error paths. JSON user records use Value's infallible string serialization rather than asserting serialization into a Vec.

The 37 retained production `expect` sites assert source regex/color validity, successfully configured child pipes, poisoned internal-state invariants, or presence/type checks immediately dominating the access. Reasons are recorded at the sites or in `src/invariants.rs`. A poisoned lock means an earlier panic interrupted internal mutation; the code deliberately does not continue with potentially partial state. It is not treated as recoverable external input.

## Verification

Ran with `PATH=$HOME/.cargo/bin:$PATH`, `LOCUST_TEST_TS_READER=/Users/locust/work/locust/packages/mission-store/dist/index.js`, and `CARGO_TARGET_DIR=$HOME/work/scratch/native-target-clean1` from this folder:

- `cargo test --release` three consecutive times: **285 passed, 0 failed, 13 ignored** each.
- `cargo build --release`: success, **zero compiler warnings**.
- `cargo clippy --release`: success, **4 warnings**, listed above.
- `scripts/test-m1-cli.py`: success, including stream/failure/lost/interrupt/flood cases.
- `scripts/test-m3a-cli.py`: success, including tools, thinking, approval, and permission bridge checks.
- All 13 ignored test entry points, invoked individually with every `LOCUST_REAL_*` removed: each failed immediately with its explicit `set LOCUST_REAL_X=1` message. This check exercised only opt-in rejection, not real turns. ACP additionally still requires `LOCUST_REAL_ACP_COMMAND` after opt-in.
- `cargo fmt --check`: success.
- Four final snapshots generated with `--size 900x900 --scale 1` under `/Users/locust/work/scratch/clean1-snaps/`: `approval.png`, `composer-queue.png`, `thread-rich-open.png`, `native-live.png`. PNG dimensions were checked and all four images visually inspected; no obvious layout breakage was observed.

One intermediate run failed `engine::tests::starts_a_conversation_then_reads_working_waiting_and_the_answer` on the first synthetic HTTP request with a connection failure. The log is retained as `work/clean1/test-transient-failure.log`. It did not recur in the subsequent consecutive runs; the cause was not established and the test was not weakened. The final verification logs are in `work/clean1/`.

A verification helper selected the production executable in `release/deps` instead of the test harness and invoked `--list --ignored`. Because those are test-harness arguments, the application launched its window. The process was terminated after discovering the mistake; no turn was sent. The corrected checker took the exact test executable path from Cargo’s `Running unittests` line and verified all 13 guards. This accidental launch violated the requested no-window scope; it is not counted as window verification.

Not verified: interactive window behavior/exit, real runtime turns, or a fresh comparison against unattached TypeScript source. Those were excluded by scope or unavailable. Synthetic scenes are not evidence of a real live turn.

## Rust lines per file

| File | Before | After |
| --- | ---: | ---: |
| `src/approval.rs` | 938 | 1204 |
| `src/composer/attachments.rs` | 97 | 99 |
| `src/composer/drafts.rs` | 111 | 101 |
| `src/composer/file_mentions.rs` | 128 | 133 |
| `src/composer/keys.rs` | 194 | 260 |
| `src/composer/mod.rs` | 52 | 51 |
| `src/composer/placeholder.rs` | 141 | 143 |
| `src/composer/queue.rs` | 256 | 258 |
| `src/composer/queue_store.rs` | 261 | 283 |
| `src/composer/send_blocked.rs` | 111 | 122 |
| `src/composer/slash.rs` | 196 | 215 |
| `src/composer/workspace_files.rs` | 39 | 109 |
| `src/engine.rs` | 587 | 876 |
| `src/invariants.rs` | — | 13 |
| `src/ledger.rs` | 1781 | 2400 |
| `src/ledger_write.rs` | 271 | 325 |
| `src/main.rs` | 4806 | 1688 |
| `src/native.rs` | 914 | 1212 |
| `src/native/tests.rs` | 732 | 1302 |
| `src/quick.rs` | 1806 | 1996 |
| `src/run_cli.rs` | 250 | 401 |
| `src/runtime/acp.rs` | 704 | 763 |
| `src/runtime/acp_events.rs` | 510 | 524 |
| `src/runtime/acp_tests.rs` | 442 | 447 |
| `src/runtime/agy_events.rs` | 414 | 730 |
| `src/runtime/antigravity.rs` | 261 | 373 |
| `src/runtime/antigravity_events.rs` | 89 | 132 |
| `src/runtime/antigravity_projects.rs` | 136 | 149 |
| `src/runtime/antigravity_tests.rs` | 426 | 511 |
| `src/runtime/claude.rs` | 563 | 551 |
| `src/runtime/claude_events.rs` | 912 | 944 |
| `src/runtime/codex.rs` | 513 | 486 |
| `src/runtime/copilot.rs` | 351 | 390 |
| `src/runtime/copilot_events.rs` | 344 | 556 |
| `src/runtime/copilot_tests.rs` | 313 | 372 |
| `src/runtime/cursor.rs` | 577 | 543 |
| `src/runtime/cursor_events.rs` | 905 | 916 |
| `src/runtime/cursor_tests.rs` | 560 | 663 |
| `src/runtime/discovery.rs` | 53 | 60 |
| `src/runtime/events.rs` | 512 | 518 |
| `src/runtime/kilo.rs` | 315 | 357 |
| `src/runtime/kilo_tests.rs` | 285 | 347 |
| `src/runtime/mod.rs` | 66 | 73 |
| `src/runtime/muse.rs` | 259 | 409 |
| `src/runtime/muse_events.rs` | 483 | 490 |
| `src/runtime/muse_tests.rs` | 448 | 486 |
| `src/runtime/opencode.rs` | 57 | 302 |
| `src/runtime/opencode_config.rs` | 56 | 199 |
| `src/runtime/opencode_events.rs` | 84 | 512 |
| `src/runtime/opencode_process.rs` | 23 | deleted |
| `src/runtime/opencode_serve.rs` | 103 | 934 |
| `src/runtime/opencode_tests.rs` | 410 | 510 |
| `src/runtime/permission_bridge.rs` | 128 | 243 |
| `src/runtime/process.rs` | — | 341 |
| `src/runtime/test_support.rs` | — | 20 |
| `src/runtime/tests.rs` | 695 | 731 |
| `src/store.rs` | 350 | 430 |
| `src/ui/activity_drawers.rs` | — | 442 |
| `src/ui/app_state.rs` | — | 272 |
| `src/ui/approval_card.rs` | — | 286 |
| `src/ui/approval_review_tests.rs` | — | 21 |
| `src/ui/composer_wiring.rs` | — | 562 |
| `src/ui/daily_controls_tests.rs` | — | 239 |
| `src/ui/messages.rs` | — | 136 |
| `src/ui/native_window_tests.rs` | — | 459 |
| `src/ui/scenes.rs` | — | 612 |
| `src/ui/sidebar.rs` | — | 205 |
| `src/ui/tests.rs` | — | 659 |
| `src/ui/thread_view.rs` | — | 731 |
