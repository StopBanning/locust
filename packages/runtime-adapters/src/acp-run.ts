import { ACP_DECLINED, ACP_PROMPT_RESULT, ACP_SESSION } from "./acp-events.js";
import { createAppServerClient } from "./app-server.js";
import type { AppServerRequest, JsonValue } from "./app-server.js";
import { NotificationQueue } from "./codex-app-server-run.js";
import type { AppServerRunProcess } from "./codex-app-server-run.js";
import { unifiedDiffOf } from "./line-diff.js";
import type { RuntimeProcessCompletion, RuntimeProcessRun } from "./process-runner.js";
import type { RuntimeCommandSpec } from "./types.js";

/**
 * One turn over the Agent Client Protocol, shaped like a process run (0.377;
 * docs/PLAN-ACP-ROUTE-2026-09-26.md).
 *
 * ACP is JSON-RPC over the agent's stdin and stdout, one message a line --
 * the framing `createAppServerClient` already speaks for Codex. The run is
 * `initialize`, then `session/new` (or `session/load` to continue one), then
 * `session/prompt`, whose answer is the turn's end; the work arrives meanwhile
 * as `session/update` notifications, and the agent ASKS before acting with
 * `session/request_permission`. MEASURED on this machine, 2026-09-26, against
 * `copilot --acp` 1.0.88 and `opencode acp` 1.18.27 (test/fixtures/acp/):
 *
 * - Locust offers the agent NOTHING: no file reading or writing, no terminal
 *   (DECISION-2026-09-20). Every request but a permission is refused as a
 *   method this client does not have.
 * - `session/load` REPLAYS the whole conversation as updates before it
 *   answers. So nothing is recorded until the prompt has been sent: the
 *   replay is the past, and taking it for this turn would put the first
 *   turn's words in this one's mouth.
 * - A permission request offers options by id, and the ids differ by agent
 *   (Copilot `allow_once`, OpenCode `once`), so the answer is chosen by the
 *   option's KIND. "Always" is never passed on: whether an agent keeps it
 *   past this run, or in its own settings, is the agent's business and was
 *   not measured -- so Locust remembers it, for this run only. Since 0.616
 *   the main process does (shared/who-decides.ts), after the saved rules:
 *   this run asks about every request, and an Always goes back as "once".
 * - One session can move between Locust's two Copilot routes. A session the
 *   print route created loaded over ACP with its history, and the print route
 *   then resumed the session ACP had used, with the ACP turn in it.
 * - ACP has no way to add to a turn while it runs. What would be added -- a
 *   denial's reason, a message to a busy teammate -- is kept and sent as the
 *   next prompt the moment this one ends (`steer`). Copilot ends its turn on
 *   a refusal (measured), so a reason is read almost at once.
 *
 * The records are the updates, verbatim, and three the run writes itself
 * (see acp-events.ts): the session id, a refusal, a prompt's end.
 */

/** One option an agent offers on a permission request. */
export interface AcpPermissionOption {
  readonly optionId: string;
  /** `allow_once`, `allow_always`, `reject_once`, `reject_always`. */
  readonly kind: string;
  readonly name: string;
}

/** What an agent asks permission for, as far as a person needs it to decide. */
export interface AcpPermissionRequest {
  readonly toolCallId: string | undefined;
  /** The agent's own description of the call ("Print the required value from the shell"). */
  readonly title: string | undefined;
  /** ACP's tool kind: execute, edit, delete, move, read, search, fetch, think, other. */
  readonly kind: string | undefined;
  /** A shell call's command. */
  readonly command: string | undefined;
  /** The files it names. */
  readonly paths: readonly string[];
  /** An edit's change as a unified diff, when the request carries its before and after. */
  readonly diff: string | undefined;
  readonly options: readonly AcpPermissionOption[];
}

/** The person's answer, as the KIND of option it is. */
export type AcpPermissionAnswer = "allow_once" | "allow_always" | "reject_once";

export interface AcpMcpServer {
  readonly name: string;
  readonly command: string;
  readonly args: readonly string[];
  readonly env: readonly { readonly name: string; readonly value: string }[];
}

