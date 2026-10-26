import { stat } from "node:fs/promises";
import { homedir } from "node:os";
import { dirname, join, sep } from "node:path";

import {
  CLAUDE_REQUIRED_FEATURES,
  CODEX_REQUIRED_FEATURES,
  COPILOT_MODEL_HINTS,
  COPILOT_REQUIRED_FEATURES,
  CURSOR_REQUIRED_FEATURES,
  detectSupportedFeatures,
  GEMINI_REQUIRED_FEATURES,
  OMNIROUTE_REQUIRED_FEATURES,
  MUSE_REQUIRED_FEATURES,
  OPENCODE_REQUIRED_FEATURES,
  parseClaudeModelHints,
  parseEffortChoices,
  parseCursorModelList,
  parseOpenCodeModelList,
} from "./commands.js";
import type {
  CommandResult,
  CommandRunner,
  ExecutableLaunch,
  ExecutableLocator,
  ParsedRuntimeVersion,
  ProbeCommand,
  RuntimeDiagnostic,
  RuntimeDiscovery,
  RuntimeFeature,
  DiscoveredRuntimeId,
  RuntimeIntegrationId,
  RuntimeIntegrationKind,
  RuntimeModelHints,
  RuntimeReadiness,
} from "./types.js";
import { parseRuntimeVersion } from "./version.js";

interface IntegrationDefinition {
  readonly id: DiscoveredRuntimeId;
  readonly kind: RuntimeIntegrationKind;
  readonly displayName: string;
  readonly commandName: string;
  readonly optional: boolean;
  readonly versionArgs: readonly string[];
  readonly capabilityArgs: readonly string[];
  readonly readinessArgs: readonly string[];
  readonly requiredFeatures: readonly RuntimeFeature[];
  /**
   * A version this Locust cannot run yet, said in words instead of a turn that fails (0.805). Undefined: it can.
   */
  readonly unsupportedVersion?: (version: ParsedRuntimeVersion) => { readonly message: string; readonly resolution: string } | undefined;
  /**
   * Whether a readiness probe that EXITED cleanly actually reported ready.
   * Most CLIs say so with their exit code; one says "Not logged in" and
   * exits 0, so its text has to be read. Absent means the exit code is the
   * whole answer.
   */
  readonly readyWhen?: (result: CommandResult) => boolean;
  /** A command that prints the runtime's model list, run only once it is ready. */
  readonly modelsArgs?: readonly string[];
  /**
   * The model list command also says whether the runtime is ready (OpenCode:
   * the list it prints IS the readiness answer), so when it is asked, the
   * plain readiness command is asked only if the list did not answer. One
   * process start where there were two.
   */
  readonly listAnswersReadiness?: true;
  /** How that list is read. Each CLI prints its own shape; none is guessed. */
  readonly parseModels?: (text: string) => RuntimeModelHints | undefined;
  /**
   * Routes a runtime offers without any list command to ask. Only for a CLI
   * that has none: the alternative is naming models nobody printed.
   */
  readonly fixedModelHints?: RuntimeModelHints;
  /**
   * A note attached to a runtime reported ready on weaker evidence than the
   * others. It rides alongside `ready`, so the route is offered and the reason
   * it is a best guess stays visible.
   */
  readonly readinessCaveat?: RuntimeDiagnostic;
  /**
   * Where a runtime keeps its credentials, for the ones that offer no free
   * way to ask whether they are signed in.
   *
   * MUSE CODE IS WHY THIS EXISTS. Its only sign-in signal is a real run,
   * and a run on Muse costs money -- so readiness fell back to the version,
   * and Muse read READY on a machine that had never logged in. Colin,
   * 2026-09-21: *"if it cant detect the account that the user has this is
   * kind of brutal to add and force this process on the user"*. He is
   * right: the first thing a new person would learn is a raw CLI error, in
   * the middle of a mission, after choosing a runtime the app called ready.
   *
   * A file is not a valid credential -- a token can be expired or a plan
   * unpaid, and neither shows here. It is the difference between "we cannot
   * tell" and "nobody has signed in on this machine", and the second is
   * worth saying out loud for free. What it buys is the ordinary SIGN IN
   * row with the command on it, the same as every other runtime.
   */
  readonly credentials?: {
    /** Environment variables that stand in for the file. */
    readonly environment: readonly string[];
    /** Path segments under the user's home directory. */
    readonly homePath: readonly string[];
    /**
     * Path segments under `$XDG_CONFIG_HOME`, for a runtime that reads its
     * config from there INSTEAD of the home path whenever it is set. Checking
     * only the home path on such a machine would report SIGN IN to someone
     * who has signed in.
     */
    readonly xdgConfigPath?: readonly string[];
  };
}

