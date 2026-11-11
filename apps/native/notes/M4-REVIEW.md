# M4 runtime review

Reviewed the native M4 runtime code and wiring against `/Users/locust/work/locust/packages/runtime-adapters/src` and its adjacent `test` directory. PLAN.md requires behavioral tests, not just feature rows. This review does **not** certify complete TypeScript test parity; the inventory below explicitly identifies incomplete and deferred coverage.

## Findings and fixes

1. **Copilot Approve-each silently allowed tools.** The native command always used print mode with `--allow-all-tools`. It now selects `--acp` for Approve-each, preserving model/effort and the session-store tool exclusion. The shared ACP driver requires the advertised Agent mode, switches a resumed Autopilot session to Agent, and sets an advertised `allow_all` option to `off` before sending any prompt. Like TypeScript, an absent `allow_all` option is accepted; refusal to change an offered option fails before prompting. Synthetic peer checks allow, deny, absent option, rejected setting, and resume. No retry is introduced.
2. **Unsupported approval modes silently degraded.** Cursor, Antigravity and Muse print drivers mapped Approve-each to Edit without cards. Their launch and start paths now reject that unsupported mode. This is a deliberate fail-closed native capability gate; existing Ask/Edit/Auto arguments remain intact.
3. **Muse wrote prompt material outside native data and could truncate a predictable existing file.** `prepare_prompt_file` now requires the originating Store, creates a unique native-owned directory with Store's no-follow exclusive writer, and retains cwd/environment/environment removals while replacing the prompt-file argument. Native passes its actual Store into the turn worker. The prompt file is still removed when the runtime drops. Test verifies native ownership, uniqueness, bytes and launch settings.
4. **OpenCode inherited writable global config/data/cache homes.** Both versions now receive native-owned XDG config/data/cache/state homes. Guard plugins remain run-owned, created through Store. Persistent session data uses a stable home so resume can see earlier sessions. Test covers both version configurations and every XDG path. Credentials are neither copied nor modified. This changes which OpenCode installations have credentials available; no real authenticated OpenCode turn was rerun under the new isolation.
5. **Kilo resume lost its data home.** Every command previously allocated a new home; a later process could not load a session saved by its predecessor. Its isolated home is now stable under native data. Cache/state homes are isolated too. Test writes a session marker through one home and reads it through the next allocation. This proves home continuity, not a real Kilo session/load.
6. **Reverted fix #6 to match Electron: Cursor displayed thinking is kept.** Colin clarified that reasoning a runtime shows its own user belongs in the thinking row; `cursor-events.ts` lines 42–46 and 275–330 record that decision. Restored per-fragment secret scrubbing without a size bound, accumulation into one reasoning step, and trimmed/bounded text in `step.completed.message` (16,384 UTF-16 units, keeping both ends). Start events have no message, completion-record text is not treated as a delta, and empty thinking omits the message. The captured-stream regression again asserts retained thinking, and a variant asserts a repeated fake secret is scrubbed. Added direct regressions for fragment joining, the TS Bearer redaction spelling, empty/reset/completion behavior, and bounding only after accumulation. All other review fixes are unchanged.
7. **Antigravity final messages depended on randomized map order.** The TS normalizer trims earlier message prefixes in insertion order. Rust used HashMap iteration, sometimes repeating earlier text in the final answer. It now retains arrival order; a repeated regression verifies three-message prefix removal.
8. **Antigravity lacked helper-wait and repeated-context notices.** Ported helper-list tracking, wait headlines/durations and the one-time third uncached whole-context-read warning. Completed helpers are excluded and an empty active-helper list clears the state. Synthetic regression covers the captured helper-list/wake-up shapes, warning position/count and clearing/cached steps.
9. **Antigravity finish was not idempotent and omitted result evidence on failure.** Repeated finish calls now emit nothing; failed runs carry result evidence when present. Quota reset timestamps use millisecond ISO precision, matching JS `toISOString`.

## Antigravity quota investigation

The original Rust code already contained the requested detector and ordering. The exact TS regression shape is `event: result`, nested `result.status: ERROR`, empty `response`, and string `result.error: Individual quota reached. Please upgrade your subscription to increase your limits. Resets in 33h54m24s.` The other recognized spelling is `RESOURCE_EXHAUSTED`. Neither requires a nonzero CLI exit code to report failure.

