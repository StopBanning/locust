import { createAppServerClient } from "./app-server.js";
import type { AppServerNotification, AppServerRequest, JsonValue } from "./app-server.js";
import { notificationThreadId } from "./app-server-events.js";
import type { AppServerEventNormalizer } from "./app-server-events.js";
import type { CodexEventNormalizer, NormalizedRuntimeEvent } from "./codex-events.js";
import type { RuntimeCommandSpec } from "./types.js";
import type {
  RuntimeJsonlRecord,
  RuntimeProcessCompletion,
  RuntimeProcessRecordStream,
  RuntimeProcessRun,
} from "./process-runner.js";

/**
 * One Codex turn over `codex app-server`, shaped like a process run.
 *
 * WHY THIS EXISTS. `codex exec --json` never streams an agent message. Run
 * directly and measured on 2026-09-10, its JSONL carries the whole reply in a
 * single `item.completed` and nothing before it -- so a Codex teammate sat in
 * silence and then dropped four paragraphs in one paint. app-server sends the
 * same reply as dozens of `item/agentMessage/delta` notifications, and the
 * normalizer for them already exists because Approve-each has used this
 * transport all along.
 *
 * So this is a transport swap, not a second mission system. It presents the
 * same two things the ordinary runner does -- a stream of records and a
 * completion receipt -- and everything that owns a mission (the ledger, the
 * follow-up rules, the disk observation, the workroom share) is untouched.
 *
 * The handshake is: `initialize`, `initialized`, then either `thread/start`
 * with the folder and the policy or `thread/resume` with a prior thread id,
 * then `turn/start` with the prompt. MEASURED 2026-09-10: `thread/resume`
 * works in a LATER process and keeps the earlier turns -- a second process
 * asked for a word the first had been told, and answered with it -- which is
 * what makes a follow-up here as warm as `codex exec resume`.
 */

/** Terminal colour codes: stderr is written for a terminal, and a card is not one. */
const TERMINAL_CODES = new RegExp(String.fromCharCode(27) + "\\[[0-9;?]*[ -/]*[@-~]", "g");

export interface AppServerRunProcess {
  /** Write one framed line to the server. */
  write(line: string): void;
  /** Kill the server and its children. */
  kill(): void;
  onData(listener: (chunk: string) => void): void;
  onExit(listener: () => void): void;
  /** The output transport ended, even if the process itself is still alive. */
  onStdoutClose?(listener: (error?: Error) => void): void;
  /**
   * The end of what the server wrote to stderr, when it is kept (QA-2026-09-29
   * round 2, N10): a server that dies says why there, and the card showed the
   * app's own sentence quoted back as "the runtime's last word".
   */
  stderrTail?(): string;
}