const DEFINITIONS: readonly IntegrationDefinition[] = [
  {
    id: "codex",
    kind: "agent-runtime",
    displayName: "Codex CLI",
    commandName: "codex",
    optional: false,
    versionArgs: ["--version"],
    capabilityArgs: ["exec", "--help"],
    readinessArgs: ["login", "status"],
    requiredFeatures: CODEX_REQUIRED_FEATURES,
  },
  {
    id: "claude",
    kind: "agent-runtime",
    displayName: "Claude Code",
    commandName: "claude",
    optional: false,
    versionArgs: ["--version"],
    capabilityArgs: ["--help"],
    readinessArgs: ["auth", "status"],
    requiredFeatures: CLAUDE_REQUIRED_FEATURES,
  },
  {
    id: "cursor",
    kind: "agent-runtime",
    displayName: "Cursor Agent",
    // The installer puts both `agent` and `cursor-agent` on disk; the longer
    // name is the one nothing else on a machine is likely to be called.
    commandName: "cursor-agent",
    optional: true,
    versionArgs: ["--version"],
    capabilityArgs: ["--help"],
    readinessArgs: ["status"],
    modelsArgs: ["--list-models"],
    parseModels: parseCursorModelList,
    requiredFeatures: CURSOR_REQUIRED_FEATURES,
    // Measured: `cursor-agent status` prints "Not logged in" and exits 0.
    readyWhen: (result) => !/not logged in/i.test(`${result.stdout}\n${result.stderr}`),
  },
  {
    id: "gemini",
    kind: "agent-runtime",
    displayName: "Gemini CLI",
    commandName: "gemini",
    optional: true,
    versionArgs: ["--version"],
    capabilityArgs: ["--help"],
    // Measured: with no auth method configured this exits 41 and names the
    // ways to sign in; signed in, it lists sessions and exits 0. It is the
    // cheapest command the CLI has that touches its credentials.
    readinessArgs: ["--list-sessions"],
    requiredFeatures: GEMINI_REQUIRED_FEATURES,
    // Also measured, after a sign-in Google then refused: the same command
    // prints "Error authenticating: IneligibleTierError ..." and exits 0. So
    // the exit code is not the whole answer here either.
    readyWhen: (result) => !/error authenticating|please set an auth method/i.test(`${result.stdout}\n${result.stderr}`),
  },
  {
    id: "opencode",
    kind: "agent-runtime",
    displayName: "OpenCode",
    commandName: "opencode",
    optional: true,
    versionArgs: ["--version"],
    capabilityArgs: ["run", "--help"],
    // MEASURED 2026-09-03: OpenCode is READY WITH NO SIGN-IN. It ships free
    // models, so there is no auth command to run and nothing that would tell
    // us anything if there were. `models` is the cheapest true statement
    // available -- it is local, it costs nothing, and a CLI that can list the
    // models it will run is a CLI that can run one.
    readinessArgs: ["models"],
    // The verbose form carries each model's variants, its reasoning efforts
    // (A6.5). The plain listing above still stands in if it cannot be read.
    modelsArgs: ["models", "--verbose"],
    /*
     * ONE `opencode` START, NOT TWO (2026-10-05). The plain `models` (readiness)
     * and `models --verbose` (the same ids, with each model's efforts) were
     * both spawned at every sweep, and each start opens OpenCode's whole
     * runtime: 1.8-2.5 s alone on Colin's machine, 4-11 s beside eight other
     * probes. The verbose list names every id the plain one does, so its answer
     * is the readiness answer too; the plain command runs only if it did not.
     */
    listAnswersReadiness: true,
    parseModels: parseOpenCodeModelList,
    requiredFeatures: OPENCODE_REQUIRED_FEATURES,
    // Version 2 is checked separately against its ACP command, before any of
    // these legacy probes can run. A future major is not assumed compatible.
    unsupportedVersion: (version) =>
      version.major >= 3
        ? {
            message: `OpenCode ${version.version} is installed; this Locust supports OpenCode 1 and OpenCode 2.`,
            resolution: "Use a supported OpenCode 1 or OpenCode 2 release, or update Locust for this version.",
          }
        : undefined,
    readyWhen: (result) =>
      parseOpenCodeModelList(`${result.stdout}\n${result.stderr}`) !== undefined,
  },
  {
    id: "copilot",
    kind: "agent-runtime",
    displayName: "Copilot CLI",
    commandName: "copilot",
    optional: true,
    versionArgs: ["--version"],
    capabilityArgs: ["--help"],
    // MEASURED 2026-09-03, and this one is a judgement call worth spelling
    // out. Copilot CLI has NO free readiness check: whether the signed-in
    // account's plan includes the CLI is only discovered by starting a run,
    // and a run costs a premium request. Discovery will not spend the user's
    // quota to fill in a status field, so a version that prints is taken as
    // ready and the caveat below rides along. The refusal, when it comes, is
    // unmistakable and lands where the user can act on it: the run fails with
    // "Copilot plan required" and the settings URL. Reporting
    // `authentication-required` instead would hide a working route from every
    // user whose plan is fine.
    readinessArgs: ["--version"],
    requiredFeatures: COPILOT_REQUIRED_FEATURES,
    readyWhen: (result) => /copilot cli/i.test(`${result.stdout}\n${result.stderr}`),
    fixedModelHints: COPILOT_MODEL_HINTS,
    readinessCaveat: {
      code: "readiness-unverifiable",
      severity: "info",
      message:
        "Copilot CLI is installed and signed in as far as its version command can tell; "
        + "whether this account's plan includes the CLI cannot be checked without spending a premium request.",
      resolution:
        "If a run fails with a policy denial, confirm the plan at https://github.com/settings/copilot",
    },
  },
  {
    /*
     * Meta's Muse Code. Every line here was MEASURED on Windows 2026-09-21
     * against 1.3.0-R3401.1, minutes after installing it; the captures are in
     * `docs/muse-probe-2026-09-21/`.
     *
     * On PATH this is `muse.cmd`, a batch shim that runs a PowerShell
     * launcher, which then starts a version-stamped 415 MB binary beside it.
     * It is NEITHER npm shim shape `path-locator` unwraps, so it keeps its
     * shell -- correct, and it means the 8,191-character cmd.exe argv limit
     * applies. `--prompt-file` exists for exactly that and is what a real run
     * must use.
     */
    id: "muse",
    kind: "agent-runtime",
    displayName: "Muse Code",
    commandName: "muse",
    optional: true,
    // Measured: `Muse Code 1.3.0 (1.3.0-R3401.1)`, exit 0.
    versionArgs: ["--version"],
    /*
     * `exec --help`, not `--help`.
     *
     * The top-level help lists subcommands and the TUI's options; every flag
     * a mission passes -- `--json`, `--workspace`, `--disable-write` -- is on
     * the exec page. Asking the wrong page meant the feature scan found none
     * of them, so discovery called a working runtime unsupported.
     */
    capabilityArgs: ["exec", "--help"],
    /*
     * READINESS IS THE VERSION, AND THAT IS A JUDGEMENT CALL WORTH SPELLING
     * OUT -- the same one Copilot forced, for a sharper reason.
     *
     * Muse has no status command: `auth` only sets a key, `login` only starts
     * a device flow. The one thing that reveals sign-in is a real run, and
     * MEASURED signed-out it says on stderr:
     *
     *   missing meta credentials: run `muse login` or set META_API_KEY, or
     *   save credentials at \Users\<you>\.config\muse\auth.json
     *
     * Fast, free and unmistakable -- but only while signed OUT. Signed IN,
     * that same probe is a billed model step, and discovery sweeps every
     * fifteen seconds and on every window focus. **A readiness check that
     * spends the user's money each sweep is not a readiness check.** So the
     * version is taken as ready and the caveat below rides along, exactly as
     * it does for Copilot.
     *
     * `--provider echo` runs with no credentials at all, so it cannot
     * distinguish either; it is how the event stream was captured for free.
     */
    readinessArgs: ["--version"],
    requiredFeatures: MUSE_REQUIRED_FEATURES,
    /*
     * `\s` and `\n`, ONE backslash each.
     *
     * This shipped as `/muse code\\s+\\d/i` -- a regex for a literal
     * backslash followed by an `s` -- so it could never match
     * `Muse Code 1.3.0` and readiness never came back. Settings read
     * **Muse Code 1.3.0 · CHECKING · "did not answer its version probe in
     * time"** forever, on a machine where that probe answers in 400ms. Seen
     * by driving the packaged build; no unit test had the runtime's real
     * stdout in front of it. There is one below now.
     *
     * Written through a bash heredoc, which ate the escape. That is the
     * eighth time this session's family of mistakes has landed one, and the
     * only reliable answer is not to write source through a heredoc.
     */
    readyWhen: (result) => /muse code\s+\d/i.test(`${result.stdout}\n${result.stderr}`),
    /*
     * MEASURED signed-out, 2026-09-21: the run stops with
     *
     *   missing meta credentials: run `muse login` or set META_API_KEY, or
     *   save credentials at C:\Users\<you>\.config\muse\auth.json
     *
     * So Muse names the file itself. Checking whether it is there costs one
     * stat and settles the common case for free -- see `credentials` on the
     * definition type for why that matters more than it sounds.
     *
     * The path is confirmed by Muse's own bundled help (1.3.0, read out of
     * the executable 2026-09-22): the config root is `$XDG_CONFIG_HOME/muse`,
     * else `$HOME/.config/muse`, and it holds `settings.json`, `auth.json`
     * and `trust.json`. `settings.json` alone exists after a first launch
     * with no login, which is why the check is on `auth.json` and not on the
     * folder.
     */
    credentials: {
      // Not META_API_KEY, though Muse itself prefers it: a run's environment
      // is an allowlist that carries no provider key, so a key-only machine
      // showed ready and every run failed (M6, the code review).
      environment: [],
      homePath: [".config", "muse", "auth.json"],
      xdgConfigPath: ["muse", "auth.json"],
    },
    readinessCaveat: {
      code: "readiness-unverifiable",
      severity: "info",
      message:
        "Muse Code has credentials on this machine, but whether they are still valid and what "
        + "plan they carry cannot be checked without starting a run, and a run on Muse costs money.",
      resolution:
        "If a run fails with 'missing meta credentials', run `muse login` in a terminal.",
    },
  },
  {
    /*
     * Kilo Code's CLI. Measured 2026-10-25 on 7.8.17: `kilo --version` prints
     * the version alone, and `kilo acp` speaks the Agent Client Protocol.
     * Models come from that session, not a list command -- a list command is
     * not run here, and neither is a turn. There is no credential file check:
     * Auto Free needs no account, and this probe does not look at
     * ~/.config/kilo or ~/.config/opencode.
     */
    id: "kilo",
    kind: "agent-runtime",
    displayName: "Kilo",
    commandName: "kilo",
    optional: true,
    versionArgs: ["--version"],
    capabilityArgs: ["acp", "--help"],
    readinessArgs: ["--version"],
    requiredFeatures: ["acp-stdio"],
    readyWhen: (result) => /\d+\.\d+\.\d+/.test(`${result.stdout}\n${result.stderr}`),
  },
  {
    id: "omniroute",
    kind: "provider-gateway",
    displayName: "OmniRoute",
    commandName: "omniroute",
    optional: true,
    versionArgs: ["--version"],
    capabilityArgs: ["doctor", "--help"],
    readinessArgs: ["doctor", "--json"],
    requiredFeatures: OMNIROUTE_REQUIRED_FEATURES,
  },
];

