import {
  boundedAnswerText,
  boundedMessageText,
  evidenceFor,
  identityValue,
  isObject,
  malformedEvidence,
  processEvidence,
  requireContextText,
  stringValue,
  toolPatchFrom,
} from "./codex-events.js";
import type {
  CodexEventEvidence,
  CodexRunFailureKind,
  NormalizedRuntimeEvent,
  NormalizedRuntimeEventType,
  NormalizedRuntimePayloadMap,
  RedactedJsonValue,
} from "./codex-events.js";
import type {
  RuntimeJsonlRecord,
  RuntimeProcessCompletion,
} from "./process-runner.js";

/**
 * GitHub Copilot CLI `--output-format json` -> product events.
 *
 * Every shape here was measured against GitHub Copilot CLI 1.0.82 on
 * 2026-09-03, under the argv `createCopilotPromptCommand` produces. The
 * captured streams are the fixtures in `test/fixtures/copilot/`: a plain
 * reply, an `apply_patch` that created a file, a read-only run where both the
 * patch and the shell were refused, a session set and then resumed, and a run
 * an organisation policy refused outright.
 *
 * What was measured, and what it means here:
 *
 * - The stream is far chattier than the ledger needs. Roughly half the record
 *   types are the CLI narrating its own model plumbing (`model.turn_started`,
 *   `model.model_call_started`, `model.response`, `model.tool_execution`,
 *   `model.messages_snapshot`, ...). They are DROPPED, not summarized: they
 *   duplicate the `assistant.*` and `tool.*` records that carry the same facts
 *   in a smaller shape, and one of them carries the entire system prompt.
 * - `model.messages_snapshot` holds the CLI's whole system prompt and the
 *   conversation verbatim. It never reaches the ledger under any code path.
 *   Neither do `session.skills_loaded` (the operator's own local skills, their
 *   descriptions and their paths) or `user.message` (the prompt, which the
 *   ledger already holds).
 * - Several records carry opaque provider blobs -- `reasoningOpaque`,
 *   `encryptedContent`, `apiCallId`, `reasoningId`. They are the model's
 *   reasoning in a form nothing here can read, and they are stripped for the
 *   same reason Cursor's thinking text is: an unreadable secret in the ledger
 *   is still a secret in the ledger.
 * - `assistant.message_delta` streams the answer; `assistant.message` then
 *   arrives with the whole thing. The complete message REPLACES the fragments
 *   under the same item id, which the stream supplies as `messageId` -- so two
 *   answers in one run stay two answers.
 * - `tool.execution_complete` reports `success` as a plain boolean, and a
 *   refusal arrives as `success: false` with `error.code` `denied`. A missing
 *   `success` is a failure: this build has never seen the field absent, and a
 *   tool whose outcome is unknown is not a tool that worked.
 * - `apply_patch` returns a real `diff --git` in `result.detailedContent`, so
 *   the change itself is recorded rather than a claim that a file was touched.
 * - The session id IS in the stream, in the terminal `result` record -- which
 *   the brief for this work did not expect. It is used when it arrives and the
 *   host's own `--session-id` stands in until then, so a run that dies before
 *   the last line still leaves something a follow-up can resume from.
 */

export interface CopilotInvocationContext {
  /** Product-owned run identifier. Never the provider's session id. */
  readonly runId: string;
  readonly missionId?: string;
  readonly requestedRouteId?: string;
  readonly resolvedRouteId?: string;
  readonly cliVersion?: string;
  /**
   * The session id the host passed as `--session-id`. Copilot prints its own
   * only in the terminal `result` record, so this is what a run that fails
   * earlier can still be resumed from.
   */
  readonly sessionId?: string;
  /** Test seam and host clock. */
  readonly now?: () => Date;
}

export interface CopilotEventNormalizer {
  readonly runtimeThreadId: string | undefined;
  readonly finalized: boolean;
  accept(record: RuntimeJsonlRecord): readonly NormalizedRuntimeEvent[];
  finish(completion: RuntimeProcessCompletion): readonly NormalizedRuntimeEvent[];
}

