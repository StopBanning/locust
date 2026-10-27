import { randomUUID } from 'node:crypto'

import { acpCallIsShell, acpShownTool, toolPatchFrom } from '@teammate/runtime-adapters'
import type {
  AcpPermissionAnswer,
  AcpPermissionRequest,
  AppServerRequest,
  JsonValue,
  MissionRuntimeId,
  NormalizedRuntimeEvent
} from '@teammate/runtime-adapters'

import type {
  CodexMissionUpdate,
  MissionApprovalAnswer,
  MissionApprovalDecision,
  MissionApprovalKind,
  MissionApprovalRequest,
  MissionQuestion,
  MissionQuestionOption
} from '../shared/ipc.js'
import { approvalPatchFrom } from './approval-patch.js'
import { relativeToFolder } from '../shared/approval-patch.js'
import { programAsRun, wholeDetail } from '../shared/approval-detail.js'
import { OUTSIDE_NOT_UNDOABLE, outsideFolder } from '../shared/approval-data.js'
import type { FileChangeRecord } from './approval-patch.js'

/**
 * The approval channel: what a runtime asks a person mid-run, and what the
 * person answers.
 *
 * This used to live inside a second mission service that existed for one
 * mode -- Approve-each ran on `codex app-server` because it was the only
 * transport that could stop and ask, and everything else ran on `codex exec`.
 * Since 0.65.0 every Codex mode runs on app-server through the one mission
 * loop, so the service that duplicated that loop for one mode is gone, and
 * the only thing Approve-each still needs of its own is this: a handler for
 * the server's requests, a registry of what is waiting on the person, and the
 * two rules below about matching an answer to what was asked.
 *
 * Nothing here knows about processes or ledgers. It is handed a run's ids and
 * the file changes it has announced, and it hands back a request handler.
 */

const MAX_PENDING_APPROVALS = 16

/** What the protocol's approval methods mean in the product's own words. */
export function approvalKindFor(method: string): MissionApprovalKind | undefined {
  if (method === 'item/commandExecution/requestApproval') return 'command'
  if (method === 'item/fileChange/requestApproval') return 'file-change'
  if (method === 'item/tool/requestUserInput') return 'question'
  return undefined
}

// Whole, line breaks kept, and anything past the cap counted (R14; approval-detail.ts).
function bounded(value: unknown): string {
  return typeof value === 'string' ? wholeDetail(value) : ''
}

/**
 * Turn a protocol approval request into something a person can decide about.
 * A card that cannot say WHAT would happen is not an approval, it is a dare.
 */
export function describeApproval(request: AppServerRequest):
  | {
      readonly kind: MissionApprovalKind
      readonly summary: string
      readonly detail: string
      readonly cwd: string | null
      readonly questions?: readonly MissionQuestion[]
    }
  | undefined {
  const kind = approvalKindFor(request.method)
  if (kind === undefined) return undefined
  const params = (typeof request.params === 'object' && request.params !== null ? request.params : {}) as Record<
    string,
    unknown
  >
  const cwd = typeof params.cwd === 'string' ? params.cwd : null

  if (kind === 'command') {
    const command = programAsRun(bounded(params.command))
    return {
      kind,
      summary: command.length > 0 ? 'Run a command' : 'Run a command it did not describe',
      detail: command,
      cwd
    }
  }
  if (kind === 'file-change') {
    const changes = Array.isArray(params.changes) ? params.changes.length : undefined
    return {
      kind,
      summary: changes === undefined ? 'Change files' : `Change ${changes} file${changes === 1 ? '' : 's'}`,
      // The diff itself is not shown here: it can be enormous, and the card's
      // job is to say what is about to happen, not to be a diff viewer.
      detail: bounded(params.summary ?? params.explanation ?? ''),
      cwd
    }
  }
  /*
   * A question, read the way the protocol actually sends it.
   *
   * This looked for singular `params.question`, `params.prompt` or
   * `params.message`. The request carries a `questions` ARRAY -- each entry
   * with `id`, `header`, `question`, `isOther`, `isSecret` and nullable
   * `options` -- so none of the three ever matched and the card drew "Answer a
   * question" with nothing under it. Verified against the schema codex-cli
   * 0.153.0 generates for itself (Astra, 2026-09-09).
   *
   * The singular keys are still read, last, because a request that carries one
   * costs nothing to honour and this build should not be the reason a question
   * goes unshown twice.
   */
  const questions = questionsOf(params)
  const first = questions[0]
  const detail =
    first !== undefined
      ? bounded(first.question)
      : bounded(params.question ?? params.prompt ?? params.message ?? '')
  return {
    kind,
    summary:
      questions.length > 1 ? `Answer ${String(questions.length)} questions` : 'Answer a question',
    detail,
    cwd,
    questions
  }
}