export interface CodexAppServerRunOptions {
  readonly spawn: (
    executablePath: string,
    args: readonly string[],
    /** H7: the launch's own environment, to merge over the host's. */
    env?: Readonly<Record<string, string>>,
  ) => AppServerRunProcess;
  readonly command: RuntimeCommandSpec;
  readonly prompt: string;
  /** The thread's sandbox, as app-server names it. See `codexAppServerPolicy`. */
  readonly sandbox: string;
  readonly approvalPolicy: string;
  readonly model?: string;
  readonly effort?: string;
  /** A prior thread to carry on, which keeps its turns. */
  readonly resumeThreadId?: string;
  /**
   * Carry on a COPY of that thread (0.461, a question asked on the side):
   * `thread/fork`, and the thread itself never sees the question. The copy is
   * kept (not `ephemeral`): a follow-up on the side forks it in turn, and an
   * ephemeral thread is never on disk to be forked.
   */
  readonly forkThread?: boolean;
  /**
   * One of Codex's own commands, typed by the person (0.428), sent as the
   * request its own terminal sends instead of a turn: `review` is
   * `review/start` (the prompt, when there is one, as its instructions),
   * `compact` is `thread/compact/start`. MEASURED 2026-09-28 on 0.157.1 with
   * gpt-6-luna: an inline review runs on the same thread and ends with its
   * findings as an ordinary agent message; a compaction is a turn of its own
   * with one `contextCompaction` item. Neither request takes a model, so the
   * thread is started or resumed with it.
   */
  readonly slashCommand?: "review" | "compact";
  /**
   * Config for THIS thread only, laid over the person's own (0.720: Locust's
   * command guard). MEASURED 2026-10-10 on Codex 0.162.1 with gpt-6-luna:
   * `hooks` and `bypass_hook_trust` in the `config` of `thread/start`,
   * `thread/resume` and `thread/fork` alike ran a PreToolUse hook and honoured
   * its deny, in a fresh app-server each time; without the bypass the hook
   * never ran. Nothing is written to the person's config.toml.
   */
  readonly threadConfig?: Readonly<Record<string, JsonValue>>;
  /**
   * Config for this thread worked out from the person's own settings, read first with `config/read` -- which starts
   * nothing -- for a run that must leave some of them off (0.824: Ask and Plan start none of the MCP servers the
   * person's Codex runs as programs). Laid under `threadConfig`, so Locust's own servers stay as it set them. A refusal
   * ends the run before any thread starts, in the refusal's own words.
   */
  readonly threadConfigFromSettings?: (settings: unknown) =>
    | { readonly ok: true; readonly config: Readonly<Record<string, JsonValue>> }
    | { readonly ok: false; readonly message: string };
  /**
   * Folders of skills for THIS process only (0.813, Locust's run-skills.ts): each holds `<name>/SKILL.md`. MEASURED
   * 2026-10-22 on Codex 0.163.0, no model called: after `skills/extraRoots/set` its `skills/list` names a skill from
   * such a folder, beside every one Codex found itself. Nothing is written to the person's config.toml; a Codex too
   * old to know the request runs the turn without them.
   */
  readonly skillRoots?: readonly string[];
  /**
   * What answers the server when it ASKS -- the approval channel. Only a
   * policy that stops for approval (`untrusted`) ever produces a request;
   * without a handler every request is refused, which under `never` is the
   * same as never being asked.
   */
  readonly onRequest?: (request: AppServerRequest) => Promise<JsonValue>;
  readonly signal?: AbortSignal;
  readonly now?: () => Date;
  /** How long the handshake may take before the run is called lost. */
  readonly handshakeTimeoutMs?: number;
  readonly maxQueuedRecords?: number;
}

/**
 * The stream yields ORDINARY JSONL records -- a sequence and one raw line --
 * rather than parsed notifications, so this really is the same shape the
 * process runner produces and nothing downstream needs a cast to believe it.
 * The line is the notification re-serialized; `asProcessNormalizer` parses it
 * back. A JSON round-trip per notification costs nothing beside the fsync the
 * ledger pays for the same batch.
 */
export type CodexAppServerRun = RuntimeProcessRun & {
  /**
   * Add a message to the turn that is running, WITHOUT stopping it (A2.10):
   * `turn/steer`, which Codex's app-server takes at the turn's next step.
   * True once the server accepted it; false when there is no turn to steer
   * (not started yet, or ended) or the server refused.
   */
  steer(text: string): Promise<boolean>;
};

const DEFAULT_HANDSHAKE_TIMEOUT_MS = 60_000;
const DEFAULT_MAX_QUEUED_RECORDS = 4096;

/**
 * A single-consumer queue of notifications, bounded the way the process
 * runner's is: a server that floods faster than the ledger can write must end
 * the run rather than grow without limit.
 */
export class NotificationQueue implements RuntimeProcessRecordStream {
  private readonly queued: RuntimeJsonlRecord[] = [];
  private readonly waiting: Array<
    (result: IteratorResult<RuntimeJsonlRecord>) => void
  > = [];
  private ended = false;
  private overflowed = false;

  constructor(private readonly maximum: number) {}

  get droppedForOverflow(): boolean {
    return this.overflowed;
  }

  push(record: RuntimeJsonlRecord): void {
    if (this.ended) return;
    const waiter = this.waiting.shift();
    if (waiter !== undefined) {
      waiter({ value: record, done: false });
      return;
    }
    if (this.queued.length >= this.maximum) {
      this.overflowed = true;
      return;
    }
    this.queued.push(record);
  }

  end(): void {
    if (this.ended) return;
    this.ended = true;
    while (this.waiting.length > 0) {
      this.waiting.shift()?.({ value: undefined, done: true });
    }
  }

  drainAvailable(): readonly RuntimeJsonlRecord[] {
    const drained = [...this.queued];
    this.queued.length = 0;
    return drained;
  }

  [Symbol.asyncIterator](): AsyncIterator<RuntimeJsonlRecord> {
    return {
      next: (): Promise<IteratorResult<RuntimeJsonlRecord>> => {
        const queued = this.queued.shift();
        if (queued !== undefined) {
          return Promise.resolve({ value: queued, done: false });
        }
        if (this.ended) {
          return Promise.resolve({ value: undefined, done: true });
        }
        return new Promise((resolve) => {
          this.waiting.push(resolve);
        });
      },
    };
  }
}