export interface AcpRunOptions {
  /** Started IN the folder as well as told it: the process's own folder and the session's are the same. */
  readonly spawn: (
    executablePath: string,
    args: readonly string[],
    env?: Readonly<Record<string, string>>,
    cwd?: string,
  ) => AppServerRunProcess;
  readonly command: RuntimeCommandSpec;
  readonly prompt: string;
  /** An earlier session to continue. */
  readonly resumeSessionId?: string;
  /** Per-run stdio MCP servers passed in session/new and session/load. */
  readonly mcpServers?: readonly AcpMcpServer[];
  /**
   * The mode the session must be in, by the agent's own id. A session loaded
   * in another mode is switched; an agent that does not offer it is not run.
   */
  readonly modeId?: string;
  /**
   * Settings that must hold before the prompt, by the agent's own config ids
   * (Copilot: `allow_all` off). One the agent does not offer is fine; one it
   * offers and will not set refuses the run.
   */
  readonly requiredConfig?: Readonly<Record<string, string>>;
  /** Refuse a requested setting the agent does not advertise (opt-in profiles). */
  readonly strictConfig?: boolean;
  /** Who answers. Absent: every request is refused, never approved. */
  readonly onPermission?: (request: AcpPermissionRequest) => Promise<AcpPermissionAnswer>;
  readonly signal?: AbortSignal;
  readonly now?: () => Date;
  /** How long the start may take, up to the prompt being sent. */
  readonly handshakeTimeoutMs?: number;
  /** How long a stopped turn is given to say it stopped before the process is ended anyway. */
  readonly cancelGraceMs?: number;
  readonly maxQueuedRecords?: number;
  /** What the agent said it can do, once it has answered `initialize` on this protocol's version (W12). */
  readonly onCapabilities?: (capabilities: AcpCapabilities) => void;
}

/**
 * WHAT AN ACP AGENT SAYS IT CAN DO (W12, 0.566), from its `initialize`
 * answer's `agentCapabilities`, in the protocol's own keys: `loadSession`,
 * `promptCapabilities` (`image`, `audio`, `embeddedContext`) and
 * `mcpCapabilities` (`http`, `sse`). Only `true` counts; a key this does not
 * know is left out, never guessed at. It is shown, never acted on: Locust's
 * own offer to the agent stays nothing (no files, no terminal).
 */
export interface AcpCapabilities {
  readonly continuesSessions: boolean;
  readonly images: boolean;
  readonly audio: boolean;
  readonly embeddedContext: boolean;
  readonly mcpOverHttp: boolean;
  readonly mcpOverSse: boolean;
}

export function acpCapabilitiesOf(agentCapabilities: unknown): AcpCapabilities {
  const given = isObject(agentCapabilities) ? agentCapabilities : {};
  const prompt = isObject(given.promptCapabilities) ? given.promptCapabilities : {};
  const mcp = isObject(given.mcpCapabilities) ? given.mcpCapabilities : {};
  return {
    continuesSessions: given.loadSession === true,
    images: prompt.image === true,
    audio: prompt.audio === true,
    embeddedContext: prompt.embeddedContext === true,
    mcpOverHttp: mcp.http === true,
    mcpOverSse: mcp.sse === true,
  };
}

export type AcpRun = RuntimeProcessRun & {
  /**
   * A message for the agent, sent as its next prompt once this one ends --
   * ACP cannot add to a turn while it runs. True when it will be sent; false
   * when the run has ended or been stopped.
   */
  steer(text: string): Promise<boolean>;
};

/** The protocol version this client speaks, and the only one it accepts. */
export const ACP_PROTOCOL_VERSION = 1;

const DEFAULT_HANDSHAKE_TIMEOUT_MS = 60_000;
const DEFAULT_CANCEL_GRACE_MS = 3_000;
const DEFAULT_MAX_QUEUED_RECORDS = 20_000;
/** A prompt takes as long as the work does; the handshake has its own clock. */
const PROMPT_TIMEOUT_MS = 24 * 60 * 60 * 1_000;
/** JSON-RPC's "Method not found": what an agent is told for anything never offered. */
const METHOD_NOT_FOUND = -32_601;

type JsonObject = Record<string, unknown>;
const isObject = (value: unknown): value is JsonObject => typeof value === "object" && value !== null && !Array.isArray(value);
const objectOf = (value: unknown): JsonObject => (isObject(value) ? value : {});
const text = (value: unknown): string | undefined => (typeof value === "string" && value.length > 0 ? value : undefined);

