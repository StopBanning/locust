import {
  boundedMessageText,
  evidenceFor,
  identityValue,
  isObject,
  malformedEvidence,
  processEvidence,
  requireContextText,
  stringValue,
} from "./codex-events.js";
import type {
  CodexEventEvidence,
  NormalizedRuntimeEvent,
  NormalizedRuntimeEventType,
  NormalizedRuntimePayloadMap,
} from "./codex-events.js";
import type { RuntimeJsonlRecord, RuntimeProcessCompletion } from "./process-runner.js";
import { providerErrorSentence } from "./provider-error.js";

/**
 * Antigravity CLI's `--output-format stream-json` -> product events (0.540).
 *
 * MEASURED 2026-10-02, agy 1.2.14 on Windows, Gemini 3.8 Flash (Low), in a
 * scratch folder; the captures are the fixtures in
 * `test/agy-events.test.ts`. What was seen:
 *
 * - `{"event":"init","conversation_id",...,"init":{"model","cwd","tools",
 *   "permission_mode"}}` first. The conversation id is what `--conversation`
 *   takes back, known from the first record.
 * - `{"event":"step_update","step_update":{"step_index","state","step_type",...}}`
 *   for everything after. `step_type` is `user_input` (the prompt read back),
 *   `agent_response` (the model; `text_delta` while ACTIVE and on the DONE
 *   record, and `usage` on DONE; a DONE with no text is a planning step), or
 *   `tool` (`tool_name`, `tool_info.parameters`, and `tool_info.output` on
 *   DONE). A tool goes ACTIVE, then DONE or ERROR.
 * - A tool the mode does not allow is ERROR, and the run's `result` names it
 *   in `denied_actions` -- `write_file` under the default mode, `command`
 *   under `--mode accept-edits`. The run itself still says SUCCESS.
 * - `{"event":"result","result":{"status","response","usage",...}}` ends the
 *   turn: `SUCCESS` with the whole answer in `response`, or `ERROR` with an
 *   `error` (an unknown model, an empty prompt).
 * - The reasoning TEXT never appears; only `thinking_tokens` in `usage`. So
 *   nothing here could persist hidden chain-of-thought even by mistake.
 */

export interface AgyInvocationContext {
  readonly runId: string;
  readonly missionId?: string;
  readonly requestedRouteId?: string;
  readonly resolvedRouteId?: string;
  readonly cliVersion?: string;
  readonly now?: () => Date;
  /** The run's mode: read-only refuses writes and commands, workspace-write commands, full access nothing. */
  readonly sandbox?: "read-only" | "workspace-write" | "full-access";
}

export interface AgyEventNormalizer {
  readonly runtimeThreadId: string | undefined;
  readonly finalized: boolean;
  accept(record: RuntimeJsonlRecord): readonly NormalizedRuntimeEvent[];
  finish(completion: RuntimeProcessCompletion): readonly NormalizedRuntimeEvent[];
}

type JsonObject = Record<string, unknown>;

/** What a tool was pointed at, in the parameter names agy prints. */
export function agyToolTarget(parameters: unknown): string | undefined {
  if (!isObject(parameters)) return undefined;
  for (const key of ["CommandLine", "AbsolutePath", "TargetFile", "Url", "Query", "query", "SearchPath", "DirectoryPath", "Pattern"]) {
    const value = stringValue(parameters[key]);
    if (value !== undefined && value.trim().length > 0) return value;
  }
  return undefined;
}

