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
  ToolPatch,
  NormalizedRuntimeEvent,
  NormalizedRuntimeEventType,
  NormalizedRuntimePayloadMap,
} from "./codex-events.js";
import type {
  RuntimeJsonlRecord,
  RuntimeProcessCompletion,
} from "./process-runner.js";
import { providerErrorSentence } from "./provider-error.js";
import { unifiedDiffFromHunks, unifiedDiffOf, type ReportedHunk } from "./line-diff.js";

/**
 * Claude Code JSONL -> product events.
 *
 * Mirrors the Codex normalizer and reuses its sanitizers rather than copying
 * them. Every shape here was measured against claude-code 2.1.252 under the
 * exact argv `createClaudePrintCommand` produces -- see
 * `docs/HANDOFF-claude-adapter.md` for the captured stream.
 */

export interface ClaudeInvocationContext {
  /** Product-owned run identifier. Never the provider's session id. */
  readonly runId: string;
  readonly missionId?: string;
  readonly requestedRouteId?: string;
  readonly resolvedRouteId?: string;
  readonly cliVersion?: string;
  /** Test seam and host clock. */
  readonly now?: () => Date;
  /**
   * The CLI's own commands, as it lists them at the start of a run (0.426):
   * handed to the host, never written into the mission's record.
   */
  readonly onCommands?: (commands: readonly RuntimeCommandInfo[]) => void;
}

/** One of a runtime's own slash commands, as the CLI names and describes it. */
export interface RuntimeCommandInfo {
  readonly name: string;
  readonly description: string;
  readonly argumentHint: string;
}

