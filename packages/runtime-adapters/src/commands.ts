import { isAbsolute } from "node:path";

import { allowRuleFor } from "./connectors.js";
import type { MissionSandbox, RuntimeModelHints, RuntimeModelName } from "./types.js";
import type {
  ExecutableLaunch,
  MissionRuntimeId,
  RuntimeCommandSpec,
  RuntimeFeature,
  RuntimeIntegrationId,
} from "./types.js";

export const CODEX_REQUIRED_FEATURES = [
  "non-interactive",
  "jsonl-events",
  "stdin-prompt",
  "workspace-selection",
  "read-only-sandbox",
] as const satisfies readonly RuntimeFeature[];

export const CLAUDE_REQUIRED_FEATURES = [
  "non-interactive",
  "jsonl-events",
  "stdin-prompt",
  "restricted-mode",
  "verbose-streaming",
  "partial-messages",
  "plan-permission-mode",
  "tool-allowlist",
  "tool-denylist",
] as const satisfies readonly RuntimeFeature[];

/**
 * Cursor's agent CLI (`cursor-agent`), read off its own `--help`: a print mode
 * with a `stream-json` output format, partial text deltas, and a read-only
 * `--mode plan`. It names no stdin prompt, so that is not required of it.
 */
export const CURSOR_REQUIRED_FEATURES = [
  "non-interactive",
  "jsonl-events",
  "partial-messages",
  "plan-permission-mode",
  "workspace-selection",
] as const satisfies readonly RuntimeFeature[];

/**
 * Gemini CLI: a headless `--prompt` mode that also reads stdin, `stream-json`
 * output, and a read-only `--approval-mode plan`.
 */
export const GEMINI_REQUIRED_FEATURES = [
  "non-interactive",
  "jsonl-events",
  "stdin-prompt",
  "plan-permission-mode",
] as const satisfies readonly RuntimeFeature[];

/**
 * OpenCode's `run` subcommand: a non-interactive turn that prints one JSON
 * object per line.
 *
 * Nothing about a read-only mode is required of it, and that is deliberate.
 * OpenCode has no read-only FLAG -- measured 2026-09-03, containment comes
 * from a permission config passed in the environment, which no help text
 * names and no probe can see. Requiring a feature that cannot be detected
 * would report every install unsupported; the enforcement lives in the
 * command builder instead, where it is applied rather than merely checked.
 */
/**
 * Meta's Muse Code in its `exec` mode.
 *
 * MEASURED on Windows 2026-09-21 against 1.3.0-R3401.1, on the free `echo`
 * provider so the capture cost nothing and needed no account. Evidence in
 * `docs/muse-probe-2026-09-21/`.
 *
 * `muse exec --json` puts JSONL on stdout and leaves its own chatter on
 * stderr, so the two streams do not have to be untangled.
 */
export const MUSE_REQUIRED_FEATURES = [
  "non-interactive",
  "jsonl-events",
] as const satisfies readonly RuntimeFeature[];

export const OPENCODE_REQUIRED_FEATURES = [
  "non-interactive",
  "jsonl-events",
] as const satisfies readonly RuntimeFeature[];

/**
 * GitHub Copilot CLI in its `-p` mode. Both tool lists matter: without
 * `--allow-all-tools` the CLI stops for approval nobody can give, and
 * `--deny-tool` is the only thing measured to actually refuse a write.
 */
export const COPILOT_REQUIRED_FEATURES = [
  "non-interactive",
  "jsonl-events",
  "tool-allowlist",
  "tool-denylist",
] as const satisfies readonly RuntimeFeature[];

export const OMNIROUTE_REQUIRED_FEATURES = [
  "json-health-check",
] as const satisfies readonly RuntimeFeature[];

/**
 * The arguments a `full-access` mission may use, and nothing else may. Each is
 * the narrowest flag that runtime has for "do not ask, and do not confine this
 * to one folder", measured off its own `--help`:
 *
 * - Cursor Agent `--force` -- "Force allow commands unless explicitly denied".
 *   Its `--yolo` alias stays forbidden: one door into a mode is enough.
 * - Copilot CLI `--allow-all-paths` -- "Disable file path verification". Its
 *   `--allow-all` stays forbidden because that also opens URLs, which is a
 *   different question than the one the person answered.
 *
 * Claude Code and Codex CLI take theirs as a flag VALUE rather than a flag, so
 * they are unlocked in the loop below instead. OpenCode's `--auto` was never
 * forbidden. Everything named "dangerously" stays refused in every mode: each
 * has a narrower flag that does the job, and reaching for the wide one would
 * be a choice nobody made.
 */
const FULL_ACCESS_ARGUMENTS = new Set(["--force", "--allow-all-paths", "--disable-sandbox"]);

const FORBIDDEN_ARGUMENTS = new Set([
  "--dangerously-bypass-approvals-and-sandbox",
  "--yolo",
  "--dangerously-bypass-hook-trust",
  "--ignore-rules",
  "--dangerously-skip-permissions",
  "--allow-dangerously-skip-permissions",
  "--full-auto",
  // Cursor Agent: "force allow commands unless explicitly denied".
  "--force",
  "-f",
  // Muse Code: runs its shell outside its own sandbox. Auto only (0.773).
  "--disable-sandbox",
  // Gemini CLI: "-y" is the short form of its yolo mode.
  "-y",
  // OpenCode: publishes the session to opencode.ai and prints a public link.
  // A mission's transcript is the customer's, and sharing it is irreversible.
  "--share",
  // GitHub Copilot CLI: each of these widens a run past the workspace the host
  // chose, or moves it onto GitHub's servers where this process cannot see it.
  // `--allow-all-tools` is NOT among them: it is what makes the run
  // non-interactive at all, and the denylist is what holds it back.
  "--allow-all-paths",
  "--allow-all-urls",
  "--allow-all",
  "--remote",
  "--remote-export",
  "--enable-memory",
  // OpenCode serve: announces the server on the local network (0.0.0.0).
  "--mdns",
]);

function requireText(value: string, label: string): string {
  if (!value.trim() || value.includes("\0")) throw new Error(`${label} must be non-empty`);
  return value;
}

/** Antigravity print mode misses a relative `--add-dir` (agy 1.4.4, measured 2026-10-22). */
function requireAbsolute(value: string, label: string): string {
  const text = requireText(value, label);
  if (!isAbsolute(text)) throw new Error(`${label} must be absolute`);
  return text;
}

function scan(help: string, token: string): boolean {
  return help.toLowerCase().includes(token.toLowerCase());
}

export function detectSupportedFeatures(
  runtime: RuntimeIntegrationId,
  helpText: string,
): readonly RuntimeFeature[] {
  const features: RuntimeFeature[] = [];
  const add = (feature: RuntimeFeature, supported: boolean): void => {
    if (supported) features.push(feature);
  };

  if (runtime === "codex") {
    add("non-interactive", true);
    add("jsonl-events", scan(helpText, "--json"));
    add("stdin-prompt", scan(helpText, "stdin"));
    add("workspace-selection", scan(helpText, "--cd") || scan(helpText, "-C"));
    add("read-only-sandbox", scan(helpText, "--sandbox") && scan(helpText, "read-only"));
  } else if (runtime === "claude") {
    add("non-interactive", scan(helpText, "--print") || scan(helpText, "-p"));
    add(
      "jsonl-events",
      scan(helpText, "--output-format") && scan(helpText, "stream-json"),
    );
    add("stdin-prompt", scan(helpText, "stdin"));
    add("restricted-mode", scan(helpText, "--restricted"));
    add("verbose-streaming", scan(helpText, "--verbose"));
    add("partial-messages", scan(helpText, "--include-partial-messages"));
    // Historical name: read-only Claude ran in plan mode until 0.32.2. What
    // the app needs is the flag itself; the modes it sends are default and
    // acceptEdits, both older than plan.
    add("plan-permission-mode", scan(helpText, "--permission-mode"));
    add("tool-allowlist", scan(helpText, "--tools"));
    add("tool-denylist", scan(helpText, "--disallowedTools"));
  } else if (runtime === "cursor") {
    add("non-interactive", scan(helpText, "--print"));
    add("jsonl-events", scan(helpText, "--output-format") && scan(helpText, "stream-json"));
    add("partial-messages", scan(helpText, "--stream-partial-output"));
    add("plan-permission-mode", scan(helpText, "--mode") && scan(helpText, "plan"));
    add("workspace-selection", scan(helpText, "--workspace"));
  } else if (runtime === "gemini") {
    add("non-interactive", scan(helpText, "--prompt"));
    add("jsonl-events", scan(helpText, "--output-format") && scan(helpText, "stream-json"));
    add("stdin-prompt", scan(helpText, "stdin"));
    add("plan-permission-mode", scan(helpText, "--approval-mode") && scan(helpText, "plan"));
  } else if (runtime === "opencode" || runtime === "copilot") {
    // These two were measured by RUNNING them, not by reading a help page:
    // this repo has never captured `opencode run --help` or `copilot --help`,
    // so nothing below is claimed on the strength of their wording. What is
    // scanned for is the exact flags the builders in this file pass. A CLI
    // that stops naming one is reported unsupported, which is the direction
    // that costs a route rather than the direction that runs an argv the
    // CLI may reject halfway through a mission.
    if (runtime === "opencode") {
      add("non-interactive", scan(helpText, "run"));
      add("jsonl-events", scan(helpText, "--format") && scan(helpText, "json"));
    } else {
      add("non-interactive", scan(helpText, "--prompt") || scan(helpText, "-p"));
      add("jsonl-events", scan(helpText, "--output-format") && scan(helpText, "json"));
      add("tool-allowlist", scan(helpText, "--allow-all-tools"));
      add("tool-denylist", scan(helpText, "--deny-tool"));
    }
  } else if (runtime === "muse") {
    /*
     * MUSE HAD NO BRANCH HERE AT ALL, and the `else` below is a gateway's:
     * it claims `json-health-check` and nothing else. So Muse's two required
     * features were never detected, discovery reported it `unsupported`, and
     * no mission could have run under it however finished the adapter was.
     * Found on 2026-09-21 by driving the packaged build, not by a test.
     *
     * Scanned for are the exact flags `createMuseExecCommand` passes, off
     * `muse exec --help` -- which is what `capabilityArgs` now asks for. The
     * TOP-LEVEL `muse --help` names the subcommands and none of these flags,
     * so it could not have answered this either.
     */
    add("non-interactive", scan(helpText, "exec"));
    add("jsonl-events", scan(helpText, "--json"));
    add("workspace-selection", scan(helpText, "--workspace"));
    add("read-only-sandbox", scan(helpText, "--disable-write") && scan(helpText, "--disable-shell"));
  } else if (runtime === "antigravity") {
    // Nothing. Antigravity's `agentapi` prints no help text this repo has ever
    // captured, and it cannot even open a conversation without a project id
    // from its own state file -- so no feature is claimed for it. Falling
    // through to the gateway branch below would have it claim a health check
    // off a help page that does not exist.
    void helpText;
  } else {
    add("json-health-check", scan(helpText, "--json"));
  }

  return features;
}

/**
 * The last gate before a runtime is launched. Every builder passes through it,
 * and it defaults to the strictest reading: called without a sandbox, it
 * refuses everything it would refuse for a read-only mission, so a caller who
 * forgets to say cannot accidentally get more.
 */
export function assertSafeRuntimeCommand(
  spec: RuntimeCommandSpec,
  sandbox?: MissionSandbox,
): void {
  // The spec's own word when the caller does not name one: the process runner
  // validates the argv again at spawn time and has nothing else to go on, and
  // judging an Auto mission by the strictest reading refused a run the person
  // had explicitly widened.
  const full = (sandbox ?? spec.sandbox) === "full-access";
  for (let index = 0; index < spec.args.length; index += 1) {
    const argument = spec.args[index];
    if (argument === undefined) continue;
    // Antigravity CLI has no other way to allow everything (agy 1.2.14), so
    // its one flag is full access's -- for that runtime alone (0.540).
    const unlocked = full && (FULL_ACCESS_ARGUMENTS.has(argument) || (spec.runtime === "antigravity" && argument === "--dangerously-skip-permissions"));
    if (FORBIDDEN_ARGUMENTS.has(argument) && !unlocked) {
      throw new Error(`Forbidden runtime argument: ${argument}`);
    }
    if (
      !unlocked &&
      [...FORBIDDEN_ARGUMENTS].some((forbidden) => argument.startsWith(`${forbidden}=`))
    ) {
      throw new Error(`Forbidden runtime argument: ${argument}`);
    }
    if (argument === "--sandbox=danger-full-access" && !full) {
      throw new Error("Forbidden Codex sandbox: danger-full-access");
    }
    if (argument.toLowerCase() === "--permission-mode=bypasspermissions" && !full) {
      throw new Error("Forbidden Claude permission mode: bypassPermissions");
    }
    if (argument === "--ask-for-approval=never") {
      throw new Error("Forbidden Codex approval mode: never");
    }
    if (argument === "danger-full-access" && spec.args[index - 1] === "--sandbox" && !full) {
      throw new Error("Forbidden Codex sandbox: danger-full-access");
    }
    if (
      argument.toLowerCase() === "bypasspermissions" &&
      spec.args[index - 1] === "--permission-mode" &&
      !full
    ) {
      throw new Error("Forbidden Claude permission mode: bypassPermissions");
    }
    if (argument === "never" && spec.args[index - 1] === "--ask-for-approval") {
      throw new Error("Forbidden Codex approval mode: never");
    }
    if (argument === "--approval-mode=yolo") {
      throw new Error("Forbidden Gemini approval mode: yolo");
    }
    if (argument === "yolo" && spec.args[index - 1] === "--approval-mode") {
      throw new Error("Forbidden Gemini approval mode: yolo");
    }
  }
}

