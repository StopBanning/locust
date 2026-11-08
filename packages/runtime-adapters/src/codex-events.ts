import type {
  RuntimeJsonlRecord,
  RuntimeProcessCompletion,
} from "./process-runner.js";
import type { MissionRuntimeId } from "./types.js";
import { providerErrorSentence } from "./provider-error.js";

/** JSON that is safe to place in the product event ledger after redaction. */
export type RedactedJsonValue =
  | null
  | boolean
  | number
  | string
  | readonly RedactedJsonValue[]
  | { readonly [key: string]: RedactedJsonValue };

export type NormalizedRuntimeEventType =
  | "run.started"
  | "plan.updated"
  | "message.delta"
  | "step.started"
  | "step.completed"
  | "step.failed"
  | "tool.started"
  | "tool.completed"
  | "tool.failed"
  | "route.limit_detected"
  | "run.cancelled"
  | "run.failed"
  | "run.completed"
  | "adapter.diagnostic";

export type CodexLimitKind = "quota-exhausted" | "temporary-rate-limit";

export type CodexRunFailureKind =
  | CodexLimitKind
  | "authentication-failed"
  | "safety-blocked"
  | "process-failed"
  | "protocol-mismatch"
  | "unknown";

export interface CodexInvocationContext {
  /** Product-owned run identifier. Never use the Codex thread ID in its place. */
  readonly runId: string;
  readonly missionId?: string;
  readonly requestedRouteId?: string;
  readonly resolvedRouteId?: string;
  /** Discovered executable version captured as invocation provenance when available. */
  readonly cliVersion?: string;
  /** Test seam and host clock. Provider events do not include timestamps. */
  readonly now?: () => Date;
}

export interface CodexEventEvidence {
  readonly transportSequence?: number;
  readonly runtimeEventType?: string;
  /** Parsed and recursively redacted provider evidence, or a redacted malformed line. */
  readonly raw?: RedactedJsonValue;
  readonly redacted: boolean;
}

interface RunStartedPayload {
  readonly runtimeThreadId: string;
  readonly evidence: CodexEventEvidence;
}

interface PlanUpdatedPayload {
  readonly itemId: string;
  readonly plan: RedactedJsonValue;
  readonly final: boolean;
  readonly evidence: CodexEventEvidence;
}

interface MessageDeltaPayload {
  readonly itemId: string;
  readonly operation: "append" | "replace";
  readonly text: string;
  readonly final: boolean;
  readonly evidence: CodexEventEvidence;
}

interface StepPayload {
  readonly stepKind: "turn" | "reasoning" | "item";
  readonly itemId?: string;
  readonly itemType?: string;
  readonly status?: string;
  readonly message?: string;
  /** How long the step took, as the runtime timed it (Antigravity CLI, 0.542). */
  readonly durationMs?: number;
  readonly evidence: CodexEventEvidence;
}

/**
 * The exact change a tool made, as the runtime reported it.
 *
 * The app used to record every edit and show a filename and a line count.
 * This is the difference between trusting a receipt and reading it. The
 * counts are DERIVED from the full patch text before any bounding, so a
 * header can never claim +61 over two rendered lines, and `truncated` says
 * when the text on disk is shorter than the change was -- so a reader is
 * told what they are not seeing rather than left to assume it was all.
 */
export interface ToolPatch {
  /** Unified diff text, bounded; see `truncated`. */
  readonly text: string;
  readonly added: number;
  readonly removed: number;
  /** Whether `text` is shorter than the change the runtime reported. */
  readonly truncated: boolean;
}

export interface ToolPayload {
  readonly itemId: string;
  readonly toolKind: string;
  readonly name: string;
  readonly command?: string;
  /**
   * What the model said it was doing, on the runtimes that carry one.
   *
   * Claude Code's Bash tool takes a `description` on every call, and that
   * sentence is why its own transcript reads in intentions rather than shell
   * pipelines. Separate from `command` on purpose: the command is evidence
   * of what ran on this machine, the description is a claim about it by the
   * thing that ran it, and a row may lead with the claim only if the
   * evidence is still there underneath.
   *
   * Codex sends no such field, so this is undefined on that transport and
   * those rows are unchanged.
   */
  readonly title?: string;
  /**
   * Whether the runtime was told to run this one IN THE BACKGROUND.
   *
   * Colin, 2026-09-21: he ran something that went to the background and
   * "when it finished we never got the follow up reply", and guessed there
   * was no UI for a background task anywhere. There was not -- and on
   * 2026-09-22 I told him Copilot was the only runtime that reports them,
   * which was wrong and wrongly reasoned. I had checked this repo's
   * adapters and fixtures, and there are no Claude or Codex fixtures at
   * all, so that check could not have answered the question. He said he was
   * "90% sure claude code and codex also report background tasks". He was
   * right about Claude.
   *
   * Claude Code's Bash tool takes `run_in_background`, so the fact arrives
   * on the tool call's own input and the adapter simply never read it --
   * `claudeToolTarget` took the command and nothing looked at the input
   * again, the same shape as the `description` miss recorded above it.
   *
   * This is the FACT, not a feature: it says the runtime was asked to
   * background the call. It does not claim to know when the work finished,
   * and nothing re-invokes anybody on the strength of it.
   */
  readonly background?: boolean;
  /**
   * A question the runtime put to the person with its OWN tool, structured.
   *
   * Antigravity's `ask_question` holds its agent until the person answers,
   * and Yurt's beta run on 2026-09-23 sat on one Locust never raised: the
   * question showed only as a row inside the folded tool calls. The desktop
   * builds its question card from this, and answers through Antigravity's
   * own server. Absent on every other tool.
   */
  readonly question?: ToolQuestion;
  readonly output?: RedactedJsonValue;
  readonly exitCode?: number;
  readonly status?: string;
  readonly patch?: ToolPatch;
  /**
   * How long the call ran, as the RUNTIME timed it, in milliseconds (0.459).
   * OpenCode's `run` prints a tool call only once it is done -- start and end
   * arrive together, so the arrival times say nothing -- but the part carries
   * its own `state.time.start` and `end`. Absent where the runtime gives none;
   * the desktop then times the call by when its start and end arrived.
   */
  readonly durationMs?: number;
  /**
   * The helper this call was made BY, when the teammate did not make it
   * itself: the item id of the call that sent the helper out (Claude Code's
   * Agent call, whose id its helper's records carry as `parent_tool_use_id`).
   * The desktop draws the call under that helper's row and keeps it out of
   * the teammate's own counts. Absent on every call the teammate made, and
   * on every runtime that does not say (helper visibility, 2026-10-05;
   * ledger v22).
   */
  readonly parentItemId?: string;
  readonly phase: "started" | "updated" | "completed";
  readonly evidence: CodexEventEvidence;
}

