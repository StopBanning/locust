import {
  boundedAnswerText,
  boundedMessageText,
  evidenceFor,
  identityValue,
  isObject,
  malformedEvidence,
  processEvidence,
  redactSecrets,
  redactText,
  requireContextText,
  stringValue,
} from "./codex-events.js";
import type {
  CodexEventEvidence,
  NormalizedRuntimeEvent,
  NormalizedRuntimeEventType,
  NormalizedRuntimePayloadMap,
  ToolPatch,
} from "./codex-events.js";
import { addedFilePatch } from "./opencode-events.js";
import type {
  RuntimeJsonlRecord,
  RuntimeProcessCompletion,
} from "./process-runner.js";

/**
 * Antigravity transcript lines -> product events. EXPERIMENTAL.
 *
 * Google's Antigravity IDE has no streaming CLI. Its `language_server.exe`
 * takes `agentapi new-conversation` / `send-message`, and everything the agent
 * then does appears in a file: one JSON object per line, appended as each step
 * completes, at
 * `~/.gemini/antigravity/brain/<conversationId>/.system_generated/logs/transcript.jsonl`.
 * The host tails that file and feeds this normalizer one line at a time; there
 * is no process whose stdout these arrive on, which is why `accept` is built to
 * survive being handed the same line again (see `seen`).
 *
 * Every shape below was measured on 2026-09-03 against TWO conversations, kept
 * verbatim in `test/fixtures/antigravity/`: a prompt answered in one word, and
 * a prompt that wrote a file and was then continued with `send-message`. Two
 * conversations is a thin basis, and the places that matter are called out.
 *
 * What was measured, and what it means here:
 *
 * - Each line carries `step_index`, `source`, `type`, `status` and `created_at`,
 *   then EITHER `content` (a string) OR `thinking` plus `tool_calls`.
 * - `USER_INPUT` (source `USER_EXPLICIT`) is the prompt the host already sent.
 *   `CHECKPOINT` (source `SYSTEM`) is Antigravity's own running summary of the
 *   conversation -- in the captures it names the objective, the user's requests
 *   and the path of the full log, and on a long conversation it would hold far
 *   more. Neither produces an event, and neither is ever copied into evidence:
 *   the checkpoint is the whole conversation in one field, and the ledger is
 *   not where it belongs.
 * - `PLANNER_RESPONSE` (source `MODEL`) is the model's turn. With `thinking` it
 *   is reasoning, and the reasoning TEXT never reaches the ledger -- the record
 *   is rebuilt without the field before evidence is taken, exactly as Cursor's
 *   thinking deltas and Codex's reasoning items are handled.
 * - `GENERIC` (source `MODEL`) carries a tool's result text. It does not name
 *   the tool it belongs to; its STEP does -- a planner line at step p with n
 *   calls is answered at p+1 .. p+n, in call order (measured 2026-09-25, see
 *   `awaiting`). A result no call claims gets a diagnostic rather than a tool
 *   call invented to hang it on.
 * - A `PLANNER_RESPONSE` with `content` and no `tool_calls` is the final answer,
 *   whether or not it also carries `thinking` -- a model that reasons and
 *   answers in one step is still answering (measured 2026-09-06; requiring no
 *   reasoning here is what left a live run's stop button on screen).
 *   That it also means the turn is OVER is a HEURISTIC: it held in both captured
 *   conversations and there is no terminal record of any kind in the file, so
 *   nothing better was available. It is exposed as `latestFinal` rather than
 *   acted on here, because the host is the one polling and only the host can
 *   decide it has waited long enough.
 * - `send-message` does NOT append a `USER_INPUT` line. Measured: the message
 *   arrives as source `SYSTEM`, type `SYSTEM_MESSAGE`, wrapped in a
 *   `<SYSTEM_MESSAGE>` envelope that tells the model it was "not actually sent
 *   by the user". It is the host's own send coming back, so it produces no
 *   event -- but it is recognised, so a follow-up is never reported as a line
 *   this adapter could not read.
 * - `status` was `DONE` on every line captured. Anything else is treated as a
 *   step still running: nothing is emitted and the index is NOT marked seen, so
 *   the host re-reads the same line once it settles and it is normalized then.
 *
 * Types not seen at all: the file only ever showed the four above. An unknown
 * type is preserved as a diagnostic rather than dropped.
 */

