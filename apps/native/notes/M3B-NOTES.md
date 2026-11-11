# M3b: Approvals

Interactive approvals for Claude Code and Codex in this copy. Saved approval rules are out of scope. M3’s real window exit test (Electron quit) is still unconfirmed.

## Changes by file and TypeScript ports

| File | Change and TypeScript reference |
| --- | --- |
| `src/approval.rs` | Ports `approval-channel.ts` (`approvalKindFor`, `describeApproval`, `protocolDecisionFor` / `protocolAnswerFor`, channel raise/decide/release, answer-after-end refuse). Ports `permission-host.ts:builtInOrConnector` for Claude `can_use_tool`. Ports `commands.ts:codexAppServerPolicy`. Status words: `Needs you`. |
| `src/runtime/mod.rs` | `Turn` carries `mode`, optional `approvals` channel, `mission_id` / `run_id`. |
| `src/runtime/codex.rs` | Approve-each uses `untrusted` + `workspace-write`; Ask keeps `never` + `read-only`. Approval requests raise a card only in Approve-each; otherwise auto-decline (M1 behaviour). |
| `src/runtime/claude.rs` | `control_request` / `can_use_tool` raises a card in Approve-each; otherwise deny + diagnostic. |
| `src/runtime/claude_events.rs` | `control_request` no longer pretends M3a declined in the normaliser (the channel answers on the wire). |
| `src/ledger_write.rs` | `append_approval` writes `mission.approval` (mission-store v20+). Separate `ledger_seq` so approvals do not skip `event.sequence`. |
| `src/native.rs` | Codex **or** Claude teammates without a private account; mode from `teammates.json`; process-wide approval channel; `waiting_approvals` / `decide_approval`; turn loop flushes `mission.approval` and releases on end. |
| `src/main.rs` | Approval card under the turn (Approve once / Always allow this session / Deny); keys 1/2/3; status `Needs you`; snap scenes `approval` / `approval-light`. |
| `src/runtime/tests.rs` | Synthetic interactive allow/deny/after-end (Claude) and allow (Codex); TS reader; `#[ignore]` real approve-each stubs. |
| `scripts/fixtures/m1-app-server.py` | `approve-allow` / `approve-deny` under Approve-each policy. |
| `scripts/fixtures/m3-claude.py` | `approval-allow` / `approval-deny` control_response behaviours. |

## Verification

```sh
PATH=$HOME/.cargo/bin:$PATH \
LOCUST_TEST_TS_READER=/Users/locust/work/locust/packages/mission-store/dist/index.js \
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-m3 cargo test --release

PATH=$HOME/.cargo/bin:$PATH \
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-m3 cargo build --release

python3 scripts/test-m1-cli.py "$HOME/work/scratch/native-target-m3/release/locust-native"
python3 scripts/test-m3a-cli.py "$HOME/work/scratch/native-target-m3/release/locust-native"

# Snaps
locust-native --snap /Users/locust/work/scratch/m3-snaps/approval-dark.png --scene approval --size 900x900 --scale 2
locust-native --snap /Users/locust/work/scratch/m3-snaps/approval-light.png --scene approval-light --size 900x900 --scale 2
```

- Release tests: **133 passed, 0 failed, 6 ignored** (prior live ignores + `real_claude_one_line_prompt` + two real approve-each stubs).
- Release build passed.
- TypeScript reader accepted interactive Claude/Codex ledgers that include `mission.approval`.
- `test-m1-cli.py`: five Codex scenarios pass.
- `test-m3a-cli.py`: Claude scenarios pass after rebuild (approval auto-deny diagnostic).
- Snaps written: `/Users/locust/work/scratch/m3-snaps/approval-dark.png`, `approval-light.png`.

## Unconfirmed

- Real window exit test with Electron quit (PLAN.md M3): Claude and Codex each ask in the window; allow and deny both work.
- `#[ignore]` `real_claude_approval_deny_then_allow` / `real_codex_approval_deny_then_allow` (create a file in a temp folder).
- Claude permission-prompt-tool bridge (`permission-host.ts`), question cards, denial reasons UI, and saved approval rules.
- Full mode matrix beyond Ask vs Approve-each (Accept edits / Plan / Auto guarantees).

## Review and fixes

Reviewed the M3 implementation against Electron's `permission-host.ts`, `ApprovalCard.tsx`, `approval-channel.ts`, `index.ts:recordAnswer`, mission-store's approval schema/bounds, and runtime-adapters' `commands.ts` and `claude-commands.ts`.

### Corrected

