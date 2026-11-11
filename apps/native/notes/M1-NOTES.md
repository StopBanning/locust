# M1 walking skeleton

Implemented the headless Codex path in this copy. The real-turn exit test remains Colin's to run; no real Codex turn was started here. M1 is not marked reached and no PARITY.md rows are ticked on synthetic evidence alone.

## Changes by file and reference ports

| File | Change and TypeScript reference |
| --- | --- |
| `src/store.rs` | `LOCUST_NATIVE_DATA`, otherwise `engine::default_data_folder()` plus `-native`. Resolves existing symlinks and `..` components before refusing Electron's folder and its descendants. Validates mission IDs and rechecks the destination after creating directories. Uses the native-store boundary in PLAN.md; no Electron writes. |
| `src/ledger_write.rs` | New-mission-only, exclusively created append-only JSONL writer. Ports `packages/mission-store/src/index.ts:createFileMissionLedger.createMission`, `appendEvents`, `appendRecords`, `syncDirectoryBestEffort`, and the schema-27 `CreatedRecord`/`EventRecord` envelopes. Includes metadata, correlated event IDs, runtime/route fields, timestamps, separate ledger/event sequences, mode 0600 on Unix, `sync_all` after each record, best-effort directory fsync after creation, external-size-change detection, 512 KiB record and 64 MiB file limits, and poisoning after uncertain writes. No retries after uncertain writes. `apps/desktop/src/main/ledger-writes.ts:countingLedgerWrites` is only a host cache invalidator; this CLI has no derived cache, so it needs no wrapper. |
| `src/runtime/mod.rs` | Window-independent `Runtime` trait: start a turn, consume events, interrupt. |
| `src/runtime/codex.rs` | Ports `codex-app-server-run.ts:startCodexAppServerRun` handshake and new-thread/new-turn path, and `app-server.ts:createAppServerClient` newline JSON-RPC transport/correlation. `initialize`, `initialized`, `config/read`, `thread/start`, `turn/start`, `turn/interrupt`. Handshake requests time out at 60 seconds; input records are capped at 8 MiB; the stdout channel is bounded. Ports `apps/desktop/src/main/codex-read-only.ts:codexReadOnlyThreadConfig` to disable local MCP programs, plugins, browser and computer use, refusing unreadable/unsafe settings. Uses `commands.ts:codexAppServerPolicy` Ask policy (`read-only`, `never`). Approval requests are explicitly declined with diagnostics, unsupported requests receive JSON-RPC errors. Unix subprocesses get their own process group, killed and reaped on completion/drop. |
| `src/runtime/events.rs` | Ports the M1 notification branches of `app-server-events.ts:createAppServerEventNormalizer`, `notificationThreadId`, `toolNameOf`, `toolCommandOf`, usage counters, rate-limit snapshots, compaction deduplication, and `codex-events.ts:boundedMessageText`, `boundedAnswerText`, `redactSecrets`. Emits `run.*`, `message.delta`, `tool.*`, `step.*`, `plan.updated`, `adapter.diagnostic`, and `route.limit_detected`. Sub-agent threads cannot speak for or finish the parent. Final assistant text replaces previous deltas. Evidence includes method name and redaction flag, never wholesale provider params. Reasoning summary is whitelisted; raw reasoning `content` and raw reasoning delta notifications are never persisted. |
| `src/runtime/discovery.rs` | Ports the Unix PATH search from `path-locator.ts:createPathExecutableLocator`, `pathDirectories`, and `defaultIsExecutableFile`: exact command name, absolute PATH entries only, executable regular file, first match wins. No login or credential reads, copies, or changes. |
| `src/run_cli.rs` | Parses `--run codex <prompt> [--folder <dir>] [--model <id>]`, creates native ledger, streams and flushes message deltas to stdout, suppresses duplicate final replacement text, prints diagnostics and ledger path to stderr. Success exits 0; failure/cancellation exits 1. Ctrl-C sends interrupt, records cancellation, and terminates the subprocess if it has not ended after five seconds. |
| `src/main.rs` | Five module declarations and a small early `--run` dispatch hook; no window is constructed for this path. No changes to `ledger.rs`, `engine.rs`, or UI behavior. |
| `src/runtime/tests.rs` | Synthetic end-to-end transport-to-writer-to-existing-native-reader tests, terminal failure/loss/cancellation cases, read-only config refusal, reasoning exclusion, secret scrubbing, bounds, limits, and exclusive-writer/external-change checks. |
| `scripts/fixtures/m1-app-server.py` | Tiny synthetic server based on `packages/runtime-adapters/test/codex-app-server-run.test.ts:fakeServer` and `app-server-events.test.ts` notification shapes. Checks handshake and policy fields, sends foreign-thread completion, streamed and final messages, reasoning summary plus forbidden raw content sentinel, tools, plan, approval, warning, usage, failure and interruption. No credentials, network, or real CLI. |
| `scripts/check-m1-ledger.mjs` | Runs the existing Electron TypeScript ledger reader over Rust output, checking recovered phase and zero recovery issues. |
| `scripts/test-m1-cli.py` | Runs the release binary against a temporary fake `codex` on PATH. Checks stdout before completion, exact final output without doubling, CLI exit codes and ledger terminal records, including actual SIGINT. |
| `Cargo.toml`, `Cargo.lock` | Adds `chrono` for UTC millisecond ISO timestamps and local reset clocks; `ctrlc` for signal handling; `regex` for the TS secret-scrubbing patterns. Direct Unix `libc` dependency uses the already-transitive package for process-group termination. Enables serde_json `preserve_order` to preserve record field insertion order. No Node runtime dependency for native operation; Python and Node are synthetic verification tools only. |