export interface AntigravityInvocationContext {
  /** Product-owned run identifier. Never the conversation id. */
  readonly runId: string;
  readonly missionId?: string;
  readonly requestedRouteId?: string;
  readonly resolvedRouteId?: string;
  readonly cliVersion?: string;
  /**
   * The id `new-conversation` printed. The transcript never repeats it -- it is
   * only in the DIRECTORY NAME the file sits under -- so the host, which owns
   * the conversation, is the only thing that can supply it.
   */
  readonly conversationId?: string;
  /** Test seam and host clock. */
  readonly now?: () => Date;
}

export interface AntigravityEventNormalizer {
  readonly runtimeThreadId: string | undefined;
  readonly finalized: boolean;
  /**
   * Whether the last line accepted was a planner answer with no tool calls.
   * The host's completion signal; see the heuristic note above.
   */
  readonly latestFinal: boolean;
  /**
   * The name of a tool that started and never reported back, if any.
   *
   * Exists for one reason: when a run goes quiet, the host has to say WHY, and
   * "the agent wrote nothing for ten minutes" blames the model for a silence
   * the app caused. Antigravity's native `ask_question` shows up here as a
   * tool that is still running -- it is waiting for an answer through a
   * channel Locust does not collect -- and a timeout that can name it can say
   * so instead (see `docs/FINDING-antigravity-ask-question.md`).
   */
  readonly pendingToolName: string | undefined;
  /**
   * Background tasks started and not yet reported ended (0.487). While any is
   * pending, an answer with no tool calls does NOT end the turn: the agent is
   * woken by the task's notice and goes on (one of Colin's runs worked 18 more
   * minutes, 88 tool calls and 5 edits, after Locust had called it done).
   */
  readonly pendingBackground: number;
  accept(record: RuntimeJsonlRecord): readonly NormalizedRuntimeEvent[];
  finish(completion: RuntimeProcessCompletion): readonly NormalizedRuntimeEvent[];
}

type JsonObject = Record<string, unknown>;

/** Lines that are the host's own words coming back, or Antigravity's private summary. */
const SILENT_TYPES = new Set(["USER_INPUT", "CHECKPOINT", "SYSTEM_MESSAGE"]);

/**
 * One tool argument, decoded.
 *
 * Measured: every value in `tool_calls[].args` is a JSON-ENCODED string, not
 * the value itself -- `"Overwrite":"true"` is the four characters `true`, and
 * `"TargetFile":"\"c:/…/hello.txt\""` is a quoted string inside a string. So a
 * caller comparing `args.Overwrite` to `true` would never match, and one using
 * `args.TargetFile` as a path would carry the quotes into the ledger. A value
 * that does not parse is returned as it arrived rather than discarded.
 */
export function antigravityToolArg(args: unknown, key: string): unknown {
  if (!isObject(args)) return undefined;
  const raw = args[key];
  if (typeof raw !== "string") return raw;
  try {
    return JSON.parse(raw) as unknown;
  } catch {
    return raw;
  }
}

function decodedString(args: unknown, key: string): string | undefined {
  const value = antigravityToolArg(args, key);
  return typeof value === "string" ? stringValue(value) : undefined;
}

/** Collapse to a single line: the ledger renders `command` on one row. */
function oneLine(value: string): string {
  return value.replace(/\s+/g, " ").trim();
}

/**
 * A bounded one-line summary of what a call acted on.
 *
 * `TargetFile` first because it is the fact; `toolAction` and `toolSummary` are
 * the model's own phrasing of it, which is better than nothing when a tool this
 * build has never seen carries no path.
 */
export function antigravityToolCommand(args: unknown): string | undefined {
  // The command itself first (0.487): a run_command row read the model's own
  // phrase -- "Checking commit 8b104218" -- where Antigravity's window shows
  // the command line. The phrase rides as the row's title instead.
  const target = decodedString(args, "CommandLine")
    ?? decodedString(args, "TargetFile")
    ?? decodedString(args, "AbsolutePath")
    ?? decodedString(args, "toolAction")
    ?? decodedString(args, "toolSummary")
    ?? decodedString(args, "Description");
  if (target === undefined) return undefined;
  const line = oneLine(target);
  return line.length === 0 ? undefined : redactText(line);
}

/** The model's own phrase for a call, when the row leads with something else. */
export function antigravityToolTitle(args: unknown, command: string | undefined): string | undefined {
  const phrase = decodedString(args, "toolAction") ?? decodedString(args, "toolSummary");
  if (phrase === undefined) return undefined;
  const line = oneLine(phrase);
  return line.length === 0 || line === command ? undefined : redactText(line);
}