/** A denied action, said for a person: what Antigravity was stopped from doing, and how to let it. */
export function agyDeniedSentence(actions: readonly string[]): string | undefined {
  if (actions.length === 0) return undefined;
  // `read_file` is a read outside the folder, not a change (0.572: it was said
  // as "not allowed to change files" because "file" matched writes).
  const reads = actions.some((action) => /read/i.test(action));
  const writes = actions.some((action) => /write|edit/i.test(action) || (/file/i.test(action) && !/read/i.test(action)));
  const commands = actions.some((action) => /command/i.test(action));
  if (reads && !writes && !commands) return "Antigravity was not allowed to read a file outside this conversation's folder in this mode. Move the file into the folder, or choose Auto to let it.";
  if (commands && !writes && !reads) return "Antigravity was not allowed to run a command in this mode. Edit lets it change files but not run commands; Auto lets it do both.";
  if (writes && !commands && !reads) return "Antigravity was not allowed to change files in this mode. Choose Edit or Auto to let it.";
  return `Antigravity was not allowed to do everything it tried in this mode (${actions.join(", ")}). Choose Auto to let it.`;
}

/**
 * Antigravity's quota used up, as the "Usage limit reached" card shows it, or undefined.
 *
 * MEASURED 2026-10-22, Gemini 4 Argon in Ask, 7 tool calls in: "Individual quota reached. Please upgrade your
 * subscription to increase your limits. Resets in 33h54m24s." with `AGY_ERROR: {"status":"RESOURCE_EXHAUSTED",
 * "error_code":429, ...}` on stderr, and the card pasted that JSON after the sentence. The reset is a duration from
 * now; it is kept as the instant it names, which the window shows as a local time.
 */
export function antigravityQuotaLimit(error: string, now: Date): string | undefined {
  if (!/quota reached|RESOURCE_EXHAUSTED/i.test(error)) return undefined;
  const span = /resets in\s+(?:(\d+)h)?\s*(?:(\d+)m)?\s*(?:(\d+)s)?/i.exec(error);
  const seconds = span === null ? 0 : Number(span[1] ?? 0) * 3600 + Number(span[2] ?? 0) * 60 + Number(span[3] ?? 0);
  if (seconds <= 0) return "Antigravity's quota is used up";
  return `Antigravity's quota is used up · resets ${new Date(now.getTime() + seconds * 1000).toISOString().replace(/\.\d{3}Z$/, "Z")}`;
}

/**
 * The turn's error under that card. The packaged 0.823 drive showed Antigravity's own text there -- "Individual quota
 * reached. Please upgrade your subscription to increase your limits. Resets in 33h16m25s." -- the card's fact again,
 * and an upgrade pitch where Muse Code's says what to do.
 */
export const ANTIGRAVITY_QUOTA_SENTENCE =
  "Antigravity's quota is used up for now, so it stopped before finishing. Pick another model to carry on, or send again once it resets.";