Added `scripts/fixtures/agy/quota-exhausted.jsonl` using that shape and drove it through the real Rust subprocess driver (synthetic Python CLI, exit 0), LedgerWriter and Electron's TypeScript ledger reader. Its last two ledger events are `route.limit_detected` (`kind: quota-exhausted`) then `run.failed` with QUOTA_SENTENCE. A separate regression asserts evidence, ordering and once-only finish for both spellings. The fixed-clock reset calculation matches `2026-10-24T02:58:08.000Z`.

This verifies the fixture-to-ledger path, not the user's earlier live run: that ledger was not supplied and no live quota was consumed. Carry-on's routing implementation is not exercised by these runtime tests.

## Launch, Stop, resume and safety audit

- `commands.ts` **does pass `--model` when explicitly selected** for both Cursor and agy. Native retains that conditional behavior and passes no model flag when absent. Cursor Ask/Plan omit `--approve-mcps`, use `--mode ask` and enable its sandbox on supported platforms. Auto alone uses force. agy uses its stream-json user envelope, accept-edits for Edit, skip-permissions only for Auto, conversation id for resume, and selected effort.
- Copilot print excludes `session_store_sql`, uses read-only tool denials and Auto-only all-path access; resume uses `--resume=<id>`. Approve-each now uses ACP as described above. Muse keeps prompt-file transport, read-only write/shell disabling and sandbox behavior.
- ACP startup/load never replays saved text into the new ledger. OpenCode/Kilo retries remain limited to transport failures **before** prompt admission, after confirmed process exit; OpenCode serve's SQLite-busy restart is before session/prompt creation. `send_prompt` sets admission before attempting the write. No uncertain prompt is resent by these fixes.
- Existing synthetic tests cover ACP Stop/cancel, pending-card exit, transport loss, output overflow, protocol refusal, approval allow/deny and selected configuration; version-split OpenCode ACP/serve startup, busy startup, streaming and native dispatch; every M4 runtime's ledger/native dispatch fixtures. They do not constitute real Stop/resume verification for every installed CLI.
- Guard and prompt files created by these changes are under Store. **No filesystem-wide trace was performed.** Third-party CLIs can still write their own state, and tools deliberately requested by a turn can write workspace files. This review does not claim all subprocess writes are confined to native data. OpenCode version probing still executes the installed CLI before configuring its run.
- Remaining feature gaps: native command construction does not yet pass Electron's optional skill/connector/browser integrations, reference-folder options, user-provider pricing, or Copilot command-guard plugin / configured-MCP-server disable list. Those tests are explicitly deferred below. In particular, the latter two prevent a claim of full Electron security-policy parity. No subscription credential store was read or edited by this review.

## TypeScript coverage inventory