/**
 * The task a result line put in the background, if it did (0.487).
 *
 * MEASURED 2026-09-30 on Colin's transcripts: a command run with a wait
 * before going async writes its result line with `status: RUNNING` and the
 * text "Tool is running as a background task with task id: <conv>/task-N" --
 * where N is that line's own step index -- and the line is NEVER rewritten as
 * DONE (0 of 48). The outcome comes later as a SYSTEM_MESSAGE from
 * `sender=<conv>/task-N`. Reading only DONE lines left all of them saying
 * "did not report".
 */
export function antigravityBackgroundTask(content: string | undefined): { readonly taskId: string; readonly doing?: string } | undefined {
  if (content === undefined) return undefined;
  const found = /running as a background task with task id:\s*(\S+\/task-\d+)/.exec(content);
  if (found === null) return undefined;
  const doing = /Task Description:\s*(.+)/.exec(content)?.[1];
  return { taskId: found[1]!, ...(doing === undefined ? {} : { doing: oneLine(doing) }) };
}

/**
 * Only a line's OWN task (0.537). A real background result names the task
 * numbered by its own step index (measured 2026-09-30, above). Colin's
 * 2026-10-02 run read its subagents' transcripts with `view_file`, and those
 * files quote other lines' "running as a background task" text: four reads
 * were counted as background work that never ended, so the run never
 * finished and its final answer sat under a working turn.
 */
export function ownBackgroundTask<T extends { readonly taskId: string }>(task: T | undefined, stepIndex: number | undefined): T | undefined {
  if (task === undefined || stepIndex === undefined) return task;
  return task.taskId.endsWith(`/task-${String(stepIndex)}`) ? task : undefined;
}

/** The task a SYSTEM_MESSAGE came from (`sender=<conv>/task-N`), if it names one. */
export function antigravityMessageSender(content: string | undefined): string | undefined {
  if (content === undefined) return undefined;
  return /sender=(\S+\/task-\d+)/.exec(content)?.[1];
}

/**
 * How a background task ended, from the SYSTEM_MESSAGE Antigravity sends when
 * it does: "Task id \"<id>\" finished with result: ... The command exited
 * with code 0." or "... was canceled with result: ...". Undefined for any other
 * system message.
 */
export function antigravityTaskEnding(content: string | undefined): { readonly taskId: string; readonly ended: "completed" | "failed" | "stopped"; readonly exitCode?: number } | undefined {
  if (content === undefined) return undefined;
  const task = /Task id "([^"]+\/task-\d+)" (finished|was canceled|failed)/.exec(content);
  if (task === null) return undefined;
  if (task[2] === "was canceled") return { taskId: task[1]!, ended: "stopped" };
  const code = /The command exited with code (-?\d+)/.exec(content);
  const exitCode = code === null ? undefined : Number(code[1]);
  const failed = task[2] === "failed" || (exitCode !== undefined && exitCode !== 0);
  return { taskId: task[1]!, ended: failed ? "failed" : "completed", ...(exitCode === undefined ? {} : { exitCode }) };
}

/**
 * The change a `write_to_file` call made, when it can be reconstructed.
 *
 * `Overwrite: true` is the only case captured, and in it Antigravity reports NO
 * before-text anywhere -- not in the call, not in the `GENERIC` result. So an
 * overwrite records the path and no patch, the way a Codex `file_change` does;
 * a diff built from `CodeContent` alone would claim every line was added when
 * most of them may not have changed at all.
 *
 * Without `Overwrite`, the file is treated as new and `CodeContent` becomes the
 * added-file diff. That branch was NOT measured -- no captured call omitted the
 * flag -- and it is the one place here that rests on the flag meaning what it
 * is named rather than on something observed.
 */
export function antigravityWritePatch(args: unknown): ToolPatch | undefined {
  const overwrite = antigravityToolArg(args, "Overwrite") === true;
  if (overwrite) return undefined;
  const target = decodedString(args, "TargetFile");
  const content = decodedString(args, "CodeContent");
  if (target === undefined || content === undefined) return undefined;
  // `false` is the literal claim "the file did not exist", which is what
  // `addedFilePatch` refuses to build a diff without.
  return addedFilePatch(target, content, false);
}

