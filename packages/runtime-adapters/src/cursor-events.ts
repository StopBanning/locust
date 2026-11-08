import {
  boundedAnswerText,
  boundedMessageText,
  evidenceFor,
  identityValue,
  isObject,
  malformedEvidence,
  processEvidence,
  redactSecrets,
  requireContextText,
  sanitizeJson,
  stringValue,
  toolPatchFrom,
} from "./codex-events.js";
import type {
  CodexEventEvidence,
  NormalizedRuntimeEvent,
  NormalizedRuntimeEventType,
  NormalizedRuntimePayloadMap,
  RedactedJsonValue,
} from "./codex-events.js";
import type {
  RuntimeJsonlRecord,
  RuntimeProcessCompletion,
} from "./process-runner.js";
import { providerErrorSentence } from "./provider-error.js";

/**
 * Cursor Agent `stream-json` -> product events.
 *
 * Every shape here was measured against cursor-agent 2026.08.31 under the
 * exact argv `createCursorPrintCommand` produces, with the prompt on stdin.
 * The captured streams are the fixtures in `test/fixtures/cursor/`: a
 * read-only run with partial output, a write-mode run whose shell commands
 * were rejected, and a resumed turn.
 *
 * What was measured, and what it means here:
 *
 * - `system`/`init` carries the session id (which `--resume` takes back), the
 *   model Cursor resolved (`"Auto"`, `"Composer 2.5"`), the permission mode,
 *   the cwd and the auth source. It is allow-listed like Claude's.
 * - `thinking` deltas are the model's reasoning. Their text is KEPT -- Colin
 *   overturned the redaction on 2026-09-15, and the long comment at the
 *   `thinking` branch says why -- with secrets scrubbed from it as from
 *   anything else. They become a `reasoning` step so the step line can show
 *   that thinking is happening. (This paragraph said REDACTED for five days
 *   after the code stopped doing it, and a smoke test written from the
 *   paragraph rather than the code reported the app broken every sweep since.
 *   A comment that outlives its code is not a stale comment, it is a false
 *   claim with a test behind it.)
 * - `assistant` records come in two forms that look alike. With
 *   `--stream-partial-output` each fragment arrives as its own record carrying
 *   `timestamp_ms` and no `model_call_id`; then the COMPLETE message arrives
 *   as one more record, either with a `model_call_id` or with no timestamp at
 *   all. The complete one REPLACES the fragments -- appending it would double
 *   every answer. A run can hold several messages (one before a tool call,
 *   one after), so each complete message closes its own item rather than all
 *   of them sharing one.
 * - `tool_call` started/completed wrap one typed key (`readToolCall`,
 *   `editToolCall`, `shellToolCall`, `globToolCall`, ...) with `args` and, on
 *   completion, a `result` whose WRAPPER KEY is the outcome: `success` or
 *   `rejected` in the captures. In print mode without `--force`, every shell
 *   command was `rejected`: a tool that did not run, so it is `tool.failed`,
 *   never completed. Any other wrapper key is also a failure -- an outcome
 *   this build has never seen must not be recorded as if it went well.
 * - `success` means the tool was ALLOWED TO RUN, not that it worked. A shell
 *   result carries the command's own `exitCode` inside that wrapper, so a
 *   command that ran and exited non-zero is a failure with its code recorded,
 *   the way the Codex adapter reads `exit_code`.
 * - A rejected call's completion carries no `args`; the command it wanted to
 *   run is inside `result.rejected.command`. Reading only `args` dropped the
 *   one fact a rejection exists to record.
 *   One capture's call ids contained a literal newline; item ids are made
 *   single-line without losing either half.
 * - `result` closes the run: `is_error` and a non-success subtype mean a
 *   failure, and its absence on a clean exit is not a success.
 */

export interface CursorInvocationContext {
  /** Product-owned run identifier. Never the provider's session id. */
  readonly runId: string;
  readonly missionId?: string;
  readonly requestedRouteId?: string;
  readonly resolvedRouteId?: string;
  readonly cliVersion?: string;
  /** Test seam and host clock. */
  readonly now?: () => Date;
}