export function createAgyEventNormalizer(context: AgyInvocationContext): AgyEventNormalizer {
  const runId = requireContextText(context.runId, "runId");
  const cliVersion = context.cliVersion === undefined ? undefined : requireContextText(context.cliVersion, "cliVersion");
  const now = context.now ?? (() => new Date());

  let runtimeThreadId: string | undefined;
  let normalizedSequence = 0;
  let finalized = false;
  let turnOpen = false;
  /** The `result` record, once it arrives. */
  let status: string | undefined;
  let resultError: string | undefined;
  let resultEvidence: CodexEventEvidence | undefined;
  // The answer arrived whole, then agy's call after it found the service unavailable (0.657), or its stream "interrupted" (0.779).
  let answeredThenUnavailable = false;
  /** Whether the last step agy reported was an answer with words, closed: nothing was left mid-way after it. */
  let answerClosed = false;
  let usage: { readonly inputTokens: number; readonly outputTokens: number } | undefined;
  /** The message item each agent_response step writes into, newest last. */
  let lastMessageItem: string | undefined;
  // Each message's words as they arrived, in order: the result's whole reply repeats the earlier ones (0.668).
  const spokenText = new Map<string, string>();
  const openTools = new Set<number>();
  /** The agent_response steps that wrote words: theirs is writing time, not thought. */
  const spoken = new Set<number>();

  const emit = <TType extends NormalizedRuntimeEventType>(type: TType, payload: NormalizedRuntimePayloadMap[TType]): NormalizedRuntimeEvent => {
    normalizedSequence += 1;
    return {
      id: `${runId}:antigravity:${normalizedSequence}`,
      runId,
      ...(context.missionId === undefined ? {} : { missionId: context.missionId }),
      sequence: normalizedSequence,
      occurredAt: now().toISOString(),
      sourceAdapter: "antigravity",
      ...(cliVersion === undefined ? {} : { cliVersion }),
      ...(context.requestedRouteId === undefined ? {} : { requestedRouteId: context.requestedRouteId }),
      ...(context.resolvedRouteId === undefined ? {} : { resolvedRouteId: context.resolvedRouteId }),
      ...(runtimeThreadId === undefined ? {} : { runtimeThreadId }),
      type,
      payload,
    } as NormalizedRuntimeEvent;
  };
  const diagnostic = (level: "info" | "warning" | "error", code: string, message: string, evidence: CodexEventEvidence): NormalizedRuntimeEvent =>
    emit("adapter.diagnostic", { level, code, message, terminal: false, evidence });

  const toolRow = (index: number, step: JsonObject) => {
    const name = stringValue(step.tool_name) ?? "tool";
    const info = isObject(step.tool_info) ? step.tool_info : {};
    const target = agyToolTarget(info.parameters);
    return { itemId: `tool_${String(index)}`, toolKind: name, name, ...(target === undefined ? {} : { command: boundedMessageText(target) }) };
  };

  /*
   * Calls that ended DONE with nothing to show, which this mode could have
   * refused: they wait for the result, whose `denied_actions` says which
   * were (0.551). Boss's review of 0.550: one held call was let go as done
   * the moment agy said a sentence after it -- and agy does explain a
   * refusal before it stops.
   */
  type Held = ReturnType<typeof toolRow> & { readonly phase: "completed"; readonly evidence: CodexEventEvidence; readonly durationMs?: number };
  const held: Held[] = [];
  const isCommand = (kind: string): boolean => /command|shell|terminal/i.test(kind);
  const isWrite = (kind: string): boolean => /write|edit|replace|create|delete|move/i.test(kind);
  // agy writes its own plan into its brain folder in every mode (measured 2026-10-02, --mode plan).
  const ownFile = (call: Held): boolean => call.command !== undefined && /[\\/]\.gemini[\\/]antigravity[^\\/]*[\\/]brain[\\/]/i.test(call.command);
  const mayBeRefused = (call: Held): boolean => {
    if (context.sandbox === "full-access") return false;
    if (isCommand(call.toolKind)) return true;
    return context.sandbox !== "workspace-write" && isWrite(call.toolKind) && !ownFile(call);
  };
  const releaseHeld = (denied: readonly string[] = []): readonly NormalizedRuntimeEvent[] => {
    const calls = held.splice(0);
    const refused = new Set<Held>();
    // Each denial is the latest held call of its kind.
    for (const action of denied) {
      const kind = /command/i.test(action) ? isCommand : /write|edit|file/i.test(action) ? isWrite : undefined;
      const call = kind === undefined ? undefined : [...calls].reverse().find((one) => kind(one.toolKind) && !refused.has(one) && !ownFile(one));
      if (call !== undefined) refused.add(call);
    }
    return calls.map((call) => (refused.has(call) ? emit("tool.failed", { ...call, output: "Not allowed in this mode.", status: "refused" }) : emit("tool.completed", call)));
  };

  function acceptStep(step: JsonObject, evidence: CodexEventEvidence): readonly NormalizedRuntimeEvent[] {
    const index = typeof step.step_index === "number" ? step.step_index : undefined;
    const type = stringValue(step.step_type);
    const state = stringValue(step.state);
    if (index === undefined || type === undefined) return [];
    // The prompt, read back: the ledger already has the person's own words.
    if (type === "user_input") return [];
    answerClosed = type === "agent_response" && state === "DONE" && (spoken.has(index) || (stringValue(step.text_delta) ?? "").length > 0);

    if (type === "agent_response") {
      const text = stringValue(step.text_delta);
      /*
       * THOUGHT, AS TIME (0.542). agy sends no thinking text, only a step that
       * ends with no words before a tool call, its length, and how many
       * thinking tokens it spent. Spent ones read "Thought for Ns" with
       * nothing to open, as Cursor's do; a step that spent none was not
       * thinking, and no line claims it was.
       */
      if ((text === undefined || text.length === 0) && state === "DONE" && !spoken.has(index)) {
        const used = isObject(step.usage) && typeof step.usage.thinking_tokens === "number" ? step.usage.thinking_tokens : 0;
        const seconds = typeof step.duration_seconds === "number" ? step.duration_seconds : undefined;
        if (used <= 0 || seconds === undefined) return [];
        const itemId = `thought_${String(index)}`;
        return [
          emit("step.started", { stepKind: "reasoning", itemId, evidence }),
          emit("step.completed", { stepKind: "reasoning", itemId, durationMs: Math.round(seconds * 1000), evidence }),
        ];
      }
      if (text === undefined || text.length === 0) return [];
      spoken.add(index);
      lastMessageItem = `msg_${String(index)}`;
      spokenText.set(lastMessageItem, (spokenText.get(lastMessageItem) ?? "") + text);
      return [emit("message.delta", { itemId: lastMessageItem, operation: "append", text: boundedMessageText(text), final: false, evidence })];
    }

    if (type === "tool") {
      const row = toolRow(index, step);
      const events: NormalizedRuntimeEvent[] = [];
      if (state === "ACTIVE") {
        if (!openTools.has(index)) {
          openTools.add(index);
          events.push(emit("tool.started", { ...row, phase: "started", evidence }));
        }
        return events;
      }
      // A step seen only when it ended still opens before it closes, so every row reads the same.
      if (!openTools.has(index)) events.push(emit("tool.started", { ...row, phase: "started", evidence }));
      openTools.delete(index);
      const info = isObject(step.tool_info) ? step.tool_info : {};
      const output = stringValue(info.output);
      const seconds = typeof step.duration_seconds === "number" ? step.duration_seconds : undefined;
      const ended = {
        ...row,
        ...(output === undefined ? {} : { output: boundedMessageText(output) }),
        ...(seconds === undefined ? {} : { durationMs: Math.round(seconds * 1000) }),
        phase: "completed" as const,
        evidence,
      };
      // A call the mode refused never ran (0.541: Colin's `git status` in Edit read
      // "failed" and "it exited non-zero"). Said refused, with its reason, as Claude's are.
      const errorText = isObject(info.error) ? stringValue(info.error.message) : undefined;
      const refused = errorText !== undefined && /permission check failed|permission denied|not allowed|denied/i.test(errorText);
      /*
       * A REFUSAL THAT ENDS "DONE" (0.550). agy also reports a refused call as
       * DONE with no output and no error, and names it only in the result's
       * `denied_actions` (Sol on 0.546: Ask's `git status` read "Ran git
       * status" / done beside "1 refused"). A call that ends with nothing to
       * show waits for what comes next: the result says whether it was refused.
       */
      if (state === "DONE" && errorText === undefined && output === undefined && mayBeRefused(ended)) {
        held.push(ended);
        return events;
      }
      events.push(
        state === "DONE"
          ? emit("tool.completed", ended)
          : emit("tool.failed", {
              ...ended,
              ...(errorText === undefined || output !== undefined ? {} : { output: boundedMessageText(refused ? "Not allowed in this mode." : errorText) }),
              status: refused ? "refused" : "failed",
            }),
      );
      return events;
    }
    return [];
  }

  function acceptParsed(record: RuntimeJsonlRecord, parsed: JsonObject): readonly NormalizedRuntimeEvent[] {
    const kind = stringValue(parsed.event);
    if (kind === undefined) return [];
    const evidence = evidenceFor(record, parsed, kind);

    if (kind === "init") {
      runtimeThreadId = runtimeThreadId ?? identityValue(parsed.conversation_id);
      if (turnOpen) return [];
      turnOpen = true;
      return [emit("step.started", { stepKind: "turn", evidence })];
    }

    if (kind === "step_update") {
      const step = isObject(parsed.step_update) ? parsed.step_update : undefined;
      if (step === undefined) return [];
      runtimeThreadId = runtimeThreadId ?? identityValue(step.conversation_id);
      return acceptStep(step, evidence);
    }

    if (kind === "result") {
      resultEvidence = evidence;
      const result = isObject(parsed.result) ? parsed.result : {};
      runtimeThreadId = runtimeThreadId ?? identityValue(result.conversation_id);
      status = stringValue(result.status) ?? "UNKNOWN";
      resultError = stringValue(result.error);
      const counted = isObject(result.usage) ? result.usage : undefined;
      if (counted !== undefined && typeof counted.input_tokens === "number" && typeof counted.output_tokens === "number") {
        usage = { inputTokens: counted.input_tokens, outputTokens: counted.output_tokens };
      }
      const events: NormalizedRuntimeEvent[] = [];
      // The whole answer, from agy's own last word, replaces what the deltas built.
      const response = stringValue(result.response);
      /*
       * ITS EARLIER WORDS ARE NOT SAID TWICE (0.668). agy's result holds every
       * sentence of the turn -- "I am about to view quoted-task.txt", then the
       * answer -- and it replaces the last message, so a sentence already shown
       * before a step came back on top of the answer. What the earlier
       * messages said, in order, is taken off its front.
       */
      let whole: string | undefined = response;
      if (whole !== undefined) {
        let left: string = whole;
        spokenText.forEach((words: string, itemId: string) => {
          if (itemId === lastMessageItem) return;
          const earlier = words.trim();
          const rest = left.trimStart();
          if (earlier.length > 0 && rest.startsWith(earlier)) left = rest.slice(earlier.length);
        });
        whole = left;
      }
      if (whole !== undefined && whole.trim().length > 0) {
        events.push(emit("message.delta", { itemId: lastMessageItem ?? "msg_final", operation: "replace", text: boundedMessageText(whole.trimStart()), final: true, evidence }));
      }
      const denied = Array.isArray(result.denied_actions)
        ? result.denied_actions.flatMap((entry) => (isObject(entry) && typeof entry.action === "string" ? [entry.action] : []))
        : [];
      events.unshift(...releaseHeld(denied));
      const said = agyDeniedSentence(denied);
      // agy ends the turn at a refusal, with no reply (0.541): say it stopped there.
      const stopped = response === undefined || response.trim().length === 0;
      if (said !== undefined) events.push(diagnostic("warning", "antigravity.denied_actions", stopped ? `It stopped there. ${said}` : said, evidence));
      /*
       * THE SERVICE FAILED AFTER THE ANSWER (0.657). Colin's Flash runs, 10/05,
       * twice in 15 minutes: the whole report streamed, the turn closed, and
       * agy's result said FAILED, "API error (attempt 1): UNAVAILABLE (code
       * 503)" -- one more call to Google after the answer, which agy does not
       * retry as it retries the calls inside a run. Locust drew a finished run
       * as "The run could not continue". With the answer in the result, the
       * run is shown finished, and the line says what happened and what to do
       * if it was not.
       */
      if (status !== "SUCCESS" && !stopped && resultError !== undefined && /\bUNAVAILABLE\b|\b503\b/.test(resultError)) {
        answeredThenUnavailable = true;
        events.push(diagnostic("warning", "antigravity.unavailable_after_answer", "Google's service was unavailable for Antigravity's last call, after this answer (code 503). The answer and its changes are kept. If it was not finished, send \"continue\".", evidence));
      }
      /*
       * AND ITS STREAM "INTERRUPTED" AFTER IT (0.779). Gemini 4 Argon's voxel
       * station, 2026-10-16: 18 minutes, a 91 KB page, the answer's own step
       * DONE and the whole answer in the result -- then status ERROR, "The
       * stream was interrupted. Please continue the task you were working
       * on.", exit 0. Locust drew "Model A failed" and "Try again" over a
       * finished page. Only when the last step agy reported was that answer,
       * closed: a stream cut while a tool or the answer was still going stays
       * a failure.
       */
      if (status !== "SUCCESS" && !stopped && !answeredThenUnavailable && answerClosed && resultError !== undefined && /stream was interrupted/i.test(resultError)) {
        answeredThenUnavailable = true;
        events.push(diagnostic("warning", "antigravity.interrupted_after_answer", "Antigravity said its stream was interrupted after this answer had finished. The answer and its changes are kept. If it was not finished, send \"continue\".", evidence));
      }
      if (turnOpen) {
        turnOpen = false;
        events.push(emit("step.completed", { stepKind: "turn", evidence }));
      }
      return events;
    }
    return [diagnostic("info", "antigravity.unknown_event", `Unhandled Antigravity CLI record: ${kind}`, evidence)];
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
        parsed = JSON.parse(record.raw.replace(/^﻿/, "")) as unknown;
      } catch {
        return [diagnostic("warning", "antigravity.malformed_record", "An Antigravity CLI record could not be parsed.", malformedEvidence(record))];
      }
      if (!isObject(parsed)) return [diagnostic("warning", "antigravity.malformed_record", "An Antigravity CLI record was not an object.", malformedEvidence(record))];
      return acceptParsed(record, parsed);
    },
    finish(completion: RuntimeProcessCompletion): readonly NormalizedRuntimeEvent[] {
      if (finalized) return [];
      const ending = [...releaseHeld()];
      finalized = true;
      const process = processEvidence(completion);
      const thread = runtimeThreadId === undefined ? {} : { runtimeThreadId };
      if (completion.cancelled) return [...ending, emit("run.cancelled", { ...thread, process })];
      if ((status === "SUCCESS" && completion.exitCode === 0) || (answeredThenUnavailable && !completion.outputLimitExceeded)) {
        return [...ending, emit("run.completed", { ...thread, ...(usage === undefined ? {} : { usage }), process })];
      }
      // A conversation Antigravity CLI does not have (one the app began, 0.541):
      // the session is over, and the next reply starts fresh.
      const lost = resultError !== undefined && /trajectory not found/i.test(resultError);
      // Its quota used up (0.823): the limit card, with the reset as an instant the window shows as a local time.
      const quota = resultError !== undefined && resultEvidence !== undefined ? antigravityQuotaLimit(resultError, now()) : undefined;
      return [
        ...ending,
        ...(quota === undefined ? [] : [emit("route.limit_detected", { kind: "quota-exhausted", message: quota, evidence: resultEvidence! })]),
        emit("run.failed", {
          kind: "process-failed",
          ...(lost ? { sessionEnded: true as const } : {}),
          ...(resultEvidence === undefined ? {} : { evidence: resultEvidence }),
          message: lost
            ? "Antigravity CLI does not have this conversation: it began in the Antigravity app. Send it again and it starts fresh, with the conversation so far."
            : completion.outputLimitExceeded
            ? "Antigravity sent output faster than Locust could record it, so the run was stopped rather than leave a gap in its record. Sending it again usually works."
            : status === undefined
              ? "Antigravity ended without a record saying the run had finished."
              : quota !== undefined
              ? ANTIGRAVITY_QUOTA_SENTENCE
              : resultError !== undefined
                ? providerErrorSentence(resultError, "antigravity") !== resultError
                  ? providerErrorSentence(resultError, "antigravity")
                  : `Antigravity could not run it: ${resultError.split(/\r?\n/)[0]}`
                : `Antigravity ended ${status.toLowerCase()}${completion.exitCode === 0 ? "" : ` (exit code ${String(completion.exitCode)})`}.`,
          ...thread,
          runtimeTerminal: status === undefined ? "missing" : status === "SUCCESS" ? "completed" : "failed",
          process,
        }),
      ];
    },
  };
}