type JsonObject = Record<string, unknown>;

/**
 * Record types this adapter deliberately says nothing about.
 *
 * Two different reasons, both worth keeping straight. The `model.*` records
 * and the ephemeral session chatter are NOISE: every fact in them arrives
 * again in a record this adapter does read. The three at the bottom are
 * CONTENT that must not be persisted -- the system prompt, the operator's
 * local skills, and the prompt echoed back.
 */
const IGNORED_TYPES = new Set([
  "model.turn_started",
  "model.turn_ended",
  "model.call_start",
  "model.call_finished",
  "model.model_call_started",
  "model.model_call_success",
  "model.message",
  "model.response",
  "model.tool_execution",
  "model.captured_assignment_context",
  "session.mcp_server_status_changed",
  "session.mcp_servers_loaded",
  "session.tools_updated",
  // Copilot CLI 1.0.88 (drive-copilot, packaged 0.348, 2026-09-25): shown in
  // the fold as "Unhandled Copilot record: session.indexed_search". Search
  // index housekeeping by its name; its shape was not captured (a second run
  // in a smaller folder did not send it). Ignored rather than reported,
  // because an unknown record's evidence is written to the ledger and a
  // search record may carry what it found.
  "session.indexed_search",
  "assistant.message_start",
  "assistant.tool_call_delta",
  "assistant.idle",
  "model.messages_snapshot",
  "session.skills_loaded",
  // Copilot CLI 1.0.95 (drive-a-copilot-command-is-guarded, 2026-10-10): a
  // command's output as it runs, shown under every answer as "Unhandled
  // Copilot record". Every byte arrives again in `tool.execution_complete`,
  // which this adapter reads, and no runtime's output is drawn live yet.
  "tool.shell_output",
  "tool.execution_partial_result",
  "user.message",
]);

/**
 * Opaque provider blobs, stripped wherever they appear.
 *
 * `sanitizeJson` already redacts a `reasoning` key; none of these are spelled
 * that way, and every one of them is either the model's reasoning in an
 * encrypted form or a request handle that identifies the account's traffic.
 */
const OPAQUE_KEYS = new Set([
  "apiCallId",
  "api_id",
  "encryptedContent",
  "encrypted_content",
  "reasoningId",
  "reasoningOpaque",
  "reasoning_opaque",
  "previousResponseId",
  "transformedContent",
]);

const MAX_SCRUB_DEPTH = 8;

/** Drop the opaque blobs before anything looks at the record for evidence. */
export function scrubCopilotRecord(value: unknown, depth = 0): unknown {
  if (depth >= MAX_SCRUB_DEPTH) return value;
  if (Array.isArray(value)) return value.map((entry) => scrubCopilotRecord(entry, depth + 1));
  if (!isObject(value)) return value;
  const result: JsonObject = {};
  for (const [key, held] of Object.entries(value)) {
    result[key] = OPAQUE_KEYS.has(key) ? "[redacted]" : scrubCopilotRecord(held, depth + 1);
  }
  return result;
}

const AUTO_MODE_KEYS = ["chosenModel", "routingMethod", "availableModels", "fallback"] as const;

/**
 * The model resolution, reduced to the four fields that say which model ran.
 * The record also carries per-category routing scores and latency, which
 * describe GitHub's router rather than this mission.
 */
export function summarizeCopilotAutoMode(value: unknown): JsonObject {
  if (!isObject(value)) return {};
  const summary: JsonObject = {};
  for (const key of AUTO_MODE_KEYS) {
    const held = value[key];
    if (typeof held === "string") summary[key] = boundedMessageText(held);
    else if (typeof held === "boolean") summary[key] = held;
    else if (Array.isArray(held)) {
      summary[key] = held.filter((entry): entry is string => typeof entry === "string");
    }
  }
  return summary;
}

/**
 * What a run cost, from the checkpoint the CLI prints once at the end.
 *
 * Only the two totals. The rest of that record is a cache-state dump holding
 * per-request handles and a full tool schema listing -- provider bookkeeping,
 * not the mission's cost.
 */