/** How many questions one request may carry, and how many options each may offer. */
const MAX_QUESTIONS = 12
const MAX_OPTIONS = 24

/** The `questions` array, defensively: a malformed entry is dropped, not guessed at. */
export function questionsOf(params: Record<string, unknown>): readonly MissionQuestion[] {
  const raw = Array.isArray(params.questions) ? params.questions : []
  const questions: MissionQuestion[] = []
  for (const entry of raw.slice(0, MAX_QUESTIONS)) {
    if (typeof entry !== 'object' || entry === null) continue
    const value = entry as Record<string, unknown>
    // An answer is filed under the id. Without one there is nowhere to put the
    // person's reply, so the question cannot be answered and is not offered.
    const id = typeof value.id === 'string' ? value.id : undefined
    const text = bounded(value.question)
    if (id === undefined || id.length === 0 || text.length === 0) continue
    const options: MissionQuestionOption[] = []
    for (const option of Array.isArray(value.options) ? value.options.slice(0, MAX_OPTIONS) : []) {
      if (typeof option !== 'object' || option === null) continue
      const shape = option as Record<string, unknown>
      const label = bounded(shape.label)
      // The label IS the identity: the protocol gives options no id or index,
      // and the server matches a returned string against the labels it sent.
      // An option with no label cannot be chosen, so it is not drawn.
      if (label.length === 0) continue
      options.push({
        label,
        description: typeof shape.description === 'string' ? bounded(shape.description) : null
      })
    }
    questions.push({
      id,
      header: typeof value.header === 'string' ? bounded(value.header) : null,
      question: text,
      options,
      isOther: value.isOther === true,
      isSecret: value.isSecret === true
    })
  }
  return questions
}

/**
 * What the renderer sent, as an answer a host can take -- or undefined.
 *
 * A question's ANSWERS never got past here. The handler rebuilt every
 * payload as `{ approvalId, decision }`, a missing decision became `deny`,
 * and the channel then refused it as a decision sent to a question -- so no
 * question card in Locust could be answered, Codex's included. Found
 * 2026-09-23, wiring Antigravity's questions to the same card.
 *
 * Anything but a recognized answer is still refused or read as a denial: a
 * malformed message must never be able to approve an action. And answers
 * reach only a question -- the channel checks the pairing.
 */
export function approvalAnswerFrom(raw: unknown): MissionApprovalAnswer | undefined {
  const payload = (typeof raw === 'object' && raw !== null ? raw : {}) as Record<string, unknown>
  if (typeof payload.approvalId !== 'string') return undefined
  if ('answers' in payload) {
    const answers = payload.answers
    if (typeof answers !== 'object' || answers === null || Array.isArray(answers)) return undefined
    const clean: Record<string, readonly string[]> = {}
    for (const [questionId, given] of Object.entries(answers as Record<string, unknown>)) {
      if (!Array.isArray(given) || !given.every((entry) => typeof entry === 'string')) return undefined
      // Bounded: an answer is words a person typed, not a file.
      clean[questionId] = (given as string[]).slice(0, 64).map((entry) => entry.slice(0, 4_000))
    }
    return { approvalId: payload.approvalId, answers: clean }
  }
  const decision = payload.decision
  if (decision === 'approve-once' || decision === 'approve-always') return { approvalId: payload.approvalId, decision }
  // A denial may say why (0.374): the person's words, bounded, and nothing
  // when there are none -- an empty reason is a plain denial.
  const reason = typeof payload.reason === 'string' ? payload.reason.replace(/\s+/g, ' ').trim().slice(0, MAX_DENY_REASON) : ''
  return { approvalId: payload.approvalId, decision: 'deny', ...(reason.length === 0 ? {} : { reason }) }
}

/** The most of a denial's reason that is passed on: a sentence or two, not a document. */
export const MAX_DENY_REASON = 500

/** What a runtime is told alongside a denial the person explained (0.374). */
export function deniedSaying(reason: string): string {
  // OpenCode's normalizer reads a person's decline by this sentence as well as
  // by "declined this in Locust" (opencode-events.ts, 0.410): with a reason it
  // had been drawn as "refused", a mode's word.
  return `The person declined this, and said: ${reason}`
}

/**
 * An OpenCode server's permission request, as the request this channel
 * already knows how to put to a person (A6.7).
 *
 * A shell call is a command card; an edit is a file-change card naming the
 * file; anything else OpenCode asks about -- a folder outside the run's own,
 * a web fetch -- is a command card that says what it is, because a card that
 * cannot say what would happen is not an approval.
 */