/** A question asked through a runtime's own tool, as the tool call carried it. */
export interface ToolQuestion {
  readonly question: string;
  readonly options: readonly string[];
  readonly multiSelect: boolean;
  /**
   * The transcript step that asked. The step that WAITS for the answer comes
   * after it, and is looked up from here.
   */
  readonly askedAtStep: number;
}

interface LimitDetectedPayload {
  readonly kind: CodexLimitKind;
  readonly message: string;
  readonly evidence: CodexEventEvidence;
}

interface DiagnosticPayload {
  readonly level: "info" | "warning" | "error";
  readonly code: string;
  readonly message: string;
  /** Provider diagnostics never decide the product run's terminal state by themselves. */
  readonly terminal: false;
  readonly evidence: CodexEventEvidence;
}

interface ProcessEvidence {
  readonly exitCode: number | null;
  readonly signal: NodeJS.Signals | null;
  readonly stderr: string;
  readonly stderrTruncated: boolean;
  readonly recordCount: number;
  readonly inputDeliveryFailed: boolean;
  readonly outputLimitExceeded: boolean;
  /** Records the host skipped for being too large to carry. */
  readonly oversizedRecordsDropped: number;
  readonly forcedTerminationAttempted: boolean;
  readonly terminationUnconfirmed: boolean;
  readonly startedAt: string;
  readonly finishedAt: string;
}

interface RunCompletedPayload {
  readonly runtimeThreadId?: string;
  readonly usage?: RedactedJsonValue;
  /**
   * The model this run turned out to be, when the runtime named it and the
   * caller asked for something less specific. Claude Code takes an alias --
   * `sonnet`, `opus`, `fable` -- and resolves it to whichever model is
   * newest in that family; its result then states the real one. Absent
   * everywhere else, and absent when a run used more than one model, since
   * there is no single answer then.
   */
  readonly resolvedModel?: string;
  readonly process: ProcessEvidence;
}

interface RunCancelledPayload {
  readonly runtimeThreadId?: string;
  readonly process: ProcessEvidence;
}

interface RunFailedPayload {
  readonly kind: CodexRunFailureKind;
  readonly message: string;
  /** Original, redacted provider error when its display message is translated. */
  readonly evidence?: CodexEventEvidence;
  readonly runtimeThreadId?: string;
  /**
   * The runtime's session cannot be continued: the next turn must start a
   * fresh one rather than resume `runtimeThreadId` (M7, the code review).
   */
  readonly sessionEnded?: true;
  readonly runtimeTerminal: "completed" | "failed" | "missing";
  readonly process: ProcessEvidence;
}

export interface NormalizedRuntimePayloadMap {
  readonly "run.started": RunStartedPayload;
  readonly "plan.updated": PlanUpdatedPayload;
  readonly "message.delta": MessageDeltaPayload;
  readonly "step.started": StepPayload;
  readonly "step.completed": StepPayload;
  readonly "step.failed": StepPayload;
  readonly "tool.started": ToolPayload;
  readonly "tool.completed": ToolPayload;
  readonly "tool.failed": ToolPayload;
  readonly "route.limit_detected": LimitDetectedPayload;
  readonly "run.cancelled": RunCancelledPayload;
  readonly "run.failed": RunFailedPayload;
  readonly "run.completed": RunCompletedPayload;
  readonly "adapter.diagnostic": DiagnosticPayload;
}

interface NormalizedRuntimeEventBase {
  readonly id: string;
  readonly runId: string;
  readonly missionId?: string;
  readonly sequence: number;
  readonly occurredAt: string;
  /** Which runtime produced this event. Every adapter emits this same shape. */
  readonly sourceAdapter: MissionRuntimeId;
  readonly cliVersion?: string;
  readonly requestedRouteId?: string;
  readonly resolvedRouteId?: string;
  readonly runtimeThreadId?: string;
}

export type NormalizedRuntimeEvent = {
  readonly [TType in NormalizedRuntimeEventType]: NormalizedRuntimeEventBase & {
    readonly type: TType;
    readonly payload: NormalizedRuntimePayloadMap[TType];
  };
}[NormalizedRuntimeEventType];

export interface CodexEventNormalizer {
  readonly runtimeThreadId: string | undefined;
  readonly finalized: boolean;
  accept(record: RuntimeJsonlRecord): readonly NormalizedRuntimeEvent[];
  finish(completion: RuntimeProcessCompletion): readonly NormalizedRuntimeEvent[];
}

type JsonObject = Record<string, unknown>;

interface ItemState {
  readonly id: string;
  type: string;
  messageText: string;
  toolStarted: boolean;
  completed: boolean;
}

const MAX_EVIDENCE_STRING_LENGTH = 8_192;
const MAX_EVIDENCE_ARRAY_LENGTH = 100;
const MAX_EVIDENCE_OBJECT_KEYS = 100;
const MAX_EVIDENCE_DEPTH = 8;

const SENSITIVE_KEY = /^(?:api[_-]?key|access[_-]?token|refresh[_-]?token|auth(?:orization)?|password|passwd|secret|cookie|set-cookie|credential|session[_-]?token)$/i;