function numberValue(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

/**
 * Evidence for a planner line, with the model's reasoning removed.
 *
 * The field is DELETED rather than blanked, so the ledger does not even record
 * that a `thinking` key was there. `sanitizeJson` would blank it anyway; that
 * is a backstop, and a backstop is not a reason to hand it the text.
 */
function plannerEvidence(
  record: RuntimeJsonlRecord,
  parsed: JsonObject,
  type: string,
): CodexEventEvidence {
  if (parsed.thinking === undefined) return evidenceFor(record, parsed, type);
  const { thinking: _reasoning, ...rest } = parsed;
  return { ...evidenceFor(record, rest, type), redacted: true };
}

interface OpenTool {
  readonly itemId: string;
  readonly toolKind: string;
  readonly name: string;
  readonly command?: string;
  readonly title?: string;
}

export function createAntigravityEventNormalizer(
  context: AntigravityInvocationContext,
): AntigravityEventNormalizer {
  const runId = requireContextText(context.runId, "runId");
  const cliVersion = context.cliVersion === undefined
    ? undefined
    : requireContextText(context.cliVersion, "cliVersion");
  const runtimeThreadId = context.conversationId === undefined
    ? undefined
    : identityValue(requireContextText(context.conversationId, "conversationId"));
  const now = context.now ?? (() => new Date());

  let normalizedSequence = 0;
  let finalized = false;
  let latestFinal = false;
  /**
   * Step indexes already turned into events. The host re-reads the transcript
   * from the top on every poll, so without this every poll would replay the
   * whole conversation into the ledger.
   */
  const seen = new Set<number>();
  /** Calls opened by a planner line and not yet closed by a `GENERIC` result. */
  const openTools: OpenTool[] = [];
  /**
   * Each open call, by the step its result will carry.
   *
   * A result names no tool and carries no id, and this used to go to the call
   * opened LAST. MEASURED 2026-09-25 (drive-antigravity-parallel, and every
   * transcript on this machine: 1,026 of 1,027 results): a planner line at
   * step p with n calls is answered at steps p+1 .. p+n, in call order. So the
   * step index is the attribution, and last-opened swapped the two answers of
   * a two-call step -- or, since the file wrote the first result BEFORE its
   * planner line, dropped one as "no tool call open" and left its call saying
   * "did not report" beside an answer that quoted it.
   */
  const awaiting = new Map<number, OpenTool>();
  /**
   * Results read before the planner line that opened their call. Held for that
   * line rather than reported as unattached, because the file's order is not
   * the conversation's: both halves share a second and the result came first.
   */
  const early = new Map<number, { readonly parsed: JsonObject; readonly evidence: CodexEventEvidence }>();

  const unattached = (evidence: CodexEventEvidence): NormalizedRuntimeEvent =>
    diagnostic(
      "info",
      "antigravity.unattached_result",
      "An Antigravity tool result arrived with no tool call open to attach it to.",
      evidence,
    );

  /** Work Antigravity put in the background, by its task id: the call it came from. */
  const background = new Map<string, OpenTool>();
  /**
   * TIMERS (0.487). The `schedule` tool is backgrounded the same way, and ends
   * one of two ways, both measured on Colin's transcript (14 timers): it FIRES
   * -- a SYSTEM_MESSAGE from its own task id carrying its prompt ("Check if
   * task-1694 finished"), 8 of them -- or the task it was set to watch
   * (`TimerCondition`) finishes first, and Antigravity drops it without a
   * word, the other 6. So a timer's own message settles it, and so does the
   * end of the task it waited on; otherwise it would hold the turn open until
   * the idle limit.
   */
  const timerWaitsOn = new Map<string, string>();
  /** A schedule call's condition, by its item id, until its task id is known. */
  const timerConditions = new Map<string, string>();
  const settle = (taskId: string, ended: "completed" | "failed" | "stopped", message: string | undefined, evidence: CodexEventEvidence): NormalizedRuntimeEvent[] => {
    const from = background.get(taskId);
    if (from === undefined) return [];
    background.delete(taskId);
    const events: NormalizedRuntimeEvent[] = [
      emit(ended === "failed" ? "step.failed" : "step.completed", {
        stepKind: "item",
        itemId: from.itemId,
        itemType: "background",
        status: ended,
        ...(message === undefined ? {} : { message }),
        evidence,
      }),
    ];
    // Timers that were only waiting on this task will never fire now.
    for (const [timer, watched] of [...timerWaitsOn]) {
      if (watched !== taskId) continue;
      timerWaitsOn.delete(timer);
      events.push(...settle(timer, "completed", "no longer needed: what it waited on finished", evidence));
    }
    return events;
  };

  const closeTool = (open: OpenTool, parsed: JsonObject, evidence: CodexEventEvidence): readonly NormalizedRuntimeEvent[] => {
    const at = openTools.indexOf(open);
    if (at >= 0) openTools.splice(at, 1);
    const content = stringValue(parsed.content);
    const task = ownBackgroundTask(antigravityBackgroundTask(content), numberValue(parsed.step_index));
    if (task !== undefined) {
      // The call is over -- it handed its work to the background -- and the
      // work is a step of its own until its task says how it ended. The same
      // two events Claude's backgrounded commands make, so the same row
      // draws them: "in the background", then done / failed / stopped.
      background.set(task.taskId, open);
      const watched = timerConditions.get(open.itemId);
      // Already over? Then this timer will never fire either: settled at once, not left holding the turn
      // open to the idle limit (2026-10-10 sweep).
      let moot = false;
      if (watched !== undefined) {
        timerConditions.delete(open.itemId);
        if (background.has(watched)) timerWaitsOn.set(task.taskId, watched);
        else moot = true;
      }
      return [
        emit("tool.completed", { ...open, background: true, phase: "completed", evidence }),
        emit("step.started", {
          stepKind: "item",
          itemId: open.itemId,
          itemType: "background",
          status: "running",
          ...(task.doing === undefined ? {} : { message: boundedMessageText(task.doing) }),
          evidence,
        }),
        ...(moot ? settle(task.taskId, "completed", "no longer needed: what it waited on had already finished", evidence) : []),
      ];
    }
    return [
      emit("tool.completed", {
        ...open,
        ...(content === undefined ? {} : { output: redactText(content) }),
        phase: "completed",
        evidence,
      }),
    ];
  };

  const emit = <TType extends NormalizedRuntimeEventType>(
    type: TType,
    payload: NormalizedRuntimePayloadMap[TType],
  ): NormalizedRuntimeEvent => {
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

  const diagnostic = (
    level: "info" | "warning" | "error",
    code: string,
    message: string,
    evidence: CodexEventEvidence,
  ): NormalizedRuntimeEvent =>
    emit("adapter.diagnostic", { level, code, message, terminal: false, evidence });

  function plannerEvents(
    record: RuntimeJsonlRecord,
    parsed: JsonObject,
    stepIndex: number,
  ): readonly NormalizedRuntimeEvent[] {
    const evidence = plannerEvidence(record, parsed, "PLANNER_RESPONSE");
    const thinking = stringValue(parsed.thinking);
    const calls = Array.isArray(parsed.tool_calls) ? parsed.tool_calls : [];
    const content = stringValue(parsed.content);

    if (calls.length === 0 && content === undefined && thinking === undefined) {
      return [
        diagnostic(
          "info",
          "antigravity.empty_planner_response",
          "An Antigravity planner line carried neither content, reasoning nor tool calls.",
          evidence,
        ),
      ];
    }

    if (calls.length === 0 && content !== undefined) {
      // The completion signal. See the heuristic note at the top of the file.
      //
      // MEASURED 2026-09-06, on a stuck run of Colin's: this used to require
      // `thinking === undefined` as well, and a `flash` model that reasons and
      // answers in the SAME step therefore never looked final. The transcript
      // ended on a planner answer, the run stayed live until the idle timeout,
      // and the composer kept its stop button ("antigravity models with stop
      // button stuck after its done with output"). Both captured conversations
      // ended on an answer with no reasoning beside it, so no fixture could
      // have shown this. Reasoning in the same step is still drawn as its own
      // step; what it is not is a reason to keep waiting.
      latestFinal = true;
      const events: NormalizedRuntimeEvent[] = [];
      if (thinking !== undefined) {
        events.push(emit("step.started", { stepKind: "reasoning", evidence }));
        events.push(emit("step.completed", { stepKind: "reasoning", evidence }));
      }
      events.push(
        emit("message.delta", {
          itemId: `msg_${String(stepIndex)}`,
          operation: "replace",
          // Both ends: the blocks the host acts on are at the end (M1).
          text: boundedAnswerText(content),
          final: true,
          evidence,
        }),
      );
      return events;
    }

    latestFinal = false;
    const events: NormalizedRuntimeEvent[] = [];
    if (thinking !== undefined) {
      // Opened AND closed here: the transcript has no separate line saying the
      // model stopped thinking, so a step left open would never be closed.
      events.push(emit("step.started", { stepKind: "reasoning", evidence }));
      events.push(emit("step.completed", { stepKind: "reasoning", evidence }));
    }
    calls.forEach((call, index) => {
      if (!isObject(call)) return;
      const name = identityValue(call.name) ?? "tool";
      const args = call.args;
      // No captured call carried an id of any kind, so the position in the file
      // is the only stable handle: the same line re-read yields the same id.
      const itemId = identityValue(call.id)
        ?? identityValue(call.call_id)
        ?? identityValue(call.toolCallId)
        ?? `tool_${String(stepIndex)}_${String(index)}`;
      /*
       * `ask_question` says what it is asking, on the row, and carries the
       * question structured for the card that answers it.
       *
       * It used to read "Prompting user with options · ask_question · still
       * running" while the run sat blocked for the full idle timeout, and the
       * question and its four options were in the record the whole time
       * (`docs/FINDING-antigravity-ask-question.md`, Colin's screenshot).
       * 2026-09-08 then found no way to answer it from Locust -- `agentapi
       * send-message` arrives as a SYSTEM_MESSAGE, not as the answer -- so the
       * row said "answer in Antigravity" and that was all.
       *
       * 2026-09-23 found the way (Yurt's beta hung on one, "the google question
       * wasnt popping up in our chat"): Antigravity's IDE answers through its
       * language server's `HandleCascadeUserInteraction`, the same server the
       * CLI talks to. So the desktop raises a question card from `question`
       * below and answers it there; the row is the record of what was asked.
       */
      const asked = name === "ask_question" ? antigravityQuestion(args) : undefined;
      const command = asked === undefined
        ? antigravityToolCommand(args)
        : oneLine(
            asked.options.length === 0
              ? asked.question
              : `${asked.question} — ${asked.options.join(" / ")}`,
          );
      const title = asked === undefined ? antigravityToolTitle(args, command) : undefined;
      if (name === "schedule") {
        const watched = decodedString(args, "TimerCondition");
        if (watched !== undefined && watched.length > 0) timerConditions.set(itemId, watched);
      }
      const open: OpenTool = {
        itemId,
        toolKind: name,
        name,
        ...(command === undefined ? {} : { command }),
        ...(title === undefined ? {} : { title }),
      };
      openTools.push(open);
      awaiting.set(stepIndex + 1 + index, open);
      const patch = name === "write_to_file" ? antigravityWritePatch(args) : undefined;
      events.push(
        emit("tool.started", {
          ...open,
          ...(patch === undefined ? {} : { patch }),
          ...(asked === undefined ? {} : { question: { ...asked, askedAtStep: stepIndex } }),
          phase: "started",
          evidence,
        }),
      );
    });
    // A result the file wrote before this line is this line's, if its step
    // says so; one from before this step that nothing claimed never will be.
    for (const [resultStep, held] of [...early].sort(([a], [b]) => a - b)) {
      const open = awaiting.get(resultStep);
      if (open !== undefined) {
        early.delete(resultStep);
        awaiting.delete(resultStep);
        events.push(...closeTool(open, held.parsed, held.evidence));
      } else if (resultStep < stepIndex) {
        early.delete(resultStep);
        events.push(unattached(held.evidence));
      }
    }
    if (content !== undefined) {
      // Content beside tool calls was never captured. It is delivered, but not
      // as a final answer: the turn plainly is not over if a tool just started.
      events.push(
        emit("message.delta", {
          itemId: `msg_${String(stepIndex)}`,
          operation: "replace",
          // Both ends: the blocks the host acts on are at the end (M1).
          text: boundedAnswerText(content),
          final: false,
          evidence,
        }),
      );
    }
    return events;
  }

  function acceptParsed(
    record: RuntimeJsonlRecord,
    parsed: JsonObject,
  ): readonly NormalizedRuntimeEvent[] {
    const type = stringValue(parsed.type);
    const stepIndex = numberValue(parsed.step_index);
    if (type === undefined || stepIndex === undefined) {
      return [
        diagnostic(
          "warning",
          "antigravity.malformed_record",
          "An Antigravity transcript line had no step_index or no type.",
          malformedEvidence(record),
        ),
      ];
    }
    if (seen.has(stepIndex)) return [];
    // A step that has not finished will be appended again with its result; the
    // index stays unseen so the settled version is the one that is normalized.
    // Except a result that handed its work to the background: that line is
    // never rewritten (0 of 48), so it is read as it stands (0.487).
    const handedOff = type === "GENERIC"
      && stringValue(parsed.status) === "RUNNING"
      && ownBackgroundTask(antigravityBackgroundTask(stringValue(parsed.content)), stepIndex) !== undefined;
    if (stringValue(parsed.status) !== "DONE" && !handedOff) return [];
    seen.add(stepIndex);

    if (SILENT_TYPES.has(type)) {
      // A new user or system turn means the previous final answer no longer
      // ends the conversation.
      latestFinal = false;
      // A background task reporting how it ended settles its row (0.487).
      if (type !== "SYSTEM_MESSAGE") return [];
      const said = stringValue(parsed.content);
      const evidence = evidenceFor(record, parsed, type);
      const ending = antigravityTaskEnding(said);
      if (ending !== undefined) {
        return settle(ending.taskId, ending.ended, ending.exitCode === undefined ? undefined : `exited with code ${String(ending.exitCode)}`, evidence);
      }
      // Any other message from a background task is a timer firing.
      const sender = antigravityMessageSender(said);
      if (sender === undefined || !background.has(sender)) return [];
      timerWaitsOn.delete(sender);
      return settle(sender, "completed", "timer went off", evidence);
    }

    if (type === "PLANNER_RESPONSE") return plannerEvents(record, parsed, stepIndex);

    if (type === "GENERIC" && stringValue(parsed.source) === "MODEL") {
      latestFinal = false;
      const evidence = evidenceFor(record, parsed, type);
      const open = awaiting.get(stepIndex);
      if (open === undefined) {
        // Its planner line may not be read yet; that line settles it.
        early.set(stepIndex, { parsed, evidence });
        return [];
      }
      awaiting.delete(stepIndex);
      return closeTool(open, parsed, evidence);
    }

    latestFinal = false;
    return [
      diagnostic(
        "info",
        "antigravity.unknown_step",
        `Unhandled Antigravity transcript line: ${redactText(type)}`,
        evidenceFor(record, parsed, type),
      ),
    ];
  }

  return {
    get runtimeThreadId() {
      return runtimeThreadId;
    },
    get finalized() {
      return finalized;
    },
    get latestFinal() {
      return latestFinal;
    },
    get pendingToolName() {
      return openTools[openTools.length - 1]?.name;
    },
    get pendingBackground() {
      return background.size;
    },

    accept(record: RuntimeJsonlRecord): readonly NormalizedRuntimeEvent[] {
      if (finalized) return [];
      let parsed: unknown;
      try {
        parsed = JSON.parse(record.raw) as unknown;
      } catch {
        return [
          diagnostic("warning", "antigravity.malformed_record", "An Antigravity transcript line could not be parsed.", malformedEvidence(record)),
        ];
      }
      if (!isObject(parsed)) {
        return [
          diagnostic("warning", "antigravity.malformed_record", "An Antigravity transcript line was not an object.", malformedEvidence(record)),
        ];
      }
      return acceptParsed(record, parsed);
    },

    finish(completion: RuntimeProcessCompletion): readonly NormalizedRuntimeEvent[] {
      if (finalized) return [];
      finalized = true;
      const process = processEvidence(completion);
      const thread = runtimeThreadId === undefined ? {} : { runtimeThreadId };
      // Results no planner line ever claimed are said, as they were said when
      // they arrived before this held them.
      const orphans = [...early.values()].map((held) => unattached(held.evidence));
      early.clear();
      if (completion.cancelled) {
        return [...orphans, emit("run.cancelled", { ...thread, process })];
      }
      // A host that only tails the file owns no process and reports no exit
      // code; a number that is not zero is the host saying its own driver died,
      // and that outranks whatever the transcript got to.
      if (typeof completion.exitCode === "number" && completion.exitCode !== 0) {
        return [
          ...orphans,
          emit("run.failed", {
            kind: "process-failed",
            message: `The Antigravity language server exited with code ${String(completion.exitCode)}.`,
            ...thread,
            runtimeTerminal: latestFinal ? "completed" : "missing",
            process,
          }),
        ];
      }
      if (!latestFinal) {
        return [
          ...orphans,
          emit("run.failed", {
            kind: "process-failed",
            message: "The Antigravity transcript never ended on a planner answer with no tool calls, so the host stopped waiting for one.",
            ...thread,
            runtimeTerminal: "missing",
            process,
          }),
        ];
      }
      return [...orphans, emit("run.completed", { ...thread, process })];
    },
  };
}

/** One question Antigravity's `ask_question` put to the person. */
export interface AntigravityQuestion {
  readonly question: string;
  readonly options: readonly string[];
  readonly multiSelect: boolean;
}

/**
 * The question inside an `ask_question` call, or undefined when there is none.
 *
 * Locust drew this tool as a bare row -- "Prompting user with options ·
 * ask_question · still running" -- while the run sat blocked for the full idle
 * timeout (`docs/FINDING-antigravity-ask-question.md`). The question and its
 * options were in the record the whole time; nothing read them.
 *
 * An answer arrives as a MODEL/GENERIC record whose content is `A1: <text>`
 * -- a tool COMPLETION, written when the card is resolved (measured
 * 2026-09-08). `agentapi send-message` is not that: it arrives as a
 * SYSTEM_MESSAGE labelled "not actually sent by the user". The completion is
 * made by the language server's `HandleCascadeUserInteraction`, which is
 * what the desktop now calls (2026-09-23); this is what its card shows.
 *
 * `args.questions` is a JSON-encoded ARRAY, and every value in an Antigravity
 * `args` is JSON-encoded (see `antigravityToolArg`). Only the first question is
 * returned: both measured payloads carry exactly one, and inventing a
 * multi-question UI for a shape never observed would be building for a guess.
 */
export function antigravityQuestion(args: unknown): AntigravityQuestion | undefined {
  const decoded = antigravityToolArg(args, "questions");
  const first = Array.isArray(decoded) ? decoded[0] : undefined;
  if (!isObject(first)) return undefined;
  const question = stringValue(first.question);
  if (question === undefined || question.length === 0) return undefined;
  // A question with no options is still a question. It is the OPTIONS that are
  // optional, not the asking -- so an empty list draws a question and no
  // buttons rather than nothing at all.
  const options = Array.isArray(first.options)
    ? first.options.filter((option): option is string => typeof option === "string" && option.length > 0)
    : [];
  return { question, options, multiSelect: first.is_multi_select === true };
}

/**
 * ANTIGRAVITY'S GAPS, FILLED FROM ITS OWN SERVER (0.389).
 *
 * Antigravity writes `<truncated N bytes>` into its transcript when it keeps
 * only part of a long record -- and the transcript is what this adapter reads
 * -- so a long answer arrived with a hole in its middle. 0.311 drew the hole
 * honestly ("1066 bytes the runtime did not keep"); Colin, 2026-09-27, over a
 * Chief of Staff answer cut at "forced-colors: acti": "slight bug". The text
 * was never lost: the language server's `GetCascadeTrajectorySteps` returns
 * every PLANNER_RESPONSE step whole, as `plannerResponse.response` and
 * `plannerResponse.modifiedResponse` (the field names read from the
 * descriptors compiled into `language_server.exe`).
 *
 * `candidates` are those texts. A transcript text is restored from one only
 * when that one begins with the words before the first gap, ends with the
 * words after the last, and holds every piece between in order -- so an
 * answer is never swapped for a different step's. Anything else keeps the
 * gap, drawn as before.
 */
const GAP = /\n?<truncated \d+ bytes>\n?/;

export function restoreTruncated(truncated: string, candidates: readonly string[]): string | undefined {
  const pieces = truncated.split(GAP);
  if (pieces.length < 2) return undefined;
  const first = pieces[0]!;
  const last = pieces[pieces.length - 1]!;
  for (const full of candidates) {
    if (GAP.test(full) || full.length <= first.length + last.length) continue;
    if (!full.startsWith(first) || !full.endsWith(last)) continue;
    let at = first.length;
    let whole = true;
    for (const piece of pieces.slice(1, -1)) {
      const found = full.indexOf(piece, at);
      if (found < 0) {
        whole = false;
        break;
      }
      at = found + piece.length;
    }
    if (whole && at <= full.length - last.length) return full;
  }
  return undefined;
}

/** The step a message item came from: `msg_12` is step 12. */
export function antigravityMessageStep(itemId: string): number | undefined {
  const matched = /^msg_(\d+)$/.exec(itemId);
  return matched === null ? undefined : Number(matched[1]);
}

/** Whether an event is a message with one of Antigravity's gaps in it. */
export function hasAntigravityGap(event: NormalizedRuntimeEvent): boolean {
  return event.type === "message.delta" && GAP.test(event.payload.text);
}

/**
 * `events`, with every gapped message whole again where one of `candidates`
 * restores it -- bounded and scrubbed exactly as the adapter bounds and scrubs
 * every text it records. Everything else is returned as it came.
 */
export function withRestoredGaps(events: readonly NormalizedRuntimeEvent[], candidates: readonly string[]): readonly NormalizedRuntimeEvent[] {
  // Scrubbed before comparing: the transcript's text was scrubbed when it was
  // read, so a key the model repeated reads "[redacted]" on one side and would
  // never match the other.
  const scrubbed = candidates.map(redactSecrets);
  return events.map((event) => {
    if (event.type !== "message.delta" || !GAP.test(event.payload.text)) return event;
    const full = restoreTruncated(event.payload.text, scrubbed);
    return full === undefined ? event : { ...event, payload: { ...event.payload, text: boundedAnswerText(full) } };
  });
}