/** What an OpenCode request is, as `openCodePermissionRequest` reads it. */
interface OpenCodeAsked {
  readonly permission: string
  readonly patterns: readonly string[]
  /** What OpenCode itself remembers on "always" (R35): said on the card. */
  readonly always?: readonly string[]
  readonly metadata: Readonly<Record<string, unknown>>
  /** Asked by a subagent the run started (R36): said in the card's action. */
  readonly bySubagent?: boolean
}

/**
 * The request, and what an Always on it remembers (0.616, shared/who-decides.ts):
 * OpenCode's own patterns for it, the ones it would have remembered itself --
 * `echo *` for `echo SERVED` -- so a later `echo other` is covered and a later
 * `rm` is not. Locust keeps it now, after the saved rules, and OpenCode is
 * answered "once" (`openCodeReplyFor`). No patterns at all, no key: asked again.
 */
export function openCodePermissionRequest(permission: OpenCodeAsked, cwd: string): AppServerRequest {
  const request = openCodeRequestOf(permission, cwd)
  const always = (permission.always ?? []).filter((pattern) => pattern.length > 0)
  const covers = always.length > 0 ? always : permission.patterns.filter((pattern) => pattern.length > 0)
  if (covers.length === 0) return request
  const params = (request.params ?? {}) as Record<string, JsonValue>
  return { ...request, params: { ...params, locustAlwaysKey: `opencode:${permission.permission}:${covers.join('\n')}` } }
}

function openCodeRequestOf(permission: OpenCodeAsked, cwd: string): AppServerRequest {
  const said = (key: string): string | undefined => {
    const value = permission.metadata[key]
    return typeof value === 'string' && value.length > 0 ? value : undefined
  }
  /*
   * WHAT "ALWAYS" LETS THROUGH (QA-2026-09-29 round 2, R35). OpenCode applies
   * its answer `always` to the patterns in its own request -- `echo *` for
   * `echo SERVED` -- and the card showed one command beside a button that
   * allowed every `echo` for the run. Its patterns are on the card now.
   */
  const always = (permission.always ?? []).filter((pattern) => pattern.length > 0)
  const alwaysCovers = always.length === 0
    ? undefined
    : always.length === 1 && always[0] === '*'
      // Measured on a real server (0.467 drive): an edit suggests `*`.
      ? ({
          edit: 'every file change it asks for',
          bash: 'every command it asks to run',
          webfetch: 'every web page it asks to fetch',
          external_directory: 'every place outside the project it asks to reach'
        } as Readonly<Record<string, string>>)[permission.permission] ?? `every “${permission.permission}” it asks for`
      : `anything matching ${always.map((pattern) => `“${pattern}”`).join(' or ')}`
  // A subagent's request says so (R36): the person asked the teammate, not it.
  const by = permission.bySubagent === true ? ', asked by a subagent it started' : ''
  const card = (words: { readonly summary?: string; readonly dataSentSays?: string; readonly reversibleSays?: string }, plain: string): Record<string, string> => {
    const out: Record<string, string> = {}
    const summary = words.summary ?? (by.length > 0 ? plain : undefined)
    for (const [key, value] of Object.entries({ ...words, summary: summary === undefined ? undefined : `${summary}${by}`, alwaysCovers })) if (value !== undefined) out[key] = value
    return out
  }
  if (permission.permission === 'bash') {
    return { id: 0, method: 'item/commandExecution/requestApproval', params: { command: said('command') ?? permission.patterns.join(' '), cwd, locustCard: card({}, 'Run a command') } }
  }
  if (permission.permission === 'edit') {
    const file = said('filepath') ?? said('filePath') ?? permission.patterns.join(', ')
    const diff = said('diff')
    const outside = outsideFolder(file, cwd)
    // The file relative to the folder, and the change itself: a fresh-profile
    // beta report of 0.345 found the card naming a long absolute path and
    // "Change files" with nothing to judge -- while OpenCode's request
    // carries the diff (measured: metadata { filepath, diff }).
    return {
      id: 0,
      method: 'item/fileChange/requestApproval',
      params: {
        summary: relativeToFolder(file, cwd),
        cwd,
        ...(diff === undefined ? {} : { unifiedDiff: diff }),
        locustCard: card(outside ? { summary: 'Change 1 file outside your project folder', reversibleSays: OUTSIDE_NOT_UNDOABLE } : {}, 'Change 1 file')
      }
    }
  }
  /*
   * NOT EVERYTHING IS A COMMAND (R37). A read outside the folder and a web
   * fetch both drew "Run a command", with a command's "a script can do
   * anything" beneath. Each is said as what it is.
   */
  if (permission.permission === 'external_directory') {
    return {
      id: 0,
      method: 'item/commandExecution/requestApproval',
      params: {
        command: permission.patterns.join('\n'),
        cwd,
        locustCard: card({
          summary: 'Reach files outside your project folder',
          dataSentSays: 'Nothing by this alone: it lets the teammate’s tools read or change these paths on this machine.',
          reversibleSays: 'Reading changes nothing. A change there is outside the project’s version control.'
        }, '')
      }
    }
  }
  if (permission.permission === 'webfetch') {
    const address = said('url') ?? permission.patterns.join('\n')
    return {
      id: 0,
      method: 'item/commandExecution/requestApproval',
      params: {
        command: address,
        cwd,
        locustCard: card({
          summary: 'Fetch a web page',
          dataSentSays: 'The address above is requested from this machine, and the page it returns goes to the model.',
          reversibleSays: 'Nothing on this machine is changed by fetching.'
        }, '')
      }
    }
  }
  return {
    id: 0,
    method: 'item/commandExecution/requestApproval',
    params: {
      // A tool OpenCode names without details -- a person's own MCP tool asks with the pattern `*` alone -- has nothing
      // exact to show, and the card said "Exact *" (0.822, the 0.821 drive's frame). Left out, the card names the tool.
      command: permission.patterns.filter((pattern) => pattern !== '*').join('\n'),
      cwd,
      locustCard: card({
        summary: `Use ${permission.permission}`,
        dataSentSays: `Unknown. OpenCode names this “${permission.permission}”, and Locust cannot tell what it sends.`,
        reversibleSays: `Unknown — Locust cannot tell what “${permission.permission}” does.`
      }, '')
    }
  }
}

