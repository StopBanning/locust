# B3, first part: Codex and Claude Code build what Electron builds, and the guard runs inside the agents (11 Nov 2026)

The command grid from B0 found every Codex and every Claude Code case different from Electron. This round closes the
differences that come from the engine itself, and leaves listed the ones that need a feature the engine does not have.

## What was built

- **Codex's plan tool.** `runtime/version.rs` reads a CLI's version the way `version.ts` does (`parseRuntimeVersion`,
  with its look-ahead checked by hand, since the regex crate has none) and compares as `commands.ts` does.
  `codex::app_server_args` adds `-c tools.update_plan.enabled=true` from 0.153.0, as `codexPlanToolArguments` does.
  A failed version probe starts Codex without it, as Electron's discovery leaves `cliVersion` out. OpenCode's version
  probe moved into the same file. A `--run codex` ledger now records the arguments it really launched with.
- **Claude Code's command line.** `claude::print_args` ports `createClaudePrintCommand` flag for flag: the mode picks
  the tools and the permission mode (`default`, `acceptEdits`, `bypassPermissions`), Auto drops `--restricted`, one
  `--mcp-config` names the permission bridge and the browser together, connector allow rules (`tool_prefix_for` and
  `allow_rule_for` port `connectors.ts`), the guard's `--settings`, `--strict-mcp-config`, skill and reference
  folders with their deny rules, model, effort, resume and fork.
- **What a turn passes.** `native::claude_args` is the composition `send` uses and the grid test checks: the bridge
  in every mode but Auto (it was Approve each only), the browser, the guard, effort, and `--resume` when the previous
  turn recorded a Claude session. Account connectors stay out (`--strict-mcp-config`), since none can be ticked here.
- **The command guard as a hook.** `locust-native --command-guard` answers one PreToolUse event on stdin with a deny or
  nothing, and always exits 0, as `locust-command-guard.mjs` does. `command_guard::claude_settings` writes the
  `--settings` file (`commandGuardSettings`) into the data folder's `guard/`. Every Claude Code turn gets it, Auto
  included.
- **A turn that ended cleanly no longer logs a transport failure.** Claude Code closes stdout as it exits. When that
  came before the exit was seen, the run recorded "Claude stdout closed before the turn completed" beside a
  completed turn. After the result it now waits up to 2 s for the exit and finishes quietly.

## Counts

- Command grid: 227 of 385 match, up from 146: Codex added 55 and Claude Code 26.
- Codex: 55 of 55. Claude Code: 26 of 55. The 29 left per mode are skills, reference folders, forks, ticked
  connectors, and `no-guard`, a case Electron's host never builds.
- Tests: 419 in the engine (15 ignored live tests), 180 in the window, all passing.

## TypeScript tests carried

- `runtime-adapters.test.ts` "parses runtime versions": its four strings, plus edge cases checked against
  `version.ts` with node (`1.2.3.4` reads as `2.3.4`, `1.2.3-rc.1.` keeps its pre-release).
- `connectors.test.ts`: the prefix rule for a `claude.ai` name, a hyphenated local server, and a name with no letter.
- `commands.test.ts`'s Claude cases are covered by the grid, which runs the TypeScript builder itself.

## How it was checked

- The full suite, and the grid against Electron 5b8ee3cf.
- `locust-native --command-guard` run by hand: a deny for `pkill -x Safari`, nothing for `ls`.
- One Haiku turn on Claude Code 2.1.322 with the settings file, in a throwaway git folder named like a Scout fix copy:
  the hook refused `git push origin main` and the model reported the refusal. A push was chosen because it is harmless
  if the guard failed; a test that asks to end a browser would end one.
- Live, with the signed-in CLIs, isolated data folders and not Electron's door: `real_claude_approval_deny_then_allow`,
  `real_codex_approval_deny_then_allow` (Codex 0.167.1, so with the plan tool switch), and the new
  `real_claude_continue`, whose second turn passed `--resume` and still knew the word from the first.
- `--permission-mode default` was run once by hand: Claude Code 2.1.322 lists `manual` among its choices, not
  `default`, but still accepts `default`, which Electron sends.

## Second part, the same day: the guard inside Codex and Copilot

- **Codex.** `command_guard::codex_config` is `codexCommandGuardConfig`: `bypass_hook_trust` and the hook, laid over
  the thread's config at `thread/start` and `thread/resume` (`Codex::with_thread_config`). `other_codex_hooks` ports
  `otherCodexHooks` (the person's `.toml` files, `hooks.json`, installed plugins except Codex's bundled ones, and any
  `.codex/` above the folder); if it finds one, or a place it cannot read, the run goes without the guard, since the
  bypass would skip the review of their hook too. This Mac has none, so its Codex runs get the guard.
- **Copilot.** `write_copilot_plugin` writes `writeCopilotCommandGuard`'s folder (a `.claude-plugin/plugin.json` and
  `hooks/hooks.json`), and every Copilot turn gets `--plugin-dir` for it before the browser's config. The hook line
  ends `|| true`, because Copilot refuses a command whose hook fails.
