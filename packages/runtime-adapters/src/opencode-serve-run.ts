import { createHash, randomBytes } from "node:crypto";

import { NotificationQueue } from "./codex-app-server-run.js";
import type { AppServerRunProcess } from "./codex-app-server-run.js";
import type { RuntimeCommandSpec } from "./types.js";
import type { RuntimeProcessCompletion, RuntimeProcessRun } from "./process-runner.js";

/**
 * OpenCode through its own server, so a run can stop and ASK (A6.7).
 *
 * `opencode run` cannot ask anyone: a permission set to "ask" is rejected on
 * the spot. `opencode serve` raises it instead. MEASURED 2026-09-25 on
 * 1.18.27 with the free Ling: a shell call under `bash: "ask"` raised
 * `permission.asked` on `/event` -- the command, its patterns, a suggested
 * "always" pattern and the tool call -- `POST /permission/{id}/reply
 * {reply:"once"}` ran it, and `{reply:"reject", message}` failed the call
 * while the model read the reason and still answered.
 *
 * The run's records are the ones `opencode run --format json` prints --
 * `step_start`, `text`, `tool_use`, `step_finish`, `error` -- rebuilt from the
 * server's part events, so the OpenCode normalizer every other OpenCode run
 * goes through reads this one unchanged. The part shapes are the server's
 * own; `run` prints the same parts once they settle (captured the same day).
 *
 * The server listens on localhost and, left to its defaults, answers anyone
 * on the machine -- including a request that approves a shell command. So
 * every server this starts gets a random password (OPENCODE_SERVER_PASSWORD;
 * measured: 401 without it, 200 with it), passed in the child's environment
 * only, never in argv or the record.
 */
export interface OpenCodePermission {
  readonly id: string;
  /** What kind of thing: `bash`, `edit`, `external_directory`, `webfetch`... */
  readonly permission: string;
  readonly patterns: readonly string[];
  /** What OpenCode would remember on "always". */
  readonly always: readonly string[];
  readonly metadata: Readonly<Record<string, unknown>>;
  /** Asked by a subagent the run started (its `task` tool), not the run itself (R36). */
  readonly bySubagent?: boolean;
}

export type OpenCodePermissionReply = "once" | "always" | "reject";
/** A reply, or a rejection with what the person said about it -- the model reads the message (0.374). */
export type OpenCodePermissionAnswer = OpenCodePermissionReply | { readonly reply: "reject"; readonly message: string };

export interface OpenCodeServeRunOptions {
  /**
   * Started IN the teammate's folder (0.378). A session made without naming
   * a folder is the server's own folder's, and a server started with none
   * stood where the app stands -- in real use its install directory, which
   * the app refuses as a workspace. Every drive passed anyway, because the
   * drives launched the app IN the workspace (drive-lib `launchElsewhere`).
   */
  readonly spawn: (
    executablePath: string,
    args: readonly string[],
    env?: Readonly<Record<string, string>>,
    cwd?: string,
  ) => AppServerRunProcess;
  readonly command: RuntimeCommandSpec;
  readonly prompt: string;
  /** `provider/model`, as every other OpenCode route names it. */
  readonly model?: string;
  readonly variant?: string;
  readonly resumeSessionId?: string;
  /**
   * One of OpenCode's own commands, typed by the person (0.427): sent as
   * `POST /session/{id}/command`, with `prompt` as its arguments, instead of
   * as a message.
   */
  readonly slashCommand?: string;
  /** Who answers. Absent: every request is refused, never approved -- unless `approveAll`. */
  readonly onPermission?: (request: OpenCodePermission) => Promise<OpenCodePermissionAnswer>;
  /**
   * Auto (0.677): every request the config did not deny is approved, as `run --auto` approves "permissions that are
   * not explicitly denied". Only for a run in Auto; never with `onPermission`.
   */
  readonly approveAll?: boolean;
  readonly signal?: AbortSignal;
  readonly now?: () => Date;
  /** Test seam. */
  readonly fetch?: typeof fetch;
  readonly startupTimeoutMs?: number;
  /** How long a taken message may show no sign of the model before the server is asked whether it began (0.763). */
  readonly beginCheckMs?: number;
  /** Overall recovery bound after premature event-stream EOF; busy turns may take up to five minutes. */
  readonly streamCloseTimeoutMs?: number;
  readonly streamClosePollMs?: number;
  /** Bound each status/message check, including response bodies, independently of a busy session. */
  readonly streamCloseRequestTimeoutMs?: number;
}

