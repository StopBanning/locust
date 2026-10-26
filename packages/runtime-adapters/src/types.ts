/**
 * Every integration this package knows how to find. Cursor Agent and Gemini
 * CLI are found, versioned and asked about sign-in like the other two; whether
 * a mission may RUN under them is the host's decision, made per runtime on the
 * strength of a measured event stream.
 */
export type RuntimeIntegrationId =
  | "codex"
  | "claude"
  | "cursor"
  | "gemini"
  | "opencode"
  | "copilot"
  /**
   * Meta's terminal coding agent, discovered like the CLIs above.
   *
   * WHY IT IS HERE AT ALL, since the same model is already reachable: the
   * free route this whole product opens on is
   * `opencode/muse-spark-1.3-contributor-free` -- Muse THROUGH OpenCode --
   * and the second route to it is Cursor, who also resell it. Both are
   * arrangements between two other companies. Colin, 2026-09-21: *"we have
   * no control over whether opencode continues to support muse so its
   * better that we integrate it ourself through meta/muse"*.
   *
   * He is right, and the earlier assessment that talked him out of it was
   * wrong twice. It said "customers already have Muse twice" -- availability
   * TODAY, which is the exact reasoning `freeStartStillFree` exists to
   * reject -- and it rested on a blocker that has since lifted: native
   * Windows landed in Muse Code 1.3.0 (2026-09), and that document's own
   * revisit condition was "if native Windows support lands".
   *
   * What it does NOT hedge is the free tier. Muse's own CLI is a paid
   * subscription, so this is a bring-your-own-account runtime like Codex,
   * Claude, Cursor and Copilot. The free promise still rests on OpenCode
   * publishing something with a `-free` suffix, which is checked separately
   * and on its own evidence.
   */
  | "muse"
  /**
   * EXPERIMENTAL. Antigravity is a mission runtime, but it is not DISCOVERED
   * like the others: its `agentapi` refuses to start a conversation without a
   * project id that only its own protobuf state file holds, so no probe this
   * package could run would say anything true about it. The host owns finding
   * it; this package only normalizes the transcript it leaves behind.
   */
  | "antigravity"
  /**
   * Kilo Code's CLI (`kilo acp`), an OpenCode fork that speaks ACP.
   * Discovered on PATH. A fresh session's own model is not what a new run
   * uses: the host starts it on Auto Free. Signing in is only for paid models.
   */
  | "kilo"
  /**
   * AN AGENT THE PERSON ADDED (0.777): any program that speaks the Agent Client
   * Protocol -- Goose, Qwen Code, Kimi CLI, Mistral Vibe and the rest of the
   * list at agentclientprotocol.com -- by the command line the person gave it.
   * Never discovered: Locust knows only what that command answers over the
   * protocol. Which agent a mission ran on is its route's model, never this id.
   * It runs only in Approve each, the one mode the protocol itself makes true:
   * every action the agent asks to take is a card.
   */
  | "acp"
  | "omniroute";

/**
 * A runtime that can actually own a mission. OmniRoute is a gateway to other
 * providers rather than a runtime a mission runs under, so it is excluded by
 * construction instead of by remembering to exclude it at each use.
 */
export type MissionRuntimeId = Exclude<RuntimeIntegrationId, "omniroute">;

/** What discovery can find: everything but an agent the person added, which is known only by its command line. */
export type DiscoveredRuntimeId = Exclude<RuntimeIntegrationId, "acp">;

export type RuntimeIntegrationKind = "agent-runtime" | "provider-gateway";

export type RuntimeAvailability = "available" | "unavailable";

export type RuntimeReadiness =
  | "ready"
  | "authentication-required"
  | "unsupported"
  | "unhealthy"
  | "unknown";

export type RuntimeFeature =
  | "acp-stdio"
  | "non-interactive"
  | "jsonl-events"
  | "stdin-prompt"
  | "workspace-selection"
  | "read-only-sandbox"
  | "restricted-mode"
  | "verbose-streaming"
  | "partial-messages"
  | "plan-permission-mode"
  | "tool-allowlist"
  | "tool-denylist"
  | "json-health-check";