/**
 * A runtime's own unified diff, as the card's patch: its `Index:` and `====`
 * banner dropped and its paths made relative, so the change is what the card
 * shows rather than the folder it lives in.
 */
export function diffPatchFrom(diff: string, cwd: string): ReturnType<typeof toolPatchFrom> | undefined {
  const lines = diff.split(/\r?\n/)
    .filter((line) => !/^Index: /.test(line) && !/^={10,}$/.test(line))
    .map((line) => {
      const header = /^(---|\+\+\+) (.+)$/.exec(line)
      return header === null ? line : `${header[1]!} ${relativeToFolder(header[2]!.trim(), cwd)}`
    })
  const text = lines.join('\n').trim()
  return text.includes('@@') ? toolPatchFrom(text) : undefined
}

/** This channel's answer, as the reply an OpenCode server takes. Anything unclear is a refusal. */
export function openCodeReplyFor(result: JsonValue): 'once' | 'always' | 'reject' | { readonly reply: 'reject'; readonly message: string } {
  const record = typeof result === 'object' && result !== null && !Array.isArray(result) ? (result as { decision?: unknown; reason?: unknown }) : {}
  if (record.decision === 'accept') return 'once'
  // "Once" to OpenCode, since 0.616: Locust remembers the Always itself, under
  // the request's own patterns, and every later request is decided after the
  // saved rules (shared/who-decides.ts). Told "always", OpenCode stopped
  // asking, and a rule saying no never saw the next command.
  if (record.decision === 'acceptForSession') return 'once'
  // OpenCode's reject carries a message the model reads (measured on
  // 1.18.27), so a reason rides the denial itself (0.374).
  return typeof record.reason === 'string' && record.reason.length > 0 ? { reply: 'reject', message: deniedSaying(record.reason) } : 'reject'
}

/**
 * An Agent Client Protocol agent's permission request, as the request this
 * channel already knows how to put to a person (0.377; Copilot's Approve
 * each).
 *
 * A shell call is a command card with the command it will run -- or, when
 * the agent did not say, one that says so, rather than passing its own
 * description off as the command. An edit, a delete or a move is a
 * file-change card naming the files, with the change itself when the agent
 * sent its before and after. Anything else -- a fetch, a path outside the
 * folder -- is a command card in the agent's own words and what it names.
 */