/** A permission request, read defensively: anything malformed is left out, never guessed at. */
export function acpPermissionRequestOf(params: unknown): AcpPermissionRequest {
  const body = objectOf(params);
  const call = objectOf(body.toolCall);
  const input = objectOf(call.rawInput);
  const content = Array.isArray(call.content) ? call.content.filter(isObject) : [];
  const diffs = content.filter((entry) => entry.type === "diff" && typeof entry.path === "string" && typeof entry.newText === "string");
  const diff = diffs
    .map((entry) => unifiedDiffOf(entry.path as string, (entry.oldText ?? null) as string | null, entry.newText as string))
    .filter((one): one is string => one !== undefined)
    .join("");
  const named = [
    ...(Array.isArray(call.locations) ? call.locations.map((location) => text(objectOf(location).path)) : []),
    ...diffs.map((entry) => text(entry.path)),
    text(input.path),
    text(input.file_path),
    text(input.filePath),
  ].filter((path): path is string => path !== undefined);
  const options = (Array.isArray(body.options) ? body.options : [])
    .filter(isObject)
    .map((option) => ({ optionId: text(option.optionId), kind: text(option.kind), name: text(option.name) ?? "" }))
    .filter((option): option is AcpPermissionOption => option.optionId !== undefined && option.kind !== undefined);
  return {
    toolCallId: text(call.toolCallId),
    title: text(call.title),
    kind: text(call.kind),
    command: text(input.command),
    paths: [...new Set(named)],
    diff: diff.length === 0 ? undefined : diff,
    options,
  };
}

/** A config option's current value, when the agent offers that option. */
function configValue(configOptions: unknown, id: string): string | undefined {
  if (!Array.isArray(configOptions)) return undefined;
  const option = configOptions.filter(isObject).find((entry) => entry.id === id);
  return option === undefined ? undefined : text(option.currentValue);
}