const LISTENING = /listening on (http:\/\/127\.0\.0\.1:\d+)/;
/*
 * A server that dies before it listens, its last words "database is locked", lost a race for its storage with another
 * OpenCode starting on this computer (0.810: a panel of three judges started three servers at once, and one died so).
 * Nothing had been sent to it, so it starts again: later each time, at a jittered moment, at most three times.
 */
const STORAGE_BUSY = /database is locked|SQLITE_BUSY/i;
const STORAGE_RESTARTS = 3;
const EXITED = "OpenCode's server exited before the turn finished.";

type JsonObject = Record<string, unknown>;
const isObject = (value: unknown): value is JsonObject => typeof value === "object" && value !== null && !Array.isArray(value);
const text = (value: unknown): string | undefined => (typeof value === "string" && value.length > 0 ? value : undefined);

type SessionMessage = { info: JsonObject; parts: unknown[] };
const finite = (value: unknown): number | undefined => typeof value === "number" && Number.isFinite(value) ? value : undefined;
const compareIds = (a: string, b: string): number => a < b ? -1 : a > b ? 1 : 0;

/** OpenCode orders messages by creation time, then id; its part ids carry their creation order. */
function chronologicalMessages(messages: SessionMessage[]): SessionMessage[] {
  return messages.sort((a, b) => {
    const createdA = finite(isObject(a.info.time) ? a.info.time.created : undefined) ?? 0;
    const createdB = finite(isObject(b.info.time) ? b.info.time.created : undefined) ?? 0;
    return createdA - createdB || compareIds(text(a.info.id)!, text(b.info.id)!);
  });
}

function chronologicalParts(parts: JsonObject[]): JsonObject[] {
  if (parts.every((part) => text(part.id)?.startsWith("prt_") === true)) {
    return parts.sort((a, b) => compareIds(text(a.id)!, text(b.id)!));
  }
  // Imported/id-less parts may still carry a clock. Sort within each step;
  // untimed markers stay in place, including multiple steps in one message.
  const ordered: JsonObject[] = [];
  let span: JsonObject[] = [];
  const flush = (): void => {
    const timed = span.map((part) => {
      const time = isObject(part.time) ? part.time : isObject(part.state) && isObject(part.state.time) ? part.state.time : {};
      return { part, at: finite(time.start) };
    });
    ordered.push(...(timed.every((entry) => entry.at !== undefined)
      ? timed.sort((a, b) => a.at! - b.at!).map((entry) => entry.part) : span));
    span = [];
  };
  for (const part of parts) {
    if (part.type === "step-start" || part.type === "step-finish") {
      flush();
      ordered.push(part);
    } else span.push(part);
  }
  flush();
  return ordered;
}

/** Full, key-order-independent fallback: two long tool results must not share an 80-character prefix key. */
function partSignature(part: JsonObject): string {
  return createHash("sha256").update(JSON.stringify(part, (_key, value: unknown) => isObject(value)
    ? Object.fromEntries(Object.entries(value).sort(([a], [b]) => compareIds(a, b))) : value)).digest("hex");
}

/**
 * The `run --format json` record for one settled part, or undefined.
 *
 * Exported for the tests, which hold it to the captured shapes: a step's
 * start and finish once each, a tool once it has completed or failed, and a
 * text part once it has ended -- the person's own prompt never (it has no
 * end), and OpenCode's synthetic parts always, because the compaction note
 * the normalizer reads arrives on a message of its own.
 */