Paths below are relative to `packages/runtime-adapters`. “Partial” means the named Rust suite covers selected behaviors, **not every TS assertion**. Unmatched cases in a partial file are deferred for the stated reason; no checklist row should be closed on a claim of complete porting. “Deferred” is an acknowledged implementation/test gap, not a declaration that the TS behavior is unnecessary. All live `#[ignore]` tests were excluded as requested.
- `src/a-binary-that-did-not-change-is-not-asked-twice.test.ts` — Deferred: Electron binary consent/cache lifecycle is not implemented by these drivers.
- `src/a-refusal-is-not-a-crash.test.ts` — Deferred: Electron probe consent refusal lifecycle is not implemented by these drivers.
- `src/one-runtimes-probes-run-at-once.test.ts` — Deferred: Electron asynchronous runtime probe scheduler is not implemented by these drivers.
- `src/probes-report-as-they-run.test.ts` — Deferred: Electron probe progress scheduler is not implemented by these drivers.
- `src/the-node-we-already-ship.test.ts` — Not applicable: native ships a Rust binary, not Electron’s bundled Node runtime.
- `src/the-prompt-muse-cannot-be-told.test.ts` — Partial → muse prompt-file argument and new native-owned file test; Node stdin/timer implementation is replaced, not ported literally.
- `test/a-backgrounded-call-says-so.test.ts` — Deferred: no complete Rust background tool display fixture port.
- `test/a-busy-model-served-says-so.test.ts` — Deferred: retry notices exist; complete served-busy fixture/timing regression not ported.
- `test/a-claude-edit-says-what-it-changed.test.ts` — Outside this review: Codex/Claude/app-server adapter tests belong to M1–M3; not rerouted to an M4 suite.
- `test/a-claude-run-that-lingers-after-its-result-completed.test.ts` — Outside this review: Codex/Claude/app-server adapter tests belong to M1–M3; not rerouted to an M4 suite.
- `test/a-cmd-shim-in-a-folder-with-a-space-runs.test.ts` — Deferred: Windows cmd shim launch semantics cannot be validated on this macOS native run.
- `test/a-codex-schema-change-cannot-silently-drop-a-read-field.test.ts` — Outside this review: Codex/Claude/app-server adapter tests belong to M1–M3; not rerouted to an M4 suite.
- `test/a-config-warning-says-its-summary.test.ts` — Deferred: Codex config-warning behavior is outside the changed M4 adapters.
- `test/a-context-inspection-keeps-the-turns-tools.test.ts` — Deferred: context-inspection feature is outside these native runtime drivers.
- `test/a-denied-read-and-a-file-row-are-said-as-they-are.test.ts` — Deferred: no direct equivalent for every TS denied-read/file tool-row fixture.
- `test/a-free-model-refused-is-never-a-sign-in.test.ts` — Partial → opencode_error_sentences_match_ts; every captured free-provider refusal shape is not ported.
- `test/a-helpers-calls-are-its-rows-children.test.ts` — Deferred: native helper-child tool projection lacks the full TS fixture regression.
- `test/a-message-taken-and-never-begun-ends-the-run.test.ts` — Deferred: recovery code exists in opencode_serve.rs, but the TS silent prompt-accept/never-start timing matrix has no direct Rust regression.
- `test/a-model-of-your-own-is-priced-step-by-step.test.ts` — Deferred: user-provider pricing integration is not implemented in native.
- `test/a-muse-plan-used-up-is-said-once.test.ts` — Deferred: quota normalization exists in muse_events.rs, but the exact duplicate terminal/task quota fixture lacks a matching Rust test.
- `test/a-rate-limited-opencode-run-says-it-is-retrying.test.ts` — Deferred: locust.retry normalization exists; exact stderr/retry deduplication fixture tests are not ported.
- `test/a-real-codex-session-replays-through-the-adapter.test.ts` — Outside this review: Codex/Claude/app-server adapter tests belong to M1–M3; not rerouted to an M4 suite.
- `test/a-real-muse-wonder-run-reads-whole.test.ts` — Partial → muse_echo_capture_fixture exercises the captured stream; complete whole-answer/Wonder-specific fixture matrix remains unported; live Muse remains ignored.
- `test/a-result-too-large-to-keep-still-answered.test.ts` — Deferred: no matching Rust behavior regression identified in this review; complete port remains owed.
- `test/a-run-is-handed-its-own-skills.test.ts` — Deferred: per-runtime native skill plugin/workspace construction is not implemented.
- `test/a-run-without-connectors-leaves-them-out.test.ts` — Deferred: per-runtime native connector exclusion/config assembly is not implemented.
- `test/a-running-codex-turn-can-be-steered.test.ts` — Outside this review: Codex/Claude/app-server adapter tests belong to M1–M3; not rerouted to an M4 suite.
- `test/a-sealed-run-uses-no-tool.test.ts` — Deferred: sealed-run option is not exposed by native Turn.
- `test/a-self-updated-cli-is-asked-again.test.ts` — Deferred: Electron probe consent/cache lifecycle is outside native driver execution.
- `test/a-server-whose-storage-is-busy-starts-again.test.ts` — Partial → opencode_serve_busy_restarts_then_runs; exhaustive backoff/deadline/cancel matrix not ported.
- `test/a-session-openCode-cannot-continue-is-not-resumed.test.ts` — Partial → OpenCode error tests exercise session-ended facts; a full native cold-follow-up after that exact encrypted-content error is unported.
- `test/a-side-question-forks-the-session.test.ts` — Deferred: fork-session option is not exposed by native Turn.
- `test/a-slow-runtime-is-shown-as-checking.test.ts` — Deferred: Electron discovery progress UI lifecycle is outside these drivers.
- `test/acp-events.test.ts` — Partial → runtime/acp_tests.rs: messages, hidden thoughts, tools, diffs, plans, limits; exhaustive TS update-shape assertions remain unported.
- `test/acp-run.test.ts` — Partial → runtime/acp_tests.rs: initialize/load/config, approvals, cancellation, steering, EOF and overflow; TS fake EventEmitter transport assertions are not all reproduced.
- `test/agy-events.test.ts` — Partial → runtime/antigravity_tests.rs: captured tools/refusals, quota, unavailable/lost sessions, arguments and ordered answer prefixes; remaining TS assertion combinations are not exhaustively ported.
- `test/an-acp-output-stream-closing-ends-the-turn.test.ts` — Partial → acp_transport_loss, acp_exit_with_pending_card; exact Node stream interleavings are not reproduced.
- `test/an-expired-sign-in-is-said.test.ts` — Partial → OpenCode/Cursor error helpers; complete multi-runtime provider-error corpus not ported.
- `test/an-opencode-2-step-says-what-it-did.test.ts` — Deferred: shared ACP normalization exists, but no complete matching V2 step-kind fixture matrix.
- `test/an-opencode-reply-streams.test.ts` — Partial → opencode_serve_streams_authenticated_sse; all partial-record ordering permutations remain unported.
- `test/an-oversized-cursor-edit-keeps-its-counts.test.ts` — Deferred: no direct Rust regression for the TS oversized-edit fixture/count preservation.
- `test/antigravity-background.test.ts` — Deferred: Electron app background RPC transport is not part of the native agy CLI driver.
- `test/antigravity-events.test.ts` — Partial → antigravity_events.rs plain transcript fixture; complete app transcript response/tool matrix not ported, since live runs use agy.
- `test/antigravity-gaps.test.ts` — Deferred: Electron app RPC gap/recovery fixtures are not implemented by the native agy CLI driver.
- `test/antigravity-question.test.ts` — Deferred: Electron app question-card RPC path is absent from the native agy CLI driver.
- `test/antigravity-waiting-is-said-as-waiting.test.ts` — Partial → agy_waiting_helper_and_third_uncached_reread; wait, reset, cached wakeups and warning timing covered; malformed-list/default-field combinations and every captured duration not separately ported.
- `test/app-server-events.test.ts` — Outside this review: Codex/Claude/app-server adapter tests belong to M1–M3; not rerouted to an M4 suite.
- `test/app-server.test.ts` — Outside this review: Codex/Claude/app-server adapter tests belong to M1–M3; not rerouted to an M4 suite.
- `test/background-work-ends-with-the-run.test.ts` — Deferred: no complete Rust background-call lifecycle regression matrix.
- `test/claude-commands.test.ts` — Outside this review: Codex/Claude/app-server adapter tests belong to M1–M3; not rerouted to an M4 suite.
- `test/claude-events.test.ts` — Outside this review: Codex/Claude/app-server adapter tests belong to M1–M3; not rerouted to an M4 suite.
- `test/codex-app-server-run.test.ts` — Outside this review: Codex/Claude/app-server adapter tests belong to M1–M3; not rerouted to an M4 suite.
- `test/codex-events.test.ts` — Outside this review: Codex/Claude/app-server adapter tests belong to M1–M3; not rerouted to an M4 suite.
- `test/codex-long-answer-keeps-its-length.test.ts` — Outside this review: Codex/Claude/app-server adapter tests belong to M1–M3; not rerouted to an M4 suite.
- `test/connector-allow-rules.test.ts` — Deferred: connector allow-rule assembly is not implemented in native.
- `test/connectors.test.ts` — Deferred: runtime connector discovery is not ported in this runtime round.
- `test/copilot-effort-levels.test.ts` — Deferred: native passes explicit effort; TS dynamic help-choice parser tests belong to discovery, not reproduced by these driver tests.
- `test/copilot-events.test.ts` — Partial → runtime/copilot_tests.rs: captured answers, edits, refusal, policy stderr and argument/dispatch tests; not every TS record-shape assertion has a matching Rust case.
- `test/copilots-new-records-are-not-news.test.ts` — Deferred: ignored-record code exists, but a direct assertion for each added record type is not ported.
- `test/copilots-search-index-record-is-not-news.test.ts` — Deferred: no direct Rust regression for this noise-record fixture.
- `test/cursor-connectors.test.ts` — Deferred: native Cursor connector discovery/allow-rule integration is not implemented.
- `test/cursor-events.test.ts` — Partial → runtime/cursor_tests.rs: real captured output, tools, usage, final replacement, resume, failures, plans; displayed thinking retention and closing-step placement restored to match TS, including a secret-bearing captured-stream variant.
- `test/cursor-secrets-in-text.test.ts` — Partial → shared text scrubber and captured Cursor stream checks; thinking-secret regression now ported directly, including the fake sk- token from TS; full answer/tool secret corpus remains unported.
- `test/effort-choices.test.ts` — Deferred: dynamic CLI help parsing is not ported for every runtime.
- `test/kilo-is-discovered-and-starts-on-auto-free.test.ts` — Partial → runtime/kilo_tests.rs: model/mode order, isolation, permissions, dispatch, stable resume home; discovery and Windows git-process fixtures not ported.
- `test/line-diff.test.ts` — Partial → ACP patch regressions exercise real unified diff paths; exhaustive TS LCS/large-input/context matrix not ported.
- `test/message-tail.test.ts` — Deferred: shared TS message-tail truncation corpus has not been ported exhaustively.
- `test/more-secrets-are-scrubbed.test.ts` — Partial → runtime/tests.rs text_is_bounded_and_scrubbed; complete TS secret-pattern corpus not ported.
- `test/muse-events.test.ts` — Partial → runtime/muse_tests.rs: captured envelope/lifecycle, tools, failures, sandbox, sessions and internal tasks; not every TS combination has a separate Rust case.
- `test/opencode-2-discovery-uses-its-acp-command.test.ts` — Partial → opencode_version_split_matches_discovery; complete catalog/probe caching behavior is outside these drivers.
- `test/opencode-acp-profile.test.ts` — Partial → opencode_acp_success_sets_locust_mode_and_records; no exhaustive advertised-configuration matrix.
- `test/opencode-asks-with-the-question-card.test.ts` — Deferred: no matching Rust behavior regression identified in this review; complete port remains owed.
- `test/opencode-commands.test.ts` — Partial → opencode_mode_config_keeps_question_off_and_mode_rules; no complete command-list transport fixture port.
- `test/opencode-error.test.ts` — Partial → opencode_error_sentences_match_ts; exhaustive nested provider-error/HTTP-status cases remain unported.
- `test/opencode-events.test.ts` — Partial → runtime/opencode_tests.rs: steps, usage, errors and streamed completion; compaction/child-tool/pricing/partial-message fixture combinations are not exhaustively ported.
- `test/opencode-every-mode-through-its-server.test.ts` — Partial → native permission configuration tests plus serve streaming; each mode’s complete server-side permission exchange still lacks a direct matching test.
- `test/opencode-serve-run.test.ts` — Partial → opencode_serve_streams_authenticated_sse and opencode_serve_busy_restarts_then_runs; Node HTTP/event timing matrix not exhaustively ported.
- `test/permission-bridge-argv.test.ts` — Partial → permission_bridge tests and Claude approval tests; exhaustive JS bridge argv cases are outside M4 transport changes.
- `test/process-runner.test.ts` — Deferred: Rust has per-runtime drivers, not the shared Node runner; exhaustive equivalent transport timing/stand-in/termination tests are still owed.
- `test/provider-error.test.ts` — Partial → OpenCode error and Cursor provider sentences are exercised; complete provider-error corpus remains unported.
- `test/qa-0365-opencode-worktree.test.ts` — Deferred: repository-root/.git reference allowances are not exposed in native command construction.
- `test/references-never-widen-a-mode.test.ts` — Deferred: reference-folder command options are not exposed by native Turn.
- `test/refusal-is-readable.test.ts` — Partial → native tool-refusal fixtures; full cross-runtime TS corpus not ported.
- `test/runtime-adapters.test.ts` — Partial → launch argument tests for each native adapter; optional integration/slash/fork/reference/provider options are absent and not certified.
- `test/stop-reaches-the-tree.test.ts` — Partial → existing ACP/Codex blocked-pipe Stop tests; Windows process-tree behavior cannot be proved by this macOS run.
- `test/the-browser-shares-one-flag-with-the-bridge.test.ts` — Deferred: native browser MCP launch integration is not implemented.
- `test/the-parent-git-is-reachable-on-every-platform.test.ts` — Deferred: repository-root git allowances and cross-platform git fixture matrix are not fully ported.
- `test/the-steps-show-what-the-runtimes-show.test.ts` — Partial → per-adapter captured-stream tests; cross-runtime presentation matrix not exhaustively ported.
- `test/your-own-model-is-a-provider.test.ts` — Deferred: user-provider declaration integration is not implemented in native.