/**
 * A turn's own ending, as the server reports it. `turn/completed` and
 * `turn/failed` both end the run; anything else that stops it -- the process
 * dying, the person cancelling -- is reported through the completion receipt
 * instead, which is what the normalizer's `finish` is for.
 */
const ENDING_METHODS = new Set(["turn/completed", "turn/failed"]);

export function startCodexAppServerRun(
  options: CodexAppServerRunOptions,
): CodexAppServerRun {
  const now = options.now ?? (() => new Date());
  const startedAt = now().toISOString();
  const records = new NotificationQueue(
    options.maxQueuedRecords ?? DEFAULT_MAX_QUEUED_RECORDS,
  );

  let recordCount = 0;
  let cancelled = false;
  let transportFailed = false;
  let stderr = "";
  let settleCompletion!: (completion: RuntimeProcessCompletion) => void;
  const completion = new Promise<RuntimeProcessCompletion>((resolve) => {
    settleCompletion = resolve;
  });
  let settled = false;
  /** The thread and the turn running on it, once the server has said them. */
  let threadId: string | undefined;
  let turnId: string | undefined;
  const turnIdOf = (value: unknown): string | undefined => {
    const turn = typeof value === "object" && value !== null ? (value as Record<string, unknown>).turn : undefined;
    const id = typeof turn === "object" && turn !== null ? (turn as Record<string, unknown>).id : undefined;
    return typeof id === "string" && id.length > 0 ? id : undefined;
  };

  const child = options.spawn(options.command.executablePath, options.command.args, options.command.env);

  const finish = (): void => {
    if (settled) return;
    settled = true;
    records.end();
    try {
      child.kill();
    } catch {
      // Already gone; the receipt below is what the mission reads.
    }
    settleCompletion({
      // A protocol run has no exit code of its own to report: the turn ended
      // or it did not, and saying `0` for a lost transport would read as a
      // clean finish. Null with `cancelled`/`stderr` is the honest shape.
      exitCode: transportFailed || cancelled ? null : 0,
      signal: null,
      stderr,
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

  /**
   * `knownCause`: Locust itself saw why (a message past the size limit), so
   * the server's stderr tail is not added -- on Colin's 10/02 failure it was
   * an unrelated config warning, in terminal colour codes, read as the cause.
   */
  const lose = (why: string, knownCause = false): void => {
    if (settled) return;
    transportFailed = true;
    // The server's own last words after ours, so the card's "last word" is its (N10).
    const said = knownCause ? "" : (child.stderrTail?.() ?? "").replace(TERMINAL_CODES, "").trim();
    stderr = stderr.length > 0 ? stderr : said.length > 0 ? `${why}
${said}` : why;
    finish();
  };

  const client = createAppServerClient({
    transport: {
      send: (line: string) => child.write(line),
      close: () => child.kill(),
    },
    onNotification: (notification) => {
      recordCount += 1;
      records.push({
        sequence: recordCount,
        raw: JSON.stringify({ method: notification.method, params: notification.params ?? null }),
      });
      // A queue that backed up has lost records nobody can name, and a run
      // that then ended "completed" had an unknown hole in its record. The
      // exec transport stops for this; so does this one (a B4 lead).
      if (records.droppedForOverflow) {
        lose("Codex sent output faster than Locust could record it, so the run was stopped rather than leave a gap in its record. Sending it again usually works.");
        return;
      }
      // Only this run's own thread starts or ends it. A sub-agent's turn runs
      // on the same connection under its own thread id and ends FIRST
      // (measured 2026-09-25); taken as ours, it ended the run -- killing the
      // server mid-turn -- and made the steer aim at the sub-agent's turn.
      const from = typeof notification.params === "object" && notification.params !== null
        ? notificationThreadId(notification.params as Record<string, unknown>)
        : undefined;
      const ours = threadId === undefined || from === undefined || from === threadId;
      if (ours && notification.method === "turn/started") turnId = turnIdOf(notification.params) ?? turnId;
      if (ours && ENDING_METHODS.has(notification.method)) finish();
    },
    // The approval channel, when the mode has one. Under `never` nothing
    // asks -- measured to raise no request at all -- and a server that asks
    // anyway is answered with a refusal rather than left waiting, because an
    // unanswered request stalls the turn forever and the person would see a
    // run that never ends.
    onRequest: options.onRequest ?? (async () => ({ decision: "reject" }) as JsonValue),
    onDiagnostic: (diagnostic) => {
      if (diagnostic.code === "buffer-overflow" || diagnostic.code === "line-too-long") {
        lose(diagnostic.message, true);
      }
    },
  });

  child.onData((chunk) => client.accept(chunk));
  child.onExit(() => {
    // A server that exits before its turn ends took the turn with it.
    lose("The runtime exited before the turn finished.");
  });

  if (options.signal !== undefined) {
    if (options.signal.aborted) {
      cancelled = true;
      finish();
    } else {
      options.signal.addEventListener(
        "abort",
        () => {
          cancelled = true;
          finish();
        },
        { once: true },
      );
    }
  }

  /** The route on the THREAD, for a command whose request takes none (0.428). */
  const threadRoute = (model: string | undefined, effort: string | undefined): Record<string, JsonValue> => ({
    ...(typeof model === "string" && model.trim().length > 0 ? { model } : {}),
    ...(typeof effort === "string" && /^[a-z]{1,16}$/.test(effort) ? { config: { model_reasoning_effort: effort } } : {}),
  });

  const handshake = async (): Promise<void> => {
    await client.request("initialize", {
      clientInfo: { name: "locust", version: "0.1.0" },
    });
    client.notify("initialized");
    if (options.skillRoots !== undefined && options.skillRoots.length > 0) {
      // Before the thread starts, so its first turn already lists them. Refused, the turn runs without them.
      await client.request("skills/extraRoots/set", { extraRoots: [...options.skillRoots] }).catch(() => undefined);
    }
    // The person's settings, when this run must leave some of them off; unreadable settings read as none, which the
    // caller refuses rather than start with something left on.
    let fromSettings: Readonly<Record<string, JsonValue>> = {};
    if (options.threadConfigFromSettings !== undefined) {
      const read = await client.request("config/read", { cwd: options.command.cwd }).catch(() => undefined);
      const made = options.threadConfigFromSettings(
        typeof read === "object" && read !== null && !Array.isArray(read) ? (read as Record<string, unknown>).config : undefined,
      );
      if (!made.ok) {
        lose(made.message, true);
        return;
      }
      fromSettings = made.config;
    }
    const forking = options.resumeThreadId !== undefined && options.forkThread === true;
    const thread = await client.request(
      options.resumeThreadId === undefined ? "thread/start" : forking ? "thread/fork" : "thread/resume",
      {
        /*
         * Only the thread's id is read from the answer. Without
         * `excludeTurns` the answer carries the whole history as ONE line:
         * Colin's 20 MB thread (a long turn with screenshots) passed the
         * 8 MB line limit, the answer was dropped, and every follow-up in
         * that conversation failed before it began (10/02).
         */
        ...(options.resumeThreadId === undefined
          ? {}
          : { threadId: options.resumeThreadId, excludeTurns: true }),
        cwd: options.command.cwd,
        sandbox: options.sandbox,
        approvalPolicy: options.approvalPolicy,
        ...(() => {
          const route = options.slashCommand === undefined ? {} : threadRoute(options.model, options.effort);
          const config = {
            ...(typeof route.config === "object" && route.config !== null && !Array.isArray(route.config) ? route.config : {}),
            ...fromSettings,
            ...(options.threadConfig ?? {}),
          };
          return { ...route, ...(Object.keys(config).length === 0 ? {} : { config }) };
        })(),
      },
    );
    const record = (typeof thread === "object" && thread !== null ? thread : {}) as Record<
      string,
      unknown
    >;
    const inner = (typeof record.thread === "object" && record.thread !== null
      ? record.thread
      : {}) as Record<string, unknown>;
    const startedThread = typeof inner.id === "string" ? inner.id : undefined;
    if (startedThread === undefined) throw new Error("The runtime did not start a thread.");
    threadId = startedThread;
    // Route and effort are per-TURN parameters here rather than flags. Only a
    // plain word is passed as either: a malformed one is refused by the server
    // and takes the whole turn with it, which the person would read as the
    // runtime failing for no stated reason.
    const model =
      typeof options.model === "string" && options.model.trim().length > 0
        ? options.model
        : undefined;
    const effort =
      typeof options.effort === "string" && /^[a-z]{1,16}$/.test(options.effort)
        ? options.effort
        : undefined;
    if (options.slashCommand === "review") {
      const instructions = options.prompt.trim();
      const reviewing = await client.request("review/start", {
        threadId: startedThread,
        target: instructions.length > 0 ? { type: "custom", instructions } : { type: "uncommittedChanges" },
        delivery: "inline",
      });
      turnId = turnIdOf(reviewing) ?? turnId;
      return;
    }
    if (options.slashCommand === "compact") {
      // Answered with nothing; the turn's id arrives on `turn/started`.
      await client.request("thread/compact/start", { threadId: startedThread });
      return;
    }
    const started = await client.request("turn/start", {
      threadId: startedThread,
      approvalPolicy: options.approvalPolicy,
      input: [{ type: "text", text: options.prompt }],
      ...(model === undefined ? {} : { model }),
      ...(effort === undefined ? {} : { effort }),
      /*
       * The thought's summary, as Codex's own app shows it ("Inspecting the
       * config"). MEASURED 2026-09-30 on Codex 0.159 with
       * `_tools/probe-codex-reasoning-summary.mjs`: with no summary asked
       * for, and with `auto`, every reasoning item completed with
       * `summary: []`; `concise` and `detailed` each sent a headline
       * ("**Calculating products**"). `detailed`, so the thought opens onto
       * more than its line. It covers this thread, not the person's config.
       */
      summary: "detailed",
    });
    turnId = turnIdOf(started) ?? turnId;
  };

  const timeout = setTimeout(() => {
    lose("The runtime did not answer in time.");
  }, options.handshakeTimeoutMs ?? DEFAULT_HANDSHAKE_TIMEOUT_MS);

  void handshake().then(
    () => clearTimeout(timeout),
    (error: unknown) => {
      clearTimeout(timeout);
      lose(
        error instanceof Error && error.message.length > 0
          ? error.message
          : "The runtime did not start a turn.",
      );
    },
  );

  void completion.finally(() => {
    clearTimeout(timeout);
    client.dispose("The turn ended.");
  });

  const steer = async (text: string): Promise<boolean> => {
    if (settled || threadId === undefined || turnId === undefined || text.trim().length === 0) return false;
    try {
      await client.request("turn/steer", {
        threadId,
        expectedTurnId: turnId,
        input: [{ type: "text", text }],
      });
      return true;
    } catch {
      return false;
    }
  };

  return { records, completion, steer };
}

/**
 * The app-server normalizer, wearing the shape the mission loop consumes.
 *
 * The two normalizers differ in exactly two ways: what a record is (a
 * notification rather than a JSONL line) and what ends a run (a reason rather
 * than a completion receipt). This closes both gaps so one mission loop can
 * drive either transport, rather than a second loop being written for the one
 * that streams.
 *
 * `finish` is only reached for a run that ended WITHOUT the server saying so.
 * A turn that completed normally already emitted its terminal event from
 * `turn/completed`, and the normalizer's own `finalized` flag makes the second
 * call empty.
 */
/**
 * The notification a record on this transport was written from, or nothing.
 *
 * Shared by the normalizer below and by anyone else reading the stream -- the
 * mission loop reads file changes off the same records, so the parse lives in
 * one place rather than being done twice with two chances to differ.
 */
export function notificationOfRecord(record: RuntimeJsonlRecord): AppServerNotification | undefined {
  let parsed: unknown;
  try {
    parsed = JSON.parse(record.raw);
  } catch {
    return undefined;
  }
  if (typeof parsed !== "object" || parsed === null) return undefined;
  const notification = parsed as { method?: unknown; params?: unknown };
  if (typeof notification.method !== "string") return undefined;
  return {
    method: notification.method,
    params: (notification.params ?? undefined) as AppServerNotification["params"],
  };
}

export function asProcessNormalizer(
  normalizer: AppServerEventNormalizer,
): CodexEventNormalizer {
  return {
    get runtimeThreadId(): string | undefined {
      return normalizer.runtimeThreadId;
    },
    get finalized(): boolean {
      return normalizer.finalized;
    },
    accept(record: RuntimeJsonlRecord): readonly NormalizedRuntimeEvent[] {
      // Every record on this transport is a notification the runner wrote out
      // as a line. A line that is not one -- which nothing here produces --
      // is dropped rather than guessed at.
      const notification = notificationOfRecord(record);
      return notification === undefined ? [] : normalizer.accept(notification);
    },
    finish(completion: RuntimeProcessCompletion): readonly NormalizedRuntimeEvent[] {
      // With the reason the run recorded for the failure, when it has one (L1).
      return normalizer.finish(completion.cancelled ? "cancelled" : "transport-lost", completion.stderr);
    },
  };
}