export function copilotUsage(value: unknown): RedactedJsonValue | undefined {
  if (!isObject(value)) return undefined;
  const usage: Record<string, number> = {};
  if (typeof value.totalPremiumRequests === "number" && Number.isFinite(value.totalPremiumRequests)) {
    usage.premiumRequests = value.totalPremiumRequests;
  }
  if (typeof value.totalNanoAiu === "number" && Number.isFinite(value.totalNanoAiu)) {
    usage.nanoAiu = value.totalNanoAiu;
  }
  return Object.keys(usage).length === 0 ? undefined : (usage as RedactedJsonValue);
}

/**
 * What a tool call was asked to do, whichever way the CLI spelled it.
 *
 * `apply_patch` gets its arguments as one string -- the patch itself -- while
 * the shell tools get an object with a `command`. Reading only one of the two
 * left every patch in the ledger with an empty command field.
 */
export function copilotToolCommand(argumentsValue: unknown): string | undefined {
  if (typeof argumentsValue === "string") return stringValue(argumentsValue);
  if (!isObject(argumentsValue)) return undefined;
  // MEASURED 2026-09-06 off `--output-format json`: glob {pattern}, view
  // {path}, grep {pattern, path}, bash {command}, edit/create {path}. The
  // fold read "view done" twice with no file until path and pattern counted.
  return stringValue(argumentsValue.command)
    ?? stringValue(argumentsValue.path)
    ?? stringValue(argumentsValue.file_path)
    ?? stringValue(argumentsValue.pattern)
    ?? stringValue(argumentsValue.description);
}

/**
 * Why the CLI refused to run at all, read from its stderr.
 *
 * Both shapes were measured. The policy refusal is the one a person can act
 * on, so its message names what is missing and where to fix it; the unknown
 * model is repeated in the CLI's own words, because it already names the model
 * it was given and inventing a friendlier sentence would lose that.
 */
export function copilotFailureFrom(stderr: string): {
  readonly kind: CodexRunFailureKind;
  readonly message: string;
} | undefined {
  if (/access denied by policy settings|copilot cli policy setting/i.test(stderr)) {
    return {
      kind: "authentication-failed",
      message:
        "Copilot plan required: this GitHub account's Copilot plan does not include the CLI, "
        + "or an organization policy blocks it. Check https://github.com/settings/copilot",
    };
  }
  const model = /Error:\s*(Model "[^"]*" from --model flag is not available\.)/.exec(stderr);
  if (model !== null) return { kind: "unknown", message: model[1]! };
  return undefined;
}