## Initial review verification results

All commands ran from `/Users/locust/Documents/Locust/native-review/m4rev/native` with `PATH=$HOME/.cargo/bin:$PATH`, `LOCUST_TEST_TS_READER=/Users/locust/work/locust/packages/mission-store/dist/index.js`, and `CARGO_TARGET_DIR=$HOME/work/scratch/native-target-m4rev`.

- Final code: `cargo test --release` **three consecutive passes**, each **281 passed, 0 failed, 13 ignored** (8.99 s, 9.03 s, 8.99 s). Logs: `review-final-1.log`, `review-final-2.log`, `review-final-3.log`. These runs include the real-shaped quota subprocess ledger compatibility regression and Copilot ACP approval regression.
- `cargo build --release`: **passed**, 26.72 s; 36 compiler warnings, predominantly unused/dead code. Log: `review-build.log`. Warnings were not promoted to errors; this was not a Clippy run.
- `scripts/test-m1-cli.py /Users/locust/work/scratch/native-target-m4rev/release/locust-native`: **passed** all five scenarios. Interrupt/flood cancellation returned in 0.043 s / 0.009 s. Log: `review-m1-cli.log`.
- `scripts/test-m3a-cli.py` with the same binary: **passed** success, failure, lost transport, interrupt, limit, tools, thinking, approval and permission-bridge initialize/list/call/unreachable-host denial. Log: `review-m3a-cli.log`.
- Earlier in this review, one full suite run failed two existing engine HTTP fixture tests: `engine::tests::daily_controls_use_the_mcp_door_and_optional_fields` (models could not be read) and `engine::tests::lists_the_team_with_the_token_in_the_header_and_nowhere_else` (0 rows versus 2). All runtime tests passed in that run. Immediate reruns and all three final runs passed. Root cause is **not established**; the failure log is retained as `review-test-1.log`. These engine tests were not edited.