/*
 * Ten seconds, from five.
 *
 * A probe that times out reads as a FAILED probe -- the checking state, which
 * schedules more sweeps -- and five was measured too close to the truth on
 * Colin's machine, 2026-09-21: with one runtime's probes running together
 * and eight runtimes staggered, `gemini`'s sign-in check took 5.5 s under
 * contention and reported `error` where it had reported `needs-signin` in
 * sequence; OpenCode's `models` and Antigravity sat at 5.2-5.5 s. The
 * timeout exists to bound a HUNG command, not to race a slow one. (Fable's
 * probing review, #6.)
 */
const PROBE_TIMEOUT_MS = 10_000;

/**
 * Whether a readiness check that did not pass said the person is SIGNED OUT,
 * rather than simply not answering right.
 *
 * Any check that failed without timing out used to read as "not signed in".
 * A fresh-profile beta report of 0.345 met it on OpenCode -- which has no
 * sign-in at all -- and on Muse, both "not signed in" on the first sweep and
 * ready on a later one. And the window never asks again about a runtime it
 * was told needs a sign-in (it re-checks only the ones not answering), so the
 * wrong word stuck: the model menu offered only the account default for about
 * two minutes. So "signed out" now needs the CLI's own words about signing
 * in, and OpenCode, with no account to be out of, is never said to be.
 * Anything else is a runtime not answering yet, which is checked again.
 */
const SIGN_IN_CHECKS: ReadonlySet<string> = new Set(["codex", "claude", "cursor", "gemini"]);

function saysSignedOut(definition: IntegrationDefinition, result: CommandResult): boolean {
  if (definition.id === "opencode") return false;
  // Where the readiness command IS a sign-in check -- `codex login status`,
  // `claude auth status`, `cursor-agent status`, gemini's session list --
  // its failing is the statement, with or without words.
  if (SIGN_IN_CHECKS.has(definition.id)) return true;
  return /sign(?:ed)?[ -]?(?:in|out)|log(?:ged)?[ -]?(?:in|out)|login|auth|credential|api[ _-]?key|unauthori[sz]ed|subscription|not entitled|account|ineligible/i
    .test(`${result.stdout}\n${result.stderr}`);
}

interface ProbeOutcome {
  readonly result?: CommandResult;
  readonly error?: string;
}

async function runProbe(
  runner: CommandRunner,
  executable: ExecutableLaunch,
  purpose: ProbeCommand["purpose"],
  args: readonly string[],
): Promise<ProbeOutcome> {
  try {
    return {
      result: await runner.run({
        purpose,
        executablePath: executable.executablePath,
        args: [...executable.prefixArgs, ...args],
        timeoutMs: PROBE_TIMEOUT_MS,
        // H7: the launch travels whole, its environment with it.
        ...(executable.env === undefined ? {} : { env: executable.env }),
      }),
    };
  } catch (error) {
    return { error: error instanceof Error ? error.message : "Unknown command-runner error" };
  }
}

function succeeded(outcome: ProbeOutcome): outcome is { readonly result: CommandResult } {
  return outcome.result?.exitCode === 0 && outcome.result.timedOut !== true;
}

/**
 * What a CLI's own files say about it, as the probes printed it.
 *
 * RAW TEXT, not the version and features read out of it. A Locust release
 * that teaches `parseRuntimeVersion` a new format, or
 * `detectSupportedFeatures` a new flag, must reach a machine whose CLIs have
 * not changed -- and it does, because the text is what is kept and the
 * reading is redone every time.
 */
