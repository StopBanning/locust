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

## Not checked, not done

- No turn ran in the window itself; the live tests drive the same `send` the window calls.
- Whether Codex's plan panel fills on the native engine: the switch is sent, but no live run looked for
  `turn/plan/updated`.
- The hook takes about 0.8 s from start to answer on this Mac, mostly the binary starting. Claude Code allows 15 s.
- The hook does not read the process table, so by-id kills of processes the run did not start are let through.
- Skills, reference folders, forks and connectors for Claude Code.
- OpenCode still loads the JavaScript guard plugin, which needs Node.
- Rust gives Codex `read_only_config` (the person's MCP programs off) in every mode; Electron does it in Ask and Plan
  only. Not changed here; it needs its own grid row, since it travels over the protocol, not the command line.
- Windows: the bridge and browser still use Unix sockets.
