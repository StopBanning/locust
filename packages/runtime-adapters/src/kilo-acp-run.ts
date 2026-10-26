import { createAcpAgentCommand } from "./commands.js";
import { startOpenCodeAcpRun } from "./opencode-acp-run.js";
import type { OpenCodeAcpRun, OpenCodeAcpRunOptions } from "./opencode-acp-run.js";
import type { ExecutableLaunch, MissionSandbox, RuntimeCommandSpec } from "./types.js";

/**
 * The model a new Kilo run starts on.
 *
 * Measured 2026-10-25 (`kilo acp` 7.8.17): a fresh session's own model is
 * `kilo/google/gemini-3-pro-image`, a paid image model. The free row Kilo
 * advertises is this id, displayed "Kilo Gateway/Auto Free".
 * https://kilo.ai/docs/getting-started/using-kilo-for-free names `kilo-auto/free`.
 */
export const KILO_AUTO_FREE_MODEL = "kilo/kilo-auto/free";

/** `account-default` would leave Kilo on the session's own model. Never do that. */
export function kiloModelForRun(model: string | undefined): string {
  if (model === undefined || model === "account-default" || model.trim().length === 0) return KILO_AUTO_FREE_MODEL;
  return model;
}

/**
 * Locust's mode, as Kilo's session `mode` config option.
 *
 * Kilo advertises `code`, `ask`, `debug`, `orchestrator` and `plan` on the
 * session (measured 2026-10-25). It does not use OpenCode 2's custom `locust`
 * agent. Ask is Kilo's `ask` agent. Plan is that same agent: Kilo's own `plan`
 * mode "can only edit plan files", and Locust's Plan changes nothing.
 * Edit and Auto are Kilo's `code` mode. Auto is the one that allows an action
 * `code` still asks about; Edit refuses it.
 *
 * ASK AND MCP. OpenCode 0.821 puts `"*": "ask"` first so a tool the rules do
 * not name, including a person's MCP tool, is not allowed. Kilo does not
 * honour that setting as OpenCode does. Its ask agent starts from `"*": "deny"`
 * and then allows the tools that only read; each MCP server named in Kilo's
 * config is set to `"ask"` (`getMcpRules` / `askGuard` in
 * packages/opencode/src/kilocode/agent/index.ts at 9723be62). A global
 * `"*": "ask"` is not what keeps an unnamed tool from running. The catch-all
 * deny does. `agent.ask.permission` in a config file merges after that guard
 * and can widen it, so a run does not load the person's Kilo config or a
 * project OpenCode config (see `kiloIsolationEnv`).
 */
export function kiloSessionMode(mode: "ask" | "plan" | "accept-edits" | "approve-each" | "auto"): "ask" | "code" {
  if (mode === "ask" || mode === "plan") return "ask";
  return "code";
}

/**
 * A directory Kilo may use instead of the person's own config and data.
 *
 * `KILO_CONFIG_DIR` and `XDG_CONFIG_HOME` / `XDG_DATA_HOME` are Kilo's own
 * names (packages/core/src/flag/flag.ts at 9723be62). This does not read or
 * write `~/.config/kilo` or `~/.config/opencode`. `KILO_DISABLE_PROJECT_CONFIG`
 * keeps a folder's `opencode.json` from widening Ask. The other switches are
 * the ones a catalog probe used so a run does not update itself, prune, or
 * load Claude Code and external skills.
 */
export function kiloIsolationEnv(directory: string): Readonly<Record<string, string>> {
  return {
    KILO_CONFIG_DIR: directory,
    XDG_CONFIG_HOME: directory,
    XDG_DATA_HOME: directory,
    KILO_DISABLE_PROJECT_CONFIG: "1",
    KILO_DISABLE_CLAUDE_CODE: "1",
    KILO_DISABLE_EXTERNAL_SKILLS: "1",
    KILO_DISABLE_AUTOUPDATE: "1",
    KILO_DISABLE_PRUNE: "1",
    KILO_TELEMETRY_LEVEL: "off",
  };
}

/** `kilo acp`. The handshake is OpenCode's ACP runner; the command and config are not. */
export function createKiloAcpCommand(
  executable: ExecutableLaunch,
  workspacePath: string,
  launch?: { readonly env?: Readonly<Record<string, string>>; readonly sandbox?: MissionSandbox },
): RuntimeCommandSpec {
  const base = createAcpAgentCommand(executable, ["acp"], workspacePath);
  return {
    ...base,
    runtime: "kilo",
    ...(launch?.sandbox === undefined ? {} : { sandbox: launch.sandbox }),
    ...(launch?.env === undefined ? {} : { env: { ...base.env, ...launch.env } }),
  };
}

export function startKiloAcpRun(options: OpenCodeAcpRunOptions): OpenCodeAcpRun {
  return startOpenCodeAcpRun({
    ...options,
    label: "Kilo",
    command: createKiloAcpCommand(options.executable, options.workspacePath, options.launch),
  });
}