export type RuntimeDiagnosticCode =
  | "executable-not-found"
  /**
   * The runtime is installed and its version printed, but no command it offers
   * can tell whether it will actually accept work without spending the user's
   * quota. Reported at `info` beside a `ready` readiness so the picker offers
   * the route while the reason it is only a best guess stays on the record.
   */
  | "readiness-unverifiable"
  /** Installed, and the check that says more has not finished; shown as such, never as signed out. */
  | "check-pending"
  | "version-probe-failed"
  | "version-unrecognized"
  | "version-unsupported"
  | "capability-probe-failed"
  | "required-capability-missing"
  | "authentication-required"
  | "readiness-probe-failed"
  | "health-check-failed";

export interface RuntimeDiagnostic {
  readonly code: RuntimeDiagnosticCode;
  readonly severity: "info" | "warning" | "error";
  readonly message: string;
  readonly resolution?: string;
}

export interface ParsedRuntimeVersion {
  readonly raw: string;
  readonly version: string;
  readonly major: number;
  readonly minor: number;
  readonly patch: number;
  readonly prerelease?: string;
  readonly build?: string;
}

/**
 * A shell-free process target. A shim is represented as its real interpreter
 * plus fixed prefix arguments rather than `shell: true`, so nothing is ever
 * handed to a shell to re-parse.
 */
export interface ExecutableLaunch {
  readonly commandName: string;
  readonly discoveredPath: string;
  readonly executablePath: string;
  readonly prefixArgs: readonly string[];
  /**
   * `node-shim` is an npm `.cmd` shim resolved PAST cmd.exe to the node
   * script it wraps. cmd.exe ends a command line at the first newline and
   * refuses one past 8191 characters, so a runtime that takes its prompt as
   * an argument lost every flag after a multi-line prompt -- including the
   * one that made it read-only. Running the script under node directly has
   * neither limit. See the locator for the measurement.
   */
  readonly kind: "native" | "powershell-shim" | "cmd-shim" | "node-shim";
  /**
   * Environment the launch itself needs, merged into the command's.
   *
   * One thing uses it: running an npm script under the HOST's own Node,
   * which on Electron means running the app binary with
   * `ELECTRON_RUN_AS_NODE=1`. Without that variable the same path opens
   * another copy of the app, so the two travel together or not at all.
   */
  readonly env?: Readonly<Record<string, string>>;
}

export interface ProbeCommand {
  readonly purpose: "version" | "capabilities" | "readiness" | "models";
  readonly executablePath: string;
  readonly args: readonly string[];
  readonly timeoutMs: number;
  /**
   * The launch's own environment, merged over the host's (H7). A CLI run
   * under the app's own Node needs ELECTRON_RUN_AS_NODE, and a probe that
   * dropped it opened another copy of Locust instead of the CLI.
   */
  readonly env?: Readonly<Record<string, string>>;
}

export interface CommandResult {
  readonly exitCode: number | null;
  readonly stdout: string;
  readonly stderr: string;
  readonly timedOut?: boolean;
}

/** Implement this in the trusted host process. It must execute without a shell. */
export interface CommandRunner {
  run(command: ProbeCommand): Promise<CommandResult>;
}

export interface ExecutableLocator {
  find(commandName: string): Promise<ExecutableLaunch | undefined>;
}

/**
 * What a runtime's own CLI advertises about models, read from its help text at
 * discovery. Aliases resolve to the newest model in a family on the runtime's
 * side, so a new release appears here without a code change -- and nothing is
 * listed that the installed CLI did not itself name.
 */
export interface RuntimeModelHints {
  readonly aliases: readonly string[];
  readonly efforts: readonly string[];
  /** Ids with the names the runtime printed beside them, when it printed a list. */
  readonly models?: readonly RuntimeModelName[];
}

export interface RuntimeModelName {
  readonly id: string;
  readonly displayName: string;
  /**
   * A short qualifier the runtime itself justifies -- OpenCode's `-free`
   * suffix, or the fact that a route lets the CLI choose the model. Never a
   * description this package invented about a model it cannot see.
   */
  readonly description?: string;
  /**
   * The reasoning efforts the runtime listed for THIS model, when it lists
   * them per model (OpenCode's variants). Absent means none were listed.
   */
  readonly efforts?: readonly string[];
}

export interface RuntimeDiscovery {
  readonly id: DiscoveredRuntimeId;
  readonly kind: RuntimeIntegrationKind;
  readonly displayName: string;
  readonly optional: boolean;
  readonly availability: RuntimeAvailability;
  readonly readiness: RuntimeReadiness;
  readonly executable?: ExecutableLaunch;
  readonly version?: ParsedRuntimeVersion;
  readonly supportedFeatures: readonly RuntimeFeature[];
  readonly requiredFeatures: readonly RuntimeFeature[];
  readonly diagnostics: readonly RuntimeDiagnostic[];
  readonly modelHints?: RuntimeModelHints;
}