## Verification

On this Mac, with `~/.cargo/bin` prepended to PATH (Cargo was absent from the inherited PATH):

```sh
LOCUST_TEST_TS_READER=/Users/locust/work/locust/packages/mission-store/dist/index.js \
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-m1 cargo test --release
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-m1 cargo build --release
python3 scripts/test-m1-cli.py "$HOME/work/scratch/native-target-m1/release/locust-native"
```

- Release tests: **65 passed, 0 failed, 2 ignored**. The ignored tests are the pre-existing live Electron/live-ledger tests; they were not run.
- Release build passed.
- The installed Node imported the reference repository's already-built `packages/mission-store/dist/index.js:createFileMissionLedger`, without installs or edits in the reference repository. Its strict reader accepted Rust success (18 events), explicit failure (3), transport loss (2), and cancellation (2), with zero issues and the expected phase. This is schema compatibility, not a claim of byte-identical JSON string serialization or identical human-facing error prose for every possible provider error.
- `ledger.rs:read_transcript` recovered the expected user prompt and a single complete `Hello world` assistant reply from Rust-written streamed/replacement records. Failure and cancellation records were recovered as terminal.
- Built-binary fake smoke: success exit 0 and stdout exactly `Hello world\n`; provider failure, transport loss and SIGINT exit 1 with the respective ledger terminal events. The test reads `Hello ` while the fake server is still running, proving streaming before completion.
- Electron path guard rejects direct descendants, normalized traversal and symlink aliases. Exclusive creation rejects a second writer. External appends poison the active writer.
- A forbidden raw-reasoning sentinel was present in fake provider input and absent from every resulting ledger; the allowed summary survived.

The first test run caught a test assertion using UTF-8 byte count for the TS UTF-16 text bound. The assertion and implementation now use UTF-16 units; the final suite passes.

## Remaining limits and unconfirmed behavior

- Colin's real authenticated Codex turn with Electron quit remains unrun by request. Live protocol drift, actual model selection and provider behavior are therefore unconfirmed. Codex reads its own authentication; native code never opens its credentials.
- This implements new threads and one turn only. Resume, steer, fork, slash commands and interactive approvals are outside this M1 CLI. M3 supplies interactive approvals.
- Discovery matches the reference's Unix PATH behavior on this Mac. Windows-specific npm `.cmd`/PowerShell launch shims and Windows installer-root discovery are not ported; Windows was not built or tested.
- The new writer owns new files only; it does not hydrate/reopen existing ledgers, repair partial records, or port all general-purpose TS ledger operations. Serialization and validation are proven for the synthetic M1 event cases, not exhaustively fuzzed against every malformed provider payload. Oversize records/files fail safely rather than porting all of `fit.ts`'s recursive shrinking behavior.
- The transport reports a clear generic failure on early EOF or malformed protocol but does not retain the TS stderr-tail diagnostic. The bounded channel applies backpressure instead of TS's notification-overflow drop/terminate policy.
- A final replacement that revises previously printed text is correct in the ledger; stdout cannot retract text already emitted. The normal streamed-prefix case is tested.
- The current window still lists Electron's read-only ledger directory. The existing native reader can read the new files (tested directly), but selecting the native store in the window belongs with M2; no extra main.rs/UI changes were made here.
- No filesystem fsync failure injection, power-loss recovery, Windows process-tree cleanup, live sandbox escape attempt, or real UI rendering test was performed.