export function runRecordFor(
  part: JsonObject,
  role: string | undefined,
): { readonly type: string; readonly part: JsonObject } | undefined {
  const type = text(part.type);
  if (type === "step-start") return role === "assistant" ? { type: "step_start", part } : undefined;
  if (type === "step-finish") return role === "assistant" ? { type: "step_finish", part } : undefined;
  if (type === "tool") {
    const status = isObject(part.state) ? text(part.state.status) : undefined;
    return role === "assistant" && (status === "completed" || status === "error") ? { type: "tool_use", part } : undefined;
  }
  if (type === "text") {
    const ended = isObject(part.time) && typeof part.time.end === "number";
    // Still being written (0.677): its text so far, for the reply to grow on screen as it is written.
    if (!ended) return role === "assistant" && part.synthetic !== true ? { type: "text_partial", part } : undefined;
    return role === "assistant" || part.synthetic === true ? { type: "text", part } : undefined;
  }
  return undefined;
}

/**
 * How often a growing text part is passed on (0.677): about eight times a second reads as live, and the server
 * reports every token -- each with the whole text so far.
 */
export const PARTIAL_EVERY_MS = 120;

export function startOpenCodeServeRun(options: OpenCodeServeRunOptions): RuntimeProcessRun {
  const now = options.now ?? (() => new Date());
  const request = options.fetch ?? fetch;
  const startedAt = now().toISOString();
  const records = new NotificationQueue(20_000);
  const password = randomBytes(24).toString("base64url");
  const auth = { authorization: `Basic ${Buffer.from(`opencode:${password}`).toString("base64")}` };
  const events = new AbortController();

  let recordCount = 0;
  let settled = false;
  let cancelled = false;
  let errored = false;
  let why = "";
  let base: string | undefined;
  let sessionId: string | undefined;
  let prompted = false;
  let settleCompletion!: (completion: RuntimeProcessCompletion) => void;
  const completion = new Promise<RuntimeProcessCompletion>((resolve) => {
    settleCompletion = resolve;
  });
  const roles = new Map<string, string>();
  /** The first new user message streamed after this run sends its prompt, never resumed history. */
  let promptId: string | undefined;
  const emitted = new Set<string>();
  /** When each part's text was last sent while it grew. */
  const partialAt = new Map<string, number>();
  /*
   * Each part's kind and text so far (0.677). The server streams a part as `message.part.delta` -- its id, the
   * field, a few characters -- and names its kind only in `message.part.updated` (MEASURED 2026-10-06, OpenCode
   * 1.18.27: one reply, 208 deltas, 7 updates; the model's reasoning streams the same way and is not the reply).
   */
  const parts = new Map<string, { type?: string | undefined; messageID?: string | undefined; synthetic?: boolean; text: string }>();
  /** What of each part was last passed on while it grew. */
  const partialSent = new Map<string, string>();
  const sendPartial = (id: string, now = false): void => {
    const held = parts.get(id)
    if (held === undefined || held.type !== "text" || held.synthetic === true) return;
    if (roles.get(held.messageID ?? "") !== "assistant" || emitted.has(`text:${id}`)) return;
    if (held.text.length === 0 || partialSent.get(id) === held.text) return;
    const last = partialAt.get(id) ?? 0;
    if (!now && Date.now() - last < PARTIAL_EVERY_MS) return;
    partialAt.set(id, Date.now());
    partialSent.set(id, held.text);
    push({ type: "text_partial", part: { id, type: "text", text: held.text, ...(held.messageID === undefined ? {} : { messageID: held.messageID }) } });
  };
  /*
   * Everything held back by the pace, passed on before a tool or a step's end (0.677): the normalizer closes what
   * streamed when one comes, and closed with the last paced text it kept a reply two characters short of its
   * finished form -- which the screen, the reply being closed, did not take (drive-a-streamed-reply-arrives-whole).
   */
  const flushPartials = (): void => {
    for (const id of parts.keys()) sendPartial(id, true);
  };

  const spawnServer = (): AppServerRunProcess => options.spawn(
    options.command.executablePath,
    options.command.args,
    {
      ...(options.command.env ?? {}),
      OPENCODE_SERVER_PASSWORD: password,
    },
    options.command.cwd,
  );
  let child = spawnServer();

  const finish = (): void => {
    if (settled) return;
    settled = true;
    if (beginCheck !== undefined) clearTimeout(beginCheck);
    if (checkTimeout !== undefined) clearTimeout(checkTimeout);
    if (streamCloseTimeout !== undefined) clearTimeout(streamCloseTimeout);
    if (stopTimeout !== undefined) clearTimeout(stopTimeout);
    if (restartTimer !== undefined) clearTimeout(restartTimer);
    events.abort();
    records.end();
    try {
      child.kill();
    } catch {
      // Already gone.
    }
    settleCompletion({
      exitCode: cancelled ? null : errored || why.length > 0 ? 1 : 0,
      signal: null,
      // The server's own last words after ours (N10), when it ended on a reason.
      stderr: why.length > 0 && (child.stderrTail?.().trim() ?? "").length > 0 ? `${why}${String.fromCharCode(10)}${child.stderrTail!().trim()}` : why,
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
    why = why.length > 0 ? why : reason;
    finish();
  };
  const push = (record: JsonObject): void => {
    if (settled) return;
    recordCount += 1;
    records.push({ sequence: recordCount, raw: JSON.stringify({ ...record, timestamp: Date.now(), sessionID: sessionId }) });
    if (records.droppedForOverflow) lose("OpenCode sent output faster than Locust could record it, so the run was stopped rather than leave a gap in its record. Sending it again usually works.");
  };
  const call = async (path: string, body?: unknown): Promise<Response> =>
    request(`${base!}${path}`, {
      method: "POST",
      signal: events.signal,
      headers: { ...auth, "content-type": "application/json" },
      body: JSON.stringify(body ?? {}),
    });

  const answer = async (asked: JsonObject): Promise<void> => {
    const id = text(asked.id);
    if (id === undefined) return;
    const permission: OpenCodePermission = {
      id,
      permission: text(asked.permission) ?? "unknown",
      patterns: Array.isArray(asked.patterns) ? asked.patterns.filter((entry): entry is string => typeof entry === "string") : [],
      always: Array.isArray(asked.always) ? asked.always.filter((entry): entry is string => typeof entry === "string") : [],
      metadata: isObject(asked.metadata) ? asked.metadata : {},
      ...(text(asked.sessionID) !== undefined && text(asked.sessionID) !== sessionId ? { bySubagent: true } : {}),
    };
    let reply: OpenCodePermissionReply = "reject";
    let message = "The person declined this in Locust.";
    try {
      const answered = options.onPermission !== undefined ? await options.onPermission(permission) : options.approveAll === true ? "once" : "reject";
      if (typeof answered === "string") {
        reply = answered;
      } else {
        reply = answered.reply;
        message = answered.message;
      }
    } catch {
      reply = "reject";
    }
    if (settled) return;
    await call(`/permission/${encodeURIComponent(id)}/reply`, reply === "reject"
      ? { reply, message }
      : { reply }).catch(() => undefined);
  };

  /*
   * A MESSAGE TAKEN AND NEVER BEGUN (0.763). The server answers `prompt_async` at once and works on it after. On
   * 2026-10-12 its own storage was locked by another OpenCode on the same computer -- two comparison columns at once
   * -- and the work failed inside it ("prompt_async failed ... SQLiteError: database is locked", in its log), and
   * nothing more came: no error, no idle, a turn on "Starting" for eight minutes. So once a message is taken, a minute
   * with no sign of the model working has the server asked: a session it does not list as busy, holding no answer, was
   * dropped, and the run ends saying so -- to be sent again (a benchmark's run asks such a model once more itself).
   */
  let begun = false;
  let beginCheck: ReturnType<typeof setTimeout> | undefined;
  let checkTimeout: ReturnType<typeof setTimeout> | undefined;
  let streamClosed = false;
  let streamCloseTimeout: ReturnType<typeof setTimeout> | undefined;
  let stopTimeout: ReturnType<typeof setTimeout> | undefined;
  let restartTimer: ReturnType<typeof setTimeout> | undefined;
  let checking = false;
  let busy = false;
  const closedReason = "OpenCode's event stream closed before the turn finished.";
  // Reuse the watchdog's status/message check after EOF, even if the model began.
  const checkBegun = async (): Promise<void> => {
    if (beginCheck !== undefined) clearTimeout(beginCheck);
    beginCheck = undefined;
    if (checking) return;
    if (settled || cancelled || (!streamClosed && begun) || base === undefined || sessionId === undefined) return;
    checking = true;
    const checkAbort = new AbortController();
    checkTimeout = setTimeout(() => {
      checkAbort.abort();
      if (streamClosed && !settled && !cancelled) lose(`${closedReason} The status check did not finish in time; its outcome could not be confirmed.`);
    }, options.streamCloseRequestTimeoutMs ?? 10_000);
    try {
      const signal = AbortSignal.any([events.signal, checkAbort.signal, ...(options.signal === undefined ? [] : [options.signal])]);
      const response = await request(`${base}/session/status`, { headers: auth, signal });
      if (settled || cancelled) return;
      if (!response.ok) throw new Error("OpenCode's session status could not be read.");
      const statuses: unknown = await response.json();
      if (settled || cancelled) return;
      if (!isObject(statuses)) throw new Error("OpenCode's session status was not readable.");
      const status = statuses[sessionId];
      const kind = isObject(status) ? text(status.type) : undefined;
      if (status !== undefined && kind !== "busy" && kind !== "retry" && kind !== "idle") throw new Error("OpenCode's session status could not be identified.");
      busy = kind === "busy" || kind === "retry";
      if (busy) return void scheduleBeginCheck();
      const messagesResponse = await request(`${base}/session/${encodeURIComponent(sessionId)}/message`, { headers: auth, signal });
      if (settled || cancelled) return;
      if (!messagesResponse.ok) throw new Error("OpenCode's session messages could not be read.");
      const messages: unknown = await messagesResponse.json();
      if (settled || cancelled) return;
      if (!Array.isArray(messages)) throw new Error("OpenCode's session messages were not readable.");
      if (!streamClosed) {
        const answered = messages.some((one: unknown) => isObject(one) && (text(isObject(one.info) ? one.info.role : undefined) ?? text(one.role)) === "assistant");
        if (answered || begun) return void scheduleBeginCheck();
        lose("OpenCode took the message but never began on it: its own storage was likely busy with another OpenCode on this computer. Send it again.");
        return;
      }
      // The streamed prompt's id ties even an unobserved answer to this turn.
      // Without that receipt only a fresh session's observed reply is safe.
      const turnMessages = chronologicalMessages(messages.filter((message: unknown): message is SessionMessage => {
        if (!isObject(message) || !isObject(message.info) || !Array.isArray(message.parts)) return false;
        const info = message.info;
        const id = text(info.id);
        const current = promptId !== undefined ? info.parentID === promptId
          : options.resumeSessionId === undefined && id !== undefined && roles.get(id) === "assistant";
        return id !== undefined && current && info.role === "assistant"
          && info.sessionID === sessionId;
      }));
      // An earlier stop cannot prove completion while a later step is unfinished.
      const completed = turnMessages.at(-1);
      if (completed === undefined || completed.info.finish !== "stop" || completed.info.error !== undefined
        || !isObject(completed.info.time) || finite(completed.info.time.completed) === undefined
        || !completed.parts.some((part: unknown) => isObject(part) && part.messageID === completed.info.id
          && part.sessionID === sessionId && part.type === "step-finish" && part.reason === "stop")) {
        lose(`${closedReason} The session is no longer busy, but this turn's completion could not be confirmed.`);
        return;
      }
      // Replay every step of this prompt, through the live path's settled-part
      // deduplication. Step finishes also carry the turn's token/cost receipts.
      for (const message of turnMessages) {
        if (settled || cancelled) return;
        accept({ type: "message.updated", properties: { sessionID: sessionId, info: message.info } });
        const ownedParts = message.parts.filter((part: unknown): part is JsonObject => isObject(part)
          && part.messageID === message.info.id && part.sessionID === sessionId);
        for (const part of chronologicalParts(ownedParts)) {
          if (settled || cancelled) return;
          accept({ type: "message.part.updated", properties: { sessionID: sessionId, part } });
        }
      }
      finish();
    } catch {
      if (streamClosed) {
        if (!settled && !cancelled) lose(`${closedReason} The server's final status could not be read; its outcome could not be confirmed.`);
      } else scheduleBeginCheck();
    } finally {
      clearTimeout(checkTimeout);
      checkTimeout = undefined;
      checking = false;
    }
  };
  const scheduleBeginCheck = (): void => {
    if (settled || cancelled || (!streamClosed && begun) || beginCheck !== undefined) return;
    beginCheck = setTimeout(() => void checkBegun(), streamClosed ? options.streamClosePollMs ?? 250 : options.beginCheckMs ?? 60_000);
  };

  const children = new Set<string>();
  const accept = (event: JsonObject): void => {
    const type = text(event.type);
    const props = isObject(event.properties) ? event.properties : {};
    const about = text(props.sessionID) ?? (isObject(props.info) ? text(props.info.sessionID) : undefined)
      ?? (isObject(props.part) ? text(props.part.sessionID) : undefined);
    if (sessionId === undefined) return;
    /*
     * A SUBAGENT'S REQUEST IS THE RUN'S (QA-2026-09-29 round 2, R36). The
     * `task` tool runs a subagent as a CHILD session of this server, and its
     * requests come on this stream with the child's id. Only the run's own id
     * was read, so a subagent's `bash` was neither shown nor answered and the
     * run sat on "Working..." waiting for it. Children are followed from
     * `session.created` (its `parentID`), grandchildren too; their requests
     * are asked like the run's own, and nothing else of theirs is read.
     */
    if (type === "session.created" || type === "session.updated") {
      const info = isObject(props.info) ? props.info : {};
      const child = text(info.id);
      const parent = text(info.parentID);
      if (child !== undefined && parent !== undefined && (parent === sessionId || children.has(parent))) children.add(child);
      if (about !== sessionId) return;
    }
    if (about === undefined) return;
    if (about !== sessionId) {
      if (children.has(about) && type === "permission.asked") void answer(props);
      return;
    }
    // A sign the model is at work on it (checkBegun): the server's status, or the answer's message or a part of it --
    // not the person's own message, which the server records before the model is asked anything.
    if (prompted && !begun) {
      const partOf = type === "message.part.delta" ? text(props.messageID) : type === "message.part.updated" && isObject(props.part) ? text(props.part.messageID) : undefined;
      if (type === "session.status" || (type === "message.updated" && isObject(props.info) && text(props.info.role) === "assistant") || (partOf !== undefined && roles.get(partOf) === "assistant")) begun = true;
    }
    if (type === "message.updated" && isObject(props.info)) {
      const id = text(props.info.id);
      const role = text(props.info.role);
      if (prompted && promptId === undefined && role === "user" && props.info.sessionID === sessionId && id !== undefined && !roles.has(id)) promptId = id;
      if (id !== undefined && role !== undefined) roles.set(id, role);
      return;
    }
    if (type === "message.part.delta") {
      const id = text(props.partID);
      const delta = typeof props.delta === "string" ? props.delta : undefined;
      if (id === undefined || delta === undefined || text(props.field) !== "text") return;
      const held = parts.get(id) ?? { text: "" };
      held.text += delta;
      if (held.messageID === undefined && text(props.messageID) !== undefined) held.messageID = text(props.messageID);
      parts.set(id, held);
      sendPartial(id);
      return;
    }
    if (type === "message.part.updated" && isObject(props.part)) {
      const part = props.part;
      // Its kind and, when it says, its whole text so far: what the deltas after it add to.
      const partId = text(part.id);
      if (partId !== undefined) {
        const held = parts.get(partId) ?? { text: "" };
        held.type = text(part.type);
        held.messageID = text(part.messageID) ?? held.messageID;
        held.synthetic = part.synthetic === true;
        if (typeof part.text === "string" && part.text.length >= held.text.length) held.text = part.text;
        parts.set(partId, held);
      }
      const record = runRecordFor(part, roles.get(text(part.messageID) ?? ""));
      if (record === undefined) return;
      // A part still being written goes the one way partial text goes: paced, and never empty.
      if (record.type === "text_partial") {
        if (partId !== undefined) sendPartial(partId);
        return;
      }
      const key = `${record.type}:${text(part.id) ?? partSignature(part)}`;
      if (emitted.has(key)) return;
      if (record.type === "tool_use" || record.type === "step_finish") flushPartials();
      emitted.add(key);
      push(record);
      return;
    }
    if (type === "permission.asked") {
      void answer(props);
      return;
    }
    // The server's word that it will try the model again: `session.status`
    // with `type: "retry"` and what the provider answered. `run` writes the
    // same thing to its log, which opencode-events reads; here it is an
    // event, and it was ignored, so an Approve-each turn on a busy free model
    // sat on "Starting" saying nothing (the 0.368 sweep's opencode-approve-each).
    if (type === "session.status" && isObject(props.status) && text(props.status.type) === "retry") {
      push({
        type: "locust.retry",
        message: text(props.status.message) ?? "",
        ...(typeof props.status.attempt === "number" ? { attempt: props.status.attempt } : {}),
      });
      return;
    }
    if (type === "session.error") {
      errored = true;
      push({ type: "error", error: isObject(props.error) ? props.error : { name: "UnknownError", data: { message: "OpenCode reported an error." } } });
      return;
    }
    if (type === "session.idle" && prompted) finish();
  };

  const readEvents = async (): Promise<void> => {
    const response = await request(`${base!}/event`, { headers: auth, signal: events.signal });
    if (!response.ok || response.body === null) throw new Error(`OpenCode's event stream answered ${String(response.status)}`);
    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    let buffer = "";
    for (;;) {
      const { value, done } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });
      let at: number;
      while ((at = buffer.indexOf("\n\n")) >= 0) {
        const chunk = buffer.slice(0, at);
        buffer = buffer.slice(at + 2);
        const data = chunk.split("\n").filter((line) => line.startsWith("data:")).map((line) => line.slice(5).trim()).join("");
        if (data.length === 0) continue;
        let event: unknown;
        try {
          event = JSON.parse(data);
        } catch {
          continue;
        }
        if (isObject(event)) accept(event);
      }
    }
    if (!settled && !cancelled) {
      streamClosed = true;
      flushPartials();
      if (beginCheck !== undefined) clearTimeout(beginCheck);
      beginCheck = undefined;
      if (sessionId === undefined || !prompted) {
        lose(`${closedReason} Its outcome could not be confirmed.`);
        return;
      }
      streamCloseTimeout = setTimeout(() => {
        if (!settled && !cancelled) lose(`${closedReason} ${busy ? "The session was still busy" : "The status check did not finish"} before the recovery deadline; its outcome could not be confirmed.`);
      }, options.streamCloseTimeoutMs ?? 300_000);
      await checkBegun();
    }
  };

  const begin = async (): Promise<void> => {
    const stream = readEvents().catch((error: unknown) => {
      if (!settled && !cancelled) lose(error instanceof Error ? error.message : "OpenCode's event stream ended.");
    });
    if (options.resumeSessionId !== undefined) {
      sessionId = options.resumeSessionId;
    } else {
      // Titled, as `run --title Locust` is (A6.3): untitled, OpenCode names it with a second call to the same model.
      const created = await call("/session", { title: "Locust" });
      const body: unknown = await created.json().catch(() => undefined);
      sessionId = isObject(body) ? text(body.id) : undefined;
      if (!created.ok || sessionId === undefined) throw new Error("OpenCode's server would not start a session.");
    }
    // Session creation may finish after the event stream was already lost.
    if (settled || cancelled) return;
    if (options.slashCommand !== undefined) {
      // Answered only when the command's turn is over, so it is not awaited:
      // the turn is read from the event stream like any other, and a refusal
      // ends the run with the server's reason.
      prompted = true;
      void call(`/session/${encodeURIComponent(sessionId)}/command`, {
        command: options.slashCommand,
        arguments: options.prompt,
        ...(options.model === undefined ? {} : { model: options.model }),
        ...(options.variant === undefined ? {} : { variant: options.variant }),
      }).then(async (answered) => {
        if (!answered.ok) lose(`OpenCode's server refused the command (${String(answered.status)}): ${(await answered.text().catch(() => "")).slice(0, 200)}`);
      }).catch((error: unknown) => lose(error instanceof Error ? error.message : "OpenCode's server could not be reached."));
      await stream;
      return;
    }
    const slash = options.model?.indexOf("/") ?? -1;
    // Marked before it is sent, as a command is (0.677): the turn's idle can come before the send is answered, and
    // read as not yet prompted it was ignored and the run waited forever. A refused send ends the run below anyway.
    prompted = true;
    const sent = await call(`/session/${encodeURIComponent(sessionId)}/prompt_async`, {
      parts: [{ type: "text", text: options.prompt }],
      ...(options.model !== undefined && slash > 0
        ? { model: { providerID: options.model.slice(0, slash), modelID: options.model.slice(slash + 1) } }
        : {}),
      ...(options.variant === undefined ? {} : { variant: options.variant }),
    });
    if (!sent.ok) throw new Error(`OpenCode's server refused the message (${String(sent.status)}).`);
    scheduleBeginCheck();
    await stream;
  };

  let stdout = "";
  let restarts = 0;
  const startup = setTimeout(() => lose("OpenCode's server did not say where it was listening."), options.startupTimeoutMs ?? 30_000);
  const restart = (): void => {
    restarts += 1;
    restartTimer = setTimeout(() => {
      restartTimer = undefined;
      if (settled) return;
      stdout = "";
      try {
        child = spawnServer();
      } catch (error) {
        lose(error instanceof Error ? error.message : "OpenCode's server could not be started again.");
        return;
      }
      attach(child);
    }, 400 * 2 ** (restarts - 1) + Math.floor(Math.random() * 400));
  };
  const attach = (server: AppServerRunProcess): void => {
    server.onData((chunk) => {
      if (server !== child || base !== undefined) return;
      stdout = `${stdout}${chunk}`.slice(-4_000);
      const found = LISTENING.exec(stdout);
      if (found === null) return;
      clearTimeout(startup);
      base = found[1];
      void begin().catch((error: unknown) => lose(error instanceof Error ? error.message : "OpenCode's server could not be reached."));
    });
    server.onExit(() => {
      if (server !== child) return;
      const busyStorage = (): boolean => STORAGE_BUSY.test(server.stderrTail?.() ?? "");
      const mayRestart = base === undefined && !settled && restarts < STORAGE_RESTARTS;
      if (mayRestart && busyStorage()) {
        restart();
        return;
      }
      if (!mayRestart) {
        clearTimeout(startup);
        lose(EXITED);
        return;
      }
      // Its last words can trail the exit by a moment, so they are read once more before it counts as a plain failure.
      restartTimer = setTimeout(() => {
        restartTimer = undefined;
        if (settled) return;
        if (busyStorage()) {
          restart();
          return;
        }
        clearTimeout(startup);
        lose(EXITED);
      }, 100);
    });
  };
  attach(child);

  const stop = (): void => {
    if (settled) return;
    cancelled = true;
    if (base === undefined || sessionId === undefined) {
      finish();
      return;
    }
    // Asked to stop first, so the session is left settled for a later turn;
    // the server goes either way.
    stopTimeout = setTimeout(finish, 1_500);
    void call(`/session/${encodeURIComponent(sessionId)}/abort`).catch(() => undefined);
  };
  if (options.signal !== undefined) {
    if (options.signal.aborted) stop();
    else options.signal.addEventListener("abort", stop, { once: true });
  }

  return { records, completion };
}
