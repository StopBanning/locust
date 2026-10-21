import {
  boundedMessageText,
  evidenceFor,
  identityValue,
  isObject,
  malformedEvidence,
  processEvidence,
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
  ToolPatch,
} from "./codex-events.js";
import { unifiedDiffOf } from "./line-diff.js";
import type { RuntimeJsonlRecord, RuntimeProcessCompletion } from "./process-runner.js";
import type { MissionRuntimeId } from "./types.js";

/**
 * AGENT CLIENT PROTOCOL -> product events (0.377; docs/PLAN-ACP-ROUTE-2026-09-26.md).
 *
 * Every shape here was measured on this machine on 2026-09-26, against
 * `opencode acp` 1.18.27 and `copilot --acp` 1.0.88; the transcripts are the
 * fixtures in `test/fixtures/acp/`. The records are the ACP run's own
 * (acp-run.ts): every `session/update` the agent sent once the prompt was
 * sent, and three the run writes itself --
 *
 * - `locust/session` -- the session id, the moment it is known, so a run that
 *   dies early still leaves something a follow-up can load;
 * - `locust/declined` -- a tool call the person refused. ACP's own account of
 *   a refusal is a failed tool call with the agent's wording, which differs
 *   by agent ("The user rejected permission to use this specific tool call.",
 *   "The user rejected this tool call."); Locust answered the request itself,
 *   so it knows, and the call is recorded as declined (the rule 0.375 made);
 * - `locust/prompt_result` -- a prompt's end: its stop reason and usage. A
 *   run can hold more than one. ACP has no way to add to a turn while it
 *   runs, so a denial's reason, or a message sent to a busy teammate, goes
 *   to the agent as the NEXT prompt once this one ends (acp-run.ts, `steer`)
 *   -- and the run paid for both, so their usage is summed.
 *
 * What the agent says arrives as `agent_message_chunk`, and Copilot's chunks
 * carry no message id at all (measured, 1.0.88). So a message is what is
 * said between two other things -- a tool call, a thought, a plan, a prompt's
 * end -- and each is marked whole and final when it is interrupted, as
 * Codex's message items are. Joined, "I'll run the tests." and the answer
 * after them would read as one run-on sentence.
 *
 * Thinking (`agent_thought_chunk`) is kept, as every other runtime's is --
 * Colin's call, 2026-09-16, recorded at `sanitizeJson`: the step carries what
 * was thought, bounded and scrubbed of secrets, for the thinking row to show.
 *
 * What is NOT kept, and why:
 * - `available_commands_update` lists the machine's own commands and skills
 *   -- the operator's, as Copilot's `session.skills_loaded` is. The run never
 *   records it, and this ignores it again in case anything else ever does.
 * - `user_message_chunk` is the person's own words, which a loaded session
 *   replays and Locust recorded when they were said.
 * - `session_info_update`, `config_option_update`, `current_mode_update` are
 *   the agent narrating its own settings; `usage_update` is its context
 *   meter (`used` of `size`), which nothing in Locust shows yet.
 */

export const ACP_SESSION = "locust/session";
export const ACP_DECLINED = "locust/declined";
export const ACP_PROMPT_RESULT = "locust/prompt_result";

export interface AcpInvocationContext {
  /** Product-owned run identifier. Never the agent's session id. */
  readonly runId: string;
  readonly missionId?: string;
  /** Whose protocol stream this is: the events say which runtime produced them. */
  readonly runtime: MissionRuntimeId;
  readonly requestedRouteId?: string;
  readonly resolvedRouteId?: string;
  readonly cliVersion?: string;
  /** A session being continued, until the stream names its own. */
  readonly sessionId?: string;
  /** Test seam and host clock. */
  readonly now?: () => Date;
}

export interface AcpEventNormalizer {
  readonly runtimeThreadId: string | undefined;
  readonly finalized: boolean;
  accept(record: RuntimeJsonlRecord): readonly NormalizedRuntimeEvent[];
  finish(completion: RuntimeProcessCompletion): readonly NormalizedRuntimeEvent[];
}