- **Grid:** 264 of 385 (Copilot added 37). Left per agent: Claude Code 29, OpenCode 25, Cursor 22, Antigravity 21,
  Copilot 13, Muse 11.
- **Live:** `real_claude_guard_refuses_a_push_from_a_fix_copy`, `real_codex_guard_refuses_a_push_from_a_fix_copy` and
  `real_copilot_guard_refuses_a_push_from_a_fix_copy` (Copilot CLI 1.0.109). Each runs the agent in Auto, where
  nothing asks first, in a git folder named like a Scout fix copy, and asks for `git push origin main`; each ledger
  holds the guard's refusal and a completed turn.
- Tests: 421 in the engine (18 ignored live tests), 180 in the window, all passing.

## Third part: Codex's modes

- `approval::codex_app_server_policy` now starts from the sandbox each mode earns (`Mode::sandbox`), as
  `codexAppServerPolicy` does over the host's mapping. Accept edits had been read-only on the native engine, so a Codex
  teammate in Accept edits could not edit; it is `workspace-write` now.
- The person's MCP programs are switched off in Ask and Plan only (`codex-read-only.ts`), and `config/read` is asked
  only then. Before, Rust switched them off in every mode, so in Auto their Codex lost its own servers.
- The fake app-server (`scripts/fixtures/m1-app-server.py`) had asserted the old rule; it now checks both sides.
- Live: `real_codex_accept_edits_writes_in_its_folder` (new) wrote hello.txt; `real_codex_continue` still passes in Ask.
- Tests: 422 in the engine, 180 in the window.

## Fourth part: the stream readers (B0's second check)

The recorded-stream check went from 0 of 37 to 32 of 37 streams read exactly as Electron reads them: every Cursor,
Copilot, Muse, Antigravity (agy) and OpenCode stream. What changed:

- **Evidence keeps the record.** `events::evidence_raw` ports codex-events.ts `evidenceFor` and `sanitizeJson` (secret
  keys and secrets in text redacted, text capped at 8,192 characters, depth, width and key limits, a reasoning item
  kept to its id and status) and `malformed_raw` its `malformedEvidence`. The Cursor, Copilot, Muse, Antigravity and
  OpenCode readers keep the line being read, sanitized, and attach it to each event as Electron does; Copilot first
  drops its opaque blobs (`scrubCopilotRecord`) and marks its evidence redacted. Diagnostics carry the record and its
  type. This is in `accept`, the entry live runs use, not only in the test-only `accept_line`: a warning that the
  scrubber was unused is what showed the first version only reached the tests.
- **Counts as Electron writes them.** Whole token counts and durations stay whole (`events::num`), and a run's
  `recordCount` counts the records the process gave, not the events made from them.
- **`terminationUnconfirmed`** is false unless stopping the process could not be confirmed, as Electron's runner
  reports it; a failed run is not that.