const QUOTA_PATTERNS = [
  /\byou(?:'ve| have) hit your usage limit\b/i,
  /\b(?:usage|quota) limit (?:has been )?(?:reached|exceeded)\b/i,
  /\b(?:quota|usage allowance) (?:is )?exhausted\b/i,
  /\binsufficient_quota\b/i,
] as const;

const RATE_LIMIT_PATTERNS = [
  /\brate[_ -]?limit(?:ed| exceeded)?\b/i,
  /\btoo many requests\b/i,
  /\bhttp\s*429\b/i,
] as const;

const AUTHENTICATION_PATTERNS = [
  /\bauthentication (?:failed|required)\b/i,
  /\bunauthorized\b/i,
  /\binvalid api key\b/i,
  /\blogin (?:required|expired)\b/i,
  // Codex's own words when the saved sign-in has expired (QA-2026-09-29 round 2, N11).
  /\baccess token could not be refreshed\b/i,
  /\bsign in again\b/i,
] as const;

const SAFETY_PATTERNS = [
  /\bsafety (?:policy|refusal|block)\b/i,
  /\bblocked by policy\b/i,
  /\bcontent policy\b/i,
] as const;

// `collab_tool_call` is Codex's subagent machinery: `spawn_agent` with a
// prompt, then wait/send/close on the agents it made. MEASURED 2026-09-06
// off Colin's own ledger: as an unknown item type it became a step, which
// left the fold empty and the sidebar silent about two agents that ran.
const TOOL_ITEM_TYPES = new Set([
  "command_execution",
  "file_change",
  "mcp_tool_call",
  "web_search",
  "collab_tool_call",
]);

export function isObject(value: unknown): value is JsonObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function stringValue(value: unknown): string | undefined {
  if (typeof value !== "string" || value.length === 0) return undefined;
  // Every persisted string originates here. The mission ledger's reader refuses
  // any string containing NUL, and a refused record stops recovery at that
  // point -- so no NUL may leave this function, whatever the provider sent.
  const clean = value.includes("\u0000") ? value.replace(/\u0000/g, "") : value;
  return clean.length > 0 ? clean : undefined;
}

// The mission ledger's reader caps identity-ish fields (item id, item type,
// tool kind, tool name, status) at 512 characters and refuses NUL outright.
// Provider records carry no such bound, so clamp here rather than emit an
// event that cannot be persisted -- a truncated tool name costs nothing next
// to a mission whose recovery stops at that record.
const MAX_IDENTITY_LENGTH = 512;
// The ledger reader's cap for free text.
const MAX_MESSAGE_TEXT_LENGTH = 16_384;
// Its cap for an answer, `message.delta`'s text (mission-store's
// LEDGER_MESSAGE_TEXT_LIMIT, 0.869): at 16,384 a long answer lost its middle,
// in the window and for every app reading it over Locust's MCP server.
const MAX_ANSWER_TEXT_LENGTH = 131_072;

/**
 * How much of the END is always kept.
 *
 * This cap used to keep the head and drop the tail, which is the wrong end.
 * Every teammate protocol block -- `<locust-share>`, the memory block, the ask,
 * the room task -- is TAUGHT to sit at the end of the reply, in as many words:
 * "end your reply with exactly this block and nothing after it". So a long
 * answer had its blocks cut off here, and the run posted nothing to the
 * workroom while the thread showed a complete, healthy turn.
 *
 * Reported by a Cursor teammate surveying this source from inside Locust
 * (2026-09-08) and confirmed against these lines: "A long Cursor turn can look
 * complete in the thread and still post nothing to the workroom."
 *
 * 4 KiB holds any of the four blocks with room to spare -- a share carrying a
 * paragraph is a few hundred characters -- and still leaves 12 KiB of the
 * answer itself, which is more than anybody reads inline.
 */
const MESSAGE_TAIL_KEPT = 4_096;
const TRUNCATION_MARK = "\n\u2026[truncated]\u2026\n";

/**
 * Bound a message, keeping BOTH ends.
 *
 * The middle is what nobody misses: the start says what the answer is about,
 * and the end carries the blocks the host has to parse.
 */
export function boundedMessageText(value: string, limit = MAX_MESSAGE_TEXT_LENGTH): string {
  // Scrubbed as well as bounded: every message text an adapter records goes
  // through here, and a key a model repeats in its answer or its thinking
  // is a secret wherever it sits. MEASURED 2026-09-17 on a live Cursor turn:
  // the evidence read [redacted], the answer text carried the key verbatim.
  const clean = redactSecrets(value);
  if (clean.length <= limit) return clean;
  const head = clean.slice(0, limit - MESSAGE_TAIL_KEPT - TRUNCATION_MARK.length);
  return `${head}${TRUNCATION_MARK}${clean.slice(-MESSAGE_TAIL_KEPT)}`;
}

/** An answer: what a `message.delta` carries, bounded at the ledger's own, larger, limit for it. */
export function boundedAnswerText(value: string): string {
  return boundedMessageText(value, MAX_ANSWER_TEXT_LENGTH);
}

export function identityValue(value: unknown): string | undefined {
  const clean = stringValue(value) ?? "";
  if (clean.trim().length === 0) return undefined;
  return clean.length > MAX_IDENTITY_LENGTH
    ? `${clean.slice(0, MAX_IDENTITY_LENGTH - 1)}\u2026`
    : clean;
}

function numberValue(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

/** Secrets only -- no size limit. What a message is scrubbed with; evidence adds a bound on top. */
export function redactSecrets(value: string): string {
  // NUL is unpersistable: the mission ledger's reader rejects any string
  // containing it, and a rejected record stops recovery at that point. Strip
  // it here, where every persisted string already passes through.
  const withoutNul = value.includes("\u0000") ? value.replace(/\u0000/g, "") : value;
  return withoutNul
    .replace(/\bBearer\s+[A-Za-z0-9._~+/=-]{8,}/gi, "Bearer [redacted]")
    .replace(/\b(?:sk|pk)-[A-Za-z0-9_-]{12,}\b/g, "[redacted]")
    .replace(/\b(?:gh[opusr]_[A-Za-z0-9]{20,}|xox[baprs]-[A-Za-z0-9-]{10,}|AKIA[A-Z0-9]{16})\b/g, "[redacted]")
    // A private key, a bare JWT, a Google API key, a Hugging Face token (0.543, the 0.536 review SEC-07).
    .replace(/-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z0-9 ]*PRIVATE KEY-----/g, "[redacted]")
    .replace(/\beyJ[A-Za-z0-9_-]{8,}\.eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}/g, "[redacted]")
    .replace(/\bAIza[0-9A-Za-z_-]{35}\b/g, "[redacted]")
    .replace(/\bhf_[A-Za-z0-9]{30,}\b/g, "[redacted]")
    .replace(
      // `["']?` before the colon (0.489): the JSON shape of a key,
      // `"accessToken": "..."`, put a quote between the name and the colon.
      /((?:api[_-]?key|access[_-]?token|refresh[_-]?token|id[_-]?token|client[_-]?secret|authorization|password|secret|cookie|credential)["']?\s*[=:]\s*["']?)([^\s,"';}]+)/gi,
      "$1[redacted]",
    )
    // Anthropic's OAuth tokens, whatever they sit beside (0.489).
    .replace(/\bsk-ant-[A-Za-z0-9_-]{12,}/g, "[redacted]");
}