type JsonObject = Record<string, unknown>;

/** Updates that say nothing about the work, or carry what must never be kept. */
const IGNORED_UPDATES = new Set([
  "available_commands_update",
  "user_message_chunk",
  "session_info_update",
  "config_option_update",
  "current_mode_update",
  "usage_update",
]);

/**
 * A start that failed because the CLI is not signed in. ACP's own error for
 * it is "Authentication required"; a CLI may say it in its own words.
 */
const SIGNED_OUT = /authenticat|not (?:logged|signed) in|\blog ?in\b|\bsign ?in\b/i;

/** An ACP tool kind, as Locust's rows name it (desktop shared/tool-kinds.ts). */
export function acpToolNaming(kind: string | undefined): { readonly toolKind: string; readonly name: string } {
  switch (kind) {
    case "execute":
      return { toolKind: "command_execution", name: "bash" };
    case "edit":
    case "delete":
    case "move":
      return { toolKind: "file_change", name: kind };
    case "read":
    case "search":
      return { toolKind: kind, name: kind };
    case "fetch":
      return { toolKind: "web_fetch", name: "webfetch" };
    default:
      return { toolKind: kind ?? "tool", name: kind ?? "tool" };
  }
}

/** The text of an ACP content block, or of every text block in a list. */
/** Usage keys kept from one run; an agent's own names beyond this are dropped. */
const MAX_USAGE_KEYS = 32;
/** What a held message or thought keeps of each end while it is still growing. */
const HELD_END = 16_384;

/**
 * A growing text, held with both ends. What is written is bounded at its close,
 * keeping its start and its end, so the middle never has to stay in memory.
 */
function heldText(text: string): string {
  if (text.length <= HELD_END * 4) return text;
  return `${text.slice(0, HELD_END)}\n…\n${text.slice(-HELD_END)}`;
}

function textOf(content: unknown): string | undefined {
  if (Array.isArray(content)) {
    const texts = content.map((entry) => textOf(isObject(entry) && isObject(entry.content) ? entry.content : entry)).filter((text): text is string => text !== undefined);
    return texts.length === 0 ? undefined : texts.join("\n");
  }
  if (!isObject(content)) return undefined;
  return content.type === "text" ? stringValue(content.text) : undefined;
}

/** A tool call's edit, when its content carries one, as a patch. */
function patchOf(content: unknown): ToolPatch | undefined {
  if (!Array.isArray(content)) return undefined;
  const diffs = content.filter((entry): entry is JsonObject => isObject(entry) && entry.type === "diff" && typeof entry.path === "string" && typeof entry.newText === "string");
  const unified = diffs
    .map((entry) => unifiedDiffOf(entry.path as string, (entry.oldText ?? null) as string | null, entry.newText as string))
    .filter((diff): diff is string => diff !== undefined)
    .join("");
  return unified.length === 0 ? undefined : toolPatchFrom(unified);
}

/**
 * What a tool call acted on: a command's own text, or the paths a file tool
 * named. A command's `locations` are only the folder it runs in -- OpenCode's
 * first report of a shell call carries that and nothing else -- so a shell row
 * waits for the command rather than showing the folder as one.
 */
function targetOf(update: JsonObject): string | undefined {
  const input = isObject(update.rawInput) ? update.rawInput : {};
  const command = stringValue(input.command);
  if (command !== undefined) return command;
  if (stringValue(update.kind) === "execute") return undefined;
  const locations = Array.isArray(update.locations) ? update.locations : [];
  const paths = locations.map((location) => (isObject(location) ? stringValue(location.path) : undefined)).filter((path): path is string => path !== undefined);
  return paths.length === 0 ? undefined : paths.join("\n");
}