export interface CursorEventNormalizer {
  readonly runtimeThreadId: string | undefined;
  readonly finalized: boolean;
  accept(record: RuntimeJsonlRecord): readonly NormalizedRuntimeEvent[];
  finish(completion: RuntimeProcessCompletion): readonly NormalizedRuntimeEvent[];
}

type JsonObject = Record<string, unknown>;

const INIT_ALLOWED_KEYS = new Set(["session_id", "model", "permissionMode", "cwd", "apiKeySource"]);

export function summarizeCursorInit(value: unknown): JsonObject {
  if (!isObject(value)) return {};
  const summary: JsonObject = {};
  for (const key of INIT_ALLOWED_KEYS) {
    const held = value[key];
    if (typeof held === "string") summary[key] = boundedMessageText(held);
  }
  return summary;
}

/**
 * Whether an `assistant` record is one streamed fragment or the complete
 * message. Measured: fragments carry `timestamp_ms` and never `model_call_id`;
 * the complete message carries `model_call_id`, or no timestamp at all.
 */
export function isCursorMessageFragment(record: JsonObject): boolean {
  return typeof record.timestamp_ms === "number" && record.model_call_id === undefined;
}

/** `readToolCall` -> `read`; anything that does not follow the pattern is kept whole. */
export function cursorToolKind(key: string): string {
  const match = /^([a-z][A-Za-z0-9]*?)ToolCall$/.exec(key);
  return match?.[1] ?? key;
}

/** A glob call names a pattern and the directory it was run in. */
/** Token counts only, as numbers, so nothing else in the record rides along. */
function sanitizedUsage(value: JsonObject): RedactedJsonValue {
  const counts: Record<string, number> = {};
  for (const [key, held] of Object.entries(value)) {
    if (typeof held === "number" && Number.isFinite(held)) counts[key] = held;
  }
  return counts as RedactedJsonValue;
}

/**
 * Put one todo into the run's plan, keyed by its own id.
 *
 * Cursor's ids are strings ("1", "2", "3"). An entry with no id cannot be
 * merged against anything, so it is kept under its own text instead -- which
 * is stable enough to replace itself and is never confused with another step.
 * A todo carrying neither an id nor content is not a step and is dropped.
 */
function rememberTodo(into: Map<string, JsonObject>, todo: unknown): void {
  if (!isObject(todo)) return;
  const content = stringValue(todo.content) ?? stringValue(todo.text);
  const id = identityValue(todo.id) ?? content;
  if (id === undefined || content === undefined) return;
  into.set(id, todo);
}

function globTarget(args: JsonObject): string | undefined {
  const pattern = stringValue(args.globPattern);
  if (pattern === undefined) return undefined;
  const directory = stringValue(args.targetDirectory);
  return directory === undefined ? pattern : `${pattern} in ${directory}`;
}

/**
 * What a completed tool call actually did.
 *
 * The wrapper key is the outcome, and `success` only means the tool was let
 * through: a shell result carries the command's own exit code inside it. An
 * outcome this build has never seen is treated as a failure, because a
 * ledger that records an unknown result as success is wrong in the direction
 * that matters.
 */
export function toolOutcome(result: JsonObject): {
  readonly failed: boolean
  readonly status?: string
  readonly exitCode?: number
} {
  const key = Object.keys(result)[0];
  if (key === undefined) return { failed: true, status: "unknown" };
  if (key !== "success") return { failed: true, status: key };
  const body = isObject(result.success) ? result.success : {};
  const exitCode = typeof body.exitCode === "number" && Number.isFinite(body.exitCode)
    ? body.exitCode
    : undefined;
  if (exitCode !== undefined && exitCode !== 0) {
    return { failed: true, status: "exit", exitCode };
  }
  return exitCode === undefined ? { failed: false } : { failed: false, exitCode };
}

function assistantText(parsed: JsonObject): string {
  const message = isObject(parsed.message) ? parsed.message : {};
  const content = Array.isArray(message.content) ? message.content : [];
  return content
    .map((block) => (isObject(block) ? stringValue(block.text) : undefined))
    .filter((value): value is string => value !== undefined)
    .join("");
}