export interface RuntimeBinaryFacts {
  readonly versionText: string;
  readonly capabilityText: string;
}

/**
 * What was learned about a binary that has not changed since.
 *
 * `--version` and `--help` are functions of the file on disk: same file,
 * same answer, and on Colin's machine each costs 0.5-1.6 s of a sweep that
 * runs at every launch. Readiness is NOT cached here and never will be --
 * whether an account is signed in changes without the file changing, which
 * is the whole reason the probe exists.
 *
 * Keyed by a fingerprint of every file the launch touches (the shim found on
 * PATH, the executable actually run, and any path among its prefix
 * arguments) with each one's size and modification time. An npm update
 * rewrites those files, so the fingerprint moves and the probes run again.
 * A file that cannot be stat'd yields no fingerprint at all, so it is probed
 * -- absence of a fact is never taken as the fact being unchanged.
 */
export interface RuntimeFactsCache {
  get(fingerprint: string): RuntimeBinaryFacts | undefined;
  set(fingerprint: string, facts: RuntimeBinaryFacts): void;
}

/** Test seam: `stat`, narrowed to the two fields a fingerprint is made of. */
export type StatFile = (path: string) => Promise<{ readonly size: number; readonly mtimeMs: number }>;

/**
 * Every file this launch depends on, in a stable order.
 *
 * A node-shim runs `node.exe` with the CLI's own script as a prefix
 * argument, so the executable that is spawned is not the file that changes
 * when the CLI is updated. Fingerprinting all of them costs three stats --
 * microseconds against a spawn -- and is the difference between noticing an
 * update and serving a stale version number for ever.
 */
function fingerprintPaths(executable: ExecutableLaunch): readonly string[] {
  const paths = [executable.discoveredPath, executable.executablePath, ...executable.prefixArgs];
  /*
   * A SCRIPT launcher stays put while the CLI behind it updates itself, so
   * its own size and time said nothing had changed and the cached version
   * and help went stale (a B4 lead). Seen on this machine: Cursor's
   * `versions` folder moved at 23:09 while cursor-agent.cmd/.ps1 kept 19:25;
   * Muse writes `.muse-version` and a new muse-bin-<version>.exe beside an
   * unchanged muse.cmd. The launcher's folder, a `versions` folder in it and
   * Muse's version file are part of the fingerprint too; one that is not
   * there is skipped like any missing path.
   */
  if (/\.(?:cmd|bat|ps1)$/i.test(executable.discoveredPath)) {
    const folder = dirname(executable.discoveredPath);
    paths.push(folder, join(folder, "versions"), join(folder, ".muse-version"));
  }
  return [...new Set(paths.filter((path) => path.length > 0))];
}

export async function fingerprintOf(executable: ExecutableLaunch, statFile: StatFile): Promise<string | undefined> {
  const parts: string[] = [];
  for (const path of fingerprintPaths(executable)) {
    let stats;
    try {
      stats = await statFile(path);
    } catch {
      // A prefix argument that is a flag rather than a file lands here, and
      // so does a file that has gone. Neither is fatal on its own; what is
      // fatal is claiming a fingerprint that does not cover everything.
      continue;
    }
    parts.push(`${path}:${String(stats.size)}:${String(stats.mtimeMs)}`);
  }
  // Nothing could be stat'd at all: there is no fact here to key on.
  return parts.length === 0 ? undefined : parts.join("|");
}

function failureMessage(outcome: ProbeOutcome): string {
  if (outcome.error) return outcome.error;
  if (outcome.result?.timedOut) return "probe timed out";
  if (outcome.result?.exitCode !== undefined) {
    return `probe exited with code ${String(outcome.result.exitCode)}`;
  }
  return "probe did not return a result";
}

function diagnostic(
  value: RuntimeDiagnostic,
): RuntimeDiagnostic {
  return value;
}

/**
 * Where to look for a credential, kept as data so a test can answer without
 * a home directory and without the machine's real environment.
 */
export interface CredentialLookup {
  readonly homeDirectory: string;
  readonly variables: Readonly<Record<string, string | undefined>>;
}

/**
 * Whether a runtime that keeps its credentials in a file has none.
 *
 * `false` for every runtime that does not declare `credentials` -- this must
 * never change an answer for a runtime with a real sign-in probe. And `false`
 * when the check cannot be made at all: a home directory we cannot read is
 * not evidence that nobody signed in, and reporting SIGN IN on a guess would
 * be the same defect pointed the other way.
 */
export async function missingCredentials(
  definition: IntegrationDefinition,
  statFile: StatFile,
  lookup: CredentialLookup,
): Promise<boolean> {
  const credentials = definition.credentials;
  if (credentials === undefined) return false;
  for (const name of credentials.environment) {
    if ((lookup.variables[name] ?? "").trim().length > 0) return false;
  }
  const xdg = (lookup.variables.XDG_CONFIG_HOME ?? "").trim();
  const path = credentials.xdgConfigPath !== undefined && xdg.length > 0
    ? [xdg, ...credentials.xdgConfigPath].join(sep)
    : lookup.homeDirectory.length === 0
      ? undefined
      : [lookup.homeDirectory, ...credentials.homePath].join(sep);
  if (path === undefined) return false;
  try {
    await statFile(path);
    return false;
  } catch {
    return true;
  }
}

function notInstalled(definition: IntegrationDefinition): RuntimeDiscovery {
  return {
    id: definition.id,
    kind: definition.kind,
    displayName: definition.displayName,
    optional: definition.optional,
    availability: "unavailable",
    readiness: "unknown",
    supportedFeatures: [],
    requiredFeatures: definition.requiredFeatures,
    diagnostics: [
      diagnostic({
        code: "executable-not-found",
        severity: definition.optional ? "info" : "warning",
        message: `${definition.displayName} was not found on PATH.`,
        resolution: definition.optional
          ? "Install and enable it only if you want this optional route."
          : `Install ${definition.displayName} and make its official CLI available on PATH.`,
      }),
    ],
  };
}

/** Only the supported OpenCode 2 protocol uses ACP; unknown and future versions do not. */
export function openCodeUsesAcp(version: { readonly version: string } | undefined): boolean {
  return version !== undefined && parseRuntimeVersion(version.version)?.major === 2;
}

