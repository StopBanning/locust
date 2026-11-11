# M4A — named runtimes on generic ACP

M4A is **not complete**. This copy adds the generic ACP runtime, **OpenCode** (V1 serve + V2 ACP), and **Kilo** (ACP). Cursor, Copilot, Antigravity and Muse still say **“runs in Locust for now”**. M4's daily-driver exit test has not been reached.

All changes and new fixtures are in `/Users/locust/Documents/Locust/native-review/m4a/native`. TypeScript sources under `/Users/locust/work/locust` were read only. No credential files were read or copied, no real turn ran, and no window was opened.

## Per-file port map

| Rust file | TypeScript source:function | Scope |
| --- | --- | --- |
| `src/runtime/acp.rs` | `packages/runtime-adapters/src/acp-run.ts:startAcpRun` (handshake, request answer, converse, stop, steer) | JSON-RPC lines over owned stdio; protocol 1; 60-second startup deadline and 24-hour prompt deadline; session/new and capability-checked session/load; replay suppression until prompt write; foreign-session filtering; model configuration; permission selection by option kind; next-prompt steering with summed usage; cancellation then process-group termination after three seconds. A delivered prompt is never retried. Profile hooks (`set_profile`, `admitted`, `confirm_exit`, `startup`) support named OpenCode/Kilo wrappers. |
| `src/runtime/acp.rs` | `apps/desktop/src/main/acp-agents.ts:launchOf`; `shared/acp-agents.ts:commandWords,parsedAcpAgents,parsedAcpRouteModel` | Read-only `acp-agents.json` in the Electron profile; twelve validated/deduplicated agents; bounded file and command length; `acp_id::model` routes; direct executable/argv, never a shell. Saved model-option IDs are passed via session/set_config_option, not CLI flags. |
| `src/runtime/acp_events.rs` | `packages/runtime-adapters/src/acp-events.ts:createAcpEventNormalizer,acpToolNaming,acpShownTool,acpCallIsShell,textOf,patchOf` | Started/message/plan/reasoning-step/tool/terminal events, message boundaries, tool updates, host-declined status, usage, refusal and length/step-limit diagnostics, unknown-update deduplication. Shell locations never replace the actual command. Raw frames, replay, machine commands and hidden reasoning are not persisted. |
| `src/runtime/acp_events.rs` | `packages/runtime-adapters/src/line-diff.ts:unifiedDiffOf,compare,side`; `codex-events.ts:toolPatchFrom` | Common prefix/suffix, LCS with the TS 1,500-line comparison bound, three context lines, absolute paths, and patch text/add/remove/truncation fields. |
| `src/approval.rs` | `acp-run.ts:acpPermissionRequestOf,answer`; existing native Channel | ACP cards use the existing turn-scoped queue/recording/Always behavior. Always returns allow_once to the agent. No callback means reject_once; missing option means cancelled. EOF, prompt result, Stop and turn release withdraw pending cards. Named runtimes reuse `ask_acp_runtime`. |
| `src/runtime/discovery.rs` | `packages/runtime-adapters/src/path-locator.ts:createPathExecutableLocator` (POSIX PATH portion) | Absolute executable PATH directories retain precedence; command names are validated. Absolute custom program paths run directly. No credential inspection or login probing. |
| `src/runtime/opencode.rs` | `opencode-acp-run.ts:startOpenCodeAcpRun,createOpenCodeAcpCommand`; `discovery.ts:openCodeUsesAcp`; desktop launch branch | Version probe (`opencode --version`); major 1 → authenticated serve; major 2 → ACP with `mode=locust` required config, strict settings, transport retries only before prompt admission, and exit confirmation before replacement; other majors refused. |
| `src/runtime/opencode_config.rs` | `commands.ts:openCodeModeEnv`; `opencode-2-launch.ts:openCode2Launch`; desktop command-guard plugin | Ask/Plan read-only permissions + `OPENCODE_PURE` / project-config disable; Approve-each asks; Auto may leave the folder; question tool always off; run-owned command-guard plugin written through Store's no-follow creator. |
| `src/runtime/opencode_serve.rs` | `opencode-serve-run.ts:startOpenCodeServeRun` | Owned loopback `serve --port 0 --hostname 127.0.0.1`, random Basic auth password, SSE `/event`, session create/prompt/abort, permission cards, SQLITE_BUSY startup restart, stream EOF recovery against `/session/status` and `/message`. |
| `src/runtime/opencode_events.rs` | `opencode-events.ts`; `opencode-error.ts:openCodeErrorFacts,openCodeErrorSentence` | V1 record normalizer (text/tool/plan/retry/error/compaction), free-model wording, rate-limit/auth sentences, external_directory and mode-blocked stderr classification. |
| `src/runtime/opencode_process.rs` | owned child helpers used by serve/version probe | Bounded stdout lines, stderr tail, process-group kill, exit confirmation. |
| `src/runtime/resources/locust-command-guard.mjs` | `apps/desktop/resources/locust-command-guard.mjs` | Copied into each run-owned guard plugin directory. |
| `src/runtime/kilo.rs` | `kilo-acp-run.ts:startKiloAcpRun,createKiloAcpCommand,kiloModelForRun,kiloSessionMode,kiloIsolationEnv,withKiloGitShim` | `kilo acp` over the shared ACP restart/confirm loop; defaults model to `kilo/kilo-auto/free`; sets model then mode (`ask`/`code`); run-owned isolation home; Unix git stdin shim; no Locust permission cards (Auto allows, otherwise reject). |
| `src/runtime/kilo_tests.rs`, `scripts/fixtures/m4-kilo-acp.py` | `kilo-is-discovered-and-starts-on-auto-free.test.ts` | Model/mode defaults; Auto Free + Approve-each raises no card; Auto allows; model-before-mode order; native teammate dispatch; ignored real one-line test. |
| `src/runtime/mod.rs` | Runtime registration | Registers ACP, OpenCode and Kilo modules. |
| `src/native.rs` | `main/codex-mission.ts` OpenCode/Kilo + generic ACP dispatch | OpenCode and Kilo teammates are native-supported; continuation resumes stored session ids; OpenCode Approve-each cards raise; Kilo never raises host cards. |
| `src/run_cli.rs` | Headless native dispatch | Adds `--run opencode` and `--run kilo`. |
| `src/ledger_write.rs` | Runtime whitelist | Allows `opencode` and `kilo` metadata/sourceAdapter and route IDs. |
| `src/main.rs` | Approval card runtime label | OpenCode and Kilo card labels. |
| `src/runtime/acp_tests.rs`, `scripts/fixtures/m4-acp.py` | ACP synthetic suite | Unchanged from the ACP slice. |
| `src/runtime/opencode_tests.rs`, `scripts/fixtures/m4-opencode-acp.py`, `scripts/fixtures/m4-opencode-serve.py` | `opencode-acp-profile.test.ts`, `opencode-serve-run.test.ts`, `opencode-error.test.ts`, `opencode-events` / mode-env coverage | Version split; mode/config/question-off; error sentences; V1 event stream; ACP success with mode/model/effort; transport retry before prompt; serve SSE success; SQLITE_BUSY restart; native teammate dispatch with approval + resume id; ignored real one-line test. |