/** The list as a CLI sent it, bounded: at most 400, names a command could have, text cut short. */
export function runtimeCommandsFrom(value: unknown): readonly RuntimeCommandInfo[] {
  if (!Array.isArray(value)) return [];
  return value.slice(0, 400).flatMap((entry): RuntimeCommandInfo[] => {
    if (!isObject(entry) || typeof entry.name !== "string") return [];
    const name = entry.name.trim().replace(/^\//, "");
    if (!/^[A-Za-z0-9][A-Za-z0-9:._-]{0,63}$/.test(name)) return [];
    const text = (field: unknown, max: number): string => (typeof field === "string" ? field.replace(/\s+/g, " ").trim().slice(0, max) : "");
    return [{ name, description: text(entry.description, 240), argumentHint: text(entry.argumentHint, 80) }];
  });
}

export interface ClaudeEventNormalizer {
  readonly runtimeThreadId: string | undefined;
  readonly finalized: boolean;
  accept(record: RuntimeJsonlRecord): readonly NormalizedRuntimeEvent[];
  finish(completion: RuntimeProcessCompletion): readonly NormalizedRuntimeEvent[];
}

type JsonObject = Record<string, unknown>;

/**
 * `system`/`init` is an environment dump: on the machine this was measured on
 * it listed every connected MCP server by name with auth status, every slash
 * command, every skill and agent, a local named-pipe path and the PowerShell
 * path. None of those keys look sensitive to a pattern matcher, and the ledger
 * is durable on disk -- so this record is allow-listed rather than redacted,
 * and counts replace the name lists.
 */
const INIT_ALLOWED_KEYS = new Set([
  "session_id",
  "model",
  "permissionMode",
  "cwd",
  "claude_code_version",
  "output_style",
  "apiKeySource",
]);

export function summarizeInit(value: unknown): JsonObject {
  if (!isObject(value)) return {};
  const summary: JsonObject = {};
  for (const key of INIT_ALLOWED_KEYS) {
    const held = value[key];
    if (typeof held === "string" || typeof held === "number" || typeof held === "boolean") {
      summary[key] = typeof held === "string" ? identityValue(held) ?? "" : held;
    }
  }
  // Counts, never the names.
  for (const [key, label] of [
    ["tools", "toolCount"],
    ["mcp_servers", "mcpServerCount"],
    ["slash_commands", "slashCommandCount"],
    ["agents", "agentCount"],
    ["skills", "skillCount"],
  ] as const) {
    const held = value[key];
    if (Array.isArray(held)) summary[label] = held.length;
  }
  return summary;
}

/**
 * A rate-limit record. `allowed_warning` is a WARNING, not exhaustion --
 * treating it as exhaustion would fall back to a worse model on every long
 * session while the good one still works.
 */
/**
 * A rate-limit snapshot only means something when it says something. Claude
 * sends one of these on ordinary turns to report usage, and `allowed` means
 * the request went through -- surfacing that as a limit puts a red card over
 * a run that was never in trouble. `allowed_warning` is the approaching-limit
 * warning, and anything else (a rejection) is the real thing.
 */
/**
 * What the model SAID it was doing, when it said.
 *
 * Claude Code's Bash tool takes a `description` alongside the command --
 * "Clear, concise description of what this command does in active voice" --
 * and the model fills it in on every call. That sentence is why Claude
 * Code's own transcript reads "Checked what the app says about the free
 * route" where a lesser one would read a shell pipeline.
 *
 * Locust threw it away. `claudeToolTarget` answered the COMMAND for Bash and
 * nothing else looked at the input again, so every command row showed the
 * pipeline. The `Task` case two lines below it already prefers the
 * description and says in its own comment that "the description is the row's
 * text" -- the pattern was known and simply not applied to the tool that
 * runs most often.
 *
 * It is kept SEPARATE from the target rather than replacing it. The command
 * is evidence of what ran on this machine; the description is a claim about
 * it, written by the thing that ran it. A row may lead with the claim, but
 * the evidence cannot stop being recorded.
 */
export function claudeToolTitle(name: string, input: unknown): string | undefined {
  if (name !== "Bash") return undefined;
  if (!isObject(input)) return undefined;
  return stringValue(input.description);
}

/**
 * Whether Claude Code was asked to run this call in the background.
 *
 * Its Bash tool takes `run_in_background`, and the flag rides on the same
 * `tool_use` input block this adapter already opens for the command and the
 * description. It was read for neither until now, so the one fact any
 * runtime gives us about backgrounded work was arriving and being dropped.
 *
 * Bash only, because that is the tool that takes the flag. Reading it off
 * every tool would invent the field on tools that do not have it.
 */
export function claudeToolBackgrounded(name: string, input: unknown): boolean {
  if (name !== "Bash") return false;
  if (!isObject(input)) return false;
  return input.run_in_background === true;
}

/**
 * How a piece of background work ended, in the words the row uses.
 *
 * MEASURED 2026-09-22, Claude Code 2.1.x in print mode on Haiku, with
 * Locust's own flags: asked to background `sleep 8 && echo finished >
 * out.txt` and reply, it started the job (`task_started`,
 * `is_backgrounded: true`), replied, printed its `result` -- and THEN sent
 * `task_updated {status: "killed"}` and `task_notification {status:
 * "stopped"}` for it. out.txt was never written. A printed run stops its
 * background work when it ends, so "the teammate never came back when it
 * finished" (Colin, 2026-09-21) was the work never finishing at all.
 *
 * `stopped-with-run` is that case, told apart by when it arrived: after the
 * run's own result, nothing but the run ending could have stopped it. A stop
 * before the result was somebody's decision mid-run, and says only `stopped`.
 */
export function backgroundEnding(
  status: string | undefined,
  afterResult: boolean,
): "completed" | "failed" | "stopped" | "stopped-with-run" | "ended" {
  if (status === "completed") return "completed";
  if (status === "failed") return "failed";
  if (status === "stopped" || status === "killed") return afterResult ? "stopped-with-run" : "stopped";
  return "ended";
}

/**
 * WHAT A STEP PRINTED, where Claude Code shows it (0.489).
 *
 * Measured 2026-09-30 (docs/DISPLAY-COVERAGE-2026-09-30.md, gap 5): a
 * tool_result's content was never carried, so a command's row in Locust had
 * nothing to open onto while Claude Code shows what it printed under every
 * command. Carried for the tools whose output Claude Code shows -- commands,
 * searches, web reads, connector calls -- and not for Read, Write or Edit,
 * which it shows as a line count or a diff, never the file's text.
 */
/**
 * WHAT A FILE CALL CHANGED (0.672).
 *
 * Every Claude Code Edit and Write read "Claude Code did not report the
 * change" -- 77 such rows in Colin's own conversations (real-thread sweep,
 * 2026-10-06) -- while Claude Code had reported it all along: the user record
 * that answers the call carries `tool_use_result`. MEASURED on Haiku, Claude
 * Code 2.x, the same day:
 *   - an Edit: `{ filePath, oldString, newString, originalFile,
 *     structuredPatch: [{ oldStart, oldLines, newStart, newLines, lines }] }`;
 *   - a Write of a new file: `{ type: "create", filePath, content,
 *     structuredPatch: [], originalFile: null }`.
 * The hunks are written out as they came; a file with no hunks but its whole
 * text (a new one, or one rewritten) is compared against what it was.
 */
export function claudeFilePatch(result: unknown): ToolPatch | undefined {
  if (!isObject(result)) return undefined;
  const path = stringValue(result.filePath);
  if (path === undefined) return undefined;
  const hunks = Array.isArray(result.structuredPatch)
    ? result.structuredPatch.filter((hunk): hunk is ReportedHunk =>
        isObject(hunk)
        && [hunk.oldStart, hunk.oldLines, hunk.newStart, hunk.newLines].every((value) => typeof value === "number" && Number.isFinite(value))
        && Array.isArray(hunk.lines) && hunk.lines.every((line) => typeof line === "string"))
    : [];
  const unified = hunks.length > 0
    ? unifiedDiffFromHunks(path, hunks)
    : typeof result.content === "string" && (result.originalFile === null || typeof result.originalFile === "string")
      ? unifiedDiffOf(path, result.originalFile, result.content)
      : undefined;
  return unified === undefined ? undefined : toolPatchFrom(unified);
}

export function claudeShowsOutput(name: string | undefined): boolean {
  if (name === undefined) return false;
  if (name.startsWith("mcp__")) return true;
  return ["Bash", "BashOutput", "PowerShell", "Grep", "Glob", "WebFetch", "WebSearch", "LS"].includes(name);
}

/** A tool_result's text: its string content, or its text blocks joined. */
export function claudeToolResultText(block: Record<string, unknown>): string | undefined {
  const content = block.content;
  if (typeof content === "string") return content.length === 0 ? undefined : content;
  if (!Array.isArray(content)) return undefined;
  const text = content
    .map((part) => (isObject(part) && stringValue(part.type) === "text" ? stringValue(part.text) : undefined))
    .filter((value): value is string => value !== undefined)
    .join("\n");
  return text.length === 0 ? undefined : text;
}

/**
 * What a Claude tool acted on, for the activity row to name.
 *
 * MEASURED 2026-09-03 by reading the card after a real run: every row said
 * `Read done`, `Glob done`, `Write failed` -- the tool and nothing else, so
 * the card could not tell anyone WHICH file was read or written. Codex rows
 * name their file or command; these had no target at all because a tool's
 * input arrives after `content_block_start`, streamed as JSON deltas, and
 * nothing picked it up from the completed block.
 */
export function claudeToolTarget(name: string, input: unknown): string | undefined {
  if (!isObject(input)) return undefined;
  const first = (...keys: readonly string[]): string | undefined => {
    for (const key of keys) {
      const value = stringValue(input[key]);
      if (value !== undefined && value.length > 0) return value;
    }
    return undefined;
  };
  switch (name) {
    case "Bash":
      return first("command");
    case "Glob":
    case "Grep":
      return first("pattern", "path");
    case "Task":
    case "Agent":
      // Claude Code 2.x names its subagent launcher Agent; older builds said
      // Task. The description is the row's text; without one, the first
      // words of the prompt (seen driving the app, 2026-09-05: a live helper
      // read "a helper, unnamed").
      return first("description", "prompt");
    case "WebFetch":
    case "WebSearch":
      return first("url", "query");
    case "Skill":
      // `{"skill":"project:review"}` (0.679, measured 2026-10-04).
      return first("skill");
    default:
      // Read, Edit, Write, NotebookEdit and anything else that names a file.
      return first("file_path", "path", "notebook_path", "command", "pattern");
  }
}

export function limitKindFor(status: unknown): "quota-exhausted" | "temporary-rate-limit" | undefined {
  const text = typeof status === "string" ? status.toLowerCase() : "";
  if (text === "allowed") return undefined;
  if (text.includes("allowed")) return "temporary-rate-limit";
  if (text.length === 0) return undefined;
  return "quota-exhausted";
}

/**
 * "5-hour window 35% used · resets 7:30 PM", from Claude Code's
 * `unifiedWindows` ({five_hour: {utilization, resetsAt}, seven_day: ...}).
 * The fuller window leads; both are named when both are known.
 */
/**
 * One window from a record that names only its own (`rateLimitType`,
 * `utilization`, `resetsAt`) -- how a warning arrives: "7-day window 79%
 * used · resets <iso>" (0.407).
 */
export function ownWindowText(info: JsonObject): string | undefined {
  const key = stringValue(info.rateLimitType);
  const utilization = info.utilization;
  if (key === undefined || typeof utilization !== "number" || !Number.isFinite(utilization)) return undefined;
  const resets = resetsAtIso(info.resetsAt);
  return `${windowName(key)} window ${String(Math.round(utilization * 100))}% used${resets === undefined ? "" : ` · resets ${resets}`}`;
}

export function usageWindowText(windows: unknown): string | undefined {
  if (!isObject(windows)) return undefined;
  const parts: { label: string; used: number; resets: string | undefined }[] = [];
  for (const [key, value] of Object.entries(windows)) {
    if (!isObject(value) || typeof value.utilization !== "number" || !Number.isFinite(value.utilization)) continue;
    const label = key === "five_hour" ? "5-hour window" : key === "seven_day" ? "7-day window" : `${key} window`;
    parts.push({ label, used: Math.round(value.utilization * 100), resets: resetsAtIso(value.resetsAt) });
  }
  if (parts.length === 0) return undefined;
  parts.sort((a, b) => b.used - a.used);
  return parts
    .map((part) => `${part.label} ${String(part.used)}% used${part.resets === undefined ? "" : ` · resets ${part.resets}`}`)
    .join(" · ");
}

/**
 * What a model call held: its whole prompt, cached or not.
 *
 * MEASURED 2026-09-23 off Colin's ledger: every call's `message_start` states
 * `input_tokens`, `cache_creation_input_tokens` and `cache_read_input_tokens`,
 * and in a long run the first two are small and the third is nearly all of it
 * (2 + 3,689 + 236,049). Undefined when the record states none of them.
 */
export function promptTokensOf(usage: unknown): number | undefined {
  if (!isObject(usage)) return undefined;
  let total: number | undefined;
  for (const key of ["input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"]) {
    const held = usage[key];
    if (typeof held === "number" && Number.isFinite(held) && held >= 0) total = (total ?? 0) + held;
  }
  return total;
}

function outputTokensOf(usage: unknown): number | undefined {
  if (!isObject(usage)) return undefined;
  const held = usage.output_tokens;
  return typeof held === "number" && Number.isFinite(held) && held >= 0 ? held : undefined;
}

/**
 * A record from a subagent's own conversation. It has a context of its own,
 * so its calls say nothing about how full the person's conversation is.
 */
function fromSubagent(parsed: JsonObject): boolean {
  return typeof parsed.parent_tool_use_id === "string" && parsed.parent_tool_use_id.length > 0;
}

/**
 * The helper a record belongs to: the id of the Agent call that sent it out,
 * which is also the item id of the helper's own row (helper visibility,
 * 2026-10-05). Undefined for the teammate's own records.
 */
function helperCallOf(parsed: JsonObject): string | undefined {
  return fromSubagent(parsed) ? identityValue(parsed.parent_tool_use_id) : undefined;
}

/** "5-hour", "7-day", "7-day opus": a window key in words. */
function windowName(key: string | undefined): string {
  if (key === undefined || key.length === 0) return "usage";
  return key.replace("five_hour", "5-hour").replace("seven_day", "7-day").replace(/_/g, " ");
}

/**
 * A limit record in words: "You've used 53% of your 7-day window · resets
 * 2026-09-28T07:00:00.000Z" (the app turns the instant into a local time).
 *
 * It said "seven_day limit allowed_warning · resets 2026-09-28T07:00:00.000Z",
 * the record's own keys, in amber, in the thread (drive, 2026-09-23). The
 * warning comes on every turn while it holds, and it is not a fixed
 * threshold -- that drive got one at 53% of the week -- so it states how much
 * is used, the way Claude Code words its own ("You've used 90% of your weekly
 * limit"), rather than claiming the limit is near.
 */
export function limitSentence(info: JsonObject): string {
  const key = stringValue(info.rateLimitType);
  const windows = isObject(info.unifiedWindows) ? info.unifiedWindows : {};
  const ownWindow = key === undefined ? undefined : windows[key];
  const utilization = typeof info.utilization === "number"
    ? info.utilization
    : isObject(ownWindow) ? ownWindow.utilization : undefined;
  const percent = typeof utilization === "number" && Number.isFinite(utilization)
    ? Math.round(utilization * 100)
    : undefined;
  const resets = resetsAtIso(info.resetsAt);
  const when = resets === undefined ? "" : ` · resets ${resets}`;
  const window = `${windowName(key)} window`;
  if (limitKindFor(info.status) === "temporary-rate-limit") {
    return percent === undefined
      ? `Your ${window} is running low${when}`
      : `You've used ${String(percent)}% of your ${window}${when}`;
  }
  // A rejection is headed "Usage limit reached" where it is shown.
  return `${window}${percent === undefined ? "" : ` ${String(percent)}% used`}${when}`;
}

/** `resetsAt` is epoch SECONDS, not milliseconds. */
export function resetsAtIso(value: unknown): string | undefined {
  if (typeof value !== "number" || !Number.isFinite(value)) return undefined;
  const milliseconds = value * 1000;
  if (!Number.isFinite(milliseconds)) return undefined;
  const date = new Date(milliseconds);
  return Number.isNaN(date.getTime()) ? undefined : date.toISOString();
}

/**
 * A tool named the way the activity rows name it.
 *
 * `mcp__claude_ai_Robinhood__get_accounts` is a machine name and reads as
 * one. The rows already split it into the tool and the connector it belongs
 * to, and a sentence about the same call has no business doing worse -- Colin
 * sent a capture of exactly that on 2026-09-09.
 *
 * Kept here rather than imported from the renderer, because this package has
 * no renderer. The split is one rule and it is checked against real tool
 * names in `connectors.test.ts`.
 */
/**
 * What was refused, short enough to read.
 *
 * The refusal names the thing it would not do, and for Bash that thing is a
 * whole command line. Colin sent a capture on 2026-09-10 of one that ran to
 * eight wrapped lines inside the amber register -- a `cd` into a long Windows
 * path, `&&`, and an entire `python3 -c` program with embedded JSON parsing.
 * The register is for a sentence a person can act on; a program printed into
 * it buries the sentence it came with.
 *
 * The first line, bounded. A command's first line is the part that says what
 * it was trying to do, and the whole of it is in the activity row and the
 * receipt, where a command belongs.
 */
export function brieflyPut(detail: string): string {
  const firstLine = detail.split(/\r?\n/)[0]?.trim() ?? '';
  const said = firstLine.length === 0 ? detail.trim() : firstLine;
  if (said.length <= REFUSAL_DETAIL_LIMIT) return said;
  // Cut on a space where there is one near the end, so the tail is not half
  // a word -- a path or a flag broken mid-token reads as corruption.
  const cut = said.slice(0, REFUSAL_DETAIL_LIMIT);
  const space = cut.lastIndexOf(" ");
  return `${(space > REFUSAL_DETAIL_LIMIT - 24 ? cut.slice(0, space) : cut).trimEnd()}...`;
}

/** Long enough for an ordinary command, short enough to stay one line. */
export const REFUSAL_DETAIL_LIMIT = 96;

/** Said when Claude Code compacts on its own, as its context fills (A2.5). */
export const CLAUDE_COMPACTED =
  "The conversation outgrew the model's context, so Claude Code summarized it and carried on from the summary.";
/** Said when Claude Code is not signed in: its own "Please run /login" names a command Locust has no place for. */
export const CLAUDE_SIGNED_OUT =
  "Claude Code is not signed in. Open a terminal, run claude, and sign in: with your Claude account, or with an Anthropic Console account to pay with an API key. Then send again. Locust never hands an API key from your environment to an agent; to use a Claude API key directly, add it in Settings, Your own models.";
/** And when it was asked to, by a `/compact` sent as the prompt. */
export const CLAUDE_COMPACTED_ON_REQUEST =
  "Claude Code summarized the conversation so far, as asked, and carries on from the summary.";

export function namedTool(name: string): string {
  const match = /^mcp__([A-Za-z0-9_]+?)__(.+)$/.exec(name);
  if (match === null) return name;
  const server = match[1]!.replace(/^claude_ai_/, "").replace(/_/g, " ");
  return `${match[2]!} on ${server}`;
}

/**
 * Why a run was refused, which is not one reason.
 *
 * This sentence was written for Bash and said "asks for approval before
 * running commands". Once Locust stopped denying `mcp__*` outright
 * (2026-09-09) the same line began firing for connector calls, where
 * "commands" is simply the wrong word.
 *
 * A connector refusal is also the one a person can act on, so it says how.
 *
 * It used to say "a printed run has no way to put that question to you",
 * which was true until 0.63.0 and false after it: Locust is the permission
 * host now, the question IS put to the person, and the first drive of a
 * narrowed teammate saw this sentence claim otherwise right under the card
 * they had just pressed Deny on. The CLI's record cannot say which of the two
 * it was, so the sentence names both.
 */
export function whyRefused(refused: readonly { readonly tool: string }[]): string {
  if (refused.every((entry) => entry.tool.startsWith("mcp__"))) {
    return "Claude Code asks before using a connector, and this one was not allowed: either you said no when it asked, or it is not one of this teammate's connectors. Give this teammate the connector in its card, or run it in Auto.";
  }
  /*
   * A READ that was refused was reaching outside this folder (0.443).
   * Reading inside it needs no approval in any mode; outside it Claude Code
   * asks. The sentence used to say "this route allows edits" whatever the
   * mode, and a read-only comparison showed it under a refused Grep (Colin's
   * first real comparison, 2026-09-28).
   */
  if (refused.every((entry) => entry.tool === "Read" || entry.tool === "Glob" || entry.tool === "Grep")) {
    return "Claude Code asks before reading outside this folder, and a printed run has no way to answer. Auto lets it read anywhere.";
  }
  return "Claude Code asks for approval before running commands in this mode, and a printed run has no way to give it.";
}

/** How Locust's command guard begins every refusal (apps/desktop/resources/locust-command-guard.mjs, 0.717). */
export const LOCUST_GUARD_SAID = "Locust stopped this command before it ran";

/**
 * What Locust's command guard stopped, said to the person (0.717). The
 * guard's reason is written to the model -- "...the person's own windows too.
 * End only what you started yourself..." -- so the person is told the same
 * fact as theirs: what it would have ended. Not the mode: on Auto the line it
 * replaces said Claude Code "asks for approval before running commands in
 * this mode", which was false (first drive, 2026-10-10).
 */
export function guardedSentence(stopped: readonly { readonly text: string; readonly guarded?: string }[]): string {
  const still = "A teammate may end only what it started itself.";
  if (stopped.length === 1) {
    const reason = stopped[0]!.guarded ?? "";
    // By id (0.718): "process 10544 is Chrome, which no teammate started, so it is the person's own."
    const byId = /process (\d+) is (.+?), which (no teammate started|this run did not start), so it is (.+?)\./.exec(reason);
    if (byId !== null) {
      const whose = byId[4]!.replace(/^the person's own$/, "yours").replace(/^the person's or another teammate's$/, "yours or another teammate's").replace(/^another teammate's or the person's$/, "another teammate's or yours");
      const started = byId[3] === "no teammate started" ? "which no teammate started" : "which this teammate did not start";
      return `Locust stopped ${stopped[0]!.text} before it ran: process ${byId[1]!} is ${byId[2]!}, ${started}, so it is ${whose}. ${still}`;
    }
    // Locust itself, or the agent the teammate runs on.
    const own = /process (\d+) is (the agent this run is|Locust itself)/.exec(reason);
    if (own !== null) {
      return `Locust stopped ${stopped[0]!.text} before it ran: process ${own[1]!} is ${own[2] === "Locust itself" ? "Locust itself" : "the agent this teammate runs on"}. ${still}`;
    }
    const match = /which ends every (.+?) on this computer, (.+?) too\./.exec(reason);
    const ended = match === null ? "a program you, or another teammate, also run" : `every ${match[1]!} on this computer, ${match[2]!.replace(/^the person's own/, "your own")} too`;
    return `Locust stopped ${stopped[0]!.text} before it ran: it would have ended ${ended}. ${still}`;
  }
  return `Locust stopped ${String(stopped.length)} commands before they ran: ${stopped.map((entry) => entry.text).join("; ")}. Each would have ended a program that you, or another teammate, also run. ${still}`;
}

export function createClaudeEventNormalizer(
  context: ClaudeInvocationContext,
): ClaudeEventNormalizer {
  const runId = requireContextText(context.runId, "runId");
  const cliVersion = context.cliVersion === undefined
    ? undefined
    : requireContextText(context.cliVersion, "cliVersion");
  const now = context.now ?? (() => new Date());

  /** Text buffers per content block index, so a replace can be recognised. */
  const openTools = new Map<string, { name: string; target?: string; title?: string; background?: boolean; parent?: string }>();
  /** A subagent's type and one-line summary, by the Agent tool call that started it. */
  const subagentKinds = new Map<string, string>();
  const subagentSummaries = new Map<string, string>();
  /**
   * Work Claude Code is running in the background, by its task id, with the
   * tool call that started it. The call returns at once; this is how its
   * row later learns what became of the work.
   */
  const backgroundTasks = new Map<string, string | undefined>();
  /**
   * Calls Claude Code refused before they ran, by tool call, with its reason.
   *
   * Claude Code 2.1.280 says so the moment it refuses -- a `system` record,
   * `permission_denied`, naming the call and why ("Contains simple_expansion")
   * -- and then returns the call as an error. Read as an error, a refused
   * command counted as one that ran and "exited non-zero": a replay of a
   * Haiku run (2026-09-23) read "ran 7 commands · 2 exited non-zero · 1
   * refused" over five that ran and two that never did.
   */
  const refusedCalls = new Map<string, string>();
  /**
   * Calls Locust's command guard stopped (0.717), by call and by command,
   * with its reason. A hook's refusal comes back only as the call's error
   * text -- MEASURED on 2.1.296, no `permission_denied` record -- so without
   * this it read as a command that ran and "exited non-zero", under a notice
   * blaming the mode.
   */
  const guardedCalls = new Map<string, string>();
  const guardedCommands = new Map<string, string>();
  let runtimeThreadId: string | undefined;
  let normalizedSequence = 0;
  let finalized = false;
  let sawResult = false;
  let terminalFailure: string | undefined;
  let terminalFailureKind: "authentication-failed" | undefined;
  let terminalFailureEvidence: CodexEventEvidence | undefined;
  let syntheticError: string | undefined;
  // What the run cost, as Claude Code itself priced it. Measured 2026-09-03:
  // the `result` record carries `total_cost_usd` and a `usage` block with
  // `input_tokens` / `output_tokens`. Only the numbers travel; the record's
  // model-usage table names the account's models and stays behind.
  let completedUsage: Record<string, number> | undefined;
  let resolvedModel: string | undefined;
  /**
   * The conversation's last model call: what its prompt held and what it
   * wrote, which is what the conversation holds now.
   *
   * NOT the result's `usage`. MEASURED 2026-09-23: that block adds up every
   * call in the run, and each call re-reads the whole conversation from
   * cache, so a 23-call run whose prompt never passed 240k reported
   * 4,978,743 -- the context ring Colin saw read "5M of 1M". Claude Code
   * reads its own context the same way this does: the last response's
   * input, cache and output, added.
   */
  let lastCall: { id: string | undefined; prompt: number; output: number } | undefined;
  /**
   * Whether this run was on a subscription's usage windows, and whether any of
   * it ran on paid extra usage past them. MEASURED 2026-09-23 on Colin's
   * three Claude runs: every one sent a `rate_limit_event` with
   * `unifiedWindows` (even a two-call run), `isUsingOverage: false`, and
   * `apiKeySource: "none"` in its init. The windows are the evidence used: a
   * key source of "none" is also what a cloud-billed setup reports, and that
   * one is paying.
   */
  let onUsageWindows = false;
  let usedExtraUsage = false;
  /**
   * The block the assistant's text is actually streaming into.
   *
   * MEASURED 2026-09-03: Claude Code does not always put the text at index 0.
   * When it sends another block first, the deltas arrive as `block_1` while
   * the completing record below replaced `block_0` -- two different items, so
   * the finished answer rendered TWICE, in full, one after the other. The
   * replace has to land on the block the text was written to.
   */
  let textBlockId: string | undefined;
  /**
   * Which of the conversation's messages is streaming, and its id.
   *
   * The text item was `block_<index>`, and the index restarts in every
   * message, so a run's second message streamed into its first message's
   * item and replaced it: a replay of a Bash-heavy Haiku run (2026-09-23) put
   * three things Claude said to the person on screen one after another in
   * the same place, and the finished thread showed only the last. It is
   * `m<ordinal>_block_<index>` now: one item per message, in order.
   */
  let messageOrdinal = 0;
  /**
   * THINKING, TIMED AND NOT KEPT (0.489). Claude Code shows "Thought for Ns"
   * over every thinking block; Locust showed nothing. The block's start and
   * stop are a step of their own, with no text: AGENTS.md forbids keeping
   * hidden chain-of-thought, and the length of it is not the thought.
   * Keyed by the block's index within its message.
   */
  const openThinking = new Map<number, string>();
  let streamingMessageId: string | undefined;

  const emit = <TType extends NormalizedRuntimeEventType>(
    type: TType,
    payload: NormalizedRuntimePayloadMap[TType],
  ): NormalizedRuntimeEvent => {
    normalizedSequence += 1;
    return {
      id: `${runId}:claude:${normalizedSequence}`,
      runId,
      ...(context.missionId === undefined ? {} : { missionId: context.missionId }),
      sequence: normalizedSequence,
      occurredAt: now().toISOString(),
      sourceAdapter: "claude",
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
    const evidence = evidenceFor(record, parsed, type);

    // A running tool's heartbeat: which tool, and how long it has been going
    // (Claude Code 2.1.280 sends one while a long tool runs, and for a
    // subagent's API retries). The tool's own result arrives separately and
    // the live line already times the step, so there is nothing in it a
    // person reads -- it was reaching the thread as "Unhandled Claude record:
    // tool_progress" (Colin, 2026-09-22, with a frame of it).
    if (type === "tool_progress") return [];

    if (type === "rate_limit_event") {
      const info = isObject(parsed.rate_limit_info) ? parsed.rate_limit_info : {};
      if (isObject(info.unifiedWindows) && Object.keys(info.unifiedWindows).length > 0) onUsageWindows = true;
      if (info.isUsingOverage === true) usedExtraUsage = true;
      const kind = limitKindFor(info.status);
      /*
       * The reading, whatever the status (0.407). Allowed, it is what Claude
       * Code's own status line shows ("35% of the 5-hour window"; MEASURED
       * 2026-09-05). But a WARNING carries the same figures, and was kept
       * only as a limit notice: Colin's runs on the morning of 2026-09-27 sent
       * nine, at 78-79% of the 7-day window, and the card stayed on the 66%
       * of the night before ("thats not true, i used claude today"). A
       * warning may name only its own window; that one window is a reading.
       */
      const window = usageWindowText(info.unifiedWindows) ?? ownWindowText(info);
      /*
       * EMITTED IN THE ORDER RETURNED (0.413). `emit` takes the next
       * sequence, and the ledger refuses a batch whose sequences do not
       * count up. 0.407 emitted the reading first and returned it second, so
       * every warning handed the ledger N+1 then N -- "Mission event sequence
       * is invalid" -- and the run was stopped at its first warning: every
       * Claude run above a usage warning line, from 0.407 to 0.412 (Colin's
       * Drop, 2026-09-27, 6 seconds in).
       */
      const limit = kind === undefined
        ? []
        : [emit("route.limit_detected", { kind, message: boundedMessageText(limitSentence(info)), evidence })];
      const reading = window === undefined
        ? []
        : [emit("adapter.diagnostic", { code: "claude.usage_window", level: "info", terminal: false, message: window, evidence })];
      return [...limit, ...reading];
    }

    if (type === "system") {
      const subtype = stringValue(parsed.subtype);
      if (subtype === "init") {
        runtimeThreadId = identityValue(parsed.session_id);
        const summary = summarizeInit(parsed);
        return [
          emit("run.started", {
            runtimeThreadId: runtimeThreadId ?? "",
            // The allow-listed summary REPLACES the raw record: the untouched
            // one describes the user's machine and this ledger is durable.
            evidence: { ...evidence, raw: summary as never, redacted: true },
          }),
        ];
      }
      // MEASURED 2026-09-05 off a live `--output-format stream-json` run of
      // Claude Code 2.1.261: the system channel also carries
      //   notification  -- "Stop hook error occurred · ctrl+o to see",
      //                    priority "immediate": the runtime telling the
      //                    person something about its own setup;
      //   task_started / task_progress / task_updated / task_notification
      //                 -- a subagent's life: its type, what it is doing,
      //                    its last tool, and its one-line summary at the end;
      //   status        -- "requesting" and the like;
      //   thinking_tokens -- an estimate, no words.
      // Colin, 2026-09-05: the different notices Claude Code and Codex give
      // are what a person uses to know what is happening; none is cut.
      if (subtype === "notification") {
        const text = stringValue(parsed.text);
        if (text === undefined) return [];
        const immediate = stringValue(parsed.priority) === "immediate";
        return [
          emit("adapter.diagnostic", {
            code: "claude.notification",
            level: immediate ? "warning" : "info",
            terminal: false,
            message: boundedMessageText(text.replace(/\s*·\s*ctrl\+o to see\s*$/i, "")),
            evidence,
          }),
        ];
      }
      if (subtype === "task_started" || subtype === "task_progress") {
        const taskId = identityValue(parsed.task_id) ?? "task";
        /*
         * BACKGROUND WORK IS NOT A HELPER, and it is not what the teammate
         * is doing now.
         *
         * Claude Code reports a backgrounded command with the same record it
         * uses for a subagent, and this read every one as a subagent: the
         * live line took the command's description as the teammate's current
         * step, and the ending landed on an item called `subagent:<task>`
         * that no row is keyed by. The record says which it is
         * (`is_backgrounded`) and names the call that started it
         * (`tool_use_id`), so the step is keyed by that call and typed
         * `background`, and the row for the call is what hears about it.
         *
         * The call itself is marked too. The runtime's word that it
         * backgrounded the work is better evidence than the input flag, and
         * it arrives before the call's result does.
         */
        const startedBy = identityValue(parsed.tool_use_id);
        if (parsed.is_backgrounded === true || backgroundTasks.has(taskId)) {
          const callId = startedBy ?? backgroundTasks.get(taskId);
          backgroundTasks.set(taskId, callId);
          const call = callId === undefined ? undefined : openTools.get(callId);
          if (callId !== undefined && call !== undefined) openTools.set(callId, { ...call, background: true });
          // A helper sent to the background is still a helper: its type rides on its row (0.492).
          const helperKind = stringValue(parsed.subagent_type);
          if (callId !== undefined && helperKind !== undefined) subagentKinds.set(callId, helperKind);
          const lastTool = stringValue(parsed.last_tool_name);
          const doing = [stringValue(parsed.description), lastTool === undefined ? undefined : `last tool ${lastTool}`]
            .filter((part): part is string => part !== undefined)
            .join(" · ") || undefined;
          return [
            emit("step.started", {
              stepKind: "item",
              itemId: callId ?? `background:${taskId}`,
              itemType: "background",
              status: "running",
              ...(doing === undefined ? {} : { message: boundedMessageText(doing) }),
              evidence,
            }),
          ];
        }
        const kind = stringValue(parsed.subagent_type);
        const doing = stringValue(parsed.description);
        const tool = stringValue(parsed.last_tool_name);
        const words = [kind, doing, tool === undefined ? undefined : `last tool ${tool}`].filter((part): part is string => part !== undefined);
        const toolUseId = identityValue(parsed.tool_use_id);
        if (toolUseId !== undefined && kind !== undefined) subagentKinds.set(toolUseId, kind);
        return [
          emit("step.started", {
            stepKind: "item",
            itemId: `subagent:${taskId}`,
            itemType: "subagent",
            ...(words.length === 0 ? {} : { message: boundedMessageText(words.join(" · ")) }),
            evidence,
          }),
        ];
      }
      if (subtype === "task_notification") {
        const taskId = identityValue(parsed.task_id) ?? "task";
        const toolUseId = identityValue(parsed.tool_use_id);
        if (backgroundTasks.has(taskId)) {
          const callId = toolUseId ?? backgroundTasks.get(taskId);
          backgroundTasks.delete(taskId);
          const ended = backgroundEnding(stringValue(parsed.status), sawResult);
          // What a background helper came back with, in its own words (0.492).
          const said = stringValue(parsed.summary);
          return [
            emit(ended === "failed" ? "step.failed" : "step.completed", {
              stepKind: "item",
              itemId: callId ?? `background:${taskId}`,
              itemType: "background",
              status: ended,
              ...(said === undefined ? {} : { message: boundedMessageText(said) }),
              evidence,
            }),
          ];
        }
        const summary = stringValue(parsed.summary);
        if (toolUseId !== undefined && summary !== undefined) subagentSummaries.set(toolUseId, summary);
        const status = stringValue(parsed.status);
        return [
          emit(status === "failed" ? "step.failed" : "step.completed", {
            stepKind: "item",
            itemId: `subagent:${taskId}`,
            itemType: "subagent",
            ...(status === undefined ? {} : { status }),
            evidence,
          }),
        ];
      }
      if (subtype === "task_updated" || subtype === "thinking_tokens") return [];
      // Kept for the call's own result, which follows (see `refusedCalls`).
      // It was a step with nothing in it.
      if (subtype === "permission_denied") {
        const callId = identityValue(parsed.tool_use_id);
        const why = stringValue(parsed.decision_reason) ?? stringValue(parsed.message);
        if (callId !== undefined) refusedCalls.set(callId, why ?? "");
        return [];
      }
      // The set of background tasks, restated whole each time it changes.
      // Every fact in it arrives per task in `task_started` and
      // `task_notification`, which also say which call each belongs to. Read
      // as a turn opening -- the fall-through below -- it opened a step with
      // nothing in it, and in the 2026-09-22 capture one of those came after
      // the run's answer.
      if (subtype === "background_tasks_changed") return [];
      // The person's slash commands and skills, each with its description --
      // MEASURED 2026-09-24 on Claude Code 2.1.281: sent twice at the start of
      // every run, 44 KB each, and read as a turn opening it became two empty
      // steps whose evidence carried the whole list into every mission's
      // record. It is the CLI's setup, not the run's news, and not something
      // the ledger should hold a copy of per run.
      // 0.426: handed to the host for the `/` menu -- still not the ledger's.
      if (subtype === "commands_changed") {
        const commands = runtimeCommandsFrom(parsed.commands);
        if (commands.length > 0) context.onCommands?.(commands);
        return [];
      }
      // The session compacted (A2.5): measured the same day with `/compact`
      // on a two-turn Haiku session -- `status` "compacting", then this
      // record with `compact_metadata.trigger` ("manual" there, "auto" when
      // the context fills), then the summary as a USER message, which this
      // adapter does not draw. Said once, as a line, the way Claude Code's own
      // transcript says it; the host reads the code to brief the next turn in
      // full.
      if (subtype === "compact_boundary") {
        const metadata = isObject(parsed.compact_metadata) ? parsed.compact_metadata : {};
        return [
          emit("adapter.diagnostic", {
            code: "claude.context_compacted",
            level: "info",
            terminal: false,
            message: stringValue(metadata.trigger) === "manual" ? CLAUDE_COMPACTED_ON_REQUEST : CLAUDE_COMPACTED,
            evidence,
          }),
        ];
      }
      return [
        emit("step.started", {
          stepKind: "turn",
          ...(subtype === undefined ? {} : { status: subtype }),
          ...(stringValue(parsed.status) === undefined ? {} : { message: stringValue(parsed.status)! }),
          evidence,
        }),
      ];
    }

    if (type === "stream_event") {
      const inner = isObject(parsed.event) ? parsed.event : {};
      const innerType = stringValue(inner.type);
      if (innerType === "message_start") {
        const message = isObject(inner.message) ? inner.message : {};
        const prompt = promptTokensOf(message.usage);
        if (prompt !== undefined && !fromSubagent(parsed)) {
          lastCall = { id: identityValue(message.id), prompt, output: outputTokensOf(message.usage) ?? 0 };
        }
        if (!fromSubagent(parsed)) {
          messageOrdinal += 1;
          textBlockId = undefined;
          streamingMessageId = identityValue(message.id);
        }
        return [emit("step.started", { stepKind: "turn", evidence })];
      }
      if (innerType === "message_delta") {
        // The call's final output count arrives here, at its end.
        const output = outputTokensOf(inner.usage);
        if (output !== undefined && lastCall !== undefined && !fromSubagent(parsed)) {
          lastCall = { ...lastCall, output: Math.max(lastCall.output, output) };
        }
        return [];
      }
      if (innerType === "content_block_start") {
        const block = isObject(inner.content_block) ? inner.content_block : {};
        const blockType = stringValue(block.type);
        if ((blockType === "thinking" || blockType === "redacted_thinking") && !fromSubagent(parsed)) {
          const index = typeof inner.index === "number" ? inner.index : 0;
          const itemId = `m${String(messageOrdinal)}_thinking_${String(index)}`;
          openThinking.set(index, itemId);
          return [emit("step.started", { stepKind: "reasoning", itemId, evidence })];
        }
        if (blockType === "tool_use") {
          const itemId = identityValue(block.id) ?? `block_${String(inner.index ?? 0)}`;
          const name = identityValue(block.name) ?? "tool";
          // Measured not to happen (a helper's calls arrive whole, below), but
          // a helper's call that did stream a start is still the helper's.
          const parent = helperCallOf(parsed);
          openTools.set(itemId, { name, ...(parent === undefined ? {} : { parent }) });
          return [
            emit("tool.started", {
              itemId,
              toolKind: "tool_use",
              name,
              ...(parent === undefined ? {} : { parentItemId: parent }),
              phase: "started",
              evidence,
            }),
          ];
        }
        return [];
      }
      if (innerType === "content_block_delta") {
        const delta = isObject(inner.delta) ? inner.delta : {};
        const text = stringValue(delta.text);
        // A subagent's words are its report to the teammate, not the
        // teammate's to the person; they arrive through the Agent call.
        if (text === undefined || fromSubagent(parsed)) return [];
        textBlockId = `m${String(messageOrdinal)}_block_${String(inner.index ?? 0)}`;
        return [
          emit("message.delta", {
            itemId: textBlockId,
            operation: "append",
            text: boundedAnswerText(text),
            final: false,
            evidence,
          }),
        ];
      }
      if (innerType === "content_block_stop") {
        const index = typeof inner.index === "number" ? inner.index : 0;
        const itemId = openThinking.get(index);
        if (itemId === undefined) return [];
        openThinking.delete(index);
        return [emit("step.completed", { stepKind: "reasoning", itemId, evidence })];
      }
      if (innerType === "message_stop") {
        openThinking.clear();
        return [emit("step.completed", { stepKind: "turn", evidence })];
      }
      return [];
    }

    if (type === "assistant") {
      // TRAP: the text has already arrived as deltas. This record is the
      // complete message, so it REPLACES the buffer -- appending it would
      // double every answer, and the checkpoint's assistantSummary rebuilds
      // the buffer exactly this way, so the bug would reach the resume summary.
      const message = isObject(parsed.message) ? parsed.message : {};
      const content = Array.isArray(message.content) ? message.content : [];
      // Claude Code's own stand-in when the call never reached a model: model "<synthetic>" with an `error` code
      // (measured 2026-10-07, 2.1.293, signed out: error "authentication_failed", text "Not logged in · Please run
      // /login"). Its words are not an answer; the result after it carries the failure, and it is said once.
      if (message.model === "<synthetic>" && typeof parsed.error === "string") {
        syntheticError = parsed.error;
        return [];
      }
      // The complete message states its call's usage too: the same call as
      // the last `message_start` when the run streams, the only word on it
      // when it does not.
      if (!fromSubagent(parsed)) {
        const id = identityValue(message.id);
        const output = outputTokensOf(message.usage);
        const prompt = promptTokensOf(message.usage);
        if (lastCall !== undefined && id !== undefined && id === lastCall.id) {
          if (output !== undefined) lastCall = { ...lastCall, output: Math.max(lastCall.output, output) };
        } else if (prompt !== undefined) {
          lastCall = { id, prompt, output: output ?? 0 };
        }
      }
      /*
       * A subagent's message is not the teammate's answer.
       *
       * It came through here as the teammate's own text and REPLACED it: in
       * the same replay the subagent's "The file data/log.txt contains 5
       * lines." stood in the teammate's place, and the teammate's next message
       * then streamed onto the end of it ("...5 lines.I executed"). If the
       * subagent reports after the teammate's last word -- a background one
       * does -- it is the answer the person is left with. What the subagent
       * found reaches the thread through its Agent call's row.
       */
      /*
       * A HELPER'S OWN CALLS ARE ITS ROW'S CHILDREN (helper visibility,
       * 2026-10-05). Colin: "users are probably going to want to inspect when
       * one of the 'helpers/agents' is sent out." Its calls were read here
       * only to be dropped, so the helper's row said what it was asked and
       * what it came back with, and nothing it did in between. They never
       * stream a start (MEASURED, Haiku, 2026-10-02): this complete message
       * is the first and only word on each, so each opens here, named, with
       * the Agent call that sent the helper out as its parent. Its words
       * still stay out (above); only its calls are kept.
       */
      if (fromSubagent(parsed)) {
        const parent = helperCallOf(parsed)!;
        const opened: NormalizedRuntimeEvent[] = [];
        for (const block of content) {
          if (!isObject(block) || stringValue(block.type) !== "tool_use") continue;
          const itemId = identityValue(block.id);
          if (itemId === undefined || openTools.has(itemId)) continue;
          const name = identityValue(block.name) ?? "tool";
          const target = claudeToolTarget(name, block.input);
          const title = claudeToolTitle(name, block.input);
          openTools.set(itemId, { name, parent, ...(target === undefined ? {} : { target }), ...(title === undefined ? {} : { title }) });
          opened.push(
            emit("tool.started", {
              itemId,
              toolKind: "tool_use",
              name,
              parentItemId: parent,
              ...(target === undefined ? {} : { command: boundedMessageText(target) }),
              ...(title === undefined ? {} : { title: boundedMessageText(title) }),
              phase: "started",
              evidence,
            }),
          );
        }
        return opened;
      }
      // The same record carries every tool_use block with its input filled
      // in, which is the first point the target is knowable.
      const restated: NormalizedRuntimeEvent[] = [];
      for (const block of content) {
        if (!isObject(block) || stringValue(block.type) !== "tool_use") continue;
        const itemId = identityValue(block.id);
        if (itemId === undefined) continue;
        const open = openTools.get(itemId);
        if (open === undefined) continue;
        const target = claudeToolTarget(open.name, block.input);
        const title = claudeToolTitle(open.name, block.input);
        const background = claudeToolBackgrounded(open.name, block.input);
        // A helper's type is on its own call (0.492), so its row carries it
        // however the launch and the runtime's notice about it are ordered.
        const helperType = /^(Agent|Task)$/.test(open.name) && isObject(block.input) ? stringValue(block.input.subagent_type) : undefined;
        if (helperType !== undefined && !subagentKinds.has(itemId)) subagentKinds.set(itemId, helperType);
        if (target !== undefined || title !== undefined || background) {
          openTools.set(itemId, {
            ...open,
            ...(target === undefined ? {} : { target }),
            ...(title === undefined ? {} : { title }),
            ...(background ? { background: true } : {})
          });
          /*
           * And the call is announced again, now that it can be named.
           *
           * The start went out when the block opened, with the tool's name
           * and nothing else, and the command and its description only
           * travelled with the call's END -- so a Bash call running for eight
           * minutes read "Using a tool... Bash" the whole time (Colin, with
           * a frame of it, 2026-09-23: "i find it hard to believe ... after 8
           * minutes of working thats the only info the user has been given").
           * The thread updates the open row it already has.
           */
          if (open.target === undefined && open.title === undefined) {
            restated.push(
              emit("tool.started", {
                itemId,
                toolKind: "tool_use",
                name: open.name,
                // Bounded as every other adapter bounds them (0.610): Fable 5.1 wrote a page through one Bash
                // heredoc, a command of 29,322 characters, past the ledger's 16,384; the ledger refused the event
                // and the run was stopped ("Mission event is not readable by the ledger reader", 2026-10-04).
                ...(target === undefined ? {} : { command: boundedMessageText(target) }),
                ...(title === undefined ? {} : { title: boundedMessageText(title) }),
                ...(background ? { background: true } : {}),
                // The helper's type while it works, not only once it reports:
                // its row and a card it raises can name it (helper visibility).
                ...(helperType === undefined ? {} : { status: helperType }),
                phase: "started",
                evidence,
              }),
            );
          }
        }
      }
      const text = content
        .map((block) => (isObject(block) ? stringValue(block.text) : undefined))
        .filter((value): value is string => value !== undefined)
        .join("");
      if (text.length === 0) return restated;
      // The block the text streamed into, not an assumed one; a message that
      // did not stream is an item of its own.
      const messageId = identityValue(message.id);
      const streamedHere = textBlockId !== undefined && (messageId === undefined || messageId === streamingMessageId);
      return [
        ...restated,
        emit("message.delta", {
          itemId: streamedHere ? textBlockId! : `${messageId ?? `m${String(messageOrdinal)}`}:text`,
          operation: "replace",
          text: boundedAnswerText(text),
          final: true,
          evidence,
        }),
      ];
    }

    /*
     * A RESULT TOO LARGE TO CARRY (0.492, DISPLAY-COVERAGE gap 4). The host
     * drops a record over its cap -- an image, a whole file -- and leaves
     * this in its place, naming the calls its first bytes answer; each such
     * call is closed as having answered, with the size it answered in.
     * It read "did not report", which blamed the runtime for the host's cut.
     */
    if (type === "locust.oversized") {
      if (parsed.recordType !== "user") return [];
      const kb = typeof parsed.bytes === "number" && Number.isFinite(parsed.bytes) ? Math.round(parsed.bytes / 1024) : undefined;
      const events: NormalizedRuntimeEvent[] = [];
      for (const itemId of Array.isArray(parsed.callIds) ? parsed.callIds.filter((id): id is string => typeof id === "string") : []) {
        const open = openTools.get(itemId);
        if (open === undefined) continue;
        openTools.delete(itemId);
        events.push(
          emit("tool.completed", {
            itemId,
            toolKind: "tool_use",
            name: open.name,
            ...(open.target === undefined ? {} : { command: boundedMessageText(open.target) }),
            ...(open.title === undefined ? {} : { title: boundedMessageText(open.title) }),
            ...(open.background === true ? { background: true } : {}),
            ...(open.parent === undefined ? {} : { parentItemId: open.parent }),
            phase: "completed",
            status: "result too large to keep",
            output: `The result was too large to keep${kb === undefined ? "" : ` (${String(kb)} KB)`}. The teammate read it; Locust keeps results up to 256 KB.`,
            evidence,
          }),
        );
      }
      return events;
    }

    if (type === "user") {
      const message = isObject(parsed.message) ? parsed.message : {};
      const content = Array.isArray(message.content) ? message.content : [];
      const events: NormalizedRuntimeEvent[] = [];
      for (const block of content) {
        if (!isObject(block) || stringValue(block.type) !== "tool_result") continue;
        const itemId = identityValue(block.tool_use_id);
        if (itemId === undefined) continue;
        const open = openTools.get(itemId);
        /*
         * A SUBAGENT'S OWN TOOL, ENDING (0.543, the 0.536 review RUN-01).
         * MEASURED on a Haiku run: a subagent's calls never stream a start,
         * and their results came through here as completions of a call nobody
         * opened, named `tool`, which the Activity panel drew as bare "tool"
         * rows. Its calls are opened above now, as children of its row
         * (helper visibility), and end here as children; a result for a call
         * nobody opened is still nobody's.
         */
        if (open === undefined && fromSubagent(parsed)) continue;
        openTools.delete(itemId);
        const failed = block.is_error === true;
        let refusedFor = refusedCalls.get(itemId);
        refusedCalls.delete(itemId);
        // Locust's own guard said no: refused, never run, in its words (0.717).
        if (failed) {
          const said = claudeToolResultText(block) ?? "";
          const at = said.indexOf(LOCUST_GUARD_SAID);
          if (at >= 0) {
            refusedFor = said.slice(at);
            guardedCalls.set(itemId, refusedFor);
            if (open?.target !== undefined) guardedCommands.set(open.target, refusedFor);
          }
        }
        const subagentKind = subagentKinds.get(itemId);
        const subagentSummary = subagentSummaries.get(itemId);
        subagentKinds.delete(itemId);
        subagentSummaries.delete(itemId);
        events.push(
          emit(failed ? "tool.failed" : "tool.completed", {
            itemId,
            toolKind: "tool_use",
            name: open?.name ?? "tool",
            ...(open?.target === undefined ? {} : { command: boundedMessageText(open.target) }),
            ...(open?.title === undefined ? {} : { title: boundedMessageText(open.title) }),
            ...(open?.background === true ? { background: true } : {}),
            ...(open?.parent === undefined ? {} : { parentItemId: open.parent }),
            phase: "completed",
            // Refused is not failed: it never ran. The reason rides along.
            ...(refusedFor !== undefined
              ? { status: "refused", ...(refusedFor.length > 0 ? { output: boundedMessageText(refusedFor) } : {}) }
              : failed ? { status: "error" } : subagentKind === undefined ? {} : { status: subagentKind }),
            // What the subagent came back with, in its own words: the row
            // reads "reported back · 3" instead of only "reported back".
            ...(subagentSummary === undefined ? {} : { output: boundedMessageText(subagentSummary) }),
            // The change a file call made, from the record that answers it (claudeFilePatch).
            ...(() => {
              if (failed || refusedFor !== undefined) return {};
              const patch = claudeFilePatch(parsed.tool_use_result);
              return patch === undefined ? {} : { patch };
            })(),
            // What the step printed, bounded and scrubbed like a message (0.489).
            ...(() => {
              if (refusedFor !== undefined || subagentSummary !== undefined || !claudeShowsOutput(open?.name)) return {};
              const printed = claudeToolResultText(block);
              return printed === undefined ? {} : { output: boundedMessageText(printed) };
            })(),
            evidence,
          }),
        );
      }
      return events;
    }

    if (type === "result") {
      sawResult = true;
      const usage: Record<string, number> = {};
      if (typeof parsed.total_cost_usd === "number" && Number.isFinite(parsed.total_cost_usd)) {
        usage.usd = parsed.total_cost_usd;
      }
      if (isObject(parsed.usage)) {
        // The cache counts belong here too: a resumed turn sends almost
        // nothing new and reads the rest from cache, so `input_tokens` alone
        // says a long conversation is using nothing. What occupies the
        // window is the whole prompt, cached or not.
        for (const [from, to] of [
          ["input_tokens", "inputTokens"],
          ["output_tokens", "outputTokens"],
          ["cache_read_input_tokens", "cacheReadTokens"],
          ["cache_creation_input_tokens", "cacheWriteTokens"],
        ] as const) {
          const held = parsed.usage[from];
          if (typeof held === "number" && Number.isFinite(held)) usage[to] = held;
        }
      }
      // MEASURED 2026-09-06: Claude Code's result carries `modelUsage`, and
      // each entry states the model's real `contextWindow` (1,000,000 for
      // claude-sonnet-5) and `maxOutputTokens`. That is what makes a "how
      // full is the context" reading a measurement rather than a guess at a
      // denominator -- so it is kept, and no runtime that does not report one
      // gets a percentage invented for it.
      if (isObject(parsed.modelUsage)) {
        // MEASURED 2026-09-06: each entry is keyed by the real model id and
        // states `canonicalModel` as well. One entry means one model ran and
        // the alias has an answer; several means the run moved between them,
        // and naming one of them would be picking a favourite.
        const entries = Object.entries(parsed.modelUsage);
        if (entries.length === 1) {
          const [key, only] = entries[0]!;
          const canonical = isObject(only) ? only.canonicalModel : undefined;
          const named = typeof canonical === "string" && canonical.length > 0 ? canonical : key;
          if (named.length > 0) resolvedModel = named;
        }
        let widest: number | undefined;
        for (const entry of Object.values(parsed.modelUsage)) {
          if (!isObject(entry)) continue;
          const window = entry.contextWindow;
          if (typeof window === "number" && Number.isFinite(window) && window > 0) {
            widest = widest === undefined ? window : Math.max(widest, window);
          }
        }
        if (widest !== undefined) usage.contextWindow = widest;
      }
      if (Object.keys(usage).length > 0) completedUsage = usage;
      const isError = parsed.is_error === true;
      const subtype = stringValue(parsed.subtype) ?? "";
      const reason = stringValue(parsed.terminal_reason) ?? subtype;
      if (isError || (subtype.length > 0 && subtype !== "success")) {
        terminalFailureEvidence = evidence;
        const said = stringValue(parsed.result) ?? "";
        if (syntheticError === "authentication_failed" || /^Not logged in\b|Please run \/login|Invalid API key/i.test(said)) {
          terminalFailureKind = "authentication-failed";
          terminalFailure = CLAUDE_SIGNED_OUT;
        } else {
          terminalFailure = boundedMessageText(
            providerErrorSentence(said || `Claude Code ended with ${reason || "an error"}.`, "claude"),
          );
        }
      }
      // What the run was NOT allowed to do.
      //
      // MEASURED 2026-09-05, running the app's own accept-edits argv by hand:
      // `--permission-mode acceptEdits` auto-approves edits but NOT Bash, so
      // asking Claude Code to run `node t.mjs` produced
      // `permission_denials: [{tool_name: "Bash", ...}]` -- and the record
      // still said `is_error: false`, `subtype: "success"`,
      // `terminal_reason: "completed"`. The mission therefore completed with
      // the thing the person asked for never attempted, and nothing on
      // screen said so (QA pass, 2026-09-05, five identical attempts).
      //
      // The runtime names each refusal, so the app can too. This is not a
      // failure of the run and is not reported as one: it is the run saying
      // what it could not do.
      const denials = Array.isArray(parsed.permission_denials) ? parsed.permission_denials : [];
      const all = denials
        .filter(isObject)
        .map((denial) => {
          const tool = stringValue(denial.tool_name) ?? "a tool";
          const input = isObject(denial.tool_input) ? denial.tool_input : {};
          const detail = stringValue(input.command) ?? stringValue(input.file_path) ?? stringValue(input.description);
          const said = namedTool(tool);
          const brief = detail === undefined ? undefined : brieflyPut(detail);
          const callId = identityValue(denial.tool_use_id);
          const guarded = (callId === undefined ? undefined : guardedCalls.get(callId)) ?? (detail === undefined ? undefined : guardedCommands.get(detail));
          return { tool, text: brief === undefined ? said : `${said} \`${brief}\``, ...(guarded === undefined ? {} : { guarded }) };
        });
      // Locust's guard is not the mode: said apart, in what it stopped and why (0.717).
      const guardedOnes = all.filter((entry) => entry.guarded !== undefined);
      const refused = all.filter((entry) => entry.guarded === undefined);
      const said: ReturnType<typeof diagnostic>[] = [];
      if (guardedOnes.length > 0) said.push(diagnostic("warning", "claude.permission_denied", guardedSentence(guardedOnes), evidence));
      if (refused.length > 0) {
        said.push(
          diagnostic(
            "warning",
            "claude.permission_denied",
            refused.length === 1
              ? `Claude Code was not permitted to use ${refused[0]!.text}, so it did not. ${whyRefused(refused)}`
              : `Claude Code was not permitted to use ${String(refused.length)} tools, so it did not: ${refused.map((entry) => entry.text).join("; ")}. ${whyRefused(refused)}`,
            evidence,
          ),
        );
      }
      return said;
    }

    return [diagnostic("info", "claude.unknown_event", `Unhandled Claude record: ${type}`, evidence)];
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
          diagnostic(
            "warning",
            "claude.malformed_record",
            "A Claude record could not be parsed.",
            malformedEvidence(record),
          ),
        ];
      }
      if (!isObject(parsed)) {
        return [
          diagnostic(
            "warning",
            "claude.malformed_record",
            "A Claude record was not an object.",
            malformedEvidence(record),
          ),
        ];
      }
      return acceptParsed(record, parsed);
    },

    finish(completion: RuntimeProcessCompletion): readonly NormalizedRuntimeEvent[] {
      if (finalized) return [];
      finalized = true;
      const process = processEvidence(completion);
      if (completion.cancelled) {
        return [emit("run.cancelled", { ...(runtimeThreadId === undefined ? {} : { runtimeThreadId }), process })];
      }
      if (terminalFailure !== undefined) {
        return [
          emit("run.failed", {
            kind: terminalFailureKind ?? "unknown",
            message: terminalFailure,
            ...(terminalFailureEvidence === undefined ? {} : { evidence: terminalFailureEvidence }),
            ...(runtimeThreadId === undefined ? {} : { runtimeThreadId }),
            runtimeTerminal: "failed",
            process,
          }),
        ];
      }
      // A clean exit that never produced a `result` record is not a success:
      // the provider never said it finished, so the run is reported as failed
      // with that stated, rather than completed on the strength of exit 0.
      // Ended by the runner AFTER its result, for something it left running (endedAfterResult, 2026-10-09):
      // the turn finished, and the record says so.
      if (!sawResult || (completion.exitCode !== 0 && completion.endedAfterResult !== true)) {
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
              ? "Claude Code sent output faster than Locust could record it, so the run was stopped rather than leave a gap in its record. Sending it again usually works."
              : sawResult
                ? `Claude Code exited with code ${String(completion.exitCode)}.`
                : "Claude Code ended without a terminal result record.",
            ...(runtimeThreadId === undefined ? {} : { runtimeThreadId }),
            runtimeTerminal: sawResult ? "completed" : "missing",
            process,
          }),
        ];
      }
      // The receipt, with what only the whole run could say: how full the
      // conversation is after its last call, and whether a subscription's
      // windows covered the run. `usd` stays as Claude Code wrote it; on a
      // subscription it is what the tokens would have cost on the API, and
      // the app decides what to show.
      const usage: Record<string, number | string> | undefined = completedUsage === undefined
        ? undefined
        : {
            ...completedUsage,
            ...(lastCall === undefined ? {} : { contextTokens: lastCall.prompt + lastCall.output }),
            ...(onUsageWindows && !usedExtraUsage ? { billing: "subscription" } : {}),
          };
      return [
        emit("run.completed", {
          ...(runtimeThreadId === undefined ? {} : { runtimeThreadId }),
          ...(usage === undefined ? {} : { usage }),
          ...(resolvedModel === undefined ? {} : { resolvedModel }),
          process,
        }),
      ];
    },
  };
}