- **Antigravity** makes no `run.started` of its own (agy-events.ts makes none; the thread rides on each event and on the
  run's end), and its failure carries the result record that caused it.
- **OpenCode** names its evidence by OpenCode's own record type (`step_start`), and writes a new file's patch as
  `newFilePatch` does, `b/` before the path.
- **The check itself**: the goldens script now hands each reader the whole completion a process runner gives (it left
  out `inputDeliveryFailed` and four more), and `finishedAt` is ignored with the other clock fields.

Left after this part: three Claude Code streams and two Antigravity transcripts. Both closed in the fifth part.

## Fifth part: the last five streams (37 of 37)

- **Claude Code's `system` records.** `claude_events.rs` now reads them as claude-events.ts does: `task_started` and
  `task_progress` open a helper's row (`subagent:<task>`) or a background row on the call that started it;
  `task_notification` closes it (`background_ending`: completed, failed, stopped, or stopped-with-run when the stop
  came after the run's own result); `permission_denied` marks the call refused for its result that follows;
  `compact_boundary` says the conversation was summarized; any other subtype is the turn moving on, with its status.
- **A tool call's end carries what Electron's does**: refused (with the reason, or the command guard's own words),
  the helper's type and its summary, `background`, the file patch from `tool_use_result` (`claude_file_patch`: its
  hunks written out, or a new or rewritten file compared with what it was), and printed output for the tools Claude
  Code shows it under. The call's start is restated once it can be named, now with `background` and the helper type.
- **A helper's own calls** open under the Agent call that sent it (`parentItemId`), as in Electron's helper
  visibility. Before, Rust dropped every record from a helper.
- **Evidence and counts.** Each Claude record is kept as evidence, sanitized, with thinking cut to its type
  (`without_thinking_evidence`). Usage counts stay whole. `contextTokens` is the last model call's prompt and output
  (`last_call`, from `message_start`, `message_delta` and the complete message). `billing: subscription` is set when
  the run was on the usage windows and did not spill into extra usage. `recordCount` counts records.
- **Antigravity's transcript reader** is a full port of `createAntigravityEventNormalizer`, replacing a sketch that
  only found final answers: tool calls and their results matched by step index (a planner line at step p with n calls
  is answered at p+1..p+n), results written before their call held for it, background tasks and timers settled by
  Antigravity's notices, `ask_question` with its options, new-file patches from `write_to_file`, decoded arguments,
  and the run's end (completed, cancelled, or failed when the transcript never ended on an answer). Electron's
  Antigravity runtime reads this; the native engine still drives the agy CLI, so the reader waits for that runtime.
- Tests: 424 in the engine (19 ignored live tests), 190 in the window, all passing; three new transcript tests.

## Not checked, not done

- No turn ran in the window itself; the live tests drive the same `send` the window calls.
- Whether Codex's plan panel fills on the native engine: the switch is sent, but no live run looked for
  `turn/plan/updated`.
- The hook answers in about 20 ms on this Mac once the binary has run once; the first run after a build took 0.8 s.
  Claude Code allows 15 s.
- The hook does not read the process table, so by-id kills of processes the run did not start are let through.
- Skills, reference folders, forks and connectors for Claude Code.
- OpenCode still loads Electron's JavaScript guard, inside OpenCode's own runtime (no Node needed), so its words can
  drift from the Rust guard's.
- The Codex sandbox and thread config travel over the protocol, so the command grid does not see them; only the
  unit tests and the fake app-server hold them to Electron.
- Windows: the bridge and browser still use Unix sockets.

## Sixth part (12 Nov): the grid at 308 of 385

- **Modes Electron's app never offers.** `scripts/command-grid.mjs` now bundles renderer/src/status.ts and records,
  per case, whether Electron's app offers that mode for that agent on a Mac (`modeRunsOn`). Approve each is offered
  only for Claude Code, Codex, OpenCode and Copilot ("Claude Code, Codex CLI, OpenCode and Copilot CLI only"), so the
  33 Approve-each cases for Cursor, Muse and Antigravity are ones no person can reach; Electron's builder still builds
  them. The grid counts them as matching when Rust refuses them, and lists them if Rust ever builds one.
- **OpenCode in Approve each** asks for `edit` alone, as commands.ts writes it; Rust also asked for `write` and
  `patch`.
- **Cursor's refusals** (Approve each, an effort level) are one function the driver and the grid both use; the grid
  had built a Cursor command with an effort level the driver refuses at start.
- Muse matches in every case. Left: Claude Code 29, OpenCode 21, Copilot 13, Cursor 8, Antigravity 6: skills,
  reference folders, forks, ticked connectors, Copilot's browser in Approve each, and `no-guard` (Electron runs
  without the guard only when writing it failed; Rust refuses to run instead, and that is kept).

## Seventh part (12 Nov): reference folders in every agent's command (grid 318 of 385)

- `core/src/references.rs` ports shared/references.ts `referenceAccess` (a reference never widens a mode: where adding
  the folder would also make it writable it is left out, with the chip's words) and workroom-briefing.ts
  `referencesSection`, the line every turn's prompt now ends with, as Electron's does ("none attached for this turn"
  when there are none).
- The builders take the folders as commands.ts gives them: Claude Code `--add-dir` with `Edit(...)` denials outside
  Ask (it was already in `print_args`; the turn now passes it), OpenCode 1's `external_directory` allow entries in a
  read-only run, Copilot's and Antigravity's `--add-dir` in a read-only run. Codex, Cursor and Muse are told in the
  prompt only.
- A turn takes its folders from `Origin.references`. Nothing fills it yet: the conversation's reference store and
  the composer's chips are the next step.
- The fake app-server now accepts the references line after the person's words.
- Left in the grid: Claude Code 25, OpenCode 15, Copilot 11, Cursor 8, Antigravity 8: skills, forks, ticked connectors,
  Copilot's browser in Approve each, and `no-guard`.

## Eighth part (12 Nov): references from the composer

- **The store** (`references.rs`, from conversation-references.ts): `conversation-references.json` in the native data
  folder, in Electron's shape, seeded from Electron's once. Folders on this computer are added (`add_folder`, the
  real path named by its last part), a conversation keeps up to five (`set`, and `bind` after each turn, with the
  folder the turn stood in), a later turn finds them through the conversation's first turn (`rootOf`), and a
  reference no conversation uses is dropped. GitHub repositories are not added from here yet; an Electron
  repository copy that was seeded is read but never moved.
- **A turn** uses the references the composer names (or, from the door or a carry-on, the conversation's own),
  filtered by `referenceAccess`, and binds them once it starts.
- **The composer**: the folder chip opens a menu with "Add a reference…"; the picker (Electron's words) adds a "Folder
  on this computer" through the native dialog (Rune's `open_folder`, which needed its `native-file-dialog` feature);
  each reference is a chip above the box with its folder mark, name, access note and remove button, faded where this
  mode leaves it out. Scenes `references` and `references-light`.
- Not checked: the dialog itself (a native window no snapshot can show); a live turn reading a reference.