export function startAcpRun(options: AcpRunOptions): AcpRun {
  const now = options.now ?? (() => new Date());
  const startedAt = now().toISOString();
  const records = new NotificationQueue(options.maxQueuedRecords ?? DEFAULT_MAX_QUEUED_RECORDS);

  let recordCount = 0;
  let settled = false;
  let cancelled = false;
  let lost = false;
  let why = "";
  let sessionId: string | undefined;
  /** This run's updates begin with its first prompt; before that is the past. */
  let recording = false;
  let prompting = false;
  let transportEnded = false;
  let killed = false;
  let handshakeTimer: ReturnType<typeof setTimeout> | undefined;
  let cancelTimer: ReturnType<typeof setTimeout> | undefined;
  /** What is waiting to be said once the prompt running now ends. */
  const queued: string[] = [];
  /**
   * Identical requests are put to the person one at a time. An agent can run
   * tools side by side, and a second card for the same command, raised while
   * the first was still up, would not know the answer the first then got.
   */
  const inLine = new Map<string, Promise<unknown>>();
  const oneAtATime = async <T>(signature: string | undefined, work: () => Promise<T>): Promise<T> => {
    if (signature === undefined) return await work();
    const mine = (inLine.get(signature) ?? Promise.resolve()).catch(() => undefined).then(work);
    inLine.set(signature, mine);
    try {
      return await mine;
    } finally {
      if (inLine.get(signature) === mine) inLine.delete(signature);
    }
  };
  let settleCompletion!: (completion: RuntimeProcessCompletion) => void;
  const completion = new Promise<RuntimeProcessCompletion>((resolve) => {
    settleCompletion = resolve;
  });
  /** Settles when the run is stopped, so a question still open is answered "cancelled". */
  let markStopped!: () => void;
  const stopping = new Promise<undefined>((resolve) => {
    markStopped = () => resolve(undefined);
  });

  const child = options.spawn(options.command.executablePath, options.command.args, options.command.env, options.command.cwd);
  const killOnce = (): void => {
    if (killed) return;
    killed = true;
    child.kill();
  };

  const finish = (): void => {
    if (settled) return;
    settled = true;
    markStopped();
    clearTimeout(handshakeTimer);
    clearTimeout(cancelTimer);
    options.signal?.removeEventListener("abort", stop);
    records.end();
    try {
      client.dispose("The turn ended.");
    } catch {
      // Already gone; the receipt below is what the mission reads.
    }
    settleCompletion({
      // Exit 0 means the run ended on its last prompt's answer (acp-events
      // reads it so); a stop or a lost transport has no exit code to report.
      exitCode: cancelled || lost ? null : 0,
      signal: null,
      stderr: why,
      stderrTruncated: false,
      recordCount,
      cancelled,
      forcedTerminationAttempted: false,
      terminationUnconfirmed: false,
      inputDeliveryFailed: false,
      outputLimitExceeded: records.droppedForOverflow,
      oversizedRecordsDropped: 0,
      startedAt,
      finishedAt: now().toISOString(),
    });
  };
  const lose = (reason: string): void => {
    if (settled) return;
    lost = true;
    why = why.length > 0 ? why : reason;
    finish();
  };
  const push = (method: string, params: unknown): void => {
    if (settled) return;
    recordCount += 1;
    records.push({ sequence: recordCount, raw: JSON.stringify({ method, params: params ?? null }) });
    if (records.droppedForOverflow) {
      lose("The agent sent output faster than Locust could record it, so the run was stopped rather than leave a gap in its record. Sending it again usually works.");
    }
  };

  const answer = async (request: AppServerRequest): Promise<JsonValue> => {
    const asked = acpPermissionRequestOf(request.params);
    const target = asked.command ?? (asked.paths.length === 0 ? undefined : asked.paths.join("\n"));
    const signature = target === undefined ? undefined : `${asked.kind ?? ""}\u0000${target}`;
    const choose = (kind: AcpPermissionAnswer): JsonValue => {
      const option = asked.options.find((offered) => offered.kind === kind);
      // Anything but a plain yes -- a refusal, or no option to say it with --
      // is written down BEFORE the answer goes back, so it precedes the failed
      // call it explains.
      if ((option === undefined || kind === "reject_once") && asked.toolCallId !== undefined) {
        push(ACP_DECLINED, { toolCallId: asked.toolCallId });
      }
      return option === undefined ? { outcome: { outcome: "cancelled" } } : { outcome: { outcome: "selected", optionId: option.optionId } };
    };
    return await oneAtATime(signature, async (): Promise<JsonValue> => {
      if (settled || cancelled) return { outcome: { outcome: "cancelled" } };
      let given: AcpPermissionAnswer | undefined = "reject_once";
      if (options.onPermission !== undefined) {
        try {
          given = await Promise.race([options.onPermission(asked), stopping]);
        } catch {
          given = "reject_once";
        }
      }
      // Stopped while the card was up: ACP asks for exactly this answer.
      if (given === undefined || settled || cancelled) return { outcome: { outcome: "cancelled" } };
      // Remembered by the host, never by the agent: "once" to the agent.
      if (given === "allow_always") return choose("allow_once");
      return choose(given);
    });
  };

  const client = createAppServerClient({
    transport: {
      send: (line: string) => child.write(line),
      close: killOnce,
    },
    requestTimeoutMs: PROMPT_TIMEOUT_MS,
    onNotification: (notification) => {
      if (notification.method !== "session/update" || !recording) return;
      const params = objectOf(notification.params);
      if (sessionId !== undefined && typeof params.sessionId === "string" && params.sessionId !== sessionId) return;
      // The machine's own commands and skills are never this run's record.
      if (objectOf(params.update).sessionUpdate === "available_commands_update") return;
      push("session/update", params);
    },
    onRequest: async (request) => {
      if (request.method === "session/request_permission") return await answer(request);
      throw Object.assign(new Error("Locust offers this agent no files and no terminal."), { code: METHOD_NOT_FOUND });
    },
    onDiagnostic: (diagnostic) => {
      if (diagnostic.code === "buffer-overflow" || diagnostic.code === "line-too-long") lose(diagnostic.message);
    },
  });

  child.onData((chunk) => client.accept(chunk));
  child.onExit(() => {
    lose("The agent exited before its turn finished.");
  });
  child.onStdoutClose?.((error) => {
    transportEnded = true;
    // A final response was accepted synchronously from the last data chunk.
    // Let its awaiting prompt consume it before treating EOF as a lost turn.
    queueMicrotask(() => lose(error === undefined
      ? "The agent's output stream closed before its turn finished."
      : "The agent's output stream failed before its turn finished."));
  });

  const requireLive = (): void => {
    if (settled || cancelled || transportEnded) throw new Error("The agent's turn has already ended.");
  };

  const handshake = async (): Promise<void> => {
    requireLive();
    const init = objectOf(
      await client.request("initialize", {
        protocolVersion: ACP_PROTOCOL_VERSION,
        // NOTHING offered: no agent may ask Locust to read, write or run anything.
        clientCapabilities: { fs: { readTextFile: false, writeTextFile: false }, terminal: false },
      }),
    );
    requireLive();
    if (init.protocolVersion !== ACP_PROTOCOL_VERSION) {
      throw new Error(`The agent speaks version ${String(init.protocolVersion)} of the Agent Client Protocol, and Locust speaks ${String(ACP_PROTOCOL_VERSION)}.`);
    }
    try {
      options.onCapabilities?.(acpCapabilitiesOf(init.agentCapabilities));
    } catch {
      // What it can do is shown in Settings; a listener's failure never stops the turn.
    }
    requireLive();
    const cwd = options.command.cwd;
    const mcpServers: JsonValue[] = (options.mcpServers ?? []).map((server) => ({
      name: server.name,
      command: server.command,
      args: [...server.args],
      env: server.env.map((entry) => ({ name: entry.name, value: entry.value })),
    }));
    let session: JsonObject;
    if (options.resumeSessionId === undefined) {
      session = objectOf(await client.request("session/new", { cwd, mcpServers }));
      requireLive();
      sessionId = text(session.sessionId);
      if (sessionId === undefined) throw new Error("The agent did not start a session.");
    } else {
      if (objectOf(init.agentCapabilities).loadSession !== true) throw new Error("The agent cannot continue an earlier session.");
      sessionId = options.resumeSessionId;
      session = objectOf(await client.request("session/load", { sessionId, cwd, mcpServers }));
      requireLive();
    }
    push(ACP_SESSION, { sessionId });

    if (options.modeId !== undefined) {
      const modes = objectOf(session.modes);
      const offered = (Array.isArray(modes.availableModes) ? modes.availableModes : []).map((mode) => text(objectOf(mode).id));
      if (!offered.includes(options.modeId)) throw new Error("The agent did not offer the mode this run needs, so it was not started.");
      if (modes.currentModeId !== options.modeId) {
        await client.request("session/set_mode", { sessionId, modeId: options.modeId });
        requireLive();
      }
    }
    for (const [configId, value] of Object.entries(options.requiredConfig ?? {})) {
      const current = configValue(session.configOptions, configId);
      if (current === undefined && options.strictConfig === true) throw new Error(`The agent did not offer the required ${configId} setting, so it was not started.`);
      if (current === undefined || current === value) continue;
      const set = objectOf(await client.request("session/set_config_option", { sessionId, configId, value }));
      requireLive();
      if (configValue(set.configOptions, configId) !== value) throw new Error(`The agent would not set ${configId} to ${value}, so it was not started.`);
      // Selecting a model can change the advertised effort/mode options.
      session = { ...session, ...set };
    }
  };

  const converse = async (): Promise<void> => {
    let next: string | undefined = options.prompt;
    while (next !== undefined && !settled && !cancelled && !transportEnded) {
      recording = true;
      prompting = true;
      const ended = objectOf(await client.request("session/prompt", { sessionId: sessionId!, prompt: [{ type: "text", text: next }] }));
      if (settled) return;
      prompting = false;
      push(ACP_PROMPT_RESULT, { stopReason: text(ended.stopReason) ?? null, ...(isObject(ended.usage) ? { usage: ended.usage } : {}) });
      // A turn cut off at a limit still sends what was queued during it: steer() said it would (2026-10-10 sweep).
      if (cancelled || (ended.stopReason !== "end_turn" && ended.stopReason !== "max_tokens" && ended.stopReason !== "max_turn_requests")) break;
      next = queued.length === 0 ? undefined : queued.splice(0).join("\n\n");
    }
    finish();
  };

  function stop(): void {
    if (settled || cancelled) return;
    cancelled = true;
    markStopped();
    if (sessionId === undefined || !prompting) {
      finish();
      return;
    }
    // ACP's own stop: the prompt then answers "cancelled" and the run ends
    // there. An agent that does not is ended anyway.
    try {
      client.notify("session/cancel", { sessionId });
    } catch {
      finish();
      return;
    }
    cancelTimer = setTimeout(finish, options.cancelGraceMs ?? DEFAULT_CANCEL_GRACE_MS);
    if (typeof cancelTimer === "object" && cancelTimer !== null && "unref" in cancelTimer) cancelTimer.unref();
  }
  if (options.signal !== undefined) {
    if (options.signal.aborted) stop();
    else options.signal.addEventListener("abort", stop, { once: true });
  }

  if (!settled) {
    handshakeTimer = setTimeout(() => lose("The agent did not answer in time."), options.handshakeTimeoutMs ?? DEFAULT_HANDSHAKE_TIMEOUT_MS);
    void (async () => {
      await handshake();
      clearTimeout(handshakeTimer);
      if (!settled && !cancelled && !transportEnded) await converse();
    })().catch((error: unknown) => {
      lose(error instanceof Error && error.message.length > 0 ? error.message : "The agent could not start its turn.");
    });
  }

  const steer = async (said: string): Promise<boolean> => {
    if (settled || cancelled || transportEnded || said.trim().length === 0) return false;
    queued.push(said);
    return true;
  };

  return { records, completion, steer };
}