interface SpecTransport {
  /** Omitted means the prompt goes on stdin, which is what most CLIs read. */
  readonly stdin?: RuntimeCommandSpec["stdin"];
  /** What the mission was allowed, so the guard can judge the argv it is given. */
  readonly sandbox?: MissionSandbox;
  readonly env?: Readonly<Record<string, string>>;
  readonly stderrRecords?: boolean;
  readonly oversizedStandIns?: boolean;
}

function baseSpec(
  runtime: MissionRuntimeId,
  executable: ExecutableLaunch,
  cwd: string,
  args: readonly string[],
  transport: SpecTransport = {},
): RuntimeCommandSpec {
  const spec: RuntimeCommandSpec = {
    runtime,
    executablePath: requireText(executable.executablePath, "Executable path"),
    args: [...executable.prefixArgs, ...args],
    cwd: requireText(cwd, "Workspace path"),
    stdin: transport.stdin ?? "prompt",
    stdout: "jsonl",
    ...(transport.sandbox === undefined ? {} : { sandbox: transport.sandbox }),
    // The launch's own environment first, so a builder's variables win a
    // collision -- they are about this run, the launch's is about how the
    // program is started at all.
    ...(executable.env === undefined && transport.env === undefined
      ? {}
      : { env: { ...executable.env, ...transport.env } }),
    ...(transport.stderrRecords === true ? { stderrRecords: true } : {}),
    ...(transport.oversizedStandIns === true ? { oversizedStandIns: true } : {}),
  };
  assertSafeRuntimeCommand(spec, transport.sandbox);
  return spec;
}

/**
 * How much a mission may touch. `read-only` is the default everywhere: a caller
 * that says nothing gets the safe mode, so write access is only ever something
 * a caller asked for explicitly.
 *
 * `full-access` is the Auto mode a person turns on for themselves (Colin,
 * 2026-09-06: "there needs to be an auto option ... to allow them to work out
 * of the workspace folder if desired by the user"). It is not a loophole in
 * the guard below: it unlocks exactly two arguments and two flag VALUES, each
 * measured off the runtime's own `--help` on 2026-09-06, and everything else
 * stays refused -- including the flags that would send a transcript somewhere
 * else, which are about disclosure rather than reach.
 *
 * Declared in `types.ts` beside the spec that carries it, and re-exported here
 * because this is where callers look for it.
 */
export type { MissionSandbox } from "./types.js";

export interface RuntimeCommandOptions {
  /** Host-selected references. No grant is added to a mode that could then write outside its folder. */
  readonly referenceFolders?: readonly string[];
  readonly workspacePath: string;
  /**
   * The repository this run's folder is a WORKTREE of, when it is one.
   *
   * A worktree's `.git` is a file pointing back at the parent repository, so
   * nothing inside it is self-contained: git reaches out, and so does a model
   * told the project is a folder it is not standing in. OpenCode auto-rejects
   * a request for a directory outside its own, and that rejection ENDS the
   * run -- three teammates on worktrees lost between one and three of their
   * runs to it, with nothing to show but "the run could not continue"
   * (drive, 2026-09-06). Naming the parent here is not an escalation: it is
   * the folder the person pointed the app at.
   */
  readonly repositoryRoot?: string;
  /**
   * The platform the command will run on, where it changes the command (Cursor
   * asks for its sandbox only where one exists). The host's own when absent.
   */
  readonly platform?: NodeJS.Platform;
  readonly model?: string;
  /** A reasoning effort the runtime reported supporting for that model. */
  readonly effort?: string;
  readonly sandbox?: MissionSandbox;
  /**
   * The runtime's own session handle, to continue that conversation instead of
   * starting a blank one. Codex calls it a thread id and Claude Code a session
   * id; both take it back on the command line, which is what makes a second
   * turn a reply rather than a stranger.
   */
  readonly resumeThreadId?: string;
  /**
   * Continue a COPY of that session, leaving the session itself as it was
   * (0.461, a question asked on the side). Claude Code takes
   * `--fork-session` beside `--resume`, OpenCode `--fork` beside `-s`, and
   * Codex's app-server has `thread/fork` (codex-app-server-run.ts). MEASURED
   * 2026-09-29 on OpenCode's free model: the fork answered from the earlier
   * turns and the original session never held the question.
   */
  readonly forkSession?: boolean;
  /**
   * The mission's prompt, for the runtimes that take it as a positional
   * argument instead of on stdin. Every other builder ignores it: the runner
   * still owns delivery there, and a prompt in two places is a prompt that
   * can disagree with itself.
   */
  readonly prompt?: string;
  /**
   * The session id the HOST minted for a first Copilot run. Copilot prints its
   * session id only in the terminal `result` record, so a run that dies before
   * that record would leave nothing to resume from; the host generating the id
   * up front is what makes a follow-up possible at all.
   */
  readonly sessionId?: string;
  /**
   * The connectors this teammate has been given, by the name the CLI prints.
   *
   * Each becomes an allow rule -- `mcp__claude_ai_Robinhood__*` -- so the run
   * may use that connector's tools without stopping to ask. Without one, the
   * tools are still OFFERED and every call prompts; a printed run has nowhere
   * to put the question, so it is denied. So this is the difference between a
   * connector a teammate can use and one it can only fail at.
   *
   * Named, never wildcarded: the CLI refuses `mcp__*` in an allow rule, and
   * naming is also the point -- a teammate given Robinhood is not thereby
   * given Gmail. Claude Code only; every other builder ignores it.
   */
  readonly connectors?: readonly string[];
  /**
   * Whether the run loads the person's connectors at all (0.798): `false` keeps every MCP server but the ones named
   * by `--mcp-config` -- Locust's own -- out of the run. Claude Code sends every tool of every connector with each
   * call, about 170k tokens on Colin's account (MEASURED 2026-10-19 on Haiku, Sonnet and Opus), so a teammate has
   * them only when it has one ticked. Absent, the CLI's own choice. Claude Code only.
   */
  readonly accountConnectors?: boolean;
  /**
   * Models the person added themselves (0.357): an OpenAI-compatible
   * endpoint -- their company's model, or one running on this machine --
   * declared to OpenCode as a provider of its own, by id. OpenCode only;
   * every other builder ignores it. The key rides in the child's own
   * environment with the rest of the config, and nowhere else: no argv, no
   * record.
   */
  readonly providers?: Readonly<Record<string, OpenCodeProvider>>;
  /**
   * Locust as the permission host for this run.
   *
   * `configPath` is an mcp.json naming Locust's bridge as a stdio server;
   * `toolName` is the bridge's one tool. Claude Code calls it before using a
   * connector and waits for the answer, which is the person's, from the card
   * the app already has. Without this a printed run has nowhere to ask and
   * every connector call is refused -- or, with `connectors` above, allowed
   * without asking. This is the asking.
   *
   * Never sent in Auto: `bypassPermissions` asks nothing, and a prompt tool
   * there would be a question nobody is asked. Claude Code only.
   */
  readonly permissionBridge?: { readonly configPath: string; readonly toolName: string };
  /**
   * Locust's command guard (0.717): a settings file naming a PreToolUse hook
   * that refuses ending a program BY NAME the person or another teammate runs
   * -- a browser, Node, Locust. Sent in EVERY mode, Auto included: it asks
   * nobody, it only refuses that. Claude Code only.
   */
  readonly commandGuard?: { readonly settingsPath: string };
  /**
   * The same guard for Copilot (0.721): a plugin folder, in Claude Code's
   * plugin format, whose PreToolUse hook is that guard -- one `--plugin-dir`,
   * on the print route and the ACP route alike. Copilot only.
   */
  readonly guardPlugin?: string;
  /**
   * The same guard for OpenCode (0.722): the file URL of a plugin whose
   * `tool.execute.before` asks it, added to the `plugin` list of the config
   * every run and server is given (`withOpenCodeGuard`). OpenCode only.
   */
  readonly openCodeGuardPlugin?: string;
  /**
   * A folder of skills for this run (0.813, Locust's run-skills.ts: `<name>/SKILL.md` in it), named in the per-run
   * config's `skills.paths`, which OpenCode 1.18.27 scans for every SKILL.md beneath it (read in its own code, 2026-10-22).
   * NOT `OPENCODE_CONFIG_DIR`: a folder named there is a config folder, and OpenCode installs its 61 MB plugin
   * package into each one before the run starts -- a new folder every run left the 0.813 drive "Starting OpenCode"
   * for five minutes.
   */
  readonly skillsDir?: string;
  /** The person's OpenCode 1 skill paths, read as JSONC by the host; never written back. */
  readonly skillPaths?: readonly string[];
  /**
   * Copilot's skills for this run (MEASURED 2026-10-22, copilot 1.0.101): one `--plugin-dir`, the run's folder.
   * Its `.plugin/plugin.json` points `skills` at `skills/<name>/SKILL.md`. `copilot skill list` shows those beside
   * the person's own, and nothing is installed. Not `copilot skill add`, which would keep the directory. Copilot only.
   */
  readonly skillPlugin?: string;
  /**
   * Antigravity's skills for this run (MEASURED 2026-10-22, agy 1.4.4): an absolute `--add-dir`. Print mode reads
   * `<dir>/.agents/skills/<name>/SKILL.md`, and a one-line prompt answered with the word only that skill held. The
   * folder was unchanged. A relative directory is not seen. Antigravity only.
   */
  readonly skillWorkspace?: string;
  /**
   * One of the runtime's own slash commands, typed by the person (0.427):
   * `opencode run --command <name>`, its arguments being what arrives on
   * stdin. MEASURED 2026-09-28 on 1.18.27 with the free Nemotron: `--command
   * init` with "Keep it under five lines." on stdin wrote a three-line
   * AGENTS.md; `--command review` with nothing on stdin reviewed the
   * uncommitted change. OpenCode only; every other builder ignores it.
   */
  readonly slashCommand?: string;
  /**
   * Claude Code plugin folders that carry the skills this run may use (0.679),
   * one `--plugin-dir` each. Outside Auto every run is `--restricted`, which
   * MEASURED 2026-10-04 drops the folder's `.claude/skills` and the person's
   * own; a plugin named on the command line is the one way a skill still
   * loads (FINDING-claude-skills-under-restricted.md). Auto runs as the
   * person and finds those itself; it is sent only the skills kept from
   * GitHub (0.710), which live in Locust's profile. Claude Code only.
   */
  readonly skillPlugins?: readonly string[];
  /**
   * Per-run browser MCP server (`locust_browser`). Passed per-run on the
   * command line or environment and never written to global agent configs.
   */
  readonly browserMcp?: BrowserMcpSpec;
  /**
   * Locust's git tool (`locust_git`, 0.818) for an OpenCode run with no shell: Ask and Plan. OpenCode only, and only
   * for a read-only run; see withOpenCodeGitReads.
   */
  readonly gitReads?: GitReadsSpec;
}

export interface BrowserMcpSpec {
  readonly configPath: string;
  readonly copilotConfigPath?: string;
  readonly command: string;
  readonly args: readonly string[];
  readonly env: Readonly<Record<string, string>>;
}

/** Locust's git tool's server (resources/locust-git-reads.mjs): its command line, and what tells a run to use it. */
export interface GitReadsSpec {
  readonly command: string;
  readonly args: readonly string[];
  readonly env: Readonly<Record<string, string>>;
  /** A file for OpenCode's `instructions`, which every agent of the run reads, a /review subagent included. */
  readonly instructions?: string;
}

/** A command name as a runtime lists it: `init`, `security-review`, `plugin:skill`. */
export const SLASH_COMMAND_NAME = /^[A-Za-z0-9][A-Za-z0-9:._-]{0,63}$/;

/**
 * Claude Code started only to say which commands it has (0.428).
 *
 * With stream-json input it lists them -- `system`/`commands_changed`, twice,
 * within about two seconds -- before it reads anything, and it reads nothing
 * until a message arrives. MEASURED 2026-09-28 on 2.1.283: with no message
 * ever written, two lists (116, then 119) and no `init`, no request, no cost.
 * `--restricted` as every run outside Auto has it, so the list is the one a
 * run will accept; `--no-session-persistence` so no session file is left.
 */
export function createClaudeCommandListCommand(
  executable: ExecutableLaunch,
  options: { readonly workspacePath: string },
): RuntimeCommandSpec {
  return baseSpec("claude", executable, options.workspacePath, [
    "--restricted",
    "--print",
    "--input-format",
    "stream-json",
    "--output-format",
    "stream-json",
    "--verbose",
    "--no-session-persistence",
  ], { stdin: "protocol", sandbox: "read-only" });
}

/**
 * `--allowedTools` and its rules, or nothing at all when there are none: the connectors a person ticked, and Locust's
 * own browser (`mcp__locust_browser__*`) when the run has it. ONE flag (2026-10-19): the browser's rule arrived as a
 * second `--allowedTools`, and whether the CLI keeps both lists or the last is a guess nobody measured.
 */
function connectorRules(names: readonly string[] | undefined, browser = false): readonly string[] {
  const rules: string[] = []
  for (const name of names ?? []) {
    const rule = allowRuleFor(name);
    if (rule !== undefined && !rules.includes(rule)) rules.push(rule);
  }
  if (browser) rules.push("mcp__locust_browser__*");
  return rules.length === 0 ? [] : ["--allowedTools", rules.join(",")];
}

const EFFORT = /^[a-z]{1,16}$/;

/** An effort level is one plain word; anything else never reaches an argv. */
function requireEffort(value: string): string {
  if (!EFFORT.test(value)) throw new Error("Effort is invalid");
  return value;
}