/** Evidence: scrubbed AND bounded to the evidence limit. Not for message text, which has its own bound. */
export function redactText(value: string): string {
  const withoutNul = value.includes("\u0000") ? value.replace(/\u0000/g, "") : value;
  const truncated = withoutNul.length > MAX_EVIDENCE_STRING_LENGTH
    ? `${withoutNul.slice(0, MAX_EVIDENCE_STRING_LENGTH)}\u2026[truncated]`
    : withoutNul;
  return truncated
    .replace(/\bBearer\s+[A-Za-z0-9._~+/=-]{8,}/gi, "Bearer [redacted]")
    .replace(/\b(?:sk|pk)-[A-Za-z0-9_-]{12,}\b/g, "[redacted]")
    .replace(/\b(?:gh[opusr]_[A-Za-z0-9]{20,}|xox[baprs]-[A-Za-z0-9-]{10,}|AKIA[A-Z0-9]{16})\b/g, "[redacted]")
    // A private key, a bare JWT, a Google API key, a Hugging Face token (0.543, the 0.536 review SEC-07).
    .replace(/-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z0-9 ]*PRIVATE KEY-----/g, "[redacted]")
    .replace(/\beyJ[A-Za-z0-9_-]{8,}\.eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}/g, "[redacted]")
    .replace(/\bAIza[0-9A-Za-z_-]{35}\b/g, "[redacted]")
    .replace(/\bhf_[A-Za-z0-9]{30,}\b/g, "[redacted]")
    .replace(
      // `["']?` before the colon (0.489): the JSON shape of a key,
      // `"accessToken": "..."`, put a quote between the name and the colon.
      /((?:api[_-]?key|access[_-]?token|refresh[_-]?token|id[_-]?token|client[_-]?secret|authorization|password|secret|cookie|credential)["']?\s*[=:]\s*["']?)([^\s,"';}]+)/gi,
      "$1[redacted]",
    )
    // Anthropic's OAuth tokens, whatever they sit beside (0.489).
    .replace(/\bsk-ant-[A-Za-z0-9_-]{12,}/g, "[redacted]");
}

/** The longest key the ledger reads (mission-store `isRedactedJson`). */
const MAX_EVIDENCE_KEY_LENGTH = 512;

export function sanitizeJson(
  value: unknown,
  state: { redacted: boolean },
  depth = 0,
  key?: string,
): RedactedJsonValue {
  /*
   * SECRETS ONLY. Reasoning used to be redacted in this same branch -- the
   * model's working-out treated as the same species as an API key -- so the
   * app could report that a run had thought and never what about.
   *
   * Colin's call, 2026-09-16: keep it. The cost is that a ledger sent to
   * somebody carries the working-out too; the gain is that the run can be
   * read. Secrets are still scrubbed from it by the very next branch, so a
   * token a model happened to repeat while thinking is redacted exactly as
   * it would be anywhere else.
   */
  if (key !== undefined && SENSITIVE_KEY.test(key)) {
    state.redacted = true;
    return "[redacted]";
  }
  if (value === null || typeof value === "boolean") return value;
  if (typeof value === "number") return Number.isFinite(value) ? value : String(value);
  if (typeof value === "string") {
    const redacted = redactText(value);
    if (redacted !== value) state.redacted = true;
    return redacted;
  }
  if (depth >= MAX_EVIDENCE_DEPTH) {
    state.redacted = true;
    return "[depth limit]";
  }
  if (Array.isArray(value)) {
    if (value.length > MAX_EVIDENCE_ARRAY_LENGTH) state.redacted = true;
    return value
      .slice(0, MAX_EVIDENCE_ARRAY_LENGTH)
      .map((entry) => sanitizeJson(entry, state, depth + 1));
  }
  if (isObject(value)) {
    const result: Record<string, RedactedJsonValue> = {};
    const entries = Object.entries(value);
    if (entries.length > MAX_EVIDENCE_OBJECT_KEYS) state.redacted = true;
    for (const [entryKey, entryValue] of entries.slice(0, MAX_EVIDENCE_OBJECT_KEYS)) {
      // The ledger reads no key over 512 characters or with a NUL in it, and
      // one such key made the write throw and stopped the run (a B4 lead).
      const safeKey = entryKey.includes("\u0000") || entryKey.length > MAX_EVIDENCE_KEY_LENGTH
        ? `${entryKey.replace(/\u0000/g, "").slice(0, MAX_EVIDENCE_KEY_LENGTH - 12)}[truncated]`
        : entryKey;
      if (safeKey !== entryKey) state.redacted = true;
      result[safeKey] = sanitizeJson(entryValue, state, depth + 1, entryKey);
    }
    return result;
  }
  state.redacted = true;
  return `[unsupported ${typeof value}]`;
}