export function createCopilotEventNormalizer(
  context: CopilotInvocationContext,
): CopilotEventNormalizer {
  const runId = requireContextText(context.runId, "runId");
  const cliVersion = context.cliVersion === undefined
    ? undefined
    : requireContextText(context.cliVersion, "cliVersion");
  const now = context.now ?? (() => new Date());

  const openTools = new Map<string, { kind: string; command?: string }>();

  const openReasoning = new Set<string>();

  // The raw reasoning id is an opaque blob; the step id is its ordinal.

  const reasoningOrdinals = new Map<string, string>();

  const reasoningKey = (raw: unknown): string => {

    const key = typeof raw === "string" && raw.length > 0 ? raw : "reasoning";

    const known = reasoningOrdinals.get(key);

    if (known !== undefined) return known;

    const next = String(reasoningOrdinals.size + 1);

    reasoningOrdinals.set(key, next);

    return next;

  };

  const unknownTypes = new Set<string>();
  // The host's id stands until the CLI prints its own in the terminal record.
  let runtimeThreadId: string | undefined = context.sessionId === undefined
    ? undefined
    : identityValue(context.sessionId);
  let normalizedSequence = 0;
  let finalized = false;
  let sawResult = false;
  let startedRun = false;
  let terminalFailure: string | undefined;
  let usage: RedactedJsonValue | undefined;
  const openTurns = new Set<string>();
  /** How much of each message the fragments have already put in the ledger. */
  const deliveredLength = new Map<string, number>();

  const emit = <TType extends NormalizedRuntimeEventType>(
    type: TType,
    payload: NormalizedRuntimePayloadMap[TType],
  ): NormalizedRuntimeEvent => {
    normalizedSequence += 1;
    return {
      id: `${runId}:copilot:${normalizedSequence}`,
      runId,
      ...(context.missionId === undefined ? {} : { missionId: context.missionId }),
      sequence: normalizedSequence,
      occurredAt: now().toISOString(),
      sourceAdapter: "copilot",
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
    // Checked BEFORE the record is scrubbed or turned into evidence, so the
    // system prompt is never held even briefly in a shape that could be
    // emitted by a later edit to this function.
    if (IGNORED_TYPES.has(type)) return [];

    const scrubbed = scrubCopilotRecord(parsed);
    const data = isObject(parsed.data) ? parsed.data : {};
    const evidence = {
      ...evidenceFor(record, scrubbed, type),
      redacted: true,
    };

    /*
     * BACKGROUND WORK, WHICH THIS APP HAS NO CONCEPT OF.
     *
     * Colin, 2026-09-21: he ran something that went to the background and
     * "when it finished we never got the follow up reply". He also guessed
     * there is no UI for a background task anywhere in Locust. He is right:
     * there is no such concept in this codebase, and Copilot CLI is the one
     * runtime that announces the state at all.
     *
     * It was in `IGNORED_TYPES`, so the only signal any runtime gives us was
     * dropped by name. That is fine as a rendering decision and useless as
     * an engineering one: nothing can be built on an event nobody has ever
     * seen the shape of, and no fixture in this repo contains one.
     *
     * So it becomes a diagnostic rather than a feature. It is not drawn as a
     * tool row, it does not re-invoke anybody, and it claims nothing about
     * what happens when the task ends -- it carries the runtime's own
     * payload into the ledger so the NEXT Copilot run that backgrounds
     * something leaves evidence to build the real thing from. Guessing the
     * shape and shipping a follow-up on it is the mistake this file's
     * history is mostly made of.
     */
    /*
     * Copilot CLI 1.0.95 sends it with `data: {}` -- 28 of them for one
     * `echo` that never went to the background (2026-10-10). An empty one
     * says nothing about any task, so it leaves nothing to build from.
     */
    if (type === "session.background_tasks_changed") {
      if (isObject(parsed.data) && Object.keys(parsed.data).length === 0) return [];
      return [
        diagnostic(
          "info",
          "copilot.background_tasks_changed",
          "Copilot reported a change to its background tasks.",
          evidence,
        ),
      ];
    }

    if (type === "session.auto_mode_resolved") {
      // The first record this adapter keeps, and the one that names the model
      // the run actually got. It opens the run.
      if (startedRun) return [];
      startedRun = true;
      return [
        emit("run.started", {
          runtimeThreadId: runtimeThreadId ?? "",
          evidence: { ...evidence, raw: summarizeCopilotAutoMode(data) as never },
        }),
      ];
    }

    if (type === "session.warning") {
      return [
        diagnostic(
          "warning",
          "copilot.session_warning",
          boundedMessageText(stringValue(data.message) ?? "Copilot CLI reported a session warning."),
          evidence,
        ),
      ];
    }

    if (type === "assistant.turn_start") {
      const turnId = identityValue(data.turnId) ?? `turn_${String(record.sequence)}`;
      if (openTurns.has(turnId)) return [];
      openTurns.add(turnId);
      return [emit("step.started", { stepKind: "turn", itemId: turnId, evidence })];
    }

    if (type === "assistant.turn_end") {
      const turnId = identityValue(data.turnId) ?? `turn_${String(record.sequence)}`;
      if (!openTurns.delete(turnId)) return [];
      return [emit("step.completed", { stepKind: "turn", itemId: turnId, evidence })];
    }

    // MEASURED 2026-09-06, Copilot CLI 1.0.83: reasoning streams as
    // `assistant.reasoning_delta` (77 of them in a one-sentence run) before
    // the whole `assistant.reasoning` block. Each delta used to become an
    // "Unhandled Copilot record" line in the thread. One step per reasoning
    // id, opened on the first delta, closed by the block.
    if (type === "assistant.reasoning_delta") {
      const reasoningId = reasoningKey(data.reasoningId);
      if (openReasoning.has(reasoningId)) return [];
      openReasoning.add(reasoningId);
      return [emit("step.started", { stepKind: "reasoning", itemId: `reasoning_${reasoningId}`, evidence })];
    }
    if (type === "assistant.reasoning") {
      const reasoningId = reasoningKey(data.reasoningId);
      const wasOpen = openReasoning.delete(reasoningId);
      return [
        ...(wasOpen ? [] : [emit("step.started", { stepKind: "reasoning", itemId: `reasoning_${reasoningId}`, evidence })]),
        emit("step.completed", { stepKind: "reasoning", itemId: `reasoning_${reasoningId}`, evidence }),
      ];
    }
    // Streams and bookkeeping with nothing a person needs from them: the
    // tool call's arguments arriving in pieces (the start record carries them
    // whole), the model call's own start and finish, the message's first
    // frame, and the idle beat between turns.
    if (
      type === "assistant.tool_call_delta"
      || type === "model.call_start"
      || type === "model.call_finished"
      || type === "assistant.message_start"
      || type === "assistant.idle"
    ) {
      return [];
    }
    // 1.0.95: each model call's outcome. A success says nothing the answer
    // does not; any other outcome is still reported, below, as unknown.
    if (type === "model.call_final_result" && data.result === "success") return [];

    if (type === "assistant.message_delta") {
      const text = stringValue(data.deltaContent);
      const messageId = identityValue(data.messageId);
      if (text === undefined || messageId === undefined) return [];
      const itemId = `msg_${messageId}`;
      const delivered = boundedMessageText(text);
      // What was DELIVERED, redacted, not the raw fragment (M5).
      deliveredLength.set(itemId, (deliveredLength.get(itemId) ?? 0) + delivered.length);
      return [
        emit("message.delta", {
          itemId,
          operation: "append",
          text: delivered,
          final: false,
          evidence,
        }),
      ];
    }

    if (type === "assistant.message") {
      const text = stringValue(data.content);
      const messageId = identityValue(data.messageId);
      // A message that only carries tool requests has empty content; the tool
      // records report those, so there is no message to write.
      if (text === undefined || messageId === undefined) return [];
      const itemId = `msg_${messageId}`;
      const bounded = boundedAnswerText(text);
      // A REPLACE shorter than what the fragments already delivered would take
      // text back out of the ledger. Fragments are bounded individually and so
      // are never truncated; the whole message can be.
      //
      // But the reply still has to be CLOSED. M5 (the code review): this
      // returned nothing, so a reply the fragments had outrun never became
      // final and its share and relay were never acted on. Measured against
      // raw fragment lengths, that was every reply that quoted a key.
      if (bounded.length < (deliveredLength.get(itemId) ?? 0)) {
        return [emit("message.delta", { itemId, operation: "append", text: "", final: true, evidence })];
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

    if (type === "tool.execution_start") {
      const itemId = identityValue(data.toolCallId) ?? `tool_${String(record.sequence)}`;
      const kind = identityValue(data.toolName) ?? "tool";
      const command = copilotToolCommand(data.arguments);
      openTools.set(itemId, { kind, ...(command === undefined ? {} : { command }) });
      return [
        emit("tool.started", {
          itemId,
          toolKind: kind,
          name: kind,
          ...(command === undefined ? {} : { command: boundedMessageText(command) }),
          phase: "started",
          evidence,
        }),
      ];
    }

    if (type === "tool.execution_complete") {
      const itemId = identityValue(data.toolCallId) ?? `tool_${String(record.sequence)}`;
      const open = openTools.get(itemId);
      openTools.delete(itemId);
      // Only an explicit `true` is a success. A field this build has never
      // seen missing is still a field that could go missing, and an unknown
      // outcome recorded as success is the failure that matters.
      const failed = data.success !== true;
      const error = isObject(data.error) ? data.error : {};
      const status = failed
        ? (stringValue(error.code) ?? stringValue(error.message) ?? "unknown").slice(0, 512) // the ledger's bound (2026-10-10 sweep)
        : undefined;
      const result = isObject(data.result) ? data.result : {};
      const detailed = stringValue(result.detailedContent);
      // Only a real unified diff becomes a patch; `detailedContent` is free
      // text for tools that have no diff to report. And only a diff that
      // changes something is a change: measured 2026-09-03, `view` answers
      // with a diff-shaped listing of the file whose every line is context,
      // which drew a "+0 -0" file row beside the real edit.
      const built = detailed !== undefined && /^diff --git |^--- /m.test(detailed)
        ? toolPatchFrom(detailed)
        : undefined;
      const patch = built !== undefined && built.added + built.removed > 0 ? built : undefined;
      const kind = open?.kind ?? identityValue(data.toolName) ?? "tool";
      return [
        emit(failed ? "tool.failed" : "tool.completed", {
          itemId,
          toolKind: kind,
          name: kind,
          ...(open?.command === undefined ? {} : { command: boundedMessageText(open.command) }),
          ...(status === undefined ? {} : { status }),
          ...(patch === undefined ? {} : { patch }),
          phase: "completed",
          evidence,
        }),
      ];
    }

    if (type === "session.usage_checkpoint") {
      usage = copilotUsage(data);
      return [];
    }

    // Copilot narrating its own configuration -- "Disabled tools:
    // session_store_sql", the exclusion Locust asks for on every run (0.378;
    // measured, copilot 1.0.88). Only this kind: any other `session.info`
    // might be something the person needs, and keeps the one line an unknown
    // record gets below.
    if (type === "session.info" && stringValue(data.infoType) === "configuration") {
      return [];
    }

    if (type === "result") {
      sawResult = true;
      // The CLI's own session id, and the only place it prints one. It is the
      // id a follow-up resumes, so it wins over the host's when both exist.
      runtimeThreadId = identityValue(parsed.sessionId) ?? runtimeThreadId;
      const exitCode = parsed.exitCode;
      if (typeof exitCode === "number" && exitCode !== 0) {
        terminalFailure = `Copilot CLI reported exit code ${String(exitCode)}.`;
      }
      return [];
    }

    // Once per type: a record type this build does not know is worth one
    // line in the signal rail, not one per record.
    if (unknownTypes.has(type)) return [];
    unknownTypes.add(type);
    return [
      diagnostic("info", "copilot.unknown_event", `Unhandled Copilot record: ${type}`, evidence),
    ];
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
          diagnostic("warning", "copilot.malformed_record", "A Copilot record could not be parsed.", malformedEvidence(record)),
        ];
      }
      if (!isObject(parsed)) {
        return [
          diagnostic("warning", "copilot.malformed_record", "A Copilot record was not an object.", malformedEvidence(record)),
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
      // The refusals this CLI reports on stderr rather than in the stream. A
      // policy denial prints session records and then dies, so the stream
      // alone would say only that the run stopped early.
      const refusal = copilotFailureFrom(completion.stderr);
      if (refusal !== undefined) {
        return [
          emit("run.failed", {
            ...refusal,
            ...thread,
            runtimeTerminal: sawResult ? "completed" : "missing",
            process,
          }),
        ];
      }
      if (terminalFailure !== undefined) {
        return [
          emit("run.failed", {
            kind: "unknown",
            message: terminalFailure,
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
              ? "Copilot CLI sent output faster than Locust could record it, so the run was stopped rather than leave a gap in its record. Sending it again usually works."
              : sawResult
              ? `Copilot CLI exited with code ${String(completion.exitCode)}.`
              : "Copilot CLI ended without a terminal result record.",
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