async function discoverOpenCodeAcp(
  definition: IntegrationDefinition,
  runner: CommandRunner,
  executable: ExecutableLaunch,
  version: ParsedRuntimeVersion,
): Promise<RuntimeDiscovery> {
  // A help command spends no provider quota. Models and permissions are
  // learned from the actual ACP session, never from V1 CLI flags or lists.
  const capability = await runProbe(runner, executable, "capabilities", ["acp", "--help"]);
  const requiredFeatures: readonly RuntimeFeature[] = ["acp-stdio"];
  const advertised = succeeded(capability)
    && /\bagent[\s-]*client[\s-]*protocol\b/i.test(`${capability.result.stdout}\n${capability.result.stderr}`);
  const diagnostics: RuntimeDiagnostic[] = [];
  if (!succeeded(capability)) {
    diagnostics.push({
      code: "capability-probe-failed",
      severity: "error",
      message: `OpenCode ACP capability detection ${failureMessage(capability)}.`,
    });
  }
  if (!advertised) {
    diagnostics.push({
      code: "required-capability-missing",
      severity: "error",
      message: "OpenCode 2 did not advertise the Agent Client Protocol on its acp command.",
      resolution: "Update OpenCode to a release whose opencode acp command supports Agent Client Protocol.",
    });
  }
  return {
    id: definition.id,
    kind: definition.kind,
    displayName: definition.displayName,
    optional: definition.optional,
    availability: "available",
    readiness: advertised ? "ready" : "unsupported",
    executable,
    version,
    supportedFeatures: advertised ? ["acp-stdio"] : [],
    requiredFeatures,
    diagnostics,
  };
}

/**
 * Kilo is ready when `kilo acp --help` names the Agent Client Protocol.
 * The help is a fact about the binary, so it is cached with the version.
 * A failed help is not cached. No model list and no credential file.
 */
async function discoverKilo(
  definition: IntegrationDefinition,
  runner: CommandRunner,
  executable: ExecutableLaunch,
  version: ParsedRuntimeVersion,
  versionText: string,
  remembered: RuntimeBinaryFacts | undefined,
  fingerprint: string | undefined,
  recall: RuntimeFactsCache | undefined,
): Promise<RuntimeDiscovery> {
  const capability: ProbeOutcome = remembered !== undefined
    ? { result: { stdout: remembered.capabilityText, stderr: "", exitCode: 0 } }
    : await runProbe(runner, executable, "capabilities", ["acp", "--help"]);
  const capabilityText = succeeded(capability) ? `${capability.result.stdout}\n${capability.result.stderr}` : "";
  const advertised = succeeded(capability) && /\bagent[\s-]*client[\s-]*protocol\b/i.test(capabilityText);
  if (remembered === undefined && fingerprint !== undefined && succeeded(capability)) {
    recall?.set(fingerprint, { versionText, capabilityText });
  }
  const diagnostics: RuntimeDiagnostic[] = [];
  if (!succeeded(capability)) {
    diagnostics.push({
      code: "capability-probe-failed",
      severity: "error",
      message: `Kilo ACP capability detection ${failureMessage(capability)}.`,
    });
  }
  if (!advertised) {
    diagnostics.push({
      code: "required-capability-missing",
      severity: "error",
      message: "Kilo did not advertise the Agent Client Protocol on its acp command.",
      resolution: "Update Kilo to a release whose kilo acp command supports the Agent Client Protocol.",
    });
  }
  return {
    id: definition.id,
    kind: definition.kind,
    displayName: definition.displayName,
    optional: definition.optional,
    availability: "available",
    readiness: advertised ? "ready" : "unsupported",
    executable,
    version,
    supportedFeatures: advertised ? ["acp-stdio"] : [],
    requiredFeatures: ["acp-stdio"],
    diagnostics,
  };
}