Not run: any `#[ignore]` real test, any installed CLI live turn, real CLI Stop/resume, window tests with Electron quit, live carry-on, Windows/Linux validation, a complete TS suite run, or a filesystem trace of third-party CLI writes. Earlier real OpenCode/Kilo/Cursor/Copilot successes supplied by the user are context, not verification of these changes. Full M4/TS parity remains incomplete for the deferred cases above.

## Cursor thinking correction — 2026-11-10

Reverted fix #6 at Colin's direction: Cursor's displayed thinking is user-visible runtime output, not hidden reasoning to discard. Its text is again retained in the completed thinking row, with TS fragment scrubbing and final trim/bounding. Updated the two Cursor coverage entries above. Code changes for this correction are confined to `src/runtime/cursor_events.rs`, its regressions in `src/runtime/cursor_tests.rs`, and a new unbounded secret-scrubbing helper in `src/runtime/events.rs`. The pre-existing `text` function and every other runtime fix remain unchanged.

Reverification used the same working directory and environment as the initial review:

- `cargo test --release`: **three consecutive passes**, each **283 passed, 0 failed, 13 ignored**, in 9.47 s, 9.09 s and 8.90 s. Logs: `review-cursor-1.log`, `review-cursor-2.log`, `review-cursor-3.log`.
- `cargo build --release`: **passed**, 26.70 s, 36 compiler warnings. Log: `review-cursor-build.log`.
- `scripts/test-m1-cli.py /Users/locust/work/scratch/native-target-m4rev/release/locust-native`: **passed** all five scenarios; interrupt/flood cancellation each returned in 0.008 s. Log: `review-cursor-m1-cli.log`.
- `scripts/test-m3a-cli.py` against that binary: **passed** all eight scenarios and permission-bridge checks. Log: `review-cursor-m3a-cli.log`.

No live/ignored runtime tests or window checks were run. The earlier review's deferred coverage and unverified items remain unchanged; this correction makes no additional claim of complete M4 parity.
