import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

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
  const command = createKiloAcpCommand(options.executable, options.workspacePath, options.launch);
  return startOpenCodeAcpRun({
    ...options,
    label: "Kilo",
    command: { ...command, env: withKiloGitShim(command.env) },
  });
}

export interface KiloGitShimDeps {
  readonly platform?: NodeJS.Platform;
  readonly pathEnv?: string;
  readonly pathKey?: string;
  readonly resolveGit?: (pathEnv: string | undefined, platform: NodeJS.Platform) => string | undefined;
  readonly directory?: string;
  readonly mkdirSync?: (path: string) => void;
  readonly writeFileSync?: (path: string, contents: string) => void;
}

/**
 * The first `git` on this Kilo child's PATH, and nowhere else.
 *
 * GIT INHERITS ACP STDIN ON WINDOWS (2026-10-26). Kilo 7.8.17 runs
 * `git config --get remote.origin.url` through Bun's shell, and that child
 * inherits the ACP pipe. On Windows it blocks until another line is written.
 * Measured the same day on 7.8.17: one process's five model requests spawned
 * that lookup once, and a second prompt in that process did not spawn it
 * again. A JSON-RPC line written while git was already blocked still reached
 * Kilo, and released git about 13ms later. A blank line sent before git is
 * blocked is only an empty protocol line, so a timer cannot close the race.
 * Bun's shell resolves a `git.cmd` placed first on the child PATH (measured).
 * That file runs the real git, found when the run starts, with stdin from NUL,
 * so the protocol pipe is never its input. OpenCode does not get this directory.
 */
export function withKiloGitShim(
  env: Readonly<Record<string, string>> | undefined,
  deps: KiloGitShimDeps = {},
): Readonly<Record<string, string>> {
  const base = { ...(env ?? {}) };
  try {
    const platform = deps.platform ?? process.platform;
    const pathKey = deps.pathKey ?? Object.keys(process.env).find((key) => key.toLowerCase() === "path") ?? "PATH";
    const pathEnv = deps.pathEnv ?? process.env[pathKey];
    const gitPath = (deps.resolveGit ?? resolveGitOnPath)(pathEnv, platform);
    if (gitPath === undefined || !gitPathIsSafe(gitPath, platform)) return base;
    const directory = deps.directory ?? join(tmpdir(), "locust-kilo-git");
    const prepare = deps.mkdirSync ?? ((path: string) => { mkdirSync(path, { recursive: true }); });
    const write = deps.writeFileSync ?? ((path: string, contents: string) => { writeFileSync(path, contents, "utf8"); });
    prepare(directory);
    const name = platform === "win32" ? "git.cmd" : "git";
    write(join(directory, name), gitShimScript(gitPath, platform));
    const separator = platform === "win32" ? ";" : ":";
    const current = base[pathKey] ?? pathEnv ?? "";
    return { ...base, [pathKey]: current.length > 0 ? `${directory}${separator}${current}` : directory };
  } catch {
    return base;
  }
}

/** `git.exe` anywhere on PATH wins over a later `git.cmd`, so the shim cannot call itself. */
export function resolveGitOnPath(pathEnv: string | undefined, platform: NodeJS.Platform): string | undefined {
  const dirs = (pathEnv ?? "").split(platform === "win32" ? ";" : ":").filter((dir) => dir.length > 0);
  const names = platform === "win32" ? ["git.exe", "git.cmd"] : ["git"];
  for (const name of names) {
    for (const dir of dirs) {
      const candidate = join(dir, name);
      try {
        if (existsSync(candidate)) return candidate;
      } catch {
        // A PATH entry that cannot be stated is not a git we can point at.
      }
    }
  }
  return undefined;
}

function gitPathIsSafe(gitPath: string, platform: NodeJS.Platform): boolean {
  if (gitPath.length === 0 || gitPath.length > 260) return false;
  if (platform === "win32") return /^[A-Za-z]:\\[^"\r\n%&|<>^]*git\.(?:exe|cmd)$/i.test(gitPath);
  return gitPath.startsWith("/") && !/[\0\r\n"'`$\\]/.test(gitPath);
}

function gitShimScript(gitPath: string, platform: NodeJS.Platform): string {
  if (platform === "win32") return `@echo off\r\n@"${gitPath.replace(/%/g, "%%")}" %* < NUL\r\n`;
  return `#!/bin/sh\nexec "${gitPath}" "$@" </dev/null\n`;
}