async function discoverOne(
  definition: IntegrationDefinition,
  runner: CommandRunner,
  locator: ExecutableLocator,
  recall: RuntimeFactsCache | undefined,
  statFile: StatFile,
  environment: CredentialLookup,
  readinessFromVersion: boolean,
): Promise<RuntimeDiscovery> {
  const executable = await locator.find(definition.commandName);
  if (!executable) return notInstalled(definition);

  const diagnostics: RuntimeDiagnostic[] = [];
  /*
   * All of one runtime's probes at once.
   *
   * They ran strictly in sequence -- version, then help, then readiness,
   * then the model list -- and none depends on another's OUTPUT. Readiness
   * used to wait for the capability probe only to be SKIPPED when a required
   * flag was missing, which is a discard after the fact, not a dependency.
   * MEASURED on Colin's machine, 2026-09-21: each `cursor-agent` probe is
   * 1.1-1.6 s warm through its `.cmd` shim, so the four in sequence were
   * ~5.1 s -- the "past six seconds" the stagger comment records was their
   * SUM. Together they cost the slowest one. (Fable's probing review, #3.)
   *
   * A list command that is the readiness command -- OpenCode's `models` is
   * both -- is spawned once and read twice.
   */
  const sameCommand =
    definition.modelsArgs !== undefined
    && definition.modelsArgs.length === definition.readinessArgs.length
    && definition.modelsArgs.every((arg, index) => arg === definition.readinessArgs[index]);
  /*
   * What this binary already told us, if it is the same binary.
   *
   * Only version and capability: the two probes whose answer is a function
   * of the file. Readiness is always asked, because signing in changes
   * nothing on disk. (Fable's probing review, #4.)
   */
  const fingerprint = recall === undefined ? undefined : await fingerprintOf(executable, statFile);
  const remembered = fingerprint === undefined ? undefined : recall?.get(fingerprint);
  const versionAsked: Promise<ProbeOutcome> =
    remembered === undefined
      ? runProbe(runner, executable, "version", definition.versionArgs)
      : Promise.resolve({ result: { stdout: remembered.versionText, stderr: "", exitCode: 0 } } as ProbeOutcome);
  if (definition.id === "opencode") {
    // V2 removed the serve protocol. Decide before starting any V1 probes,
    // including the model-list command which V1 also uses for readiness.
    const outcome = await versionAsked;
    const version = succeeded(outcome)
      ? parseRuntimeVersion(`${outcome.result.stdout}\n${outcome.result.stderr}`)
      : undefined;
    if (version !== undefined && openCodeUsesAcp(version)) {
      // Legacy cached facts do not record which help command produced them.
      // Never reuse them for ACP, or cache ACP help under the legacy key.
      return discoverOpenCodeAcp(definition, runner, executable, version);
    }
    const refused = version === undefined ? undefined : definition.unsupportedVersion?.(version);
    if (version !== undefined && refused !== undefined) {
      return {
        id: definition.id,
        kind: definition.kind,
        displayName: definition.displayName,
        optional: definition.optional,
        availability: "available",
        readiness: "unsupported",
        executable,
        version,
        supportedFeatures: [],
        requiredFeatures: [],
        diagnostics: [{ code: "version-unsupported", severity: "error", ...refused }],
      };
    }
  }
  if (definition.id === "kilo") {
    const outcome = await versionAsked;
    const versionText = succeeded(outcome) ? `${outcome.result.stdout}\n${outcome.result.stderr}` : "";
    const version = succeeded(outcome) ? parseRuntimeVersion(versionText) : undefined;
    if (version !== undefined) {
      return discoverKilo(definition, runner, executable, version, versionText, remembered, fingerprint, recall);
    }
  }
  /*
   * WHEN THE READINESS QUESTION IS THE VERSION QUESTION, ASK IT ONCE.
   *
   * Copilot and Muse have no free sign-in check, so their readiness command
   * IS `--version` -- and it was spawned a second time beside the version
   * probe, and again on every launch after the version was remembered.
   * Measured at boot on Colin's machine (2026-09-22): Copilot 0.93 s, Muse
   * 0.60 s, for an answer already in hand. The same outcome is read twice.
   *
   * And a runtime the CALLER does not run (`readinessFromVersion`) is not
   * asked at all past its version: Gemini CLI's `--list-sessions` starts the
   * whole CLI and was the slowest probe of every launch -- 3.2 s warm, 7.4 s
   * cold -- for a runtime Locust lists as planned and cannot start a
   * mission on. Its readiness logic stays here, tested, for the day it can.
   */
  const readinessIsVersion =
    readinessFromVersion
    || (definition.readinessArgs.length === definition.versionArgs.length
      && definition.readinessArgs.every((arg, index) => arg === definition.versionArgs[index]));
  const listedAsked: Promise<ProbeOutcome> | undefined =
    definition.modelsArgs === undefined || sameCommand
      ? undefined
      : runProbe(runner, executable, "models", definition.modelsArgs);
  const readinessAsked: Promise<ProbeOutcome> = readinessIsVersion
    ? versionAsked
    : definition.listAnswersReadiness === true && listedAsked !== undefined
      // The list is the answer when it passes the same test the plain command would.
      ? listedAsked.then((outcome) =>
          succeeded(outcome) && (definition.readyWhen?.(outcome.result) ?? true)
            ? outcome
            : runProbe(runner, executable, "readiness", definition.readinessArgs),
        )
      : runProbe(runner, executable, "readiness", definition.readinessArgs);
  const [versionOutcome, capabilityOutcome, readinessOutcome, listedOutcome] = await Promise.all([
    versionAsked,
    remembered === undefined
      ? runProbe(runner, executable, "capabilities", definition.capabilityArgs)
      : Promise.resolve({ result: { stdout: remembered.capabilityText, stderr: "", exitCode: 0 } } as ProbeOutcome),
    readinessAsked,
    listedAsked ?? Promise.resolve(undefined),
  ]);
  const combinedVersionOutput = succeeded(versionOutcome)
    ? `${versionOutcome.result.stdout}\n${versionOutcome.result.stderr}`
    : "";
  const version = parseRuntimeVersion(combinedVersionOutput);
  if (!succeeded(versionOutcome)) {
    diagnostics.push(
      diagnostic({
        code: "version-probe-failed",
        severity: "warning",
        message: `${definition.displayName} was found, but its version probe ${failureMessage(versionOutcome)}.`,
      }),
    );
  } else if (!version) {
    diagnostics.push(
      diagnostic({
        code: "version-unrecognized",
        severity: "warning",
        message: `${definition.displayName} returned an unrecognized version string.`,
      }),
    );
  }

  const capabilityText = succeeded(capabilityOutcome)
    ? `${capabilityOutcome.result.stdout}\n${capabilityOutcome.result.stderr}`
    : "";
  /*
   * Kept only when BOTH probes answered, and only when this was a real
   * probe rather than a recollection.
   *
   * A failure is not a fact about the binary -- a CLI that timed out once
   * under load would otherwise be remembered as versionless for as long as
   * nobody reinstalled it, which is the "absence of an answer is not an
   * answer" rule this project keeps relearning, written into a cache.
   */
  if (
    remembered === undefined
    && fingerprint !== undefined
    && succeeded(versionOutcome)
    && succeeded(capabilityOutcome)
  ) {
    recall?.set(fingerprint, { versionText: combinedVersionOutput, capabilityText });
  }

  const supportedFeatures = succeeded(capabilityOutcome)
    ? detectSupportedFeatures(definition.id, capabilityText)
    : [];
  // The same help text names the models the CLI accepts; only Claude's does.
  // Claude's help also names model ALIASES, so it keeps its own reader. Every
  // other runtime still has effort levels worth reading -- Copilot names seven
  // in its help and Locust reported none, so the composer said "fixed" and the
  // levels the CLI accepts were unreachable (measured 2026-09-08).
  let modelHints = definition.id === "claude"
    ? parseClaudeModelHints(capabilityText)
    : (() => {
        const efforts = parseEffortChoices(capabilityText);
        return efforts.length === 0 ? undefined : { aliases: [], efforts };
      })();

  if (!succeeded(capabilityOutcome)) {
    diagnostics.push(
      diagnostic({
        code: "capability-probe-failed",
        severity: "error",
        message: `${definition.displayName} capability detection ${failureMessage(capabilityOutcome)}.`,
      }),
    );
  }

  const missingFeatures = definition.requiredFeatures.filter(
    (feature) => !supportedFeatures.includes(feature),
  );
  if (missingFeatures.length > 0) {
    diagnostics.push(
      diagnostic({
        code: "required-capability-missing",
        severity: "error",
        message: `${definition.displayName} is missing required safe-integration capabilities: ${missingFeatures.join(", ")}.`,
        resolution: `Update ${definition.displayName}; unsafe permission-bypass flags will not be substituted.`,
      }),
    );
  }

  const refusedVersion = version === undefined ? undefined : definition.unsupportedVersion?.(version);
  if (refusedVersion !== undefined) {
    diagnostics.push(
      diagnostic({
        code: "version-unsupported",
        severity: "error",
        message: refusedVersion.message,
        resolution: refusedVersion.resolution,
      }),
    );
  }

  let readiness: RuntimeReadiness = missingFeatures.length > 0 || refusedVersion !== undefined ? "unsupported" : "unknown";
  if (missingFeatures.length === 0 && refusedVersion === undefined) {
    const signedOut = await missingCredentials(definition, statFile, environment);
    if (signedOut) {
      /*
       * The probe said ready and the credential file is not there. For a
       * runtime whose readiness IS its version -- the only kind that can
       * carry `credentials` -- that combination means exactly one thing:
       * installed, never signed in. Said here, for free, instead of as a
       * raw CLI error in the middle of somebody's first mission.
       */
      readiness = "authentication-required";
      diagnostics.push(
        diagnostic({
          code: "authentication-required",
          severity: "warning",
          message: `${definition.displayName} is installed but nobody has signed in on this machine.`,
          resolution: `Run \`${definition.commandName} login\` in a terminal, then retry discovery.`,
        }),
      );
    } else if (succeeded(readinessOutcome) && (definition.readyWhen?.(readinessOutcome.result) ?? true)) {
      readiness = "ready";
      if (definition.readinessCaveat !== undefined) {
        diagnostics.push(diagnostic(definition.readinessCaveat));
      }
    } else if (
      definition.id !== "omniroute" &&
      readinessOutcome.result !== undefined &&
      readinessOutcome.result.timedOut !== true &&
      saysSignedOut(definition, readinessOutcome.result)
    ) {
      readiness = "authentication-required";
      diagnostics.push(
        diagnostic({
          code: "authentication-required",
          severity: "warning",
          message: `${definition.displayName} is installed but its official authentication status command did not report ready.`,
          resolution: `Authenticate in the official ${definition.displayName} CLI, then retry discovery.`,
        }),
      );
    } else {
      readiness = "unhealthy";
      diagnostics.push(
        diagnostic({
          code: definition.id === "omniroute" ? "health-check-failed" : "readiness-probe-failed",
          severity: "error",
          message: `${definition.displayName} readiness ${failureMessage(readinessOutcome)}.`,
        }),
      );
    }
  }

  // A model list is read only from a signed-in runtime: measured, the
  // signed-out command prints an error instead of a list.
  if (definition.modelsArgs !== undefined && readiness === "ready") {
    // Spawned above with the rest; the readiness answer itself when the list
    // command is the readiness command.
    const modelsOutcome = sameCommand ? readinessOutcome : listedOutcome;
    // Both streams, like every other probe in this file: the one model
    // listing this repo has actually captured arrived on stderr.
    const read = (outcome: typeof modelsOutcome) =>
      outcome !== undefined && succeeded(outcome)
        ? definition.parseModels?.(`${outcome.result.stdout}\n${outcome.result.stderr}`)
        : undefined;
    // A richer list command that fails (an older CLI without the flag) falls
    // back to the readiness answer, when that answer is itself a list.
    const listed = read(modelsOutcome) ?? (sameCommand ? undefined : read(readinessOutcome));
    if (listed !== undefined) {
      modelHints = listed;
    } else if (modelsOutcome !== undefined && succeeded(modelsOutcome)) {
      modelHints = undefined;
      // A list that could not be read is not an empty list. Saying nothing
      // here would leave the picker offering one runtime no models with no
      // hint that anything went wrong.
      diagnostics.push(
        diagnostic({
          code: "capability-probe-failed",
          severity: "warning",
          message: `${definition.displayName} listed its models in a form this build does not recognise.`,
          resolution: "Its account default is still offered; update Locust if the list stays empty.",
        }),
      );
    }
  }

  // A runtime with no list command to ask offers what it can honestly offer:
  // the route where the CLI chooses the model for itself.
  if (definition.fixedModelHints !== undefined && readiness === "ready") {
    // The fixed MODELS, and the efforts the help really named: the fixed
    // hints carry none, and replacing wholesale threw Copilot's seven away,
    // so the composer said its effort was fixed (a B4 lead).
    const read = modelHints?.efforts ?? [];
    modelHints = read.length === 0 ? definition.fixedModelHints : { ...definition.fixedModelHints, efforts: read };
  }

  const base: RuntimeDiscovery = {
    id: definition.id,
    kind: definition.kind,
    displayName: definition.displayName,
    optional: definition.optional,
    availability: "available",
    readiness,
    executable,
    supportedFeatures,
    requiredFeatures: definition.requiredFeatures,
    diagnostics,
    ...(modelHints === undefined ? {} : { modelHints }),
  };
  return version ? { ...base, version } : base;
}