export function createAcpEventNormalizer(context: AcpInvocationContext): AcpEventNormalizer {
  const runId = requireContextText(context.runId, "runId");
  const cliVersion = context.cliVersion === undefined ? undefined : requireContextText(context.cliVersion, "cliVersion");
  const now = context.now ?? (() => new Date());
  const source = context.runtime;

  let runtimeThreadId: string | undefined = context.sessionId === undefined ? undefined : identityValue(context.sessionId);
  let normalizedSequence = 0;
  let finalized = false;
  let startedRun = false;
  /** How many prompts ended, the last one's reason, and what they cost together. */
  let prompts = 0;
  let stopReason: string | undefined;
  /** The last prompt ended "cancelled" on the host's own refusal, not on a stop (acp-run.ts). */
  let endedOnDecline = false;
  const usage: Record<string, number> = {};
  const declined = new Set<string>();
  const openTools = new Map<string, { readonly toolKind: string; readonly name: string; command: string | undefined; readonly title: string | undefined }>();
  /** The message being written, and what it has said so far. */
  let message: { readonly itemId: string; readonly agentId: string | undefined; text: string } | undefined;
  let messageOrdinal = 0;
  /** How many messages each agent message id has been cut into. */
  const segmentsOf = new Map<string, number>();
  /** One thought at a time: opened by its first chunk, closed by what follows it. */
  let reasoning: { readonly itemId: string; text: string } | undefined;
  let reasoningOrdinal = 0;
  const unknownUpdates = new Set<string>();

  const emit = <TType extends NormalizedRuntimeEventType>(type: TType, payload: NormalizedRuntimePayloadMap[TType]): NormalizedRuntimeEvent => {
    normalizedSequence += 1;
    return {
      id: `${runId}:${source}:${String(normalizedSequence)}`,
      runId,
      ...(context.missionId === undefined ? {} : { missionId: context.missionId }),
      sequence: normalizedSequence,
      occurredAt: now().toISOString(),
      sourceAdapter: source,
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

  const closeReasoning = (evidence: CodexEventEvidence): NormalizedRuntimeEvent[] => {
    if (reasoning === undefined) return [];
    const { itemId, text } = reasoning;
    reasoning = undefined;
    const said = boundedMessageText(text.trim());
    return [emit("step.completed", { stepKind: "reasoning", itemId, ...(said.length === 0 ? {} : { message: said }), evidence })];
  };

  /** The message so far, marked whole: what interrupted it ends it. */
  const closeMessage = (evidence: CodexEventEvidence): NormalizedRuntimeEvent[] => {
    if (message === undefined) return [];
    const { itemId, text } = message;
    message = undefined;
    return text.length === 0 ? [] : [emit("message.delta", { itemId, operation: "replace", text: boundedMessageText(text), final: true, evidence })];
  };

  const openMessage = (agentId: string | undefined): { readonly itemId: string; readonly agentId: string | undefined; text: string } => {
    if (agentId === undefined) {
      messageOrdinal += 1;
      return { itemId: `message_${String(messageOrdinal)}`, agentId, text: "" };
    }
    // An agent that names its messages and keeps the name across a tool call
    // gets one item per stretch, so no stretch replaces another.
    const segment = (segmentsOf.get(agentId) ?? 0) + 1;
    segmentsOf.set(agentId, segment);
    return { itemId: segment === 1 ? agentId : `${agentId}:${String(segment)}`, agentId, text: "" };
  };

  const start = (evidence: CodexEventEvidence): NormalizedRuntimeEvent[] => {
    if (startedRun) return [];
    startedRun = true;
    return [emit("run.started", { runtimeThreadId: runtimeThreadId ?? "", evidence })];
  };

  function acceptUpdate(record: RuntimeJsonlRecord, update: JsonObject): readonly NormalizedRuntimeEvent[] {
    const kind = stringValue(update.sessionUpdate);
    if (kind === undefined) return [];
    // Checked before the update becomes evidence, so what must not be kept
    // is never held in a shape a later edit could emit.
    if (IGNORED_UPDATES.has(kind)) return [];

    if (kind === "agent_thought_chunk") {
      const text = textOf(update.content) ?? "";
      // The words are the step's message; the evidence only names the record.
      const evidence = { transportSequence: record.sequence, runtimeEventType: kind, redacted: true } satisfies CodexEventEvidence;
      if (reasoning !== undefined) {
        reasoning.text = heldText(reasoning.text + text);
        return [];
      }
      reasoningOrdinal += 1;
      reasoning = { itemId: `reasoning_${String(reasoningOrdinal)}`, text };
      return [...start(evidence), ...closeMessage(evidence), emit("step.started", { stepKind: "reasoning", itemId: reasoning.itemId, evidence })];
    }

    const evidence = { ...evidenceFor(record, { sessionUpdate: kind, ...(update.toolCallId === undefined ? {} : { toolCallId: update.toolCallId }) }, kind), redacted: true };

    if (kind === "agent_message_chunk") {
      const text = textOf(update.content);
      if (text === undefined || text.length === 0) return [];
      const agentId = identityValue(update.messageId);
      const events: NormalizedRuntimeEvent[] = [...start(evidence), ...closeReasoning(evidence)];
      if (message !== undefined && agentId !== undefined && message.agentId !== agentId) events.push(...closeMessage(evidence));
      const current = message ?? (message = openMessage(agentId));
      // Held bounded as it grows: the closing replace keeps both ends anyway,
      // so the middle of a long answer is never needed in memory.
      current.text = heldText(current.text + text);
      // An agent may send its answer whole rather than token by token; the
      // chunk is bounded like every other message text (review 2026-10-15, C1).
      events.push(emit("message.delta", { itemId: current.itemId, operation: "append", text: boundedMessageText(text), final: false, evidence }));
      return events;
    }

    if (kind === "plan") {
      const entries = Array.isArray(update.entries) ? update.entries : [];
      const state = { redacted: false };
      return [
        ...start(evidence),
        ...closeReasoning(evidence),
        ...closeMessage(evidence),
        emit("plan.updated", { itemId: "plan", plan: sanitizeJson(entries.map((entry) => (isObject(entry) ? { content: entry.content, status: entry.status } : entry)), state), final: false, evidence }),
      ];
    }

    if (kind === "tool_call" || kind === "tool_call_update") {
      const itemId = identityValue(update.toolCallId);
      if (itemId === undefined) return [];
      const events: NormalizedRuntimeEvent[] = [...start(evidence), ...closeReasoning(evidence)];
      let open = openTools.get(itemId);
      const target = targetOf(update);
      const title = stringValue(update.title);
      if (open === undefined) {
        // What was said before the call is a message of its own.
        events.push(...closeMessage(evidence));
        open = { ...acpToolNaming(stringValue(update.kind)), command: target, title };
        openTools.set(itemId, open);
        events.push(
          emit("tool.started", {
            itemId,
            toolKind: open.toolKind,
            name: open.name,
            ...(target === undefined ? {} : { command: boundedMessageText(target) }),
            // A title that only repeats the tool's name (OpenCode's `bash`) says nothing.
            ...(title === undefined || title === target || title === open.name ? {} : { title: boundedMessageText(title) }),
            phase: "started",
            evidence,
          }),
        );
      } else if (target !== undefined) {
        // The command often arrives on the update, not the first report.
        open.command = target;
      }
      const status = stringValue(update.status);
      if (status !== "completed" && status !== "failed") return events;
      openTools.delete(itemId);
      const refused = declined.has(itemId);
      const failed = status === "failed";
      const output = textOf(update.content);
      const patch = failed ? undefined : patchOf(update.content);
      events.push(
        emit(failed ? "tool.failed" : "tool.completed", {
          itemId,
          toolKind: open.toolKind,
          name: open.name,
          ...(open.command === undefined ? {} : { command: boundedMessageText(open.command) }),
          ...(open.title === undefined || open.title === open.command || open.title === open.name ? {} : { title: boundedMessageText(open.title) }),
          ...(failed ? { status: refused ? "declined" : "failed" } : { status: "completed" }),
          ...(output === undefined ? {} : { output: boundedMessageText(output) }),
          ...(patch === undefined ? {} : { patch }),
          phase: "completed",
          evidence,
        }),
      );
      return events;
    }

    if (unknownUpdates.has(kind)) return [];
    unknownUpdates.add(kind);
    return [diagnostic("info", "acp.unknown_update", `Unhandled ACP update: ${kind}`, evidence)];
  }

  function acceptParsed(record: RuntimeJsonlRecord, parsed: JsonObject): readonly NormalizedRuntimeEvent[] {
    const method = stringValue(parsed.method);
    const params = isObject(parsed.params) ? parsed.params : {};
    if (method === ACP_SESSION) {
      runtimeThreadId = identityValue(params.sessionId) ?? runtimeThreadId;
      return [];
    }
    if (method === ACP_DECLINED) {
      const id = identityValue(params.toolCallId);
      if (id !== undefined) declined.add(id);
      return [];
    }
    if (method === ACP_PROMPT_RESULT) {
      prompts += 1;
      stopReason = stringValue(params.stopReason);
      endedOnDecline = params.endedOnDecline === true;
      if (isObject(params.usage)) {
        for (const [key, amount] of Object.entries(params.usage)) {
          if (typeof amount !== "number" || !Number.isFinite(amount)) continue;
          // Any key an agent invents is kept, up to a number nobody reads past.
          if (usage[key] === undefined && Object.keys(usage).length >= MAX_USAGE_KEYS) continue;
          usage[key] = (usage[key] ?? 0) + amount;
        }
      }
      const evidence = { transportSequence: record.sequence, runtimeEventType: "prompt_result", redacted: true } satisfies CodexEventEvidence;
      // The message the prompt ended on, marked final: what a teammate SAID
      // is its last final message (the transcript tracker), and the chunks
      // alone never say they were the last.
      // Cut off at a limit is not finished: said, so the turn does not read as done (2026-10-10 sweep).
      const limit = stopReason === "max_tokens"
        ? "The agent stopped this turn at its length limit, so the answer may be cut short."
        : stopReason === "max_turn_requests"
          ? "The agent stopped this turn at its limit of steps, so the work may be unfinished."
          : undefined;
      return [...closeReasoning(evidence), ...closeMessage(evidence), ...(limit === undefined ? [] : [diagnostic("warning", "acp.stopped_at_limit", limit, evidence)])];
    }
    if (method === "session/update") {
      const update = isObject(params.update) ? params.update : undefined;
      return update === undefined ? [] : acceptUpdate(record, update);
    }
    return [];
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
        return [diagnostic("warning", "acp.malformed_record", "An ACP record could not be parsed.", malformedEvidence(record))];
      }
      if (!isObject(parsed)) {
        return [diagnostic("warning", "acp.malformed_record", "An ACP record was not an object.", malformedEvidence(record))];
      }
      return acceptParsed(record, parsed);
    },

    finish(completion: RuntimeProcessCompletion): readonly NormalizedRuntimeEvent[] {
      if (finalized) return [];
      finalized = true;
      const process = processEvidence(completion);
      const thread = runtimeThreadId === undefined ? {} : { runtimeThreadId };
      if (completion.cancelled || (stopReason === "cancelled" && !endedOnDecline)) {
        return [emit("run.cancelled", { ...thread, process })];
      }
      // No prompt ended, or the run was lost during a later one (acp-run.ts
      // reports a run that ended on its last prompt's answer with exit 0 and
      // anything else without): the turn did not finish.
      if (prompts === 0 || completion.exitCode !== 0) {
        const said = completion.stderr.trim();
        return [
          emit("run.failed", {
            kind: SIGNED_OUT.test(said) ? "authentication-failed" : "process-failed",
            message: said.length > 0 ? boundedMessageText(said) : "The agent ended before its turn did.",
            ...thread,
            runtimeTerminal: "missing",
            process,
          }),
        ];
      }
      // A turn the agent ended for a reason of its own that is not the work
      // being done is said as such.
      if (stopReason === "refusal") {
        return [emit("run.failed", { kind: "safety-blocked", message: "The agent refused to continue this turn.", ...thread, runtimeTerminal: "failed", process })];
      }
      const state = { redacted: false };
      return [
        emit("run.completed", {
          ...thread,
          ...(Object.keys(usage).length === 0 ? {} : { usage: sanitizeJson(usage, state) }),
          process,
        }),
      ];
    },
  };
}