function sanitizeReasoningRecord(record: JsonObject): JsonObject {
  const item = isObject(record.item) ? record.item : undefined;
  if (item === undefined || item.type !== "reasoning") return record;
  const safeItem: JsonObject = { type: "reasoning" };
  const id = stringValue(item.id);
  const status = stringValue(item.status);
  if (id !== undefined) safeItem.id = id;
  if (status !== undefined) safeItem.status = status;
  return { type: record.type, item: safeItem, reasoning_content: "[redacted]" };
}

export function evidenceFor(
  record: RuntimeJsonlRecord,
  parsed: unknown,
  runtimeEventType?: string,
): CodexEventEvidence {
  const state = { redacted: false };
  const safeSource = isObject(parsed) ? sanitizeReasoningRecord(parsed) : parsed;
  const raw = sanitizeJson(safeSource, state);
  return {
    transportSequence: record.sequence,
    ...(runtimeEventType === undefined ? {} : { runtimeEventType }),
    raw,
    redacted: state.redacted || safeSource !== parsed,
  };
}

export function malformedEvidence(record: RuntimeJsonlRecord): CodexEventEvidence {
  const redacted = redactText(record.raw);
  return {
    transportSequence: record.sequence,
    raw: redacted,
    redacted: redacted !== record.raw,
  };
}

function messageFromError(value: unknown): string | undefined {
  if (typeof value === "string") return value;
  if (!isObject(value)) return undefined;
  return stringValue(value.message) ?? stringValue(value.error);
}

function limitKind(message: string): CodexLimitKind | undefined {
  if (QUOTA_PATTERNS.some((pattern) => pattern.test(message))) {
    return "quota-exhausted";
  }
  if (RATE_LIMIT_PATTERNS.some((pattern) => pattern.test(message))) {
    return "temporary-rate-limit";
  }
  return undefined;
}

/** What a failure message says went wrong; shared with the app-server transport (M3). */
export function failureKind(message: string | undefined): CodexRunFailureKind {
  if (message === undefined) return "unknown";
  const routeLimit = limitKind(message);
  if (routeLimit !== undefined) return routeLimit;
  if (AUTHENTICATION_PATTERNS.some((pattern) => pattern.test(message))) {
    return "authentication-failed";
  }
  if (SAFETY_PATTERNS.some((pattern) => pattern.test(message))) {
    return "safety-blocked";
  }
  return "unknown";
}

function toolName(item: JsonObject, itemType: string): string {
  if (itemType === "command_execution") return "shell";
  // Named so the thread reads every collab call as a subagent row: spawn,
  // wait, send_input, close. The verb after the colon is the runtime's.
  if (itemType === "collab_tool_call") return `subagent:${identityValue(item.tool) ?? "spawn_agent"}`;
  if (itemType === "web_search") return "web_search";
  if (itemType === "file_change") return "file_change";
  const server = identityValue(item.server) ?? identityValue(item.server_name);
  const tool = identityValue(item.tool) ?? identityValue(item.tool_name) ?? "mcp_tool";
  // Both halves are individually bounded; the join is not, so bound it again.
  return identityValue(server === undefined ? tool : `${server}.${tool}`) ?? "mcp_tool";
}

export function processEvidence(completion: RuntimeProcessCompletion): ProcessEvidence {
  return {
    exitCode: completion.exitCode,
    signal: completion.signal,
    stderr: redactText(completion.stderr),
    stderrTruncated: completion.stderrTruncated,
    recordCount: completion.recordCount,
    inputDeliveryFailed: completion.inputDeliveryFailed,
    outputLimitExceeded: completion.outputLimitExceeded,
    oversizedRecordsDropped: completion.oversizedRecordsDropped,
    forcedTerminationAttempted: completion.forcedTerminationAttempted,
    terminationUnconfirmed: completion.terminationUnconfirmed,
    startedAt: completion.startedAt,
    finishedAt: completion.finishedAt,
  };
}

/**
 * A patch is larger than a message but not unbounded: 64 KiB holds any
 * change a person would review inline, and a ledger record stays a record.
 */
const MAX_PATCH_TEXT_LENGTH = 64 * 1024;

/**
 * Turn a runtime's unified diff into the ledger's patch record. The counts
 * come from the WHOLE text, then the text is bounded, in that order, so the
 * numbers describe the change and not the excerpt.
 *
 * COUNTED BY THE HUNKS (QA-2026-09-29 round 2, R32), the way the viewer reads
 * them: inside a hunk that still expects lines, a line is a row whatever it
 * begins with. Every line starting `---` or `+++` used to be skipped as a
 * header, so a removed `-- comment` (sent as `--- comment`) or an added
 * `++ x` counted as nothing, and a SQL edit read +0 -0.
 */
export function toolPatchFrom(unified: string): ToolPatch | undefined {
  if (unified.length === 0) return undefined;
  let added = 0;
  let removed = 0;
  let oldLeft = 0;
  let newLeft = 0;
  // A `@@` line without counts (some runtimes write a bare one): its rows run
  // to the next file header, a `---` line followed by a `+++` line.
  let open = false;
  const lines = unified.split("\n");
  // No hunk at all: every +/- line is a row, as before, headers aside.
  if (!lines.some((line) => line.startsWith("@@"))) {
    for (const line of lines) {
      if (line.startsWith("+++") || line.startsWith("---")) continue;
      if (line.startsWith("+")) added += 1;
      else if (line.startsWith("-")) removed += 1;
    }
  }
  for (let at = 0; at < lines.length; at += 1) {
    const line = lines[at]!;
    if (oldLeft > 0 || newLeft > 0) {
      if (line.startsWith("+") && newLeft > 0) { added += 1; newLeft -= 1; continue; }
      if (line.startsWith("-") && oldLeft > 0) { removed += 1; oldLeft -= 1; continue; }
      if (line.startsWith(" ") && oldLeft > 0 && newLeft > 0) { oldLeft -= 1; newLeft -= 1; continue; }
    }
    const hunk = /^@@ -\d+(?:,(\d+))? \+\d+(?:,(\d+))? @@/.exec(line);
    if (hunk !== null) {
      oldLeft = Number(hunk[1] ?? "1");
      newLeft = Number(hunk[2] ?? "1");
      open = false;
      continue;
    }
    if (line.startsWith("@@")) { oldLeft = 0; newLeft = 0; open = true; continue; }
    if (line.startsWith("--- ") && (lines[at + 1] ?? "").startsWith("+++ ")) { open = false; at += 1; continue; }
    if (!open) continue;
    if (line.startsWith("+")) added += 1;
    else if (line.startsWith("-")) removed += 1;
  }
  const clean = unified.replaceAll("\0", "");
  const truncated = clean.length > MAX_PATCH_TEXT_LENGTH;
  return {
    text: truncated ? clean.slice(0, MAX_PATCH_TEXT_LENGTH) : clean,
    added,
    removed,
    truncated,
  };
}