export function createCursorEventNormalizer(
  context: CursorInvocationContext,
): CursorEventNormalizer {
  const runId = requireContextText(context.runId, "runId");
  const cliVersion = context.cliVersion === undefined
    ? undefined
    : requireContextText(context.cliVersion, "cliVersion");
  const now = context.now ?? (() => new Date());

  const openTools = new Map<string, { kind: string; target?: string }>();
  /*
   * The run's todo list, held by id because Cursor sends partial updates.
   *
   * A Map rather than an array so `merge: true` means what it says: an item
   * that arrives again replaces itself in place and keeps its position, and
   * an item that is not mentioned this time is left exactly as it was. An
   * array would either lose the unmentioned ones or reorder the plan.
   */
  const cursorTodos = new Map<string, JsonObject>();
  let runtimeThreadId: string | undefined;
  let normalizedSequence = 0;
  let finalized = false;
  let sawResult = false;
  let terminalFailure: string | undefined;
  let terminalFailureEvidence: CodexEventEvidence | undefined;
  let usage: RedactedJsonValue | undefined;
  /** Which message the next fragment belongs to; closed by a complete message. */
  let messageIndex = 0;
  let thinking = false;
  /** Reasoning text gathered across this turn's deltas, emitted once. */
  let reasoning = "";
  /** How much of each message the fragments have already put in the ledger. */
  const deliveredLength = new Map<string, number>();
  /*
   * What each open message has actually said so far, so the COMPLETE message
   * can be recognised by what it contains when Cursor does not mark it.
   * See `restatesTheWholeMessage` below.
   */
  const deliveredText = new Map<string, string>();
  /** How many fragments each open message is built from; see the rule below. */
  const deliveredPieces = new Map<string, number>();
  /** Record types already reported as unhandled, so each is said once a run. */
  const unknownTypesSaid = new Set<string>();

  const emit = <TType extends NormalizedRuntimeEventType>(
    type: TType,
    payload: NormalizedRuntimePayloadMap[TType],
  ): NormalizedRuntimeEvent => {
    normalizedSequence += 1;
    return {
      id: `${runId}:cursor:${normalizedSequence}`,
      runId,
      ...(context.missionId === undefined ? {} : { missionId: context.missionId }),
      sequence: normalizedSequence,
      occurredAt: now().toISOString(),
      sourceAdapter: "cursor",
      ...(cliVersion === undefined ? {} : { cliVersion }),
      ...(context.requestedRouteId === undefined ? {} : { requestedRouteId: context.requestedRouteId }),
      ...(context.resolvedRouteId === undefined ? {} : { resolvedRouteId: context.resolvedRouteId }),
      ...(runtimeThreadId === undefined ? {} : { runtimeThreadId }),
      type,
      payload,
    } as NormalizedRuntimeEvent;
  };

  const diagnostic = (
    level: "info" | "warning" | "error",
    code: string,
    message: string,
    evidence: CodexEventEvidence,
  ): NormalizedRuntimeEvent =>
    emit("adapter.diagnostic", { level, code, message, terminal: false, evidence });

  function acceptParsed(
    record: RuntimeJsonlRecord,
    parsed: JsonObject,
  ): readonly NormalizedRuntimeEvent[] {
    const type = stringValue(parsed.type);
    if (type === undefined) return [];

    if (type === "thinking") {
      /*
       * THE REASONING IS KEPT NOW.
       *
       * It used to be replaced with "[redacted]" before the record was
       * rebuilt, so the app knew a model had thought for fifty-eight seconds
       * and could say nothing about what it thought. Colin, 2026-09-15: "i
       * feel like it gives way more insight into the thinking and structure
       * of what its doing... i wanted to sacrifice nothing."
       *
       * He is right, and the objection I first gave him was softer than I
       * made it sound: his whole ledger is about 15MB across 51 missions,
       * and roughly doubling it costs nothing against the disk. What it does
       * cost is that a ledger sent to somebody now carries the model's
       * working-out too -- which is a thing to know, not a reason to throw
       * the reasoning away.
       *
       * `evidenceFor` still runs its secret redaction over the text, so an
       * API key the model happened to repeat while thinking is scrubbed
       * exactly as it would be anywhere else.
       *
       * The deltas still collapse into ONE step, which is unchanged and
       * deliberate: a row per fragment is a log nobody reads. The text is
       * accumulated and carried on the step instead.
       */
      const evidence = evidenceFor(record, parsed, type);
      /*
       * The comment above promised that a key the model repeated while
       * thinking was scrubbed "exactly as it would be anywhere else". It was
       * not: `evidenceFor` redacts the EVIDENCE, and this text was taken raw
       * from the record and carried on the step as the message. MEASURED
       * 2026-09-17 on a live Cursor turn (Grok 4.6 Low, one fake sk- key in
       * the prompt): the evidence read `[redacted]`, the answer's
       * `message.delta` text carried the key verbatim, and the reasoning
       * path was the same code. Redacted here, where the text enters.
       */
      const fragment = redactSecrets(stringValue(parsed.text) ?? "");
      if (stringValue(parsed.subtype) === "completed") {
        if (!thinking) return [];
        thinking = false;
        const said = boundedMessageText(reasoning.trim());
        reasoning = "";
        return [
          emit("step.completed", {
            stepKind: "reasoning",
            ...(said.length === 0 ? {} : { message: said }),
            evidence,
          }),
        ];
      }
      reasoning += fragment;
      if (thinking) return [];
      thinking = true;
      return [emit("step.started", { stepKind: "reasoning", evidence })];
    }

    const evidence = evidenceFor(record, parsed, type);

    if (type === "system") {
      if (stringValue(parsed.subtype) !== "init") {
        // Opened AND closed. A step that only ever starts leaves the thread
        // showing work in progress that nothing ever finishes.
        return [
          emit("step.started", { stepKind: "turn", evidence }),
          emit("step.completed", { stepKind: "turn", evidence }),
        ];
      }
      runtimeThreadId = identityValue(parsed.session_id);
      return [
        emit("run.started", {
          runtimeThreadId: runtimeThreadId ?? "",
          evidence: { ...evidence, raw: summarizeCursorInit(parsed) as never, redacted: true },
        }),
      ];
    }

    if (type === "user") {
      // The prompt, echoed back. The ledger already holds the prompt.
      return [];
    }

    if (type === "assistant") {
      // Redacted at the door, so every path below -- fragment, restatement,
      // the bounded replace -- measures and stores the same scrubbed text.
      const text = redactSecrets(assistantText(parsed));
      if (text.length === 0) return [];
      const itemId = `msg_${String(messageIndex)}`;
      /*
       * THE SECOND TELL, and now the load-bearing one.
       *
       * `isCursorMessageFragment` reads a field: fragments carry
       * `timestamp_ms` and no `model_call_id`, the complete message carries
       * `model_call_id`. That was measured, and it rotted. TWO RUNS FROM ONE
       * EVENING, 2026-09-11, same CLI:
       *
       *   f8dedec3  complete message keys: type,message,session_id,model_call_id,timestamp_ms
       *   fd0adba1  complete message keys: type,message,session_id,timestamp_ms
       *
       * In the second, nothing distinguishes the complete message from a
       * fragment -- so it was appended, and the reply said itself twice:
       * "...to Yurt for his take.Looking up NVIDIA forward earnings now,
       * then I'll hand the numbers to Yurt for his take." (Colin's
       * screenshot, and 2872 characters of it in the ledger.)
       *
       * The doubling was the visible half. The worse half is that the item
       * was never CLOSED: nothing was ever marked final, and everything the
       * host reads out of a reply reads the last FINAL message -- the share,
       * the memory block, the room task, the workroom post. So a teammate
       * who had plainly written something was reported as "Jimothy's turn
       * ended without a reply", which is what Colin saw next.
       *
       * So the content decides when the field does not. A complete message
       * RESTATES exactly what its fragments already delivered; a fragment
       * adds to it. That is a fact about what the stream contains rather
       * than about which keys this version chose to send.
       *
       * TWO GUARDS, and the second was earned by this repo's own test.
       *
       * A message of 20,000 identical characters arrives as two fragments of
       * 10,000 identical characters -- so the SECOND fragment equals
       * everything delivered so far, and a naive content rule closed the
       * message at half its length. That test was written for a different
       * defect and caught this one, which is the whole reason to run the
       * suite before believing a rule.
       *
       * So a restatement must cover SEVERAL fragments, not one. Cursor
       * streams token by token, so the complete message always restates many
       * of them; a single fragment that happens to equal everything before it
       * is one fragment repeating one fragment, which is text.
       *
       * `text.length > 1` keeps the other honestly ambiguous case: a single
       * character repeated ("a" then "a") is a real two-character message.
       */
      const restatesTheWholeMessage =
        text.length > 1
        && (deliveredPieces.get(itemId) ?? 0) > 1
        && text === (deliveredText.get(itemId) ?? "");
      if (isCursorMessageFragment(parsed) && !restatesTheWholeMessage) {
        deliveredLength.set(itemId, (deliveredLength.get(itemId) ?? 0) + text.length);
        deliveredText.set(itemId, `${deliveredText.get(itemId) ?? ""}${text}`);
        deliveredPieces.set(itemId, (deliveredPieces.get(itemId) ?? 0) + 1);
        return [
          emit("message.delta", {
            itemId,
            operation: "append",
            text: boundedAnswerText(text),
            final: false,
            evidence,
          }),
        ];
      }
      // TRAP: the complete message. Its text has already arrived as fragments
      // when partial output is on, so it REPLACES the item -- and closes it,
      // so the next message does not overwrite this one.
      messageIndex += 1;
      const bounded = boundedMessageText(text);
      // A REPLACE that is shorter than what the fragments already delivered
      // would take text back out of the ledger. Each fragment is bounded on
      // its own and so is never truncated, but the whole message can be: a
      // long answer ended up SHORTER on disk than it had been a moment
      // earlier. When that happens the fragments stand, and the item is
      // closed without rewriting it.
      const alreadyDelivered = deliveredLength.get(itemId) ?? 0;
      if (bounded.length < alreadyDelivered) {
        // The fragments stand -- but the item must still be CLOSED.
        //
        // This used to `return []`, emitting nothing at all, so the message was
        // never marked final. Everything the host parses out of a reply reads
        // the last FINAL message: the share in `peer-exchange.ts` ("if (final)
        // latestFinal = next"), the memory block, a decision, a room task. So a
        // long Cursor answer -- exactly the case that lands here -- rendered
        // perfectly in the thread and posted nothing to the workroom, with
        // nothing on screen saying so.
        //
        // An APPEND of nothing changes no text and sets the flag:
        // `assistantMessages` computes `existing + ''`. The fragments are kept,
        // which is what this branch is for, and the turn is closed, which is
        // what it forgot to do.
        return [
          emit("message.delta", {
            itemId,
            operation: "append",
            text: "",
            final: true,
            evidence,
          }),
        ];
      }
      return [
        emit("message.delta", {
          itemId,
          operation: "replace",
          text: bounded,
          final: true,
          evidence,
        }),
      ];
    }

    if (type === "tool_call") {
      const wrapper = isObject(parsed.tool_call) ? parsed.tool_call : {};
      const key = Object.keys(wrapper).find((name) => name.endsWith("ToolCall"));
      const call = key !== undefined && isObject(wrapper[key]) ? (wrapper[key] as JsonObject) : {};
      const args = isObject(call.args) ? call.args : {};
      const outcome = isObject(call.result) ? call.result : {};
      // A rejected call reports no `args`; what it wanted to run is inside
      // the rejection itself.
      const refused = isObject(outcome.rejected) ? outcome.rejected : undefined;
      // Measured: one capture's ids held a literal newline between two
      // halves ("call-…-0\nfc_…_0"), in both the top-level `call_id` and the
      // typed call's `toolCallId`. An item id is one line; the halves are
      // joined with a separator that keeps them distinct.
      const rawId = identityValue(wrapper.toolCallId) ?? identityValue(parsed.call_id);
      const itemId = rawId === undefined ? `tool_${String(record.sequence)}` : rawId.replace(/\s+/g, "|");
      const kind = key === undefined ? "tool" : cursorToolKind(key);
      const target = stringValue(args.command)
        ?? stringValue(args.path)
        ?? stringValue(refused?.command)
        ?? stringValue(refused?.path)
        // A subagent call names its ask, not a path (seen driving, 2026-09-05:
        // a live Cursor subagent read "a subagent, unnamed").
        ?? stringValue(args.description)
        ?? stringValue(args.prompt)
        ?? globTarget(args);
      const subtype = stringValue(parsed.subtype);

      /*
       * Cursor keeps a plan, and Locust threw it away for ninety missions.
       *
       * MEASURED 2026-09-13, a real `cursor-agent` run on grok-4.6 asked for
       * three files: `updateTodosToolCall` arrived four times, each carrying
       * `{id, content, status: TODO_STATUS_*}`. Every one of them was drawn
       * as an ordinary tool row, so the plan advanced four times in silence
       * -- the same defect Astra found in OpenCode, on the runtime Colin
       * actually uses for 90 of his 147 recorded missions.
       *
       * THE TRAP, and the reason this cannot be a copy of the OpenCode
       * mapping: after the first call every update sets `merge: true` and
       * sends ONLY THE ITEMS THAT CHANGED. Update two carried ids 1 and 2,
       * update four carried id 3 alone. Taking `args.todos` as the plan would
       * shrink a three-step plan to one step as it neared the end -- a
       * progress display that goes backwards.
       *
       * So the completed record's `result.success.todos` is preferred: Cursor
       * sends the whole list back there, which is the authoritative snapshot.
       * The merge below is the fallback for the `started` half, which has no
       * result yet.
       */
      if (key === "updateTodosToolCall") {
        // Only the completed half speaks: both halves carry the same list, so
        // reading each would count every update twice, and the completed one
        // is the half that carries the authoritative whole list. Measured,
        // they arrive milliseconds apart -- nothing is waiting for this.
        if (subtype !== "completed") return [];
        const fromResult = isObject(outcome.success) ? (outcome.success as JsonObject).todos : undefined;
        if (Array.isArray(fromResult)) {
          cursorTodos.clear();
          for (const todo of fromResult) rememberTodo(cursorTodos, todo);
        } else {
          // No result to read: fall back to what was sent, honouring `merge`.
          if (args.merge !== true) cursorTodos.clear();
          for (const todo of Array.isArray(args.todos) ? args.todos : []) rememberTodo(cursorTodos, todo);
        }
        const plan = [...cursorTodos.values()];
        // A todo call carrying nothing is not a plan, and must not become an
        // empty one.
        if (plan.length === 0) return [];
        const planState = { redacted: false };
        return [
          emit("plan.updated", {
            itemId,
            plan: sanitizeJson(plan, planState),
            // Cursor never says which update is the last one.
            final: false,
            evidence,
          }),
        ];
      }

      if (subtype === "started") {
        openTools.set(itemId, { kind, ...(target === undefined ? {} : { target }) });
        return [
          emit("tool.started", {
            itemId,
            toolKind: kind,
            name: kind,
            ...(target === undefined ? {} : { command: boundedMessageText(target) }),
            phase: "started",
            evidence,
          }),
        ];
      }
      if (subtype === "completed") {
        const open = openTools.get(itemId);
        openTools.delete(itemId);
        const verdict = toolOutcome(outcome);
        // Measured: an edit's success carries the unified diff as
        // `diffString`. It is the change itself, so it travels as its own
        // field -- evidence strings are bounded far too tightly to hold one.
        const success = isObject(outcome.success) ? outcome.success : {};
        const diff = stringValue(success.diffString);
        const patch = diff === undefined ? undefined : toolPatchFrom(diff);
        return [
          emit(verdict.failed ? "tool.failed" : "tool.completed", {
            itemId,
            toolKind: open?.kind ?? kind,
            name: open?.kind ?? kind,
            ...(target === undefined ? {} : { command: boundedMessageText(target) }),
            ...(verdict.status === undefined ? {} : { status: verdict.status }),
            ...(verdict.exitCode === undefined ? {} : { exitCode: verdict.exitCode }),
            ...(patch === undefined ? {} : { patch }),
            phase: "completed",
            evidence,
          }),
        ];
      }
      return [];
    }

    if (type === "result") {
      sawResult = true;
      if (isObject(parsed.usage)) usage = sanitizedUsage(parsed.usage);
      const isError = parsed.is_error === true;
      const subtype = stringValue(parsed.subtype) ?? "";
      if (isError || (subtype.length > 0 && subtype !== "success")) {
        terminalFailureEvidence = evidence;
        terminalFailure = boundedMessageText(
          providerErrorSentence(stringValue(parsed.result) ?? `Cursor Agent ended with ${subtype || "an error"}.`, "cursor"),
        );
      }
      return [];
    }

    /*
     * `interaction_query` is the SAME tool call, announced twice.
     *
     * Cursor sends a web search and a web fetch on two channels: a
     * `tool_call` record carrying `webSearchToolCall` / `webFetchToolCall`,
     * which this adapter already turns into a proper tool row, and an
     * `interaction_query` request/response pair carrying the same work. They
     * are the same call -- MEASURED on Colin's ledgers, 2026-09-13: the
     * `toolCallId` inside the query is byte-for-byte the `call_id` on the
     * tool_call beside it (234 of them across his missions, 184 searches and
     * 50 fetches, in matched request/response pairs).
     *
     * So it is dropped rather than drawn: rendering it would put every web
     * search on screen twice, once as a tool row and once as something with
     * no name.
     */
    if (type === "interaction_query") return [];

    /*
     * CURSOR'S OWN NETWORK RECOVERY (0.575). Colin, 10/03, of a 42-minute Grok
     * run: "small error at the end ... not sure if it was intended". Two lines
     * read "Unhandled Cursor record: connection" and "... retry". In his
     * ledger they are Cursor losing its connection and picking the turn back
     * up: `connection` / `reconnecting` with an attempt and an endpoint, then
     * `retry` / `starting` with `is_resume`; the run carried on. Said in words,
     * once per attempt, and nothing about the endpoint.
     */
    if (type === "connection" || type === "retry") {
      const subtype = stringValue(parsed.subtype);
      const attempt = typeof parsed.attempt === "number" && Number.isFinite(parsed.attempt) ? ` (attempt ${String(parsed.attempt)})` : "";
      const said = type === "connection"
        ? subtype === "reconnecting"
          ? `Cursor lost its connection and is reconnecting${attempt}.`
          : `Cursor's connection: ${subtype ?? "changed"}${attempt}.`
        : subtype === "starting"
          ? parsed.is_resume === true
            ? `Cursor is picking the turn back up after the reconnect${attempt}.`
            : `Cursor is retrying${attempt}.`
          : `Cursor's retry: ${subtype ?? "changed"}${attempt}.`;
      return [diagnostic("info", `cursor.${type}`, said, evidence)];
    }

    /*
     * A tool result too large to carry (the same stand-in Claude has had
     * since 0.492). Cursor's edit of a large file completes as one record
     * holding the file twice. Measured 2026-10-05, two turns: four edits of
     * index.ts (399 KB) and one of missionView.ts (293 KB) started and never
     * closed, and those turns dropped four records and one. The small files
     * in the same turns closed on the same id. The host had thrown the
     * completion away before this adapter saw it, so the files card read
     * "not confirmed" on a change that had landed.
     *
     * The stand-in names the call and its size, and none of the file. An
     * open call closes. One that was never opened, or already closed, is
     * left alone.
     */
    if (type === "locust.oversized") {
      if (parsed.recordType !== "tool_call") return [];
      const kb = typeof parsed.bytes === "number" && Number.isFinite(parsed.bytes) ? Math.round(parsed.bytes / 1024) : undefined;
      // An edit's own counts and diff, read from the record's head (0.693): the change, without the file twice.
      const edit = isObject(parsed.edit) ? parsed.edit : undefined;
      const added = typeof edit?.added === "number" && Number.isSafeInteger(edit.added) && edit.added >= 0 ? edit.added : undefined;
      const removed = typeof edit?.removed === "number" && Number.isSafeInteger(edit.removed) && edit.removed >= 0 ? edit.removed : undefined;
      const diff = typeof edit?.diff === "string" ? edit.diff : undefined;
      const events: NormalizedRuntimeEvent[] = [];
      for (const itemId of Array.isArray(parsed.callIds) ? parsed.callIds.filter((id): id is string => typeof id === "string") : []) {
        const open = openTools.get(itemId);
        if (open === undefined) continue;
        openTools.delete(itemId);
        if (open.kind === "edit" && added !== undefined && removed !== undefined) {
          const fromDiff = diff === undefined ? undefined : toolPatchFrom(diff);
          events.push(
            emit("tool.completed", {
              itemId,
              toolKind: open.kind,
              name: open.kind,
              ...(open.target === undefined ? {} : { command: boundedMessageText(open.target) }),
              patch: fromDiff ?? { text: "", added, removed, truncated: true },
              phase: "completed",
              evidence,
            }),
          );
          continue;
        }
        events.push(
          emit("tool.completed", {
            itemId,
            toolKind: open.kind,
            name: open.kind,
            ...(open.target === undefined ? {} : { command: boundedMessageText(open.target) }),
            phase: "completed",
            status: "result too large to keep",
            output: `The result was too large to keep${kb === undefined ? "" : ` (${String(kb)} KB)`}. Locust keeps results up to 256 KB.`,
            evidence,
          }),
        );
      }
      return events;
    }

    /*
     * A record type nobody handled is worth saying ONCE.
     *
     * This said it per record, and Cursor sends a lot of them: Colin's room
     * filled with two dozen identical amber lines reading "Unhandled Cursor
     * record: interaction_query", which buried the actual conversation and
     * read as the app being broken. The fact is about the STREAM -- this
     * version sends a kind we do not read -- and a fact about the stream does
     * not get truer by being repeated.
     */
    if (unknownTypesSaid.has(type)) return [];
    unknownTypesSaid.add(type);
    return [diagnostic("info", "cursor.unknown_event", `Unhandled Cursor record: ${type}`, evidence)];
  }

  return {
    get runtimeThreadId() {
      return runtimeThreadId;
    },
    get finalized() {
      return finalized;
    },

    accept(record: RuntimeJsonlRecord): readonly NormalizedRuntimeEvent[] {
      if (finalized) return [];
      let parsed: unknown;
      try {
        parsed = JSON.parse(record.raw) as unknown;
      } catch {
        return [
          diagnostic("warning", "cursor.malformed_record", "A Cursor record could not be parsed.", malformedEvidence(record)),
        ];
      }
      if (!isObject(parsed)) {
        return [
          diagnostic("warning", "cursor.malformed_record", "A Cursor record was not an object.", malformedEvidence(record)),
        ];
      }
      return acceptParsed(record, parsed);
    },

    finish(completion: RuntimeProcessCompletion): readonly NormalizedRuntimeEvent[] {
      if (finalized) return [];
      finalized = true;
      const process = processEvidence(completion);
      const thread = runtimeThreadId === undefined ? {} : { runtimeThreadId };
      if (completion.cancelled) {
        return [emit("run.cancelled", { ...thread, process })];
      }
      if (terminalFailure !== undefined) {
        return [
          emit("run.failed", {
            kind: "unknown",
            message: terminalFailure,
            ...(terminalFailureEvidence === undefined ? {} : { evidence: terminalFailureEvidence }),
            ...thread,
            runtimeTerminal: "failed",
            process,
          }),
        ];
      }
      if (!sawResult || completion.exitCode !== 0) {
        return [
          emit("run.failed", {
            kind: "process-failed",
            // The host stopped this, not the runtime: a single line of
            // output went past the 256 KB cap in process-runner.ts and the
            // process was killed. Checked FIRST because nothing the runtime
            // said afterwards describes it better, and because saying
            // "ended without a terminal result record" for a kill we
            // performed is the vaguest possible account of the one thing we
            // know for certain.
            message: completion.outputLimitExceeded
              ? "Cursor Agent sent more output than Locust could take in, so the run was stopped."
              : sawResult
              ? `Cursor Agent exited with code ${String(completion.exitCode)}.`
              // A skipped record is the host's doing, and when the run then
              // ends with nothing to close it, blaming the runtime for the
              // silence would be blaming it for our own gap.
              : completion.oversizedRecordsDropped > 0
              ? `Cursor Agent ended without a terminal result record, after Locust skipped ${String(completion.oversizedRecordsDropped)} piece${completion.oversizedRecordsDropped === 1 ? "" : "s"} of output too large to take in. The answer above is what arrived before that.`
              : "Cursor Agent ended without a terminal result record.",
            ...thread,
            runtimeTerminal: sawResult ? "completed" : "missing",
            process,
          }),
        ];
      }
      return [emit("run.completed", { ...thread, ...(usage === undefined ? {} : { usage }), process })];
    },
  };
}