## Deliberate differences and remaining scope

PLAN.md forbids retaining hidden reasoning. ACP's TypeScript normalizer retains thought text; this port follows PLAN.md instead, preserving only step lifecycle. Evidence contains no raw protocol objects. Text uses the existing native secret scrubber and UTF-16 bounds.

Generic agents can be selected from the profile's existing saved configuration. Native settings to add/test/save/remove agents, capability display, browser/MCP server forwarding, attachments, and UI steering are not implemented here. The driver has queued steering, but the current Runtime trait/window does not expose it. There are no native model-catalog probes for custom agents.

OpenCode native coverage matches the Electron execution split: V2 ACP with Locust-owned config/guard and V1 authenticated serve. Kilo reuses that ACP restart loop with its own command, isolation home, Auto Free default, session mode mapping and no host permission cards. Not ported in this slice: slash-command listing UX, git-reads MCP, browser MCP, skills folders, worktree/reference-folder permission widening, sealed runs, cloud OpenCode, provider injection, Windows launch shims, or Windows `git.cmd` shim. Cursor, Copilot, Antigravity and Muse remain deferred.

## Verification

Commands run from this folder, using `PATH=$HOME/.cargo/bin:$PATH` and `CARGO_TARGET_DIR=$HOME/work/scratch/native-target-m4a`:

- `LOCUST_TEST_TS_READER=/Users/locust/work/locust/packages/mission-store/dist/index.js cargo test --release`: **181 passed, 0 failed, 9 ignored**, with TypeScript reader checks enabled (OpenCode and Kilo ledgers accepted).
- OpenCode synthetic coverage: 9 tests passed, `real_opencode_one_line` ignored.
- Kilo synthetic coverage: 5 tests passed, `real_kilo_one_line` ignored.
- `cargo build --release`: passed.

`real_acp_one_line`, `real_opencode_one_line` and `real_kilo_one_line` are `#[ignore]`. None were run. No window, real CLI conversation, live OpenCode/Kilo install, or login command was used. Windows launch shims, Windows process-tree termination, real custom-agent compatibility, and live 60-second / 24-hour deadlines were not tested.

Current checklist totals, counted from the existing feature cells: **14 done of 153**, 13 partial, 126 missing. The composite named-runtime row stays partial: OpenCode and Kilo run natively; Cursor/Copilot/Antigravity/Muse do not. No whole feature row or milestone is claimed complete by this slice.