/**
 * Told as each probe begins and ends, so a screen can show the WAIT.
 *
 * `started` fires BEFORE the subprocess is spawned, not after it returns.
 * That ordering is the whole contract: the interesting moment is the gap
 * between issuing a command and hearing back, and a caller told about both
 * at once has nothing to show during it. The boot screen exists to fill
 * exactly that gap (design handoff, 2026-09-15).
 */
export interface RuntimeProbeWatcher {
  started(runtime: { readonly id: string; readonly displayName: string; readonly bin: string }): void;
  finished(runtime: { readonly id: string }, discovery: RuntimeDiscovery): void;
}

export interface DiscoverInstalledRuntimesOptions {
  readonly runner: CommandRunner;
  readonly locator: ExecutableLocator;
  readonly includeOmniRoute?: boolean;
  readonly watch?: RuntimeProbeWatcher;
  /**
   * Milliseconds between one probe STARTING and the next.
   *
   * Not between one finishing and the next starting -- see below. Zero, the
   * default, starts them all at once, which is what every caller but the
   * app wants.
   */
  readonly staggerMs?: number;
  /**
   * Runtimes whose sign-in state this caller does not use: their readiness
   * is read from the version answer instead of spawning their own check.
   * Locust passes the runtimes it lists but cannot run a mission on.
   */
  readonly readinessFromVersion?: ReadonlySet<string>;
  /**
   * Ask only these runtimes. For the moment a run is about to start on one
   * runtime and its last answer has gone stale: that runtime is the only
   * question, and asking all of them made the start wait for the slowest.
   */
  readonly only?: ReadonlySet<string>;
  /**
   * Do not ask these: a check for them is already going, and a second one
   * beside it would start the same CLI twice.
   */
  readonly skip?: ReadonlySet<string>;
  /**
   * What the binaries said last time, so a file that has not changed is not
   * asked its version and its help text again. Absent means ask everything,
   * which is what every caller but the app wants.
   */
  readonly recall?: RuntimeFactsCache;
  /** Test seam: how a file's size and modification time are read. */
  readonly statFile?: StatFile;
  /** Test seam: where a credential file would live, and what is exported. */
  readonly credentialLookup?: CredentialLookup;
}

