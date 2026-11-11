# M3a: Claude Code driver, headless

Headless Claude Code Ask-mode path in this copy. No window, no real Claude turn, no credential reads. Interactive approvals are M3b (declined here with a diagnostic). M3 is not marked reached.

## Changes by file and TypeScript ports

| File | Change and TypeScript reference |
| --- | --- |
| `src/runtime/discovery.rs` | Generalized `find_named` from `path-locator.ts` / M1 `find()`; `claude` and `codex` share PATH rules (absolute entries only, executable regular file). |
| `src/runtime/claude.rs` | Ports `commands.ts:createClaudePrintCommand` Ask argv (`--restricted --print --input-format/--output-format stream-json --verbose --include-partial-messages --permission-mode default --tools Read,Glob,Grep,Task,Skill,WebSearch,WebFetch,mcp__*`, optional `--model`). Ports `process-runner.ts` stream-json user line, stdin left open until `result`, SIGINT then 5s SIGKILL stop (`killProcessTree`). Removes `ANTHROPIC_API_KEY` / `CLAUDE_API_KEY` from the child env. Approval `control_request` → deny `control_response` + diagnostic. |
| `src/runtime/claude_events.rs` | Ports `claude-events.ts:createClaudeEventNormalizer` for M3a: `system/init` allow-list (`summarizeInit`), text append/replace, tool start/complete with targets, thinking as `step.*` reasoning with thinking deltas dropped, `rate_limit_event` → `route.limit_detected` + `claude.usage_window`, `result` / cancel / missing-result finish, permission denials as diagnostics. No TodoWrite → `plan.updated` (TS does not emit it for Claude). |
| `src/runtime/mod.rs` | Registers `claude` + `claude_events`. |
| `src/ledger_write.rs` | `create_runtime` so missions can be `runtime: "claude"` with matching `sourceAdapter` and real argv in metadata (TS reader requires adapter/runtime agreement). |
| `src/run_cli.rs` | `--run claude "<prompt>" [--folder] [--model]` beside existing Codex path. |
| `src/runtime/tests.rs` | Synthetic Claude scenarios: text, tools, thinking, limit, failure, lost, Ctrl-C, approval decline; TS reader check; `#[ignore]` `real_claude_one_line_prompt`. |
| `scripts/fixtures/m3-claude.py` | Fake Claude stream-json from `claude-events.test.ts` shapes; asserts Ask argv when launched as `claude`. |
| `scripts/test-m3a-cli.py` | Release-binary smoke for Claude scenarios (mirrors `test-m1-cli.py`). |

## Verification

```sh
PATH=$HOME/.cargo/bin:$PATH \
LOCUST_TEST_TS_READER=/Users/locust/work/locust/packages/mission-store/dist/index.js \
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-m3 cargo test --release

PATH=$HOME/.cargo/bin:$PATH \
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-m3 cargo build --release

python3 scripts/test-m1-cli.py "$HOME/work/scratch/native-target-m3/release/locust-native"
python3 scripts/test-m3a-cli.py "$HOME/work/scratch/native-target-m3/release/locust-native"
```

- Release tests: **127 passed, 0 failed, 4 ignored** (existing live Electron/Codex ignores + `real_claude_one_line_prompt`).
- Release build passed.
- TypeScript reader accepted Claude synthetic ledgers (text, tools, thinking, limit, failure, lost, interrupt, approval) with zero issues.
- `ledger.rs:read_transcript` recovered assistant text for success/tools/thinking; raw thinking sentinel and init dump names (Gmail, powershell) absent from ledgers.
- `test-m1-cli.py`: all five Codex scenarios still pass.
- `test-m3a-cli.py`: success, failure, lost, interrupt, limit, tools, thinking, approval all matched expected exits and terminal events.

## Unconfirmed

- Real signed-in `claude` turn (`cargo test --release real_claude_one_line_prompt -- --ignored`) was not run.
- Interactive approvals (M3b), resume/fork, skill plugins, permission bridge, connectors, and window wiring are out of scope.
- Windows `.cmd` / PowerShell launch shims for `claude` are not ported; only macOS PATH discovery was exercised.
- Full parity with every `claude-events.ts` branch (background tasks, subagent nesting, oversized stand-ins, command guard) is not claimed; M3a covers the listed synthetic scenarios.