## Fixes

Review fixes applied in this M1 copy (2026-11-09):

- `src/ledger_write.rs` retains exclusive creation and adds Unix `O_NOFOLLOW`. Before writing the first record, `src/store.rs` validates the opened descriptor using fstat (`File::metadata`), macOS `F_GETPATH` or Linux `/proc/self/fd`, the Electron-folder guard, and a device/inode match against the reported location. Location verification fails closed on unsupported platforms. A rejected open drops/closes the descriptor without writing or deleting; the Electron-folder error reports the actual location and that the empty file was left in place.
- The new ledger regression deterministically swaps an already-checked native ancestor for a symlink to a temporary Electron folder before opening. It verifies rejection, the reported destination, and an existing zero-byte file. A final-component symlink is also refused without changing its target.
- `src/runtime/codex.rs` replaces blocking stdout channel sends with `try_send`. Overflow of the 256-record queue records a transport failure and immediately wakes an independent process-group kill watchdog. Transport failure emits an `adapter.diagnostic` followed by a failed/cancelled terminal event, preempting pending deltas. This supersedes the earlier backpressure-only limitation above. The referenced TypeScript source was not attached/read this turn; this implements the requested overflow/termination policy.
- A dedicated stdin writer owns the pipe; callers serialize and `try_send` to its own bounded queue. Approval replies and interrupt requests cannot block on pipe I/O. Queue saturation or writer failure terminates the transport with diagnostics; an interrupt enqueue failure still permits the terminal ledger event to be recorded.
- `stop()` arms a five-second deadline before enqueueing interrupt. A condition-variable watchdog kills the process group independently of stdout delivery, stdin delivery, and event polling. Drop also kills and reaps without waiting on either I/O thread. Terminal events already normalized remain deliverable.
- New runtime regressions prove stdout overflow kills a producer with no consumer draining, and a child that never reads stdin is killed after the cancel deadline even with no event polling; `stop()` itself returns within 250 ms. The blocked-stdin test uses a 1 MiB queued message to exceed pipe capacity.
- The fake app-server and CLI smoke now include unbounded deltas while the server never reads stdin again. The smoke observes a delta before sending actual SIGINT, requires exit within six seconds (the five-second deadline plus scheduling margin), verifies the fake server no longer exists, and accepts only cancelled/failed terminal ledger records.

Verification on this Mac:

```sh
PATH=$HOME/.cargo/bin:$PATH CARGO_TARGET_DIR=$HOME/work/scratch/native-target-m1 cargo test --release
PATH=$HOME/.cargo/bin:$PATH CARGO_TARGET_DIR=$HOME/work/scratch/native-target-m1 cargo build --release
python3 scripts/test-m1-cli.py "$HOME/work/scratch/native-target-m1/release/locust-native"
```

- Release tests: **68 passed, 0 failed, 2 ignored** (the existing live Electron/live-ledger tests).
- Release build passed.
- All five fake-server CLI scenarios passed: success, provider failure, transport loss, SIGINT, and flood plus SIGINT. The final smoke measured ordinary cancellation at 0.064 seconds and flood termination at 0.008 seconds after SIGINT; flood ended `run.failed` and its server was gone. The separate blocked-stdin regression exercised the full five-second watchdog deadline.
- No real Codex turn was run. Linux descriptor-location code was not built or executed; only the macOS branch was exercised. The optional external TypeScript ledger reader was not run this turn, and no external reference folder was read or changed.