export function acpPermissionRequest(asked: AcpPermissionRequest, cwd: string): AppServerRequest {
  /*
   * WHAT AN ALWAYS REMEMBERS (0.616, shared/who-decides.ts): the same command,
   * or the same files, again -- what the run used to remember itself
   * (acp-run.ts) before the saved rules were read. Said on the card too.
   */
  // A shell's locations path is the folder, not the command an Always would remember.
  const shell = acpCallIsShell(asked.kind, asked.title, acpShownTool(asked.kind, asked.title))
  const target = asked.command ?? (shell || asked.paths.length === 0 ? undefined : asked.paths.join('\n'))
  const request = acpRequestOf(asked, cwd)
  if (target === undefined) return request
  const params = (request.params ?? {}) as Record<string, JsonValue>
  const covers = asked.command !== undefined ? 'this same command again' : 'the same change to these files again'
  const card = typeof params.locustCard === 'object' && params.locustCard !== null && !Array.isArray(params.locustCard) ? params.locustCard : {}
  return { ...request, params: { ...params, locustAlwaysKey: `acp:${asked.kind ?? ''}:${target}`, locustCard: { ...card, alwaysCovers: covers } } }
}

function acpRequestOf(asked: AcpPermissionRequest, cwd: string): AppServerRequest {
  const base = { cwd, ...(asked.toolCallId === undefined ? {} : { itemId: asked.toolCallId }) }
  const files = asked.paths.map((path) => relativeToFolder(path, cwd)).join(', ')
  // Kind `execute`, or a plain tool word that is a shell (`shell`, `bash`). The folder in locations is not the command.
  if (asked.kind === 'execute' || acpCallIsShell(asked.kind, asked.title, acpShownTool(asked.kind, asked.title))) {
    return { id: 0, method: 'item/commandExecution/requestApproval', params: { ...base, command: asked.command ?? '' } }
  }
  if (asked.kind === 'edit' || asked.kind === 'delete' || asked.kind === 'move') {
    return {
      id: 0,
      method: 'item/fileChange/requestApproval',
      params: { ...base, summary: files.length > 0 ? files : (asked.title ?? ''), ...(asked.diff === undefined ? {} : { unifiedDiff: asked.diff }) }
    }
  }
  const what = asked.title ?? `Use ${asked.kind ?? 'a tool'}`
  const target = asked.command ?? (files.length > 0 ? files : undefined)
  return { id: 0, method: 'item/commandExecution/requestApproval', params: { ...base, command: target === undefined ? what : `${what}: ${target}` } }
}

/**
 * This channel's answer, as the kind of ACP option to choose. "Always" stays
 * "always" here; the run keeps it for itself and never passes it on
 * (acp-run.ts). Anything unclear is a refusal.
 */
export function acpAnswerFor(result: JsonValue): AcpPermissionAnswer {
  const record = typeof result === 'object' && result !== null && !Array.isArray(result) ? (result as { decision?: unknown }) : {}
  if (record.decision === 'accept') return 'allow_once'
  if (record.decision === 'acceptForSession') return 'allow_always'
  return 'reject_once'
}

/** The protocol decision for each of the product's three authorization answers. */
export function protocolDecisionFor(decision: MissionApprovalDecision): string {
  if (decision === 'approve-once') return 'accept'
  // Session-scoped, never durable: a forever-grant is a Settings decision, not
  // one to take mid-run under time pressure.
  if (decision === 'approve-always') return 'acceptForSession'
  return 'reject'
}

/**
 * The protocol result for one answer, whichever kind it is.
 *
 * A question is answered, an action is authorized, and they do not share a
 * payload. `item/tool/requestUserInput` wants
 *
 *     { answers: { <questionId>: { answers: [ "<literal label or free text>" ] } } }
 *
 * and a decision is not a member of that type. Locust sent one anyway, for all
 * three buttons: the server logs the deserialization failure, substitutes an
 * EMPTY answer map, and tells the model the person said nothing. Whatever they
 * had chosen was discarded on the way (Astra, 2026-09-09, from the version-
 * matched upstream handler).
 *
 * An option is identified by its LITERAL LABEL. The protocol gives options no
 * id and no index, and the server matches the strings it gets back against the
 * labels it sent -- so sending `"3"` for the third option answers with the
 * string "3".
 */
export function protocolAnswerFor(answer: MissionApprovalAnswer): JsonValue {
  if ('answers' in answer) {
    const answers: Record<string, JsonValue> = {}
    for (const [questionId, given] of Object.entries(answer.answers)) {
      // An empty list is a question the person left alone. Sent as an empty
      // array rather than omitted, so "asked and not answered" and "never
      // asked" stay distinguishable on the wire.
      answers[questionId] = { answers: [...given] }
    }
    return { answers }
  }
  return { decision: protocolDecisionFor(answer.decision) }
}