/** A future runner may bind the prompt to stdin; this package does not execute it. */
/**
 * How much a mission may touch. Declared here, beside the spec that carries
 * it, because the spec is what actually gets spawned: the runner validates
 * the argv again at that moment, and a spec that did not say what it was
 * allowed could only be judged by the strictest reading -- which refused a
 * mission the person had explicitly widened (measured driving Auto mode,
 * 2026-09-06).
 */
export type MissionSandbox = "read-only" | "workspace-write" | "full-access";

export interface RuntimeCommandSpec {
  readonly runtime: MissionRuntimeId;
  readonly executablePath: string;
  readonly args: readonly string[];
  readonly cwd: string;
  /**
   * Where the prompt is. `prompt` means the runner writes it to stdin, which
   * is what Codex, Claude Code and Gemini read. `none` means it is already in
   * `args`, because the CLI takes it as a positional and reading stdin was
   * never measured working -- see the OpenCode and Copilot builders.
   * `protocol` means it is in neither: the process is a server spoken to over
   * a request/response protocol, and the prompt travels inside a request.
   * The ordinary process runner refuses that spec rather than guessing, so a
   * protocol command can only be run by something that speaks the protocol.
   *
   * `prompt-file` means the runner writes the prompt to a temporary file and
   * substitutes its path for `PROMPT_FILE_PLACEHOLDER` in `args`. Muse Code
   * is the first runtime that needs it and the reason it exists:
   *
   *   - MEASURED 2026-09-21, `muse exec` with the prompt piped in exits 2
   *     with `usage: muse exec [OPTIONS] [PROMPT]`. It does not read stdin.
   *   - It is reached through `muse.cmd`, so `cmd.exe` caps the command line
   *     at 8,191 characters.
   *
   * Those two together are the failure OpenCode already had and already fixed
   * once: a 1,200-character reply quoted inside a 2,215-character standing
   * brief came to ~7,500 characters and was refused, and the person never got
   * their report (2026-09-17). OpenCode escaped it through stdin. Muse cannot,
   * so the prompt goes in a file instead of argv, and `commandTooLong` --
   * which only limits `stdin: "none"` -- correctly stops applying.
   *
   * `stream-json` is Claude Code's `--input-format stream-json`: the prompt
   * goes as the first user turn and input stays open until the turn's result,
   * so a message can be handed to the running turn (A2.10).
   *
   * `agy-json` is Antigravity CLI's `--input-format stream-json` (0.540): one
   * `{"event":"user",...}` line with the prompt as a text block, then stdin
   * closes -- MEASURED 2026-10-02, agy 1.2.14: a closed stdin ends the turn.
   */
  readonly stdin: "prompt" | "none" | "protocol" | "prompt-file" | "stream-json" | "agy-json";
  readonly stdout: "jsonl";
  /**
   * What this mission was allowed. Absent reads as `read-only` everywhere it
   * is judged, so a spec built without one cannot be widened by omission.
   */
  readonly sandbox?: MissionSandbox;
  /**
   * Variables this run needs on top of the runner's allowlist. OpenCode has no
   * read-only flag; its permission config arrives this way, so a spec that
   * claims read-only carries the thing that enforces it rather than trusting a
   * caller to remember.
   */
  readonly env?: Readonly<Record<string, string>>;
  /**
   * Each line the process writes to stderr also joins its records, in order,
   * as `{"type":"locust.stderr","line":...}`. For a runtime whose only word
   * about a retry is a log line (OpenCode: MEASURED 2026-09-25, `opencode run`
   * retrying a rate-limited provider prints nothing on stdout until it gives
   * up). Lines are bounded and the prompt is redacted from them, as it is
   * from the stderr kept for the completion.
   */
  readonly stderrRecords?: boolean;
  /**
   * A record too large to carry leaves a stand-in in its place (0.492):
   * `{"type":"locust.oversized","bytes":N,"recordType":"user","callIds":[...]}`,
   * never any of its content. For a
   * runtime whose tool results can be huge (Claude Code: an image, a whole
   * file), so the call the dropped result belonged to can still be closed --
   * it read "did not report" when the runtime had in fact reported, too much.
   */
  readonly oversizedStandIns?: boolean;
}