/**
 * STAGGERED, which is what "one at a time" was actually for.
 *
 * Three versions of this, and the reasoning matters more than the code.
 *
 * It was `Promise.all`. Probed all at once every row appears together,
 * every elapsed counter ticks together, and a log meant to be read is
 * noise. So the boot-screen handoff asked for sequential, on the stated
 * assumption that "the total cost is a few hundred ms".
 *
 * MEASURED, on Colin's own machine, 2026-09-14: that assumption is false
 * where the runtimes are actually installed. `cursor-agent` ran past six
 * seconds and copilot was still out at 1.3s, and strictly sequential makes
 * the total the SUM of those -- so the slowest runtime became everybody
 * else's wait, on every launch. He felt it immediately: "maybe its taking
 * so long because its not realizing the runtimes are connected?"
 *
 * The stagger keeps what sequential was FOR and drops what it cost. Each
 * probe begins a beat after the one before it, so rows still appear one at
 * a time, in order, and the log still reads. They then overlap, so the
 * total is the slowest probe rather than the sum of all of them.
 */
export async function discoverInstalledRuntimes(
  options: DiscoverInstalledRuntimesOptions,
): Promise<readonly RuntimeDiscovery[]> {
  return Promise.all(discoverInstalledRuntimesEach(options).map((check) => check.result));
}

/**
 * One runtime's check, as it is going.
 *
 * `discoverInstalledRuntimes` answers when the SLOWEST runtime has, which made
 * every launch as slow as OpenCode (measured 2026-10-05: 4-11 s, with the
 * sweep, the first screen and every start behind it). A caller that will not
 * wait for the slowest asks for the checks themselves: each is its own promise,
 * and a runtime that has not answered can still be SHOWN -- installed, being
 * checked -- because the one cheap step, finding the executable, is already done.
 */
export interface RuntimeCheck {
  readonly id: DiscoveredRuntimeId;
  /** The runtime's answer, exactly as `discoverInstalledRuntimes` would carry it. */
  readonly result: Promise<RuntimeDiscovery>;
  /**
   * Settles when the executable has been looked for (a PATH search: tens of
   * milliseconds), or when the whole check has. After it, `pending()` is true.
   */
  readonly located: Promise<void>;
  /**
   * What is known before the answer: installed (the file is there), nothing
   * more. Never signed out, never missing for being slow -- the diagnostic
   * says the check is still going. A runtime that is not installed has
   * already answered, so this is only ever asked about one that is.
   */
  pending(): RuntimeDiscovery;
}

export function discoverInstalledRuntimesEach(
  options: DiscoverInstalledRuntimesOptions,
): readonly RuntimeCheck[] {
  const definitions = (options.includeOmniRoute === false
    ? DEFINITIONS.filter((definition) => definition.id !== "omniroute")
    : DEFINITIONS
  )
    .filter((definition) => options.only === undefined || options.only.has(definition.id))
    .filter((definition) => options.skip === undefined || !options.skip.has(definition.id));
  const stagger = options.staggerMs ?? 0;
  const credentialLookup: CredentialLookup = options.credentialLookup ?? {
    homeDirectory: homedir(),
    variables: process.env,
  };
  const statFile = options.statFile ?? (async (path: string) => {
    const stats = await stat(path);
    return { size: stats.size, mtimeMs: stats.mtimeMs };
  });
  /*
   * NEVER WAIT LONGER THAN THE RUNTIME DOES.
   *
   * A fixed stagger put a floor under the whole sweep: seven definitions at
   * 240ms is about 1.7 seconds before the last one even starts, and that
   * floor is paid by exactly the people who should pay nothing -- somebody
   * with one runtime installed, or none. A probe for a binary that is not
   * on PATH answers in about 10ms because nothing is ever spawned, so on
   * that machine the stagger WAS the entire wait.
   *
   * Colin, 2026-09-14: "lets make sure if ian or anyone else gets the app
   * that doesnt have all the runtimes that the loading screen isnt a
   * nuisance."
   *
   * So each probe waits for the one before it to finish, OR for the
   * stagger, whichever comes first. Slow runtimes still arrive a readable
   * beat apart, because the beat is shorter than they are. Instant ones
   * chain straight through, because there is no wait to fill.
   */
  /*
   * A GATE PER PROBE, opened by the one before it.
   *
   * The first attempt raced a shared `previous` inside `Promise.all`, and
   * every callback runs synchronously up to its first await -- so all seven
   * raced the same already-resolved promise and started together. The
   * measurement caught it: the gaps between starts were zero.
   *
   * Each probe holds a gate the next one waits on, and opens it when it
   * finishes OR after the stagger, whichever comes first -- timed from when
   * that probe actually began.
   */
  const checks: RuntimeCheck[] = [];
  let gate: Promise<void> = Promise.resolve();
  for (const definition of definitions) {
    const mine = gate;
    let openNext: () => void = () => undefined;
    gate = new Promise<void>((resolve) => {
      openNext = resolve;
    });
    // What the PATH search found, kept so a check that is still going can be shown as installed.
    let foundExecutable: ExecutableLaunch | undefined;
    let markLocated: () => void = () => undefined;
    const located = new Promise<void>((resolve) => {
      markLocated = resolve;
    });
    const watchedLocator: ExecutableLocator = {
      find: async (commandName) => {
        try {
          foundExecutable = await options.locator.find(commandName);
          return foundExecutable;
        } finally {
          markLocated();
        }
      },
    };
    const result = (async () => {
      try {
        await mine;
        // Before the spawn. A watcher told afterwards learns nothing it
        // could not have read from the result.
        options.watch?.started({
          id: definition.id,
          displayName: definition.displayName,
          bin: definition.commandName,
        });
        const running = discoverOne(
          definition,
          options.runner,
          watchedLocator,
          options.recall,
          statFile,
          credentialLookup,
          options.readinessFromVersion?.has(definition.id) === true,
        );
        if (stagger > 0) {
          void Promise.race([
            running.then(
              () => undefined,
              () => undefined,
            ),
            new Promise((resolve) => setTimeout(resolve, stagger)),
          ]).then(() => openNext());
        } else {
          openNext();
        }
        const discovery = await running;
        options.watch?.finished({ id: definition.id }, discovery);
        return discovery;
      } finally {
        // Never leave a caller waiting to learn where the file is.
        markLocated();
        openNext();
      }
    })();
    checks.push({
      id: definition.id,
      result,
      located,
      pending: () =>
        foundExecutable === undefined
          ? notInstalled(definition)
          : {
              id: definition.id,
              kind: definition.kind,
              displayName: definition.displayName,
              optional: definition.optional,
              availability: "available",
              readiness: "unknown",
              executable: foundExecutable,
              supportedFeatures: [],
              requiredFeatures: definition.requiredFeatures,
              diagnostics: [
                diagnostic({
                  code: "check-pending",
                  severity: "info",
                  message: `${definition.displayName} is installed and is still being checked.`,
                }),
              ],
            },
    });
  }
  return checks;
}