/**
 * A fileChange item's tool rows carry the change itself.
 *
 * The app-server normaliser writes "N file change(s)" as the row's command
 * and no patch, so after an APPROVED edit the activity fold read "Codex CLI
 * did not report the change" beside a row for a file whose diff the
 * approval card had just shown (seen driving the app, 2026-09-05). The
 * item's `changes` are remembered by id for the approval; the same record
 * gives the row its paths and its patch.
 */
export function withFileChanges(
  events: readonly NormalizedRuntimeEvent[],
  changesByItem: ReadonlyMap<string, readonly FileChangeRecord[]>,
  workspacePath: string
): readonly NormalizedRuntimeEvent[] {
  return events.map((event) => {
    if (event.type !== 'tool.started' && event.type !== 'tool.completed' && event.type !== 'tool.failed') return event
    const payload = event.payload as { readonly itemId?: string; readonly toolKind?: string; readonly command?: string; readonly patch?: unknown }
    if (payload.toolKind !== 'fileChange' || payload.itemId === undefined) return event
    const changes = changesByItem.get(payload.itemId)
    if (changes === undefined || changes.length === 0) return event
    // A change that failed or was declined wrote nothing: it names its
    // files, and carries no diff that would read as written (M2).
    const patch = event.type === 'tool.failed' ? undefined : approvalPatchFrom(changes, workspacePath)
    const paths = changes.map((change) => relativeToFolder(change.path, workspacePath)).join('\n')
    return {
      ...event,
      payload: {
        ...payload,
        command: paths,
        ...(patch === undefined ? {} : { patch })
      }
    } as NormalizedRuntimeEvent
  })
}

/**
 * A call the person declined, recorded as declined (0.374).
 *
 * Codex 0.157 runs a command inside a script, and a declined approval comes
 * back as the script FAILING -- `Rejected("approval request failed")`, no
 * `declined` status -- so the row read "failed" in red and the fold counted
 * it as a command that "exited non-zero" (drive-deny-with-reason on Codex).
 * Nothing failed: the person said no. Locust answered that approval itself,
 * so it does not need Codex's words for it -- a failure of an item the
 * person declined is said as declined, and reads "refused" like every other
 * runtime's.
 */
export function withDeclines(
  events: readonly NormalizedRuntimeEvent[],
  declined: ReadonlySet<string> | undefined
): readonly NormalizedRuntimeEvent[] {
  if (declined === undefined || declined.size === 0) return events
  return events.map((event) => {
    if (event.type !== 'tool.failed') return event
    const payload = event.payload as { readonly itemId?: string; readonly status?: string }
    if (payload.itemId === undefined || !declined.has(payload.itemId) || payload.status === 'declined') return event
    return { ...event, payload: { ...payload, status: 'declined' } } as NormalizedRuntimeEvent
  })
}

/** What a request handler needs to know about the run it is answering for. */
export interface ApprovalRun {
  readonly runId: string
  readonly missionId: string
  /** Where the run works; a card's paths are shown relative to it. */
  readonly cwd: string
  /**
   * File changes the run has announced, by item id, so a file-change card
   * can carry the diff Codex attached to the item the approval is about.
   */
  readonly changesByItem: ReadonlyMap<string, readonly FileChangeRecord[]>
  /** Who is asking, so the card names them. Absent: Codex, as before. */
  readonly runtime?: MissionRuntimeId
}

export interface ApprovalChannel {
  /** The `onRequest` for one run: raises the card and waits for the answer. */
  requestHandlerFor(run: ApprovalRun): (request: AppServerRequest) => Promise<JsonValue>
  /** The person's answer. False when nothing was waiting under that id. */
  decide(answer: MissionApprovalAnswer): boolean
  /** The run is over: refuse whatever it was still asking, so no card waits forever. */
  release(runId: string): void
  /** The items in a run the person declined, by item id (`withDeclines`). */
  declined(runId: string): ReadonlySet<string>
  readonly pendingCount: number
}

export interface ApprovalChannelOptions {
  readonly emitApproval: (request: MissionApprovalRequest) => void
  /** Notices about a request that was refused without reaching the person. */
  readonly emitUpdate?: (update: CodexMissionUpdate) => void
  /**
   * A denial the person explained, on a runtime whose reply cannot carry it
   * (Codex's app-server): the reason is shown to the run as its next input
   * instead (0.374). OpenCode's reply carries it and never comes here.
   */
  readonly onDeniedSaying?: (denied: { readonly runId: string; readonly missionId: string; readonly reason: string }) => void
  readonly createId?: () => string
  readonly now?: () => Date
}