export function requireContextText(value: string, label: string): string {
  if (!value.trim() || value.includes("\0")) throw new Error(`${label} must be non-empty`);
  return value;
}

/**
 * Stateful, forward-compatible normalizer for `codex exec --json` output.
 *
 * It intentionally does not make a terminal decision while consuming provider
 * records. `finish` correlates those records with host-owned process metadata;
 * only a `turn.completed` record plus a clean exit is success, and cancellation
 * is synthesized exclusively from `completion.cancelled`.
 */
export function createCodexEventNormalizer(
  context: CodexInvocationContext,
): CodexEventNormalizer {
  const runId = requireContextText(context.runId, "runId");
  const cliVersion = context.cliVersion === undefined
    ? undefined
    : requireContextText(context.cliVersion, "cliVersion");
  const now = context.now ?? (() => new Date());
  const items = new Map<string, ItemState>();
  const emittedLimits = new Set<CodexLimitKind>();
  let runtimeThreadId: string | undefined;
  let normalizedSequence = 0;
  let previousTransportSequence = 0;
  let sawTurnCompleted = false;
  let sawTurnFailed = false;
  let finalized = false;
  let lastTerminalMessage: string | undefined;
  let completedUsage: RedactedJsonValue | undefined;

  const emit = <TType extends NormalizedRuntimeEventType>(
    type: TType,
    payload: NormalizedRuntimePayloadMap[TType],
  ): NormalizedRuntimeEvent => {
    normalizedSequence += 1;
    return {
      id: `${runId}:codex:${normalizedSequence}`,
      runId,
      ...(context.missionId === undefined ? {} : { missionId: context.missionId }),
      sequence: normalizedSequence,
      occurredAt: now().toISOString(),
      sourceAdapter: "codex",
      ...(cliVersion === undefined ? {} : { cliVersion }),
      ...(context.requestedRouteId === undefined
        ? {}
        : { requestedRouteId: context.requestedRouteId }),
      ...(context.resolvedRouteId === undefined
        ? {}
        : { resolvedRouteId: context.resolvedRouteId }),
      ...(runtimeThreadId === undefined ? {} : { runtimeThreadId }),
      type,
      payload,
    } as NormalizedRuntimeEvent;
  };

  const diagnostic = (
    level: DiagnosticPayload["level"],
    code: string,
    message: string,
    evidence: CodexEventEvidence,
  ): NormalizedRuntimeEvent => emit("adapter.diagnostic", {
    level,
    code,
    message,
    terminal: false,
    evidence,
  });

  const maybeEmitLimit = (
    message: string,
    evidence: CodexEventEvidence,
    displayMessage = message,
  ): NormalizedRuntimeEvent | undefined => {
    const kind = limitKind(message);
    if (kind === undefined || emittedLimits.has(kind)) return undefined;
    emittedLimits.add(kind);
    return emit("route.limit_detected", {
      kind,
      message: redactText(providerErrorSentence(displayMessage, "codex")),
      evidence,
    });
  };

  const itemIdentity = (
    item: JsonObject,
    record: RuntimeJsonlRecord,
    eventType: string,
    evidence: CodexEventEvidence,
    events: NormalizedRuntimeEvent[],
  ): { id: string; type: string; state: ItemState } => {
    const rawId = identityValue(item.id);
    const rawType = identityValue(item.type);
    const id = rawId ?? `codex-item-${record.sequence}`;
    const itemType = rawType ?? "unknown";
    if (rawId === undefined || rawType === undefined) {
      events.push(diagnostic(
        "warning",
        "codex.item_identity_missing",
        `${eventType} omitted an item id or type; a transport-scoped identity was used`,
        evidence,
      ));
    }
    const existing = items.get(id);
    if (existing !== undefined) {
      if (existing.type === "unknown" && itemType !== "unknown") existing.type = itemType;
      return { id, type: existing.type, state: existing };
    }
    const state: ItemState = {
      id,
      type: itemType,
      messageText: "",
      toolStarted: false,
      completed: false,
    };
    items.set(id, state);
    return { id, type: itemType, state };
  };

  const messageEvent = (
    item: JsonObject,
    state: ItemState,
    final: boolean,
    evidence: CodexEventEvidence,
  ): NormalizedRuntimeEvent | undefined => {
    const explicitDelta = stringValue(item.delta) ?? stringValue(item.text_delta);
    const replacement = stringValue(item.text);
    let operation: MessageDeltaPayload["operation"];
    let text: string;

    if (explicitDelta !== undefined) {
      operation = "append";
      text = explicitDelta;
      state.messageText += explicitDelta;
    } else if (replacement !== undefined) {
      if (state.messageText.length > 0 && replacement.startsWith(state.messageText)) {
        operation = "append";
        text = replacement.slice(state.messageText.length);
      } else {
        operation = "replace";
        text = replacement;
      }
      state.messageText = replacement;
    } else if (final) {
      operation = "append";
      text = "";
    } else {
      return undefined;
    }
    // Bounded and scrubbed in one place. This used to bound to 16,384 and
    // then hand the result to `redactText`, which cut it again at the
    // 8,192 EVIDENCE limit -- so every long Codex answer lost its second
    // half in the ledger while the thread had shown all of it.
    text = boundedAnswerText(text);

    return emit("message.delta", {
      itemId: state.id,
      operation,
      text,
      final,
      evidence,
    });
  };

  const toolPayload = (
    item: JsonObject,
    itemId: string,
    itemType: string,
    phase: ToolPayload["phase"],
    evidence: CodexEventEvidence,
  ): ToolPayload => {
    // A file_change item names its files -- path and kind, no diff. The
    // paths ARE the command for the activity row: without them the row read
    // "file_change · Codex CLI did not report the change" beside an edit that
    // had named two files. Measured 2026-09-03.
    const changedPaths = Array.isArray(item.changes)
      ? item.changes
          .map((change) => (isObject(change) ? stringValue(change.path) : undefined))
          .filter((path): path is string => path !== undefined)
      : [];
    // A subagent call names its ask, not a path.
    const command = stringValue(item.command) ?? stringValue(item.prompt) ?? (changedPaths.length > 0 ? changedPaths.join("\n") : undefined);
    const status = identityValue(item.status);
    const exitCode = numberValue(item.exit_code) ?? numberValue(item.exitCode);
    const rawOutput = item.aggregated_output ?? item.output ?? item.result;
    const outputState = { redacted: false };
    const output = rawOutput === undefined
      ? undefined
      : sanitizeJson(rawOutput, outputState);
    return {
      itemId,
      toolKind: itemType,
      name: toolName(item, itemType),
      ...(command === undefined ? {} : { command: redactText(command) }),
      ...(output === undefined ? {} : { output }),
      ...(exitCode === undefined ? {} : { exitCode }),
      ...(status === undefined ? {} : { status }),
      phase,
      evidence,
    };
  };

  const acceptItem = (
    eventType: "item.started" | "item.updated" | "item.completed",
    record: RuntimeJsonlRecord,
    parsed: JsonObject,
    evidence: CodexEventEvidence,
  ): readonly NormalizedRuntimeEvent[] => {
    const events: NormalizedRuntimeEvent[] = [];
    if (!isObject(parsed.item)) {
      return [diagnostic(
        "warning",
        "codex.item_missing",
        `${eventType} did not contain an item object`,
        evidence,
      )];
    }
    const item = parsed.item;
    const identity = itemIdentity(item, record, eventType, evidence, events);
    const { id, type: itemType, state } = identity;
    const final = eventType === "item.completed";

    if (state.completed && eventType !== "item.completed") {
      events.push(diagnostic(
        "warning",
        "codex.item_after_completion",
        `Received ${eventType} after item completion`,
        evidence,
      ));
    }

    if (itemType === "agent_message") {
      const message = messageEvent(item, state, final, evidence);
      if (message !== undefined) events.push(message);
    } else if (itemType === "reasoning") {
      if (eventType === "item.started") {
        events.push(emit("step.started", {
          stepKind: "reasoning",
          itemId: id,
          itemType,
          evidence,
        }));
      } else if (final) {
        const status = stringValue(item.status);
        events.push(emit("step.completed", {
          stepKind: "reasoning",
          itemId: id,
          itemType,
          ...(status === undefined ? {} : { status }),
          evidence,
        }));
      }
    } else if (itemType === "error") {
      const message = stringValue(item.message) ?? "Codex reported an item diagnostic";
      events.push(diagnostic(
        "error",
        "codex.item_error",
        redactText(providerErrorSentence(message, "codex")),
        evidence,
      ));
    } else if (itemType === "todo_list") {
      const stateForPlan = { redacted: false };
      events.push(emit("plan.updated", {
        itemId: id,
        plan: sanitizeJson(item.items ?? item.todos ?? item, stateForPlan),
        final,
        evidence,
      }));
    } else if (TOOL_ITEM_TYPES.has(itemType)) {
      if (eventType === "item.started") {
        state.toolStarted = true;
        events.push(emit("tool.started", toolPayload(
          item,
          id,
          itemType,
          "started",
          evidence,
        )));
      } else if (eventType === "item.updated") {
        if (!state.toolStarted) state.toolStarted = true;
        events.push(emit("tool.started", toolPayload(
          item,
          id,
          itemType,
          "updated",
          evidence,
        )));
      } else {
        const status = stringValue(item.status)?.toLowerCase();
        const exitCode = numberValue(item.exit_code) ?? numberValue(item.exitCode);
        const failed = status === "failed" || status === "error" || exitCode !== undefined && exitCode !== 0;
        events.push(emit(failed ? "tool.failed" : "tool.completed", toolPayload(
          item,
          id,
          itemType,
          "completed",
          evidence,
        )));
      }
    } else if (eventType === "item.started") {
      events.push(emit("step.started", {
        stepKind: "item",
        itemId: id,
        itemType,
        evidence,
      }));
    } else if (final) {
      const status = stringValue(item.status);
      events.push(emit("step.completed", {
        stepKind: "item",
        itemId: id,
        itemType,
        ...(status === undefined ? {} : { status }),
        evidence,
      }));
    } else {
      events.push(diagnostic(
        "info",
        "codex.unknown_item_update",
        `Preserved an update for unknown item type ${itemType}`,
        evidence,
      ));
    }

    if (final) state.completed = true;
    return events;
  };

  const accept = (record: RuntimeJsonlRecord): readonly NormalizedRuntimeEvent[] => {
    if (finalized) throw new Error("Codex event normalizer is already finalized");
    const prefixEvents: NormalizedRuntimeEvent[] = [];
    if (!Number.isSafeInteger(record.sequence) || record.sequence <= 0) {
      prefixEvents.push(diagnostic(
        "warning",
        "codex.invalid_transport_sequence",
        "Runtime record had an invalid transport sequence",
        malformedEvidence(record),
      ));
    } else if (record.sequence <= previousTransportSequence) {
      prefixEvents.push(diagnostic(
        "warning",
        "codex.nonmonotonic_transport_sequence",
        "Runtime record sequence did not increase monotonically",
        malformedEvidence(record),
      ));
    }
    previousTransportSequence = Math.max(previousTransportSequence, record.sequence);

    let parsed: unknown;
    try {
      parsed = JSON.parse(record.raw) as unknown;
    } catch {
      return [...prefixEvents, diagnostic(
        "warning",
        "codex.malformed_json",
        "Codex emitted a malformed JSONL record",
        malformedEvidence(record),
      )];
    }

    if (!isObject(parsed)) {
      return [...prefixEvents, diagnostic(
        "warning",
        "codex.non_object_record",
        "Codex emitted a JSONL value that was not an object",
        evidenceFor(record, parsed),
      )];
    }

    const eventType = stringValue(parsed.type);
    const evidence = evidenceFor(record, parsed, eventType);
    if (eventType === undefined) {
      return [...prefixEvents, diagnostic(
        "warning",
        "codex.event_type_missing",
        "Codex emitted an event without a type",
        evidence,
      )];
    }

    switch (eventType) {
      case "thread.started": {
        const threadId = stringValue(parsed.thread_id);
        if (threadId === undefined) {
          return [...prefixEvents, diagnostic(
            "warning",
            "codex.thread_id_missing",
            "thread.started did not include a runtime thread ID",
            evidence,
          )];
        }
        if (runtimeThreadId !== undefined) {
          return [...prefixEvents, diagnostic(
            "warning",
            runtimeThreadId === threadId
              ? "codex.thread_started_duplicate"
              : "codex.thread_id_changed",
            runtimeThreadId === threadId
              ? "Codex emitted thread.started more than once"
              : "Codex changed the runtime thread ID during one invocation",
            evidence,
          )];
        }
        runtimeThreadId = threadId;
        return [...prefixEvents, emit("run.started", {
          runtimeThreadId: threadId,
          evidence,
        })];
      }
      case "turn.started":
        return [...prefixEvents, emit("step.started", {
          stepKind: "turn",
          evidence,
        })];
      case "turn.completed": {
        sawTurnCompleted = true;
        if (parsed.usage !== undefined) {
          completedUsage = sanitizeJson(parsed.usage, { redacted: false });
        }
        return [...prefixEvents, emit("step.completed", {
          stepKind: "turn",
          status: "completed",
          evidence,
        })];
      }
      case "turn.failed": {
        sawTurnFailed = true;
        const message = messageFromError(parsed.error) ?? stringValue(parsed.message)
          ?? "Codex reported that the turn failed";
        lastTerminalMessage = redactText(numberValue(parsed.status ?? parsed.statusCode) !== undefined ? record.raw : message);
        const limit = maybeEmitLimit(message, evidence, lastTerminalMessage);
        return [
          ...prefixEvents,
          ...(limit === undefined ? [] : [limit]),
          emit("step.failed", {
            stepKind: "turn",
            status: "failed",
            message: redactText(providerErrorSentence(lastTerminalMessage, "codex")),
            evidence,
          }),
        ];
      }
      case "item.started":
      case "item.updated":
      case "item.completed":
        return [...prefixEvents, ...acceptItem(eventType, record, parsed, evidence)];
      case "error": {
        const message = stringValue(parsed.message) ?? messageFromError(parsed.error)
          ?? "Codex reported a runtime error";
        lastTerminalMessage = redactText(numberValue(parsed.status ?? parsed.statusCode) !== undefined || isObject(parsed.error) ? record.raw : message);
        const limit = maybeEmitLimit(message, evidence, lastTerminalMessage);
        return [
          ...prefixEvents,
          ...(limit === undefined ? [] : [limit]),
          diagnostic("error", "codex.runtime_error", providerErrorSentence(lastTerminalMessage, "codex"), evidence),
        ];
      }
      default:
        return [...prefixEvents, diagnostic(
          "info",
          "codex.unknown_event",
          `Preserved unknown Codex event type ${redactText(eventType)}`,
          evidence,
        )];
    }
  };

  const finish = (
    completion: RuntimeProcessCompletion,
  ): readonly NormalizedRuntimeEvent[] => {
    if (finalized) return [];
    finalized = true;
    const process = processEvidence(completion);

    if (completion.cancelled) {
      return [emit("run.cancelled", {
        ...(runtimeThreadId === undefined ? {} : { runtimeThreadId }),
        process,
      })];
    }

    const cleanExit = completion.exitCode === 0
      && completion.signal === null
      && !completion.inputDeliveryFailed
      && !completion.outputLimitExceeded
      && !completion.terminationUnconfirmed;
    if (cleanExit && sawTurnCompleted && !sawTurnFailed) {
      return [emit("run.completed", {
        ...(runtimeThreadId === undefined ? {} : { runtimeThreadId }),
        ...(completedUsage === undefined ? {} : { usage: completedUsage }),
        process,
      })];
    }

    const runtimeTerminal: RunFailedPayload["runtimeTerminal"] = sawTurnFailed
      ? "failed"
      : sawTurnCompleted
        ? "completed"
        : "missing";
    let kind = failureKind(lastTerminalMessage);
    let message = lastTerminalMessage ?? "Codex invocation did not complete successfully";
    if (completion.outputLimitExceeded) {
      // Checked FIRST: this is the one branch where the host, not the
      // runtime, ended the run, so nothing the runtime said afterwards
      // describes it better. Naming the cause is the difference between a
      // person retrying the same prompt forever and narrowing it once.
      kind = "process-failed";
      message =
        "Codex sent output faster than Locust could record it, so the run was stopped rather than leave a gap in its record. Sending it again usually works.";
    } else if (sawTurnCompleted && !cleanExit) {
      kind = "protocol-mismatch";
      message = "Codex reported turn completion but the host process did not exit cleanly";
    } else if (!sawTurnCompleted && cleanExit && !sawTurnFailed) {
      kind = "protocol-mismatch";
      message = "Codex exited successfully without a turn.completed event";
    } else if (kind === "unknown" && !cleanExit) {
      kind = "process-failed";
    }

    return [emit("run.failed", {
      kind,
      message: providerErrorSentence(message, "codex"),
      ...(runtimeThreadId === undefined ? {} : { runtimeThreadId }),
      runtimeTerminal,
      process,
    })];
  };

  return {
    get runtimeThreadId() {
      return runtimeThreadId;
    },
    get finalized() {
      return finalized;
    },
    accept,
    finish,
  };
}