- **Claude permissions were not wired to the route Electron uses.** Approve-each inherited Ask's read-only tool list and never supplied a permission-prompt tool. Native now exposes Electron's editing tool list in Approve-each and supplies `--mcp-config` plus `--permission-prompt-tool mcp__locust__approve`. The native binary serves the stdio MCP `initialize`, `tools/list`, and `tools/call` protocol. A per-turn private Unix socket replaces Electron's token-authenticated HTTP hop: its containing directory is created with mode 0700, the bridge fails closed if the host is unavailable, and its lifetime follows the turn. No Node or Electron process is needed. An ended/cancelled/disconnected/timed-out request cannot acquire a later grant.
- **The installed Claude CLI no longer advertises `default` as a permission mode.** `claude --help` lists `manual` instead, so native uses `manual`. An initialize-only probe of the installed CLI accepted the revised restricted print/stream-json arguments, editing tool list, native MCP configuration, and permission-prompt-tool flag, returning `control_response/success`. This was a protocol handshake with no user message/model turn; it does not prove a live model's permission handling. The direct stream-json control-request handler remains a fail-closed compatibility path, exercised by the existing synthetic fixtures; it is not claimed equivalent to Electron's bridge.
- **Approval records could cross missions.** The process-wide channel previously let any turn drain every answer. Native now drains by mission ID. Records retain Electron's schema (`approvalId`, `kind`, `asked`, `answer`, `by`, optional `words`, `askedAt`, `occurredAt`); the internal mission ID is never serialized into the approval object. Asked text is deduplicated, bounded in UTF-16 units like TypeScript, and given Electron's empty-text fallback. Ledger sequence remains separate from runtime event sequence.
- **Lifecycle safety.** Registering a card and testing whether its mission ended now occur under one lock. Transport readers withdraw cards when the provider reports completion or disconnects, including while the consumer is waiting for a decision. Codex completion is checked against the current thread so a foreign thread cannot close this turn; foreign-thread approval requests are rejected. Failed deliveries do not leave session grants behind. There is no retry of a denied action in the native driver.
- **Claude Always had no effect.** Explicit session grants now apply only to the chosen tool and mission, produce `earlier-always` records on subsequent uses, and are removed at turn end. Process-control commands cannot acquire or consume a broad session grant and are asked about every time. This check is deliberately conservative (it can also match quoted process-control names); it is not a full port of Electron's shell/reach parser.
- **Card words and placement.** The card is now a child of the padded message column, uses `Codex CLI`/`Claude Code`, Electron's title/action/effects/data/reversibility/note wording, and `Deny…` opening the optional-reason controls (`Deny`, `Deny and say why`, `Back`). Enter submits the reason and Escape goes back. Removed the invented numeric approval shortcuts and their hint; Electron's authorization card has no such hints. Fresh cards ignore approval for 500 ms. Only the selected mission's card is shown. The approval preview is consistently Codex. The simple `touch hello.txt` preview also has Electron's “In short” sentence; full command summaries, question forms, saved rules, patches, and the complete command-reach parser remain outside this change.
- **Path descriptions.** Sibling-prefix paths and `..` escapes are described as outside the project, using Electron's reversibility warning. Web fetch and connector descriptions use the reference route's wording.

### Real tests, implemented but NOT run

`native::tests::real_claude_approval_deny_then_allow` and `native::tests::real_codex_approval_deny_then_allow` replace the two empty ignored functions. Each creates an isolated project and fake Electron/native stores, calls `native::send` in Approve-each, waits through `waiting_approvals`, denies the first request with `decide_approval`, and checks that no file exists and a denial is recorded. A second independent turn approves once, checks the exact file bytes, the matching approval ID and allowed record, and `run.completed`. Extra approval requests fail rather than being automatically approved. The tests check that Electron's fake folder is unchanged and late decisions are refused; cleanup stops a still-running turn. Claude's live test uses the adjacent release `locust-native` binary for its MCP child, so run `cargo build --release` before it.

The same two-turn native API/file/ledger assertions run non-ignored against synthetic Codex and Claude providers (`scripts/fixtures/native-approvals.py`). Those tests additionally validate both missions with the configured TypeScript reader. Bridge-specific tests cover private permissions, allow/deny wire responses, and refusal after end; channel tests cover record isolation, session scope, ledger bounds, path boundaries, and card wording; runtime/UI tests cover a result arriving during an unanswered card and the fresh-card guard.

### Verification after review

Using the requested PATH, TypeScript reader, and `CARGO_TARGET_DIR=$HOME/work/scratch/native-target-m3`:

- `cargo test --release`: **144 passed, 0 failed, 6 ignored**. The six live tests, including both implemented approval tests, remained ignored. The TypeScript reader accepted the synthetic allow/deny mission ledgers with zero issues.
- `cargo build --release`: passed. Compiler warnings remain for unused API/record fields; there were no compiler errors in the final build.
- `python3 scripts/test-m1-cli.py <release binary>`: all five scenarios passed. The flood smoke permits either failed or cancelled termination; on the final run it reported `run.failed`, with cancellation observed in 0.006 s.
- `python3 scripts/test-m3a-cli.py <release binary>`: all eight scenarios passed, plus the added actual-binary MCP initialize/list/call smoke (unreachable host returned deny).
- Re-saved `/Users/locust/work/scratch/m3-snaps/approval-dark.png` and `approval-light.png` at 900×900, scale 2. Visually inspected both: the card aligns with the message column on both sides, all controls and the note fit, and the Codex teammate/request labels agree.
- One intermediate full-suite run failed in the existing `native::tests::a_continuation_follows_the_newest_turn_not_the_anchor` with “The conversation's ledger could not be found.” The final full-suite rerun passed. That intermittent failure was not diagnosed or represented as fixed.

**Still unverified:** real Claude/Codex deny-then-allow turns and the real window M3 exit test with Electron fully quit. No ignored tests were run. M3 is not marked reached. The initialize-only CLI probe proves flag acceptance, not live permission semantics. Advanced card branches and a complete shell parser are not claimed at parity.