/** Codex asks again after an Always: Locust remembers the exact action, not the runtime. */
function codexAlwaysFor(kind: MissionApprovalKind, params: Record<string, unknown>, changes: readonly FileChangeRecord[] | undefined): { key: string; covers: string } | undefined {
  if (kind === 'command' && typeof params.command === 'string' && params.command.length > 0) {
    return { key: `codex:command:${params.command}`, covers: 'this same command again' }
  }
  if (kind !== 'file-change') return undefined
  const named = changes ?? (Array.isArray(params.changes) ? params.changes : [])
  const paths: string[] = []
  for (const entry of named) {
    if (typeof entry !== 'object' || entry === null) return undefined
    const change = entry as Record<string, unknown>
    if (typeof change.path !== 'string' || change.path.length === 0) return undefined
    paths.push(change.path)
    if (typeof change.movePath === 'string' && change.movePath.length > 0) paths.push(change.movePath)
  }
  // JSON keeps path boundaries unambiguous, even when a path contains a newline.
  return paths.length === 0 ? undefined : { key: `codex:files:${JSON.stringify(paths.sort())}`, covers: 'the same change to these files again' }
}

export function createApprovalChannel(options: ApprovalChannelOptions): ApprovalChannel {
  const createId = options.createId ?? randomUUID
  const now = options.now ?? (() => new Date())

  interface Pending {
    readonly runId: string
    readonly missionId: string
    /**
     * What the runtime asked for, kept so an answer can be checked against it.
     *
     * A question takes answers and an action takes a decision, and the server
     * validates neither -- it deserializes, fails silently, and substitutes an
     * empty answer. So the pairing is enforced here, and that needs the kind
     * to still be in hand when the person replies.
     */
    readonly kind: MissionApprovalKind
    /** Whose request: OpenCode's reply carries a denial's reason, Codex's does not. */
    readonly runtime: MissionRuntimeId | undefined
    /** The item the request is about, when the runtime named one. */
    readonly itemId: string | undefined
    readonly alwaysKey: string | undefined
    readonly resolve: (value: JsonValue) => void
  }
  const approvals = new Map<string, Pending>()
  const declinedByRun = new Map<string, Set<string>>()

  const release = (runId: string): void => {
    declinedByRun.delete(runId)
    for (const [approvalId, pending] of approvals) {
      if (pending.runId !== runId) continue
      approvals.delete(approvalId)
      pending.resolve({ decision: 'reject' })
    }
  }

  return {
    requestHandlerFor(run) {
      const { runId, missionId } = run
      return async (request) => {
        const refuse = (why: string): JsonValue => {
          options.emitUpdate?.({ kind: 'relay-notice', runId, missionId, message: why })
          return { decision: 'reject' }
        }
        const described = describeApproval(request)
        if (described === undefined) {
          return refuse(
            `Locust denied a request it does not recognise (${request.method}) because it could not say what would happen. This build is older than the runtime's protocol.`
          )
        }
        if (approvals.size >= MAX_PENDING_APPROVALS) {
          return refuse(
            `Locust denied an action because ${String(MAX_PENDING_APPROVALS)} approvals are already waiting on you. Answer some of them and ask again.`
          )
        }
        const approvalId = `ap_${createId()}`
        const requestParams = (typeof request.params === 'object' && request.params !== null ? request.params : {}) as Record<string, unknown>
        const itemId = typeof requestParams.itemId === 'string' ? requestParams.itemId : undefined
        let changes = described.kind === 'file-change' && itemId !== undefined ? run.changesByItem.get(itemId) : undefined
        /*
         * THE CHANGE, WAITED FOR (QA-2026-09-29 round 2, R30). Codex sends a
         * file item's changes on `item/started` and then the approval, which
         * names only the item. The changes are recorded when the mission loop
         * READS that record, behind a durable ledger write, so an approval
         * arriving within a few milliseconds of its item was drawn first: a
         * card with no file and no diff. Bounded: a quarter second, then the
         * card says what it was told.
         */
        // Codex only: the one runtime whose approval names an item announced before it.
        const codex = run.runtime === undefined || run.runtime === 'codex'
        if (codex && described.kind === 'file-change' && itemId !== undefined && changes === undefined && typeof requestParams.unifiedDiff !== 'string') {
          for (let waited = 0; waited < 250 && changes === undefined; waited += 25) {
            await new Promise((settle) => setTimeout(settle, 25))
            changes = run.changesByItem.get(itemId)
          }
        }
        // Words the asking route put on the request for its card (R35, R37): see openCodePermissionRequest.
        const said = (key: string): string | undefined => {
          const card = requestParams.locustCard
          const value = typeof card === 'object' && card !== null ? (card as Record<string, unknown>)[key] : undefined
          return typeof value === 'string' && value.length > 0 ? value : undefined
        }
        const offered = typeof requestParams.unifiedDiff === 'string' ? diffPatchFrom(requestParams.unifiedDiff, run.cwd) : undefined
        const patch = approvalPatchFrom(changes, run.cwd) ?? offered
        const codexAlways = codex ? codexAlwaysFor(described.kind, requestParams, changes) : undefined
        const alwaysKey = codex
          ? codexAlways?.key
          : typeof requestParams.locustAlwaysKey === 'string' && requestParams.locustAlwaysKey.length > 0
            ? requestParams.locustAlwaysKey.slice(0, 2_000)
            : undefined
        const alwaysCovers = codexAlways?.covers ?? said('alwaysCovers')
        return await new Promise<JsonValue>((resolve) => {
          approvals.set(approvalId, { runId, missionId, kind: described.kind, runtime: run.runtime, itemId, alwaysKey, resolve })
          options.emitApproval({
            approvalId,
            runId,
            missionId,
            runtime: run.runtime ?? 'codex',
            kind: described.kind,
            ...(described.questions === undefined || described.questions.length === 0
              ? {}
              : { questions: described.questions }),
            ...(requestParams.isBlocking === false ? { blocking: false } : {}),
            summary: changes !== undefined && changes.length > 0
              ? `Change ${String(changes.length)} file${changes.length === 1 ? '' : 's'}`
              : said('summary') ?? (offered !== undefined ? 'Change 1 file' : described.summary),
            detail: described.detail,
            cwd: described.cwd,
            requestedAt: now().toISOString(),
            ...(alwaysCovers === undefined ? {} : { alwaysCovers }),
            ...(alwaysKey === undefined ? {} : { alwaysKey }),
            ...(said('dataSentSays') === undefined ? {} : { dataSentSays: said('dataSentSays')! }),
            ...(said('reversibleSays') === undefined ? {} : { reversibleSays: said('reversibleSays')! }),
            ...(patch === undefined ? {} : { patch: { text: patch.text, added: patch.added, removed: patch.removed, truncated: patch.truncated } })
          })
        })
      }
    },

    decide(answer) {
      const pending = approvals.get(answer.approvalId)
      // An unknown or already-answered id is ignored rather than throwing: a
      // double-click on the card must not take the run down.
      if (pending === undefined) return false
      approvals.delete(answer.approvalId)
      /*
       * A question's answer must not reach a command's request, or the reverse.
       *
       * The two payloads are different types, and the server does not validate
       * which one it got -- it deserializes, fails quietly, and substitutes an
       * empty answer. So the pairing is checked HERE, where both the pending
       * request's kind and the answer's shape are in hand.
       */
      const answering = 'answers' in answer
      if (answering !== (pending.kind === 'question')) {
        options.emitUpdate?.({
          kind: 'relay-notice',
          runId: pending.runId,
          missionId: pending.missionId,
          message: answering
            ? 'Locust did not send that answer: it belongs to a question, and the runtime was waiting on an authorization.'
            : 'Locust did not send that decision: the runtime asked a question, which is answered rather than approved.'
        })
        // Refused rather than forwarded. Resolving with the wrong shape is
        // exactly what discarded the person's answer before.
        pending.resolve(protocolAnswerFor({ approvalId: answer.approvalId, decision: 'deny' }))
        return false
      }
      if ('decision' in answer && answer.decision === 'deny' && pending.itemId !== undefined) {
        const declined = declinedByRun.get(pending.runId) ?? new Set<string>()
        declined.add(pending.itemId)
        declinedByRun.set(pending.runId, declined)
      }
      const reason = 'decision' in answer && answer.decision === 'deny' ? answer.reason : undefined
      if (reason === undefined) {
        const codexAlways = (pending.runtime === undefined || pending.runtime === 'codex') && pending.alwaysKey !== undefined && 'decision' in answer && answer.decision === 'approve-always'
        pending.resolve(protocolAnswerFor(codexAlways ? { ...answer, decision: 'approve-once' } : answer))
        return true
      }
      if (pending.runtime === 'opencode') {
        // Read by `openCodeReplyFor` here in main; it never reaches Codex.
        pending.resolve({ ...(protocolAnswerFor(answer) as Record<string, JsonValue>), reason })
        return true
      }
      pending.resolve(protocolAnswerFor(answer))
      options.onDeniedSaying?.({ runId: pending.runId, missionId: pending.missionId, reason })
      return true
    },

    release,

    declined(runId) {
      return declinedByRun.get(runId) ?? new Set<string>()
    },

    get pendingCount() {
      return approvals.size
    }
  }
}