/**
 * The effort levels a CLI says it takes, read from its own `--help`.
 *
 * Two shapes, because two CLIs write it differently and both are real:
 *
 *   Claude   --effort <level>  ... (low, medium, high)
 *   Copilot  --effort, --reasoning-effort <level>  Set the reasoning effort
 *            level (choices: "none", "minimal", "low", ...)
 *
 * The original pattern required `--effort <level>` immediately and accepted
 * only bare words, so Copilot's list -- quoted, wrapped across three lines,
 * and behind a second flag name -- matched nothing. Copilot therefore reported
 * no efforts at all, the composer drew "effort - fixed", and seven levels the
 * installed CLI genuinely accepts could never be chosen (measured 2026-09-08
 * against copilot 1.0.83).
 *
 * Nothing is guessed: a CLI that names no choices yields none.
 */
export function parseEffortChoices(helpText: string): readonly string[] {
  // And a third, copilot 1.0.88 (read 2026-09-24): `--reasoning-effort
  // <level>  ... [possible values: none, minimal, low, ...]` -- square
  // brackets, which the parenthesised pattern never matched.
  const clause = /--(?:reasoning-)?effort\b[\s\S]{0,240}?(?:\(\s*(?:choices:)?\s*([^)]*)\)|\[\s*possible values:\s*([^\]]*)\])/i.exec(helpText);
  if (clause === null) return [];
  return [...new Set(
    (clause[1] ?? clause[2] ?? '')
      .split(",")
      .map((entry) => entry.trim().replace(/^["']|["']$/g, "").trim())
      .filter((entry) => EFFORT.test(entry))
  )];
}

/**
 * Read what the Claude Code CLI says about models in its own `--help`: the
 * aliases it accepts for the newest model of each family, and the effort
 * levels it takes. Missing text yields nothing, never a guess.
 */
export function parseClaudeModelHints(helpText: string): RuntimeModelHints | undefined {
  const aliasClause = /alias for the latest model \(e\.g\.\s*([^)]*)\)/.exec(helpText);
  const aliases = aliasClause
    ? [...aliasClause[1]!.matchAll(/'([a-z0-9][a-z0-9.-]{0,30})'/g)].map((match) => match[1]!)
    : [];
  const efforts = parseEffortChoices(helpText);
  if (aliases.length === 0 && efforts.length === 0) return undefined;
  return { aliases: [...new Set(aliases)], efforts: [...new Set(efforts)] };
}

/**
 * Read `cursor-agent --list-models` as it prints: a heading, then one
 * `<id> - <display name>` per line. Measured 2026-09-02; the ids carry the
 * effort (`cursor-grok-4.6-high`), which is why no effort list comes back.
 * Nothing is listed that the CLI did not print.
 */
/**
 * Whether Cursor's SANDBOX runs on this platform: macOS and Linux only. On
 * Windows the CLI exits 1 saying so (measured 2026-09-02).
 */
export function cursorSandboxAvailable(platform: NodeJS.Platform): boolean {
  return platform === "darwin" || platform === "linux";
}

/**
 * Whether Cursor can hold a mission read-only on this platform. Everywhere
 * now: its `--mode ask` ("read-only", cursor-agent --help, 2026.09.26).
 *
 * MEASURED 2026-09-30 on Windows, three turns on Grok 4.7 told firmly to
 * write -- with its edit tool, with a shell `echo hi > file`, and with a
 * write dressed up as a "read-only diagnostic" (`python -c open(...,'w')`,
 * `cmd /c type nul > touched.txt`). All three refused, naming ask mode, and
 * the folder held only what it had. Plan mode had written two files under
 * the same kind of push (2026-09-02), which is why Windows was refused until
 * now. Where Cursor's sandbox runs it is asked for too; on Windows ask mode
 * stands alone, and the host's own watch of the folder still shows a file
 * that changes.
 */
export function cursorCanEnforceReadOnly(_platform: NodeJS.Platform): boolean {
  return true;
}

export function parseCursorModelList(text: string): RuntimeModelHints | undefined {
  const models: RuntimeModelName[] = [];
  for (const line of text.split(/\r?\n/)) {
    const match = /^([a-z0-9][a-z0-9.-]{0,60})\s+-\s+(.{1,80})$/.exec(line.trim());
    if (match === null || models.some((model) => model.id === match[1])) continue;
    models.push({ id: match[1]!, displayName: match[2]!.trim() });
  }
  if (models.length === 0) return undefined;
  return { aliases: models.map((model) => model.id), efforts: [], models };
}

function sandboxArgument(sandbox: MissionSandbox | undefined): MissionSandbox {
  if (sandbox === undefined) return "read-only";
  if (sandbox !== "read-only" && sandbox !== "workspace-write" && sandbox !== "full-access") {
    throw new Error("Unsupported mission sandbox");
  }
  return sandbox;
}

/** Codex's own name for the widest policy, which is not the host's name for it. */
function codexSandboxArgument(sandbox: MissionSandbox | undefined): string {
  const chosen = sandboxArgument(sandbox);
  return chosen === "full-access" ? "danger-full-access" : chosen;
}

/**
 * What a Codex thread is started with on the app-server transport.
 *
 * The exec transport carries these as argv, where `assertSafeRuntimeCommand`
 * can see them; here they are JSON on a socket, so the same two rules are
 * enforced at the point they are chosen instead. `danger-full-access` needs a
 * mission that really was granted full access, and nothing else may ask for
 * it -- the reading a person gets from the mode chip is the reading the
 * runtime is given.
 */
export interface CodexAppServerPolicy {
  readonly sandbox: string;
  readonly approvalPolicy: string;
}

/**
 * MEASURED 2026-09-10 against `codex app-server` 0.153.0: all three sandboxes
 * start a thread with `approvalPolicy: "never"`, and every one of them streams
 * `item/agentMessage/delta` (41-57 deltas on a three-sentence reply). `never`
 * is what the exec transport already does by being non-interactive -- it is
 * not a widening, it is the same run without the silence.
 */
export function codexAppServerPolicy(
  sandbox: MissionSandbox | undefined,
  mode?: string,
): CodexAppServerPolicy {
  const chosen = sandboxArgument(sandbox);
  // Approve-each is the one mode that STOPS: `untrusted` makes the server ask
  // before every consequential action, and the approval channel answers. It
  // is never paired with full access -- a run that may do anything has
  // nothing to ask -- so the sandbox here is whatever the mode earned, which
  // for Approve-each is workspace-write.
  if (mode === "approve-each") {
    return { sandbox: chosen === "full-access" ? "workspace-write" : chosen, approvalPolicy: "untrusted" };
  }
  if (chosen === "full-access") {
    return { sandbox: "danger-full-access", approvalPolicy: "never" };
  }
  return { sandbox: chosen, approvalPolicy: "never" };
}

/**
 * `codex app-server`: the transport that streams.
 *
 * MEASURED 2026-09-10: `codex exec --json` sends an agent message ONCE, whole,
 * as a single `item.completed` -- there is no delta in that JSONL at all. So a
 * Codex teammate outside Approve-each sat silent and then dropped the whole
 * reply in one paint, which is the opposite of what Codex's own TUI does.
 * app-server sends the same reply as dozens of deltas.
 *
 * Nothing about the run travels in this argv: the folder, the sandbox, the
 * model, the effort and the prompt are all parameters of `thread/start` and
 * `turn/start`. The spec exists so the ledger records what was launched and so
 * the ordinary runner refuses to launch it -- hence `stdin: "protocol"`.
 */
export function createCodexAppServerCommand(
  executable: ExecutableLaunch,
  options: {
    readonly workspacePath: string;
    readonly sandbox?: MissionSandbox;
    /** The CLI's own version, as discovered; decides whether the plan tool is switched on. */
    readonly cliVersion?: string;
  },
): RuntimeCommandSpec {
  return baseSpec(
    "codex",
    executable,
    options.workspacePath,
    ["app-server", ...codexPlanToolArguments(options.cliVersion)],
    {
      sandbox: sandboxArgument(options.sandbox),
      stdin: "protocol",
    },
  );
}

/**
 * THE PLAN TOOL, SWITCHED ON (0.304).
 *
 * Colin, 2026-09-23, on a Codex reply that opened with a typed "TODO" list of
 * "In progress: ..." and "Pending: ..." lines: "planui looks like its failing
 * in here in a codex chart". Codex 0.153 offers its `update_plan` tool only
 * when its config says `tools.update_plan.enabled`, and nothing said so. The
 * model, briefed to keep a todo list with its own tool, had no such tool and
 * typed the list into its reply instead -- no `turn/plan/updated` ever came,
 * so the plan panel had nothing to draw. The session's own log (GPT-5.6-Sol,
 * code mode) never once mentions update_plan.
 *
 * MEASURED 2026-09-24 (_tools/probe-codex-plan-tool.mjs, one GPT-5.6-Luna
 * turn each way, Locust's own todo sentence): on 0.153.0 and 0.156.1 alike,
 * no plan update without the setting; with it, three, every step with its
 * status, and no list typed into the reply. Only from 0.153.0 up: older
 * builds were never measured with it, and one that read `tools.update_plan`
 * as something else could refuse its own configuration.
 */
export const CODEX_PLAN_TOOL_FROM = "0.153.0";

export function codexPlanToolArguments(cliVersion: string | undefined): readonly string[] {
  return cliVersion !== undefined && versionAtLeast(cliVersion, CODEX_PLAN_TOOL_FROM)
    ? ["-c", "tools.update_plan.enabled=true"]
    : [];
}

/** `major.minor.patch` compared as numbers; anything unreadable is not at least anything. */
function versionAtLeast(version: string, floor: string): boolean {
  const parse = (text: string): readonly number[] | undefined => {
    const match = /^v?(\d+)\.(\d+)\.(\d+)/.exec(text.trim());
    return match === null ? undefined : [Number(match[1]), Number(match[2]), Number(match[3])];
  };
  const have = parse(version);
  const need = parse(floor);
  if (have === undefined || need === undefined) return false;
  for (let index = 0; index < 3; index += 1) {
    if (have[index]! !== need[index]!) return have[index]! > need[index]!;
  }
  return true;
}

export function createCodexExecCommand(
  executable: ExecutableLaunch,
  options: RuntimeCommandOptions,
): RuntimeCommandSpec {
  // Writes, when allowed at all, are confined to the workspace Codex is given.
  // The host chooses that directory; the renderer only ever chooses the mode.
  // `codex exec resume <SESSION_ID>` is a subcommand, not a flag, so the verb
  // itself changes when a conversation is being continued.
  //
  // MEASURED 2026-09-02: `codex exec resume` takes NEITHER `--sandbox` NOR
  // `-C`. Passing them made the CLI exit 2 with "unexpected argument", so
  // every Codex reply failed with "Codex invocation did not complete
  // successfully" -- the unit tests asserted the argv I had written rather
  // than the argv the CLI accepts, and no live run had exercised it. The
  // sandbox goes through the config override the subcommand does support,
  // and the working directory needs no flag because the process is spawned
  // in it.
  // MEASURED 2026-09-03: without --skip-git-repo-check, a workspace that is
  // not a git repository (and not on Codex's trusted list) fails the run
  // before the model is ever reached -- exit 1 in half a second, stderr "Not
  // inside a trusted directory and --skip-git-repo-check was not specified."
  // That check is Codex protecting a folder it cannot undo changes in; here
  // the host owns that decision instead, through the sandbox argument below,
  // the approval gate, and a recorded diff of every write. Accepted by both
  // `exec` and `exec resume` (both --help checked on 0.151.0-alpha.7.2).
  const args = options.resumeThreadId === undefined
    ? ["exec", "--json", "--skip-git-repo-check", "--sandbox", codexSandboxArgument(options.sandbox), "-C", options.workspacePath]
    : [
        "exec",
        "resume",
        requireText(options.resumeThreadId, "Session id"),
        "--json",
        "--skip-git-repo-check",
        "-c",
        `sandbox_mode=${codexSandboxArgument(options.sandbox)}`,
      ];
  if (options.model !== undefined) {
    args.push("--model", requireText(options.model, "Model"));
  }
  if (options.effort !== undefined) {
    // `codex exec` has no effort flag; the config override is how the CLI's
    // own docs set reasoning effort for a run.
    args.push("-c", `model_reasoning_effort=${requireEffort(options.effort)}`);
  }
  args.push("-");
  return baseSpec("codex", executable, options.workspacePath, args, { sandbox: sandboxArgument(options.sandbox) });
}

/** Claude's file permissions use Edit for Edit, Write and NotebookEdit, and // for an absolute path. */
export function claudeReferenceDenyRule(folder: string): string {
  let path = requireText(folder, "Reference folder").replace(/\\/g, "/").replace(/\/+$/, "");
  if (/^[A-Za-z]:\//.test(path)) path = `/${path[0]!.toLowerCase()}${path.slice(2)}`;
  // A rule must name one literal folder, not a glob or a second tool. Never grant an unprotected reference.
  if (!path.startsWith("/") || path.startsWith("//") || /[\r\n,()*?\[\]]/.test(path)) {
    throw new Error("This reference path cannot be protected by Claude Code's file rules. Nothing was sent; remove it or choose a local folder without pattern characters.");
  }
  return `Edit(/${path}/**)`;
}

export function createClaudePrintCommand(
  executable: ExecutableLaunch,
  options: RuntimeCommandOptions,
): RuntimeCommandSpec {
  // Claude Code CAN edit; this used to be hard-coded so it never did.
  //
  // MEASURED 2026-09-03 off `claude --help`: `--permission-mode` takes
  // `acceptEdits` among others, and `--restricted` does NOT block editing --
  // it removes the command-running tools and WebFetch *unless --tools names
  // them*, and ignores user/project settings files. So `--restricted` is
  // worth keeping in both modes (the tool list stays explicit and auditable,
  // and none of the person's own Claude settings leak into a mission); what
  // held every Claude run read-only was `--permission-mode plan` with a
  // three-tool list, sent whatever the composer asked for. The composer said
  // "Accept edits" and the header said "read-only" on the same screen.
  //
  // Now the mode decides. Read-only keeps exactly what it had. Accept edits
  // names the file-editing tools and Bash, so a Claude mission can do the
  // same work a Codex one can -- edit, then run the tests it just changed.
  // Auto is the person's own choice to let this run work outside the folder
  // (Colin, 2026-09-06). `acceptEdits` accepts edits INSIDE the working
  // directory and still stops for anything outside it, which is a stop nobody
  // can answer in a headless run -- so the mode that matches what was asked
  // for is `bypassPermissions`, measured off `claude --help` the same day.
  const auto = sandboxArgument(options.sandbox) === "full-access";
  const editing = sandboxArgument(options.sandbox) !== "read-only";
  const args = [
    // MEASURED 2026-09-06: `--restricted` and `bypassPermissions` cannot be
    // combined -- the CLI exits at once with "bypassPermissions not supported
    // in restricted mode", which the app saw as a run that ended without a
    // result. So Auto drops it, and that is the honest trade: `--restricted`
    // is what keeps the person's own Claude settings out of a mission, and
    // Auto is the mode where they have said to run as themselves, without
    // asking, anywhere. Every other mode keeps it.
    ...(auto ? [] : ["--restricted"]),
    "--print",
    // A2.10: the prompt goes as a stream-json user turn and input stays open,
    // so a teammate's message can be handed to the running turn. MEASURED
    // 2026-09-25 on claude 2.1.282 with exactly these flags: a message written
    // after the first tool call was taken into the SAME turn -- one result,
    // and the answer did what it asked.
    "--input-format",
    "stream-json",
    "--output-format",
    "stream-json",
    "--verbose",
    "--include-partial-messages",
    // Read-only used to be plan mode. Plan mode has side effects of its own:
    // Claude Code writes its plan under ~/.claude/plans (which the activity
    // fold then counted as "Edited 1 file", outside the folder) and calls
    // ExitPlanMode, which fails without a person to answer it (seen driving
    // the app, 2026-09-05). The tool list is what keeps a run read-only;
    // default mode with only reading tools has nothing to plan or exit.
    //
    // Task is the subagent launcher. Without it no Claude Code teammate
    // could spawn a helper at all (Colin, 2026-09-05: "do we have the
    // ability to run subagents through the capable models?"). A helper
    // inherits the tools of the run that asked for it, so a read-only run's
    // helpers read only.
    "--permission-mode",
    auto ? "bypassPermissions" : editing ? "acceptEdits" : "default",
    // THE PERSON'S CONNECTORS, IN EVERY MODE.
    //
    // This used to allow `mcp__*` in Auto only, and deny it everywhere else,
    // on the belief that `--restricted` had already kept the person's MCP
    // servers out of those runs and there was therefore nothing to allow.
    //
    // That belief was wrong, and the CLI says so in its own help:
    //
    //   --restricted  ... ignores user, project and local settings files
    //                 (managed settings and --settings still apply; ADD
    //                 --strict-mcp-config TO SKIP MCP SERVERS TOO)
    //
    // MEASURED 2026-09-09, running the argv by hand in an empty temp folder:
    // `claude --restricted --print --tools "Read,Glob,Grep,Task,mcp__*"`
    // listed every connector on the account. `--restricted` never blocked
    // them. Locust's own `--disallowedTools mcp__*` was the entire reason a
    // teammate outside Auto could not reach a connector, and the app then
    // told the person it was the mode.
    //
    // So the denial is gone. Colin asked the right question -- "do you think
    // thats acceptable for the user to only have access for mcp tools under
    // auto or is that standard?" -- and it is neither. Claude Code itself
    // makes connectors available in every permission mode, and welding them
    // to Auto meant the only way to let a teammate READ a watchlist was to
    // let it edit anything on the machine. That is backwards: it charges the
    // most dangerous permission for the most harmless capability.
    //
    // What the mode still decides is this machine: Ask and Plan refuse every
    // write to disk, Accept edits confines them to the folder. What it cannot
    // decide is a connector, because a connector acts somewhere else. That is
    // said on the mode menu rather than left to be discovered. Colin,
    // 2026-09-09, ruling on exactly that: "we can let the user decide that
    // with the model, i doubt these models are just gonna randomly start
    // buying crypto."
    "--tools",
    // Skill runs a skill: instructions, using only the tools listed beside it
    // (0.679). Without it, "use the review skill" went to the nearest
    // connector with "skill" in its name, and stopped there for a permission
    // a printed run cannot give (measured 2026-10-04).
    // WebSearch and WebFetch (0.711, Colin: "build your suggestion"), in every
    // mode. Auto runs them unasked, as the person. Everywhere else Claude Code
    // asks before each, and the asking goes through Locust's bridge below to a
    // card -- so a folder that tells a teammate to fetch a URL carrying what
    // it read is stopped by a person reading that URL.
    editing ? "Read,Glob,Grep,Edit,Write,NotebookEdit,Bash,Task,Skill,WebSearch,WebFetch,mcp__*" : "Read,Glob,Grep,Task,Skill,WebSearch,WebFetch,mcp__*",
    /*
     * The connectors this teammate was given, one allow rule each.
     *
     * Removing the denial above only got as far as the tools being offered:
     * Claude Code asks before using one and a printed run cannot answer, so
     * the call is denied and the row reads `failed`. An allow rule is what
     * turns that into a call.
     *
     * Not sent in Auto, where `bypassPermissions` already asks nothing, and
     * where a rule would be a second, weaker statement of the same thing.
     * Empty in every other mode until a person ticks a connector, so the
     * default stays "asks, and therefore cannot" rather than "may".
     *
     * `allowRuleFor` returns nothing for a name that sanitises to nothing --
     * that would produce `mcp____*`, which the CLI ACCEPTS and which widens
     * a scope nobody chose. Failing closed there is deliberate.
     */
    ...(auto ? [] : connectorRules(options.connectors, options.browserMcp !== undefined)),
    /*
     * And somewhere to ASK, for everything the rules above did not cover.
     *
     * MEASURED 2026-09-10: with `--mcp-config` naming Locust's bridge and
     * `--permission-prompt-tool` naming its tool, a `--print` run under
     * `--restricted` in the strictest mode called the tool before using a
     * connector, waited, and honoured the answer; a denial's message reached
     * the model verbatim. Bash and Edit never route through it -- they are
     * granted by `--tools` -- so this is connectors only, which is the one
     * thing the mode was never able to govern.
     */
    // No teammate ends the person's browser (0.717, main/command-guard.ts).
    ...(options.commandGuard === undefined ? [] : ["--settings", requireText(options.commandGuard.settingsPath, "Command guard settings")]),
    /*
     * ONE `--mcp-config` naming every server file this run takes (2026-10-19): the permission bridge outside Auto,
     * and Locust's browser when the run has it. The browser's file arrived as a second `--mcp-config`, and if the CLI
     * kept only the last, the bridge -- and with it every connector's question to the person -- would be gone.
     */
    ...((): readonly string[] => {
      const bridge = auto || options.permissionBridge === undefined ? undefined : options.permissionBridge
      const files = [
        ...(bridge === undefined ? [] : [requireText(bridge.configPath, "Permission bridge config")]),
        ...(options.browserMcp === undefined ? [] : [requireText(options.browserMcp.configPath, "Browser MCP config")])
      ]
      return [
        ...(files.length === 0 ? [] : ["--mcp-config", ...files]),
        ...(bridge === undefined ? [] : ["--permission-prompt-tool", requireText(bridge.toolName, "Permission tool")])
      ]
    })(),
    /*
     * THE PERSON'S CONNECTORS, ONLY WHERE A TEAMMATE HAS ONE TICKED (0.798). Claude Code sends every tool of every
     * connector on the account with each call: MEASURED 2026-10-19 (_tools/drive-haiku-turn-two-context.mjs), a
     * second "say hello" turn held 191,214 tokens on Haiku, 190,424 on Sonnet and 195,329 on Opus -- 235 of 246 tools
     * from ten connectors, and on Opus still 182k written to the cache with 222 of them marked deferred -- against
     * 17,763 with them left out. `--strict-mcp-config` keeps only the servers named above, Locust's own.
     */
    ...(options.accountConnectors === false ? ["--strict-mcp-config"] : []),
  ];
  // `--add-dir` too outside Auto: `--restricted` confines the file tools to
  // the working directories, and a skill reads the files kept beside its
  // SKILL.md. MEASURED 2026-10-06 on Haiku in Ask: the skill loaded, and its
  // own second-word.md was refused, "not permitted to use Read". The folder
  // is a copy made for this run alone (claude-skills.ts), removed after it.
  // Auto is sent only the skills kept from GitHub (0.710), which nothing
  // else finds; it runs as the person, so needs no `--add-dir`.
  for (const dir of options.skillPlugins ?? []) {
    const folder = requireText(dir, "Skill folder");
    args.push("--plugin-dir", folder, ...(auto ? [] : ["--add-dir", folder]));
  }
  if (!auto) {
    for (const folder of options.referenceFolders ?? []) args.push("--add-dir", requireText(folder, "Reference folder"));
    if (editing && (options.referenceFolders?.length ?? 0) > 0) {
      args.push("--disallowedTools", ...options.referenceFolders!.map(claudeReferenceDenyRule));
    }
  }
  if (options.model !== undefined) {
    args.push("--model", requireText(options.model, "Model"));
  }
  if (options.effort !== undefined) {
    args.push("--effort", requireEffort(options.effort));
  }
  if (options.resumeThreadId !== undefined) {
    args.push("--resume", requireText(options.resumeThreadId, "Session id"));
    if (options.forkSession === true) args.push("--fork-session");
  }
  return baseSpec("claude", executable, options.workspacePath, args, {
    stdin: "stream-json",
    sandbox: sandboxArgument(options.sandbox),
    oversizedStandIns: true,
  });
}

/** Reuse the turn's complete argv. Only transport and session persistence differ for the free built-in command. */
export function createClaudeContextCommand(turn: RuntimeCommandSpec, sessionId: string): RuntimeCommandSpec {
  if (turn.runtime !== "claude" || !turn.args.includes("--print") || turn.args.includes("--disable-slash-commands") || turn.args.some((arg) => arg.includes("…"))) throw new Error("The Claude launch flags are unavailable.");
  if (!/^[A-Za-z0-9_-]{1,128}$/.test(sessionId)) throw new Error("The Claude session is unavailable.");
  const args: string[] = [];
  for (let index = 0; index < turn.args.length; index++) {
    const arg = turn.args[index]!;
    if (["--input-format", "--output-format", "--resume"].includes(arg)) { index++; continue; }
    if (["--verbose", "--include-partial-messages", "--fork-session", "--no-session-persistence"].includes(arg)) continue;
    args.push(arg);
  }
  args.push("--output-format", "json", "--no-session-persistence", "--resume", sessionId, "--", "/context");
  return { ...turn, args, stdin: "none" };
}

/**
 * Cursor Agent in its print mode.
 *
 * MEASURED 2026-09-02, and it corrects something this file used to claim.
 * `--mode plan` is described by the CLI as "read-only/planning (analyze,
 * propose plans, no edits)", and a plainly-worded mission does respect it.
 * It is NOT enforcement: told insistently to write, a plan-mode run created
 * two files with its edit tool. Its shell calls were rejected, so plan mode
 * gates commands and not edits.
 *
 * The enforcement Cursor does have is `--sandbox enabled`, and on Windows it
 * answers "Sandbox mode is enabled but not available on this system. Sandbox
 * requires macOS or Linux." So a read-only mission asks for BOTH, and the
 * host refuses to start one where the sandbox cannot run rather than record
 * a containment it cannot keep.
 *
 * The CLI names no effort flag, so an effort is refused rather than dropped:
 * a caller that was shown an effort and had it silently ignored would believe
 * a claim nothing honoured.
 */
export function createCursorPrintCommand(
  executable: ExecutableLaunch,
  options: RuntimeCommandOptions,
): RuntimeCommandSpec {
  const args = [
    "--print",
    "--output-format",
    "stream-json",
    "--stream-partial-output",
    // Measured: without this a headless run in a directory Cursor has not seen
    // stops on a "Workspace Trust Required" prompt nobody can answer. The
    // host chose this workspace, so trusting it for this run is the truth.
    "--trust",
    /*
     * The same shape as `--trust` above it, and missed for the same reason.
     *
     * A connector in `~/.cursor/mcp.json` is approved by ANSWERING A PROMPT,
     * and a headless run has nobody to answer it -- so the prompt resolves
     * to "no" and every connector call fails. MEASURED 2026-09-11, one
     * headless run against Colin's own configured server:
     *
     *   Failed: user rejected MCP `robinhood-trading-get_accounts`.
     *
     * Not an auth failure, though it arrives beside one: the run was
     * REJECTED before the credential mattered. That is why the connector
     * worked in Cursor's own app and not here, which is how Colin found it
     * ("works on cursor agent but not on locust") -- the app has someone to
     * ask and this does not.
     *
     * So the effective policy without this flag is not "safer", it is
     * "always no", for servers the person themselves configured and that
     * Cursor's own app uses freely. Colin's standing rule is the other way
     * round: "just let them have access to the mcp tools if the client have
     * access to it." Locust adds no server of its own and reads no
     * credential -- what a teammate can reach here is exactly what its CLI
     * was already set up to reach.
     */
    "--approve-mcps",
    "--workspace",
    options.workspacePath,
  ];
  if (sandboxArgument(options.sandbox) === "read-only") {
    // Ask mode, not plan mode: plan mode wrote files when pushed, ask mode
    // refused every write it was pushed to make (cursorCanEnforceReadOnly).
    // The sandbox as well wherever it runs.
    args.push("--mode", "ask");
    if (cursorSandboxAvailable(options.platform ?? process.platform)) args.push("--sandbox", "enabled");
  }
  if (options.model !== undefined) {
    args.push("--model", requireText(options.model, "Model"));
  }
  if (options.effort !== undefined) {
    throw new Error("Cursor Agent takes no effort level");
  }
  if (sandboxArgument(options.sandbox) === "full-access") {
    // "Force allow commands unless explicitly denied" -- cursor-agent --help,
    // measured 2026-09-06. Its `--yolo` alias does the same thing and stays
    // forbidden; one door into this mode is enough.
    args.push("--force");
  }
  if (options.resumeThreadId !== undefined) {
    args.push("--resume", requireText(options.resumeThreadId, "Session id"));
  }
  return baseSpec("cursor", executable, options.workspacePath, args, {
    sandbox: sandboxArgument(options.sandbox),
    /*
     * An edit of a large file completes as one record that holds the file
     * twice. Measured 2026-10-05: index.ts (399 KB) four times in one turn
     * and missionView.ts (293 KB) once in the turn before it. Each completion
     * was past the 256 KB cap, the host dropped it, and the call stayed open
     * -- the files card read "not confirmed" on edits that had landed. The
     * stand-in names the call and none of the file, the way Claude's does.
     */
    oversizedStandIns: true,
  });
}

/**
 * Gemini CLI, headless. The prompt arrives on stdin, which its help says a
 * headless run reads. `--skip-trust` trusts the workspace for this run only:
 * measured on an untrusted folder, the CLI silently overrode plan mode to its
 * default, so without it a read-only mission would not be one.
 *
 * `yolo` is never an approval mode here, and `-y` is refused outright.
 */
export function createGeminiPrintCommand(
  executable: ExecutableLaunch,
  options: RuntimeCommandOptions,
): RuntimeCommandSpec {
  const args = [
    "--output-format",
    "stream-json",
    "--skip-trust",
    "--approval-mode",
    sandboxArgument(options.sandbox) === "read-only" ? "plan" : "auto_edit",
  ];
  if (options.model !== undefined) {
    args.push("--model", requireText(options.model, "Model"));
  }
  if (options.effort !== undefined) {
    throw new Error("Gemini CLI takes no effort level");
  }
  if (options.resumeThreadId !== undefined) {
    args.push("--resume", requireText(options.resumeThreadId, "Session id"));
  }
  return baseSpec("gemini", executable, options.workspacePath, args);
}

/**
 * Antigravity CLI (`agy`), headless (0.540). Google moved personal accounts
 * off Gemini CLI to it on 2026-06-18; Colin's AI Pro sign-in was refused by
 * Gemini CLI ("This client is no longer supported for Gemini Code Assist for
 * individuals") and taken by `agy`. Every flag below is off `agy --help`,
 * 1.2.14, and each behaviour MEASURED 2026-10-02 in a scratch folder:
 *
 * - `--output-format stream-json` prints `init`, `step_update` and `result`
 *   events (agy-events.ts).
 * - The prompt goes on stdin as one stream-json `user` line (`agy-json`), so
 *   a long brief never meets the command-line limit; `-p=` asks for print
 *   mode with no prompt in argv. Bare `-p` would swallow the next flag.
 * - Read-only is the default mode: a write or a command is auto-denied and
 *   named in the result's `denied_actions`. `--mode accept-edits` allows file
 *   edits and still denies commands. `--dangerously-skip-permissions` allows
 *   everything, and is only ever full access's.
 * - `--conversation <id>` carries a conversation on; measured, it answered
 *   from the turn before.
 */
export function createAgyPrintCommand(
  executable: ExecutableLaunch,
  options: RuntimeCommandOptions,
): RuntimeCommandSpec {
  const sandbox = sandboxArgument(options.sandbox);
  const args = ["--output-format", "stream-json", "--input-format", "stream-json"];
  if (options.skillWorkspace !== undefined) args.push("--add-dir", requireAbsolute(options.skillWorkspace, "Skills folder"));
  if (sandbox === "read-only") for (const folder of options.referenceFolders ?? []) args.push("--add-dir", requireText(folder, "Reference folder"));
  if (sandbox === "workspace-write") args.push("--mode", "accept-edits");
  if (sandbox === "full-access") args.push("--dangerously-skip-permissions");
  if (options.model !== undefined) args.push("--model", requireText(options.model, "Model"));
  if (options.effort !== undefined) args.push("--effort", requireText(options.effort, "Effort"));
  if (options.resumeThreadId !== undefined) args.push("--conversation", requireText(options.resumeThreadId, "Conversation id"));
  args.push("-p=");
  return baseSpec("antigravity", executable, options.workspacePath, args, { stdin: "agy-json", sandbox });
}

/**
 * Read `agy models` as it prints (1.2.14): a "Fetching" line, then one
 * `id<TAB>Display name` per line.
 */
export function parseAgyModelList(text: string): RuntimeModelHints | undefined {
  /*
   * ONE MODEL, ITS EFFORTS BESIDE IT. agy lists each effort as its own row
   * -- `gemini-3.8-flash-low  Gemini 3.8 Flash (Low)` -- and MEASURED
   * 2026-10-02 it also takes `--model gemini-3.8-flash --effort low`. So the
   * rows are folded into one model with its efforts, as Claude's and Codex's
   * are, and the picker does not list Flash three times. Colin, at the
   * first build: "effort is showing in model picker".
   */
  const models: { id: string; displayName: string; efforts: string[] }[] = [];
  for (const line of text.split(/\r?\n/)) {
    const match = /^([a-z0-9][a-z0-9.-]{0,80})\t(.{1,80})$/.exec(line.trim());
    if (match === null) continue;
    const variant = /^(.*)-(low|medium|high|max)$/.exec(match[1]!);
    const id = variant === null ? match[1]! : variant[1]!;
    const displayName = variant === null ? match[2]!.trim() : match[2]!.replace(/\s*\((low|medium|high|max)\)\s*$/i, "").trim();
    const known = models.find((model) => model.id === id);
    if (known === undefined) models.push({ id, displayName, efforts: variant === null ? [] : [variant[2]!] });
    else if (variant !== null && !known.efforts.includes(variant[2]!)) known.efforts.push(variant[2]!);
  }
  if (models.length === 0) return undefined;
  const order = ["low", "medium", "high", "max"];
  const named: RuntimeModelName[] = models.map((model) => ({
    id: model.id,
    displayName: model.displayName,
    ...(model.efforts.length === 0 ? {} : { efforts: [...model.efforts].sort((a, b) => order.indexOf(a) - order.indexOf(b)) }),
  }));
  return { aliases: named.map((model) => model.id), efforts: [], models: named };
}

/**
 * Read `opencode models` as it prints: one `provider/model` id per line and
 * nothing else. Measured 2026-09-03 on opencode-ai 1.18.27.
 *
 * The display name is the half after the first slash, because a picker that
 * shows `opencode/` in front of every row is showing the same word ten times.
 * A `-free` suffix is the runtime's own claim about its billing, so it is
 * repeated as a description rather than interpreted; nothing else is added.
 */
export function parseOpenCodeModelList(text: string): RuntimeModelHints | undefined {
  const models: RuntimeModelName[] = [];
  const lines = text.split(/\r?\n/);
  for (let index = 0; index < lines.length; index += 1) {
    const match = /^([a-z0-9][a-z0-9._-]{0,60})\/([A-Za-z0-9][A-Za-z0-9._-]{0,80})$/.exec(lines[index]!.trim());
    if (match === null) continue;
    const id = `${match[1]!}/${match[2]!}`;
    if (models.some((model) => model.id === id)) continue;
    const displayName = match[2]!;
    const efforts = openCodeVariantsAfter(lines, index);
    models.push({
      id,
      displayName,
      ...(displayName.endsWith("-free") ? { description: "free" } : {}),
      ...(efforts.length === 0 ? {} : { efforts }),
    });
  }
  if (models.length === 0) return undefined;
  return { aliases: models.map((model) => model.id), efforts: [], models };
}

/**
 * The effort words a variant may be offered as: the effort control's own
 * scale. A variant named anything else is a provider's private option, not a
 * reasoning effort, and is not offered.
 */
const OPENCODE_EFFORT_VARIANTS: readonly string[] = ["none", "minimal", "low", "medium", "high", "xhigh", "max"];

/**
 * A model's reasoning efforts, from `opencode models --verbose` (A6.5).
 *
 * That form prints each id followed by the model as a JSON object, and its
 * `variants` are what `run --variant` takes -- "provider-specific reasoning
 * effort, e.g., high, max, minimal" (`run --help`, 1.18.27). MEASURED
 * 2026-09-25 on the free models: Ling 3.0 Flash Fin lists low/medium/high,
 * Muse Spark minimal..xhigh, Space Bunny low..max, the rest none; and the
 * variant reaches the provider -- Ling at `high` reasoned more than at `low`
 * in four runs of four (median ~2,040 tokens against ~730). A variant the
 * model does not list is accepted and silently ignored, so only the listed
 * ones are ever offered. The plain form has no object after the id, and a
 * model read from it simply offers no effort, as before.
 */
function openCodeVariantsAfter(lines: readonly string[], idLine: number): readonly string[] {
  if (lines[idLine + 1]?.trim() !== "{") return [];
  const body: string[] = [];
  for (let index = idLine + 1; index < lines.length; index += 1) {
    body.push(lines[index]!);
    if (lines[index] === "}") break;
  }
  let model: unknown;
  try {
    model = JSON.parse(body.join("\n"));
  } catch {
    return [];
  }
  const variants = typeof model === "object" && model !== null
    ? (model as { variants?: unknown }).variants
    : undefined;
  if (typeof variants !== "object" || variants === null || Array.isArray(variants)) return [];
  // In the scale's order, whatever order the runtime printed them in.
  return OPENCODE_EFFORT_VARIANTS.filter((effort) => Object.hasOwn(variants, effort));
}

/**
 * What Copilot CLI offers before a run has been paid for.
 *
 * There is no `copilot models` command, and the only place model names appear
 * is inside a run, in `session.auto_mode_resolved.data.availableModels` --
 * account-specific, and unreadable without spending a premium request. So the
 * single route offered is the CLI choosing for itself, which is exactly what
 * omitting `--model` does. No model id this build has not been handed is ever
 * named here.
 */
export const COPILOT_MODEL_HINTS: RuntimeModelHints = {
  aliases: ["auto"],
  efforts: [],
  models: [{ id: "auto", displayName: "Auto", description: "Copilot picks the model" }],
};

/**
 * OpenCode's permission config, as an environment value.
 *
 * MEASURED 2026-09-03, and it is the only containment this runtime has. With
 * this set, the write tool is not offered to the model at all: the read-only
 * capture shows the run answering "no write tool is available" and listing the
 * eight tools it did get. Without it, `run` edits files with no flag asked
 * for -- the default `build` agent allows everything, so `--auto` buys
 * nothing and is not passed.
 *
 * `--agent plan` is NOT this. Measured in the same session, plan mode is an
 * instruction the model narrates ("In PLAN MODE -- read-only") and not a rule
 * anything upholds, so it is never what a read-only mission rests on.
 */
/**
 * The read-only denials, in ONE place, because they were in two and drifted.
 *
 * `OPENCODE_READ_ONLY_CONFIG` was corrected on 2026-09-18 from `bash: deny`
 * to `bash: ask` -- see the long note below for the bisect that found it.
 * `opencodeWorktreeConfig` builds the same denials for a worktree run and
 * was NOT corrected, so it kept `bash: "deny"` and kept the 403.
 *
 * The blast radius of that miss: a teammate with its own branch, in Ask or
 * Plan mode, on the free model the first screen recommends -- which is three
 * defaults at once -- answered nothing but
 * "OpenCode's free tier can only be used from within OpenCode".
 *
 * REPRODUCED 2026-09-22 against opencode-ai 1.18.27 on the free model, both
 * ways round, from the two configs this file actually builds: `deny` returns
 * HTTP 403 `FreeTierError`, `ask` answers. Spreading one object means the
 * next correction cannot land on one of them.
 */
/**
 * A refused shell call must not END a read-only run (A6.1).
 *
 * `opencode run` answers every "ask" with a bare reject, and a bare reject
 * stops OpenCode's loop unless this is set (oc/session/processor.ts:200-202 in
 * OpenCode's own source, read 2026-09-24). So a teammate in Ask mode whose
 * model tried `git status` first simply stopped -- 3 of 4 such turns on the
 * 0.315 drive. MEASURED with it set, same config otherwise: `git status` was
 * refused, the model read the file instead, and the run finished with its
 * own stop step, exit 0. "Experimental", and gone in OpenCode's v2 -- so the
 * brief still tells the model the shell is off (A2.20); this is the net
 * under that line, not a replacement for it.
 */
const OPENCODE_KEEP_GOING = { continue_loop_on_deny: true } as const

/**
 * GIT THAT ONLY READS, IN ASK MODE -- NOT THROUGH THE SHELL (0.818).
 *
 * 0.744 let the read-only shell run the git commands that only read (`git
 * status*`, `git diff*` and the rest, each flag that writes asked about again),
 * because the 0.738 sweep's `/review` in Ask mode could not see the change it
 * was asked to review. MEASURED 2026-10-22 against OpenCode 1.18.27 on
 * deepseek-v4.1-flash-free, with this file's own Ask config, one `opencode run`
 * per case in a throwaway repository:
 * - OpenCode checks a command against the patterns with its redirects taken
 *   off: `git status > status.txt` was let through and wrote the file, and
 *   `git log -1 > ../outside.txt` wrote one outside the folder. A pattern
 *   cannot see a redirect, so any allow here is a way to write;
 * - an environment prefix IS part of what is checked: `GIT_EXTERNAL_DIFF=touch
 *   git diff` was asked about whole, and refused;
 * - review subagents put `echo "---DIFF---"` between their git reads, so the
 *   0.817 sweep's `/review` was refused all the same.
 *
 * So the shell refuses everything again ("ask" is a refusal in an unattended
 * run), and git reaches the model through Locust's own git tool, which runs git
 * itself with no shell (withOpenCodeGitReads).
 *
 * EVERY TOOL THESE RULES DO NOT NAME ASKS (0.821). They named the tools that
 * write and left the rest to OpenCode's default, which allows a tool it is not
 * told about -- and a person's own MCP servers load in every run (OPENCODE_PURE
 * drops plugins, not MCP servers), each tool named only by its server. MEASURED
 * 2026-10-22, OpenCode 1.18.27 on deepseek-v4.1-flash-free, this config: a
 * `save_note` tool from an MCP server in the person's own global config ran
 * without asking and wrote note.txt into the folder. So `"*": "ask"` comes
 * first -- OpenCode's last matching rule wins -- and the tools that only read
 * are allowed after it by name (OPENCODE_READING_TOOLS). The same probe, one
 * run per case: the MCP tool "auto-rejecting" and no file written; read, glob,
 * grep, todowrite and a task subagent's read completed; a `.env` refused.
 * Locust's own tools are allowed where they are added (locust_git,
 * locust_browser), after this.
 */
/** A `.env` file is never read where nothing may be asked, and asked about where something may. */
const openCodeRead = (secrets: "ask" | "deny") => ({ "*": "allow", "*.env": secrets, "*.env.*": secrets, "*.env.example": "allow" }) as const;
/**
 * The tools that only read, by OpenCode's own names (its config schema's PermissionConfig, plus 1.18's todoread and
 * codesearch): files, symbols, the run's to-do list, a subagent -- which keeps these same rules -- and a skill.
 */
const OPENCODE_READING_TOOLS = {
  glob: "allow",
  grep: "allow",
  list: "allow",
  lsp: "allow",
  task: "allow",
  todowrite: "allow",
  todoread: "allow",
  skill: "allow",
} as const;
const OPENCODE_READ_ONLY_PERMISSIONS = {
  "*": "ask",
  read: openCodeRead("deny"),
  ...OPENCODE_READING_TOOLS,
  // The web's reads stay what they were in this mode, allowed: they change nothing.
  webfetch: "allow",
  websearch: "allow",
  codesearch: "allow",
  edit: "deny",
  write: "deny",
  bash: "ask",
  patch: "deny",
} as const;

export const OPENCODE_READ_ONLY_CONFIG = JSON.stringify({
  /*
   * `bash` is "ask", NOT "deny", and the difference is whether the free model
   * answers at all.
   *
   * MEASURED 2026-09-18 on Windows, OpenCode 1.18.27, zero credentials, after
   * Grok's passes 9 and 10 got four refusals in a row from Locust's Ask mode
   * and named the mechanism. Bisected, one denial at a time, on
   * `muse-spark-1.3-contributor-free`:
   *
   *     bash: deny                     -> 403 "OpenCode's free tier can only
   *                                       be used from within OpenCode"
   *     edit + write + patch: deny     -> "2"
   *     bash: { "*": "deny" }          -> 403
   *     bash: ask                      -> "2"; and asked to run `echo`, the
   *                                       runtime printed "permission
   *                                       requested: bash; auto-rejecting"
   *                                       and the call ended in error
   *
   * So the provider recognises OpenCode by the tools it offers, and a run
   * with no bash tool is refused as somebody else's client. "ask" keeps the
   * tool on the list and `opencode run`, being non-interactive, rejects
   * every use of it. The write is still refused; it is refused by the
   * runtime at the moment of the call rather than by its absence from the
   * list, and the free model that the first screen promises now answers in
   * the mode a careful person picks first.
   *
   * `external_directory` is stated for the same reason it is stated below:
   * left to the default, a look outside the folder ends the run rather than
   * being refused.
   */
  permission: { ...OPENCODE_READ_ONLY_PERMISSIONS, external_directory: "deny" },
  experimental: OPENCODE_KEEP_GOING,
});

/**
 * What OpenCode is told about the repository a worktree belongs to.
 *
 * `permission.external_directory` takes "ask" | "allow" | "deny", or a map of
 * path patterns to those -- read off OpenCode's own config schema
 * (`opencode.ai/config.json`, `$defs/PermissionConfig`), version 1.18.27,
 * 2026-09-06, rather than guessed.
 *
 * It grants the repository's `.git` and NOTHING ELSE. The first version of
 * this granted the whole parent folder, all three teammates then finished --
 * and one of them wrote its file into the shared folder as well as its own
 * worktree, which is precisely what a worktree exists to prevent (drive,
 * 2026-09-06). A worktree needs the parent's `.git` because its own `.git` is
 * a file pointing there; it does not need the parent's working tree, and
 * being able to reach it is the whole bug.
 */
/**
 * The parent repository's `.git`, as a pattern in the root's own spelling: backslashes for a Windows path,
 * slashes otherwise. It was backslashes everywhere, which names no path on a Mac (2026-10-10 sweep).
 */
export function gitDirPattern(repositoryRoot: string): string {
  return /^[A-Za-z]:[\\/]|\\/.test(repositoryRoot) ? `${repositoryRoot}\\.git\\*` : `${repositoryRoot.replace(/\/+$/, "")}/.git/*`;
}

export function opencodeWorktreeConfig(repositoryRoot: string, readOnly: boolean): string {
  return JSON.stringify({
    permission: {
      // `bash` is "ask" here for the same measured reason it is "ask" in
      // OPENCODE_READ_ONLY_PERMISSIONS: denying it outright makes the free
      // provider refuse the run as somebody else's client. This line said
      // "deny" until 2026-09-22, and it cost every read-only worktree run
      // on the free model -- three defaults at once.
      ...(readOnly ? OPENCODE_READ_ONLY_PERMISSIONS : {}),
      external_directory: { [gitDirPattern(repositoryRoot)]: "allow", "*": "deny" },
    },
    ...(readOnly ? { experimental: OPENCODE_KEEP_GOING } : {}),
  });
}

/**
 * Being told no must not end the run.
 *
 * With no permission config at all -- which is what every ordinary OpenCode
 * mission got -- a request for a directory outside the workspace is answered
 * by OpenCode's default: it prints `permission requested: external_directory
 * (...); auto-rejecting` and the process ENDS without a step that says it
 * stopped. The person is left with a red card and no answer, for a run that
 * merely looked somewhere it was not allowed.
 *
 * MEASURED 2026-09-06, three runs of the same prompt asking a run to list a
 * directory outside its workspace:
 *
 *   no config            -- auto-rejected, run never reached its own end
 *   external_directory   -- refused cleanly, model SAID it could not, and the
 *     : "deny"              run finished normally
 *   external_directory   -- allowed, which is not what a workspace-write run
 *     : "allow"             should get
 *
 * So the denial is stated rather than left to the default. The run is
 * confined exactly as before; what changes is that it comes back.
 */
export const OPENCODE_CONFINED_CONFIG = JSON.stringify({
  permission: { external_directory: "deny" },
});

/**
 * What an Auto run is told: it may leave the folder, which is what Auto is for.
 *
 * Auto used to carry OPENCODE_CONFINED_CONFIG, on the reading that `--auto`
 * would approve the rest. It does not override an explicit deny -- "auto-
 * approve permissions that are not explicitly denied" -- so Auto stayed in the
 * folder (a B4 lead). MEASURED 2026-09-25, opencode `run --auto` on the free
 * `ling-3.0-flash-fin-free`, asked to read a file outside its folder:
 *
 *   external_directory: "deny"   -> "prevents you from using this specific
 *                                    tool call", the file never read
 *   external_directory: "allow"  -> the file's contents, verbatim
 *
 * Stated rather than left to `--auto`'s default for the same reason the
 * denial is stated: nothing about where a run may go is left implicit. It
 * applies in a worktree as well, as danger-full-access does for Codex there --
 * the person chose Auto for that teammate knowing it may leave its folder.
 */
export const OPENCODE_AUTO_CONFIG = JSON.stringify({
  permission: { external_directory: "allow" },
});

/**
 * What OpenCode is told for a mode, as environment: its permission config and, for a read-only run, no plugins.
 * One function for `run` and the server (0.677), so a mode means the same thing on either route.
 */
export function openCodeModeEnv(options: { readonly sandbox?: RuntimeCommandOptions["sandbox"]; readonly workspacePath?: string; readonly repositoryRoot?: string; readonly providers?: RuntimeCommandOptions["providers"]; readonly openCodeGuardPlugin?: string; readonly browserMcp?: BrowserMcpSpec; readonly gitReads?: GitReadsSpec; readonly referenceFolders?: readonly string[]; readonly skillsDir?: string; readonly skillPaths?: readonly string[] }): Readonly<Record<string, string>> {
  const auto = sandboxArgument(options.sandbox) === "full-access";
  const readOnly = sandboxArgument(options.sandbox) === "read-only";
  // A worktree run needs its parent repository; a read-only one still needs
  // the denials. When both apply the config carries both, because the two
  // used to be written into the same environment variable and the second
  // would simply have replaced the first. Auto reaches past both.
  const config = auto
    ? OPENCODE_AUTO_CONFIG
    : options.repositoryRoot !== undefined
      ? opencodeWorktreeConfig(options.repositoryRoot, readOnly)
      : readOnly
        ? OPENCODE_READ_ONLY_CONFIG
        : OPENCODE_CONFINED_CONFIG;
  // A6.2: a read-only run loads no plugins. A repo's .opencode/plugin/*.ts
  // runs in OpenCode's own process, which would put code the repository
  // chose outside everything the permission config holds back
  // (OPENCODE_PURE, "run without external plugins"; plugin/index.ts:181).
  // Only for read-only runs: a run that may edit is already trusted with
  // the folder, and the person's own plugins are theirs to have there.
  // Read-only mode already refuses edits and shell writes. Auto already has access, so leave its config alone.
  const parsed = JSON.parse(config) as { permission: Record<string, unknown> };
  if (readOnly && (options.referenceFolders?.length ?? 0) > 0) {
    const old = parsed.permission.external_directory;
    parsed.permission.external_directory = {
      "*": "deny",
      ...(typeof old === "object" && old !== null ? old : {}),
      ...Object.fromEntries(options.referenceFolders!.flatMap((folder) => {
        const path = requireText(folder, "Reference folder").replace(/\\/g, "/").replace(/\/+$/, "");
        const spellings = /^[A-Za-z]:\//.test(path) ? [path, path.replace(/\//g, "\\")] : [path];
        return spellings.flatMap((spelling) => [[spelling, "allow"], [`${spelling}${spelling.includes("\\") ? "\\" : "/"}**`, "allow"], [`${spelling}${spelling.includes("\\") ? "\\" : "/"}*`, "allow"]]);
      })),
    };
  }
  const configured = withOpenCodeSkills(withOpenCodeBrowserMcp(
    withOpenCodeGuard(withoutOpenCodeQuestionTool(withOpenCodeProviders(JSON.stringify(parsed), options.providers)), options.openCodeGuardPlugin),
    options.browserMcp,
  ), options.skillsDir, options.skillPaths);
  return {
    // Only a run with no shell gets the git tool: any other mode's shell runs git itself.
    OPENCODE_CONFIG_CONTENT: readOnly && options.workspacePath !== undefined ? withOpenCodeGitReads(configured, options.gitReads, options.workspacePath) : configured,
    ...(readOnly ? { OPENCODE_PURE: "1" } : {}),
  };
}

/**
 * LOCUST'S GIT TOOL, FOR A RUN WITH NO SHELL (0.818): `locust_git`, a stdio MCP
 * server OpenCode starts for this run alone, in its folder
 * (resources/locust-git-reads.mjs). It runs git with no shell and checks every
 * argument, so nothing a model sends can redirect, chain or add an option.
 *
 * MEASURED 2026-10-22 on OpenCode 1.18.27, deepseek-v4.1-flash-free, this Ask
 * config with the shell refusing everything:
 * - OPENCODE_PURE leaves an MCP server alone (it drops plugins only), and the
 *   free tier answered with the tool listed;
 * - asked what the uncommitted change was, the model called
 *   `locust_git_status` and `locust_git_diff` and tried no shell command;
 * - `/review`'s subagent tried the shell first, as OpenCode's review prompt
 *   says to, and fell back to reading the file. With one line in
 *   `instructions`, which every agent of a run reads, the subagent and the
 *   main agent both went to `locust_git_*` and tried no shell command;
 * - a config's `instructions` are added to the person's and the project's
 *   (`opencode debug config`: global, project, then this one), not put in
 *   their place.
 */
export function withOpenCodeGitReads(config: string, gitReads: GitReadsSpec | undefined, folder: string): string {
  if (gitReads === undefined) return config;
  const parsed = JSON.parse(config) as Record<string, unknown>;
  const mcp = (parsed.mcp as Record<string, unknown> | undefined) ?? {};
  const permission = (parsed.permission as Record<string, unknown> | undefined) ?? {};
  const instructions = Array.isArray(parsed.instructions) ? parsed.instructions : [];
  return JSON.stringify({
    ...parsed,
    mcp: {
      ...mcp,
      locust_git: {
        type: "local",
        command: [requireText(gitReads.command, "Git tool command"), ...gitReads.args],
        environment: { ...gitReads.env, LOCUST_GIT_FOLDER: requireText(folder, "Folder") },
        enabled: true,
      },
    },
    permission: { ...permission, "locust_git_*": "allow" },
    ...(gitReads.instructions === undefined ? {} : { instructions: [...instructions, gitReads.instructions] }),
  });
}

/** A run's own skills for OpenCode 1 (0.813): its folder in the config's `skills.paths`, or the config as it was. */
export function withOpenCodeSkills(config: string, dir: string | undefined, paths: readonly string[] = []): string {
  if (dir === undefined || dir.length === 0) return config;
  const parsed = JSON.parse(config) as Record<string, unknown>;
  const skills = typeof parsed.skills === "object" && parsed.skills !== null ? parsed.skills as Record<string, unknown> : {};
  const oldPaths = Array.isArray(skills.paths) ? skills.paths.filter((path): path is string => typeof path === "string") : [];
  const permission = parsed.permission as Record<string, unknown> | undefined;
  const external = permission?.external_directory;
  // The native skill loader can read SKILL.md; the read tool still needs the copy's supporting files.
  // Grant just this run's subtree, after the default denial, without changing edit or shell permissions.
  const path = requireText(dir, "Skills folder").replace(/\\/g, "/").replace(/\/+$/, "");
  const spellings = /^[A-Za-z]:\//.test(path) ? [path, path.replace(/\//g, "\\")] : [path];
  const allow = Object.fromEntries(spellings.flatMap((spelling) => [[spelling, "allow"], [`${spelling}${spelling.includes("\\") ? "\\" : "/"}**`, "allow"]]));
  return JSON.stringify({
    ...parsed,
    skills: { ...skills, paths: [...new Set([...paths, ...oldPaths, requireText(dir, "Skills folder")])] },
    ...(permission === undefined || external === "allow" ? {} : { permission: { ...permission, external_directory: { ...(typeof external === "object" && external !== null ? external : { "*": external ?? "ask" }), ...allow } } }),
  });
}

/**
 * OPENCODE'S OWN QUESTION TOOL, OFF (0.700).
 *
 * OpenCode gives a model a `question` tool whenever its client is `app`, `cli`
 * or `desktop` -- and every command, `serve` included, is `cli` unless told
 * otherwise. A model asked to check something with the person calls it, the
 * server says `question.asked`, and the turn waits for a reply on
 * `/question/{id}/reply` that Locust never sends: the teammate read "Working…"
 * until the person pressed Stop (drive-needs-you: 11 minutes). MEASURED
 * 2026-10-07 on `opencode serve` 1.18.27, nemotron-3-ultra-free, the drive's
 * own prompt: with the tool, 3 of 3 asked through it and were still waiting at
 * 60-90 s; without it, 3 of 3 finished in 11-23 s. Every other agent asks with
 * Locust's question block, which ends the turn and draws the question card;
 * without the tool OpenCode's models do the same. Merged into any tools map
 * already there (a chat-only model's `"*": false` stays).
 */
export function withoutOpenCodeQuestionTool(config: string | undefined): string {
  const parsed = config === undefined ? {} : (JSON.parse(config) as Record<string, unknown>);
  const tools = (parsed.tools as Record<string, unknown> | undefined) ?? {};
  return JSON.stringify({ ...parsed, tools: { ...tools, question: false } });
}

/**
 * Locust's command guard, added to a config's `plugin` list (0.722). OpenCode
 * adds that list to the person's own rather than replacing it (MEASURED
 * 2026-10-10, 1.18.27), so their plugins still load beside it. A read-only
 * run is OPENCODE_PURE and loads none, this one included; it runs no commands.
 */
export function withOpenCodeGuard(config: string, plugin: string | undefined): string {
  if (plugin === undefined) return config;
  const parsed = JSON.parse(config) as Record<string, unknown>;
  const plugins = Array.isArray(parsed.plugin) ? parsed.plugin : [];
  return JSON.stringify({ ...parsed, plugin: [...plugins, requireText(plugin, "Command guard plugin")] });
}

export function withOpenCodeBrowserMcp(config: string, browserMcp: BrowserMcpSpec | undefined): string {
  if (browserMcp === undefined) return config;
  const parsed = JSON.parse(config) as Record<string, unknown>;
  const mcp = (parsed.mcp as Record<string, unknown> | undefined) ?? {};
  const permission = (parsed.permission as Record<string, unknown> | undefined) ?? {};
  return JSON.stringify({
    ...parsed,
    mcp: {
      ...mcp,
      locust_browser: {
        type: "local",
        command: [requireText(browserMcp.command, "Browser MCP command"), ...browserMcp.args],
        environment: browserMcp.env,
        enabled: true,
      },
    },
    permission: {
      ...permission,
      "locust_browser_*": "allow",
    },
  });
}

/**
 * OpenCode in its non-interactive `run` mode.
 *
 * The prompt goes on STDIN. It went in argv until 2026-09-17, on the
 * grounds that nothing in this repo had ever seen `opencode run` read a
 * prompt from stdin. Then the first Chief of Staff exchange lost its last
 * hop: the reply into the person's conversation -- a 1,200-character share
 * quoted inside a 2,215-character standing brief plus the relay brief --
 * was refused by `command-length.ts` at about 7,500 characters, because
 * `cmd.exe` stops at 8,191 and this app reaches the npm `.cmd` shim through
 * it. The person never got the report.
 *
 * MEASURED the same hour, against the real CLI on the free model:
 *
 *     echo "Reply with exactly the single word PEBBLE" | opencode run --format json
 *     -> "text":"PEBBLE"
 *
 * With no positional, `opencode run` reads the message from stdin, and the
 * JSON event stream is the same. So the prompt travels the way it does for
 * Codex and Claude, the argv ceiling no longer applies, and `commandTooLong`
 * skips this runtime by its own rule. The option is still required, because
 * a run with nothing to say is a mission that hangs.
 *
 * The working directory is the workspace and there is no flag for it: OpenCode
 * takes the directory it is spawned in.
 */
/**
 * Where the runner substitutes the path of the file it wrote the prompt into.
 *
 * A literal rather than a positional index, so a builder can put it anywhere
 * in its argument list and a reader of the spec can see what it is. Chosen to
 * be something no real path could be.
 */
export const PROMPT_FILE_PLACEHOLDER = "<locust:prompt-file>";

/**
 * Meta's Muse Code, headless.
 *
 * MEASURED on Windows 2026-09-21 against 1.3.0-R3401.1, captures in
 * `docs/muse-probe-2026-09-21/`. Every flag below was read from
 * `muse exec --help` on the installed binary, not from documentation.
 *
 * THE PROMPT GOES IN A FILE, and that is not a preference:
 *
 *   - piping it in exits 2 with `usage: muse exec [OPTIONS] [PROMPT]`, so
 *     stdin is not an option the way it is for OpenCode;
 *   - `muse` on PATH is `muse.cmd`, a PowerShell launcher that is neither npm
 *     shim shape `path-locator` unwraps, so it keeps its shell and cmd.exe
 *     caps the command line at 8,191 characters.
 *
 * Locust routinely sends more than that -- a peer reply quoted inside a
 * standing brief reached ~7,500 characters and was refused on OpenCode in
 * 2026-09-17, and the person never got their report. `--prompt-file` is the
 * escape Muse offers, and taking it also lifts `commandTooLong`, which only
 * limits specs whose prompt is in argv.
 *
 * READ-ONLY IS REAL HERE, which is worth saying because it is the thing
 * Cursor cannot give us on Windows. `--disable-write` stops non-shell
 * workspace writes and `--disable-shell` stops shell execution; both are
 * flags on the binary rather than a config file we have to smuggle in.
 *
 * Approval and the sandbox are ON by default. A headless run cannot answer an
 * approval prompt, so the mode that would ask is set to one that does not --
 * `--approval-mode never` for a run allowed to write, which is what
 * `accept-edits` means, and the read-only flags above for one that is not.
 * `--yolo` is deliberately never passed: it also disables the sandbox and
 * trusts the workspace, which is three decisions hiding in one flag.
 *
 * AUTO ADDS `--disable-sandbox` (0.773), and only Auto: the guard refuses it
 * anywhere else. On Windows Muse's sandbox needs an elevated
 * `muse sandbox windows setup` (local sandbox users and a WFP filter) before
 * any command runs, and until then every shell task fails before it starts --
 * Muse Wonder wrote its voxel page (2026-10-13) without one command. Auto is
 * Codex's `danger-full-access` and Claude's `bypassPermissions`: the person
 * chose a run that may do anything, so a box that was never built is not
 * what stops it. MEASURED 2026-10-15 on 1.4.7: `--approval-mode never
 * --disable-sandbox` ran `Write-Output hello` to exit 0 while
 * `muse sandbox windows check` still said setup_required.
 */
export function createMuseExecCommand(
  executable: ExecutableLaunch,
  options: RuntimeCommandOptions,
): RuntimeCommandSpec {
  const sandbox = sandboxArgument(options.sandbox);
  requireText(options.prompt ?? "", "Prompt");
  const args = ["exec", "--json", "--prompt-file", PROMPT_FILE_PLACEHOLDER];
  // The workspace is named as well as entered. `--workspace PATH` is what
  // roots the policy-gated tools; the measured run printed
  // `muse: workspace root: ... (explicit)` for it.
  args.push("--workspace", requireText(options.workspacePath, "Workspace path"));
  if (options.model !== undefined) {
    args.push("--model", requireText(options.model, "Model"));
  }
  if (options.effort !== undefined) {
    // Measured from `muse exec --help`: none|minimal|low|medium|high|xhigh|
    // max|ultra. An effort outside that set is refused here rather than
    // passed on, because the CLI would reject the whole run for it.
    const effort = requireText(options.effort, "Effort");
    if (!MUSE_EFFORTS.includes(effort)) {
      throw new Error(`Muse Code takes no reasoning effort called "${effort}"`);
    }
    args.push("--reasoning-effort", effort);
  }
  if (options.resumeThreadId !== undefined) {
    args.push("--session-id", requireText(options.resumeThreadId, "Session id"));
  }
  if (sandbox === "read-only") {
    args.push("--disable-write", "--disable-shell");
  } else {
    // Nothing can answer a prompt in a headless run, and the default
    // `on-request` would wait for one. The sandbox is left ON, except in Auto.
    args.push("--approval-mode", "never");
    if (sandbox === "full-access") args.push("--disable-sandbox");
  }
  return baseSpec("muse", executable, options.workspacePath, args, {
    stdin: "prompt-file",
    sandbox,
  });
}

/** The reasoning efforts `muse exec --help` lists, measured 2026-09-21; the same on 1.4.6 (2026-10-12). */
export const MUSE_EFFORTS: readonly string[] = [
  "none",
  "minimal",
  "low",
  "medium",
  "high",
  "xhigh",
  "max",
  "ultra",
];

/** An OpenAI-compatible endpoint, as OpenCode is told of it. */
export interface OpenCodeProvider {
  /** What the person called it; OpenCode shows it as the provider's name. */
  readonly name: string;
  /** The endpoint's base address, ending before `/chat/completions`. */
  readonly baseUrl: string;
  /** Absent for an endpoint that asks for none. */
  readonly apiKey?: string;
  /** The model ids it serves that runs may name. */
  readonly models: readonly string[];
  /**
   * False for a model that only chats (0.358). OpenCode sends every model
   * its tools -- ten of them, `tool_choice: "auto"` -- and a model or server
   * without tool support answers 400 "does not support tools" and the run
   * ends. Declaring the model without them in OpenCode's own model config
   * changes nothing (MEASURED 2026-09-26: still ten tools); turning every
   * tool off for the run does -- no `tools`, no `tool_choice`, and the same
   * model answers. Absent means it takes tools, as every model did before.
   */
  readonly toolCalls?: boolean;
  /**
   * What a model costs, by its id, as its provider lists it (0.810). OpenCode
   * is told it, and the run's steps are priced from it (listedStepCost);
   * absent, the run reports tokens only.
   */
  readonly prices?: Readonly<Record<string, OpenCodePrice>>;
}

/**
 * A model's price in dollars a million tokens, in models.dev's own words,
 * which are the ones OpenCode's config takes. A tier is a dearer price for a
 * step whose context is over its size. OpenCode 1.18.27 keeps only the base
 * of a price its config gives (MEASURED 2026-10-21, probe-opencode-own-price),
 * so the tiers are applied by Locust, which prices each step itself.
 */
export interface OpenCodePrice {
  readonly input: number;
  readonly output: number;
  readonly cache_read?: number;
  readonly cache_write?: number;
  readonly tiers?: readonly {
    readonly input: number;
    readonly output: number;
    readonly cache_read?: number;
    readonly cache_write?: number;
    readonly tier: { readonly type: "context"; readonly size: number };
  }[];
  readonly context_over_200k?: { readonly input: number; readonly output: number; readonly cache_read?: number; readonly cache_write?: number };
}

const PROVIDER_ID = /^[a-z0-9][a-z0-9-]{0,47}$/;

/**
 * The run's config with the person's own providers declared in it.
 *
 * `@ai-sdk/openai-compatible` is the package OpenCode's own docs name for
 * any OpenAI-compatible endpoint -- vLLM, Ollama's /v1, LM Studio, a
 * company's gateway -- and the shape MEASURED working through this very
 * builder on 2026-09-25 (_tools/probe-opencode-rate-limit.mjs). An endpoint
 * that asks for no key is still sent a placeholder: the SDK refuses to
 * start without one, and a server that checks none ignores it.
 */
export function withOpenCodeProviders(config: string | undefined, providers: Readonly<Record<string, OpenCodeProvider>> | undefined): string | undefined {
  const entries = Object.entries(providers ?? {});
  if (entries.length === 0) return config;
  const parsed = config === undefined ? {} : (JSON.parse(config) as Record<string, unknown>);
  const declared: Record<string, unknown> = {};
  for (const [id, provider] of entries) {
    if (!PROVIDER_ID.test(id)) throw new Error(`"${id}" is not a provider id OpenCode can take`);
    if (!/^https?:\/\/[^\s]+$/i.test(provider.baseUrl)) throw new Error(`${provider.name}'s address is not an http(s) address`);
    declared[id] = {
      npm: "@ai-sdk/openai-compatible",
      name: provider.name,
      options: { baseURL: provider.baseUrl, apiKey: provider.apiKey ?? "not-needed" },
      models: Object.fromEntries(
        provider.models.map((model) => {
          const price = provider.prices?.[model];
          return [model, { name: model, ...(price === undefined ? {} : { cost: price }) }];
        }),
      ),
    };
  }
  // A run on a model that only chats is a run with no tools at all: it can
  // talk, and it can neither read nor change a file.
  const chatOnly = entries.some(([, provider]) => provider.toolCalls === false);
  return JSON.stringify({
    ...parsed,
    provider: { ...((parsed.provider as Record<string, unknown> | undefined) ?? {}), ...declared },
    ...(chatOnly ? { tools: { "*": false } } : {}),
  });
}

export function createOpenCodeRunCommand(
  executable: ExecutableLaunch,
  options: RuntimeCommandOptions,
): RuntimeCommandSpec {
  // `--print-logs --log-level ERROR`: the one place OpenCode says it is
  // retrying. MEASURED 2026-09-25 against a local provider answering 429:
  // `run` retried nine times over twelve seconds with nothing on stdout or
  // stderr, then printed an `error` record and exited 1 -- and a real free
  // model's longer retry-after is a run that reads "Starting" for minutes.
  // At ERROR each attempt is one stderr line, `message="stream error" ...
  // agent=build ... error.error="AI_APICallError: Rate limit exceeded"`, which
  // the spec forwards as records (stderrRecords). ERROR only: WARN names
  // every duplicate skill on the machine.
  const args = ["run", "--format", "json", "--print-logs", "--log-level", "ERROR"];
  if (options.slashCommand !== undefined) {
    if (!SLASH_COMMAND_NAME.test(options.slashCommand)) throw new Error(`"${options.slashCommand}" is not a command name`);
    args.push("--command", options.slashCommand);
  }
  if (options.model !== undefined) {
    args.push("-m", requireText(options.model, "Model"));
  }
  if (options.effort !== undefined) {
    // A model's effort is its variant (A6.5). Only an effort word: the
    // runtime accepts any name and silently ignores one it does not know, so
    // anything else would be a setting shown as applied and never applied.
    const effort = requireText(options.effort, "Effort");
    if (!OPENCODE_EFFORT_VARIANTS.includes(effort)) {
      throw new Error(`OpenCode takes no effort level called "${effort}"`);
    }
    args.push("--variant", effort);
  }
  if (options.resumeThreadId !== undefined) {
    args.push("-s", requireText(options.resumeThreadId, "Session id"));
    if (options.forkSession === true) args.push("--fork");
  } else {
    // A6.3: a new session names itself with a second model call -- the
    // `title` agent, on the same model -- unless it is given a title (read
    // in OpenCode's session/prompt.ts, 2026-09-24). Locust names missions
    // itself, so that call is pure cost against a free model's rate limit.
    args.push("--title", "Locust");
  }
  const auto = sandboxArgument(options.sandbox) === "full-access";
  if (auto) {
    // "auto-approve permissions that are not explicitly denied" -- opencode
    // run --help, measured 2026-09-06.
    args.push("--auto");
  }
  requireText(options.prompt ?? "", "Prompt");
  const env = openCodeModeEnv(options);
  return baseSpec("opencode", executable, options.workspacePath, args, {
    stdin: "prompt",
    sandbox: sandboxArgument(options.sandbox),
    ...(Object.keys(env).length === 0 ? {} : { env }),
    stderrRecords: true,
  });
}

/**
 * OpenCode's own server, for a run that stops and asks (A6.7): Approve-each.
 *
 * Fixed arguments only -- an ephemeral port on 127.0.0.1, never `--mdns`,
 * which would announce it on the network. The password is not here: the
 * transport makes one per server and passes it in the child's environment,
 * so no spec, argv or record ever carries it.
 *
 * The permission config asks for everything that acts: an edit, a shell
 * command, a fetch, a folder outside the run's own. Measured 2026-09-25:
 * under "ask" the server raises `permission.asked` and waits for the answer,
 * where `run` could only reject. A worktree still reaches its own `.git`
 * without asking, as every OpenCode worktree run does.
 */
export function createOpenCodeServeCommand(
  executable: ExecutableLaunch,
  options: {
    readonly referenceFolders?: readonly string[];
    readonly workspacePath: string;
    readonly repositoryRoot?: string;
    /** The person's own models, as for a run (withOpenCodeProviders). */
    readonly providers?: Readonly<Record<string, OpenCodeProvider>>;
    /**
     * Any mode but Approve each (0.677): the server is told what `run` is told for that mode (openCodeModeEnv), so
     * a reply can stream through it without the mode meaning anything new. Absent: Approve each, which asks for
     * everything that acts.
     */
    readonly sandbox?: RuntimeCommandOptions["sandbox"];
    /** Locust's command guard, in every mode (0.722, withOpenCodeGuard). */
    readonly openCodeGuardPlugin?: string;
    /** Per-run browser MCP server (`locust_browser`). */
    readonly browserMcp?: BrowserMcpSpec;
    /** Locust's git tool, for a mode with no shell (0.818, `RuntimeCommandOptions.gitReads`). */
    readonly gitReads?: GitReadsSpec;
    /** The run's own skills (0.813, `RuntimeCommandOptions.skillsDir`). */
    readonly skillsDir?: string;
    readonly skillPaths?: readonly string[];
  },
): RuntimeCommandSpec {
  if (options.sandbox !== undefined) {
    return baseSpec("opencode", executable, options.workspacePath, ["serve", "--port", "0", "--hostname", "127.0.0.1"], {
      stdin: "protocol",
      sandbox: sandboxArgument(options.sandbox),
      env: openCodeModeEnv(options),
    });
  }
  const config = JSON.stringify({
    permission: {
      // Every tool not named here asks too (0.821): a person's own MCP tool ran with no card, as it did in Ask
      // (OPENCODE_READ_ONLY_PERMISSIONS). Reading is never a card; a `.env` file is. A web search stays what
      // "What Locust can stop" says it is in every mode, not asked about (shared/what-locust-can-stop.ts).
      "*": "ask",
      read: openCodeRead("ask"),
      ...OPENCODE_READING_TOOLS,
      edit: "ask",
      bash: "ask",
      webfetch: "ask",
      websearch: "allow",
      codesearch: "allow",
      external_directory: options.repositoryRoot === undefined
        ? "ask"
        : { [gitDirPattern(options.repositoryRoot)]: "allow", "*": "ask" },
    },
  });
  return baseSpec("opencode", executable, options.workspacePath, ["serve", "--port", "0", "--hostname", "127.0.0.1"], {
    stdin: "protocol",
    sandbox: "workspace-write",
    env: {
      OPENCODE_CONFIG_CONTENT: withOpenCodeSkills(withOpenCodeBrowserMcp(
        withOpenCodeGuard(withoutOpenCodeQuestionTool(withOpenCodeProviders(config, options.providers) ?? config), options.openCodeGuardPlugin),
        options.browserMcp,
      ), options.skillsDir, options.skillPaths),
    },
  });
}

/**
 * Copilot's tools that a Locust teammate is never given (0.378).
 *
 * `session_store_sql` queries Copilot's LOCAL SESSION STORE -- every Copilot
 * conversation on the machine, not only this teammate's -- and Copilot marks
 * it safe, so it runs without asking, even in Approve each. MEASURED
 * 2026-09-26 (drive-copilot-approve-each, first run): a cold follow-up made
 * twelve calls to it and answered with a word from an unrelated session. A
 * teammate knows its own conversation; the person's other Copilot history
 * does not belong in a Locust thread, or a room. Excluded, the model is not
 * offered the tool at all -- rather than asking for it and being refused.
 */
export const COPILOT_EXCLUDED_TOOLS = ["session_store_sql"] as const;
const COPILOT_EXCLUDED = `--excluded-tools=${COPILOT_EXCLUDED_TOOLS.join(",")}`;

/**
 * GitHub Copilot CLI, non-interactive.
 *
 * `--allow-all-tools` is not a bypass here, it is the only way to run without
 * a terminal: without it the CLI waits for an approval no host can give. What
 * holds the run back is `--deny-tool=write,shell`, measured refusing
 * `apply_patch` with "Permission to run this tool was denied due to the
 * following rules: `write`" and PowerShell with "rules: `shell`", leaving no
 * file behind.
 *
 * The session id is the host's, passed in on the first run. Copilot prints one
 * of its own in the terminal `result` record, but only there -- a run that
 * fails earlier would print none, and a mission that cannot be continued after
 * a failure is the one case where continuing matters most.
 */
export function createCopilotPromptCommand(
  executable: ExecutableLaunch,
  options: RuntimeCommandOptions,
): RuntimeCommandSpec {
  const args = [
    "-p",
    requireText(options.prompt ?? "", "Prompt"),
    "--output-format",
    "json",
    "--allow-all-tools",
    // Colour codes would land in the JSON strings this adapter parses.
    "--no-color",
    COPILOT_EXCLUDED,
  ];
  if (sandboxArgument(options.sandbox) === "read-only") {
    args.push("--deny-tool=write,shell");
    for (const folder of options.referenceFolders ?? []) args.push("--add-dir", requireText(folder, "Reference folder"));
  }
  if (sandboxArgument(options.sandbox) === "full-access") {
    // "Disable file path verification and allow" access outside the workspace
    // -- copilot --help, measured 2026-09-06. `--allow-all` would do this and
    // open every URL as well, which is a different question; it stays refused.
    args.push("--allow-all-paths");
  }
  if (options.model !== undefined) {
    args.push("--model", requireText(options.model, "Model"));
  }
  if (options.effort !== undefined) {
    // `--effort, --reasoning-effort <level>`, choices none/minimal/low/medium/
    // high/xhigh/max (copilot 1.0.83, measured 2026-09-08). This used to throw
    // "Copilot CLI takes no effort level", which was simply untrue -- and the
    // discovery side never read its choices either, so the composer said the
    // effort was fixed. Both halves are fixed together; either alone would
    // leave the app offering a level it refuses to send, or refusing one it
    // offers.
    args.push("--effort", requireEffort(options.effort));
  }
  if (options.resumeThreadId !== undefined) {
    // Measured: the resume flag takes its value with `=`, and the run recalled
    // what the first turn had been told.
    args.push(`--resume=${requireText(options.resumeThreadId, "Session id")}`);
  } else if (options.sessionId !== undefined) {
    args.push("--session-id", requireText(options.sessionId, "Session id"));
  }
  // No teammate ends the person's browser (0.721, main/command-guard.ts).
  if (options.skillPlugin !== undefined) {
    args.push("--plugin-dir", requireText(options.skillPlugin, "Skills plugin"));
  }
  if (options.guardPlugin !== undefined) {
    args.push("--plugin-dir", requireText(options.guardPlugin, "Command guard plugin"));
  }
  if (options.browserMcp !== undefined) {
    const mcpPath = requireText(options.browserMcp.copilotConfigPath ?? options.browserMcp.configPath, "Browser MCP config");
    args.push("--additional-mcp-config", `@${mcpPath}`);
  }
  return baseSpec("copilot", executable, options.workspacePath, args, { stdin: "none", sandbox: sandboxArgument(options.sandbox) });
}

/**
 * GitHub Copilot CLI as an Agent Client Protocol server, for Approve each
 * (0.377; acp-run.ts). The same CLI and the same model rule as the print
 * route, and none of the flags that exist only because print mode cannot
 * ask: no `--allow-all-tools` -- asking is the point -- and no prompt or
 * session id on the argv, since both travel over the protocol. MEASURED
 * 2026-09-26 on copilot 1.0.88: `--acp` starts with `--model` and `--effort`
 * beside it (though a model it does not know is not refused -- the turn ran
 * anyway -- so only a model the picker offered is ever passed).
 */
export function createCopilotAcpCommand(
  executable: ExecutableLaunch,
  options: RuntimeCommandOptions,
): RuntimeCommandSpec {
  const args = ["--acp", COPILOT_EXCLUDED];
  if (options.model !== undefined) {
    args.push("--model", requireText(options.model, "Model"));
  }
  // The effort the person chose, as the print route sends it (the 2026-10-10 sweep: it never reached either route).
  // MEASURED on 1.0.95: `--effort` is still taken beside `--reasoning-effort`, and a value it does not know stops it.
  if (options.effort !== undefined) {
    args.push("--effort", requireEffort(options.effort));
  }
  // The hook refuses before anyone is asked (MEASURED over --acp, 1.0.95).
  if (options.skillPlugin !== undefined) {
    args.push("--plugin-dir", requireText(options.skillPlugin, "Skills plugin"));
  }
  if (options.guardPlugin !== undefined) {
    args.push("--plugin-dir", requireText(options.guardPlugin, "Command guard plugin"));
  }
  return baseSpec("copilot", executable, options.workspacePath, args, { stdin: "protocol", sandbox: "workspace-write" });
}

/**
 * An agent the person added (0.777): their own command line, run as an Agent
 * Client Protocol server in the folder. Nothing of Locust's goes on the argv --
 * the prompt, the session and the model all travel over the protocol -- and the
 * person's own arguments meet the same refusals every runtime's do, so a flag
 * that would skip the agent's asking is not run.
 */
export function createAcpAgentCommand(
  executable: ExecutableLaunch,
  args: readonly string[],
  workspacePath: string,
): RuntimeCommandSpec {
  return baseSpec("acp", executable, workspacePath, args.map((arg) => requireText(arg, "Argument")), { stdin: "protocol", sandbox: "workspace-write" });
}

/**
 * What an Approve-each session on Copilot must be, by Copilot's own ids
 * (session/new, copilot 1.0.88): its Agent mode -- which asks -- never Plan or
 * Autopilot ("enables allow-all"), and its `allow_all` switch off. A loaded
 * session in another state is put back; one that cannot be is not run.
 */
export const COPILOT_ACP_SESSION = {
  modeId: "https://agentclientprotocol.com/protocol/session-modes#agent",
  requiredConfig: { allow_all: "off" },
} as const;
