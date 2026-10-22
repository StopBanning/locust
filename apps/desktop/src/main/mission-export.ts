import type { MissionApproval, RecoveredMission } from '@teammate/mission-store'
import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'

import { runtimeDisplayName } from '../shared/runtimes.js'
import { scrubSecrets } from '../shared/secrets.js'
import { byHelper, isShellTool, isEditCommand, reportsAChange } from '../shared/tool-kinds.js'

/**
 * SAVE THE RECORD: one conversation, as one Markdown file.
 *
 * A pure function from what the ledger holds -- the missions of one
 * conversation, oldest first -- to text. It reads nothing else and says
 * nothing the ledger does not: where the ledger is silent, the file says it is
 * silent, in those words, rather than leaving a gap that reads as "nothing
 * happened".
 *
 * WHAT THE LEDGER DOES NOT HOLD, and so what this file cannot say (read off
 * the code, 2026-10-03, not assumed):
 *   - An approval card that was ANSWERED YES. A card, and the person's answer
 *     to it, travel over IPC and are never written down; the call that was
 *     allowed simply runs, and looks like any call that needed no approval.
 *   - Who answered a card or how (a click, "don't ask again", a saved rule),
 *     except where the words recorded WITH a call say so: the host's own
 *     sentence for a saved rule's denial is carried into the call's output on
 *     the runtimes whose reply can carry it.
 *   - The folder a turn ran in. A mission records a workspace id; the path is
 *     passed in by the caller, from where it is kept now, and labelled so.
 *   - The Locust build that ran a turn. The build that WROTE the file is named.
 * What it does hold about approvals is the status of the call itself:
 * `declined` (the person's answer was no) and `refused` (the mode or runtime
 * said no before it ran). Those are listed; nothing is inferred beyond them.
 */

export interface RecordSource {
  /** The conversation's missions, oldest first: the whole `continuesFrom` chain. */
  readonly missions: readonly RecoveredMission[]
  /** The teammate's name, or undefined when the roster no longer says. */
  readonly teammate?: string
  /** Where the teammate works now, when that is known. The ledger holds only a workspace id. */
  readonly folder?: string
  /** The build writing this file. */
  readonly locustVersion: string
  /** When the file is written (ISO), so the same ledger always gives the same file. */
  readonly savedAt: string
  /** A parent the chain points at that the ledger no longer holds. */
  readonly missingParent?: string
}

/** The host's own sentence, written by `answerByDecision` in index.ts when a saved rule says no. */
export const SAVED_RULE_DENIAL = 'A rule the person saved says no:'

/** The section the host writes a reply's words into (handoff.ts), last, under this sentence. */
const HANDOFF_INSTRUCTION_SECTION = '\n\nThe person now asks:\n\n'

const NOT_RECORDED = 'not recorded'

function handoffInstruction(prompt: string): string | undefined {
  const at = prompt.lastIndexOf(HANDOFF_INSTRUCTION_SECTION)
  if (at === -1) return undefined
  const asked = prompt.slice(at + HANDOFF_INSTRUCTION_SECTION.length).trim()
  return asked.length === 0 ? undefined : asked
}

/** A fence long enough that nothing inside it can close it. */
function fenced(text: string, language = ''): string {
  let ticks = 3
  for (const run of text.match(/`+/g) ?? []) ticks = Math.max(ticks, run.length + 1)
  const fence = '`'.repeat(ticks)
  return `${fence}${language}\n${text.replace(/\n+$/, '')}\n${fence}`
}

/** Words quoted, so a heading or a list inside them cannot become this file's own. */
function quoted(text: string): string {
  return text
    .replace(/\r\n?/g, '\n')
    .replace(/\n+$/, '')
    .split('\n')
    .map((line) => (line.length === 0 ? '>' : `> ${line}`))
    .join('\n')
}

function oneLine(text: string): string {
  return text.replace(/\s+/g, ' ').trim()
}

function outputText(output: unknown): string | undefined {
  if (output === undefined || output === null) return undefined
  return typeof output === 'string' ? output : JSON.stringify(output, null, 2)
}

/**
 * What the file holds in place of a key. The ledger's own words that are
 * written verbatim pass through `scrubbed`, which keeps the count the header
 * reports. The count belongs to one `missionRecordMarkdown` call: it is zeroed
 * there, and the file is built in one synchronous pass.
 */
let piecesReplaced = 0
function scrubbed(text: string): string {
  const clean = scrubSecrets(text)
  piecesReplaced += clean.replaced
  return clean.text
}

/** `outputText`, scrubbed: for the places a call's output is written into the file, not where it is only read. */
function outputWords(output: unknown): string | undefined {
  const words = outputText(output)
  return words === undefined ? undefined : scrubbed(words)
}

/** What the assistant said in one turn, rebuilt the way the thread rebuilds it: deltas append or replace per item. */
function saidIn(events: readonly NormalizedRuntimeEvent[]): string {
  const order: string[] = []
  const buffers = new Map<string, string>()
  for (const event of events) {
    if (event.type !== 'message.delta') continue
    const { itemId, operation, text } = event.payload
    if (!buffers.has(itemId)) order.push(itemId)
    buffers.set(itemId, operation === 'replace' ? text : `${buffers.get(itemId) ?? ''}${text}`)
  }
  return order
    .map((itemId) => buffers.get(itemId) ?? '')
    .filter((text) => text.trim().length > 0)
    .join('\n\n')
}

interface Call {
  readonly itemId: string
  readonly name: string
  readonly toolKind: string
  readonly command?: string
  readonly title?: string
  readonly status?: string
  readonly exitCode?: number
  readonly output?: unknown
  readonly patch?: { readonly text: string; readonly added: number; readonly removed: number; readonly truncated: boolean }
  readonly durationMs?: number
  readonly startedAt: string
  readonly background?: boolean
  /** Made by a helper the teammate sent out, not by the teammate (ledger v22). */
  readonly byHelper?: true
  /** A tool.completed or tool.failed was recorded. */
  readonly settled: boolean
  readonly failed: boolean
}

/** Tool events folded into one row per call, in the order the calls began. */
function callsIn(events: readonly NormalizedRuntimeEvent[]): readonly Call[] {
  const order: string[] = []
  const calls = new Map<string, Call>()
  for (const event of events) {
    if (event.type !== 'tool.started' && event.type !== 'tool.completed' && event.type !== 'tool.failed') continue
    const p = event.payload
    const before = calls.get(p.itemId)
    if (before === undefined) order.push(p.itemId)
    const settled = event.type !== 'tool.started'
    calls.set(p.itemId, {
      itemId: p.itemId,
      name: p.name,
      toolKind: p.toolKind,
      ...(p.command !== undefined ? { command: p.command } : before?.command !== undefined ? { command: before.command } : {}),
      ...(p.title !== undefined ? { title: p.title } : before?.title !== undefined ? { title: before.title } : {}),
      ...(p.status !== undefined ? { status: p.status } : before?.status !== undefined ? { status: before.status } : {}),
      ...(p.exitCode !== undefined ? { exitCode: p.exitCode } : before?.exitCode !== undefined ? { exitCode: before.exitCode } : {}),
      ...(p.output !== undefined ? { output: p.output } : before?.output !== undefined ? { output: before.output } : {}),
      ...(p.patch !== undefined ? { patch: p.patch } : before?.patch !== undefined ? { patch: before.patch } : {}),
      ...(p.durationMs !== undefined ? { durationMs: p.durationMs } : before?.durationMs !== undefined ? { durationMs: before.durationMs } : {}),
      ...(p.background === true || before?.background === true ? { background: true } : {}),
      ...(byHelper(p) || before?.byHelper === true ? { byHelper: true as const } : {}),
      startedAt: before?.startedAt ?? event.occurredAt,
      settled: settled || before?.settled === true,
      failed: event.type === 'tool.failed' || before?.failed === true
    })
  }
  return order.map((id) => calls.get(id)!)
}

type CallKind = 'command' | 'change' | 'other'

function kindOf(call: Call): CallKind {
  const probe = { name: call.name, toolKind: call.toolKind, ...(call.command === undefined ? {} : { command: call.command }) }
  if (call.patch !== undefined || reportsAChange(probe)) return 'change'
  if (isShellTool(call.name, call.toolKind) && !(call.command !== undefined && isEditCommand(call.command))) return 'command'
  return 'other'
}

const isDeclined = (call: Call): boolean => call.status === 'declined'
const isRefused = (call: Call): boolean => call.status === 'refused'
const namesASavedRule = (call: Call): boolean => (outputText(call.output) ?? '').includes(SAVED_RULE_DENIAL)

function what(call: Call): string {
  const target = call.command !== undefined && call.command.length > 0 ? call.command : call.name
  if (isShellTool(call.name, call.toolKind)) return target
  return call.name !== target && call.command !== undefined ? `${call.name}: ${target}` : target
}

function duration(ms: number | undefined): string | undefined {
  if (ms === undefined) return undefined
  if (ms < 1000) return `${String(Math.round(ms))} ms`
  const seconds = ms / 1000
  return seconds < 60 ? `${seconds.toFixed(1)} s` : `${String(Math.floor(seconds / 60))} min ${String(Math.round(seconds % 60))} s`
}

function callState(call: Call): string {
  // A helper's call is still a call that ran here; the record says whose it was.
  const by = call.byHelper === true ? ' · made by a helper the teammate sent out' : ''
  return `${ownCallState(call)}${by}`
}

function ownCallState(call: Call): string {
  if (isDeclined(call)) return 'declined; it did not run'
  if (isRefused(call)) return 'refused; it did not run'
  if (!call.settled) return 'no result was recorded'
  const parts = [call.failed ? 'failed' : 'completed']
  if (call.status !== undefined && !/^(completed|failed|started)$/i.test(call.status)) parts.push(`runtime status "${call.status}"`)
  if (call.exitCode !== undefined) parts.push(`exit code ${String(call.exitCode)}`)
  const took = duration(call.durationMs)
  if (took !== undefined) parts.push(took)
  return parts.join(' · ')
}

const ANSWERED: Record<MissionApproval['answer'], string> = {
  allowed: 'Allowed',
  'allowed-always': 'Allowed for the rest of the session',
  denied: 'Denied',
  answered: 'Answered'
}
const ANSWERED_BY: Record<MissionApproval['by'], string> = {
  card: 'by the person, on the card',
  'card-saving-a-rule': 'by the person, on the card, saving it as a rule',
  'saved-rule': 'by a rule the person saved, before the card reached them',
  // v21 (0.616): no card was drawn; the person's Always on an earlier one covered it.
  'earlier-always': 'by the person’s Always on an earlier card of this run, with no card of its own'
}

/** The cards answered on a turn (ledger v20): what each asked, the answer, and who gave it. */
function cardsAnswered(approvals: readonly MissionApproval[]): string[] {
  const lines: string[] = []
  for (const approval of approvals) {
    lines.push(`- **${ANSWERED[approval.answer]}** ${ANSWERED_BY[approval.by]} · ${approval.kind} · asked ${approval.askedAt} · answered ${approval.occurredAt}`)
    lines.push('', quoted(`The card asked: ${approval.asked}`), '')
    if (approval.words !== undefined) lines.push(quoted(`Said with the answer: ${scrubbed(approval.words)}`), '')
  }
  return lines
}

function approvalsSection(calls: readonly Call[], events: readonly NormalizedRuntimeEvent[], mission: RecoveredMission): string {
  // From ledger v20 every card answered is its own record; before it, only a call's own status says anything.
  const recordsCards = (mission.schemaVersion ?? 0) >= 20
  // Answers the host could not write down (0.587): each left a note under "How it ended".
  const unwritten = events.filter((event) => event.type === 'adapter.diagnostic' && event.payload.code === 'host.approval-not-recorded').length
  const lines: string[] = recordsCards ? cardsAnswered(mission.approvals) : []
  for (const call of calls) {
    if (!isDeclined(call) && !isRefused(call) && !namesASavedRule(call)) continue
    const words = outputWords(call.output)
    const asked = `\`${oneLine(what(call))}\``
    if (namesASavedRule(call)) {
      lines.push(`- **Denied, and the words recorded with it name a saved rule.** Asked: ${asked}. The call did not run.`)
    } else if (isDeclined(call) && recordsCards) {
      lines.push(`- **Declined.** Asked: ${asked}. The call did not run; the answer that declined it is listed above.`)
    } else if (isDeclined(call)) {
      lines.push(
        `- **Declined.** Asked: ${asked}. The ledger records the call as declined: the answer was no, given through Locust's approval card or a rule the person saved. `
        + 'It does not record which, and the words recorded with the call do not say.'
      )
    } else {
      lines.push(`- **Refused before it ran.** Asked: ${asked}. The mode or the runtime refused it; no answer from a person is recorded.`)
    }
    if (words !== undefined && words.length > 0) lines.push('', quoted(`Recorded with the call: ${words}`), '')
  }
  for (const event of events) {
    if (event.type !== 'adapter.diagnostic') continue
    if (!/denied|permission/i.test(event.payload.code)) continue
    lines.push(`- **The runtime reported a denial** (\`${event.payload.code}\`, ${event.occurredAt}): ${oneLine(event.payload.message)}`)
  }
  const nothing = lines.length === 0
  return [
    nothing
      ? recordsCards
        ? 'No card was answered, and no call was declined or refused, in this turn.'
        : 'No declined or refused call is recorded in this turn.'
      : lines.join('\n').replace(/\n\n\n+/g, '\n\n'),
    '',
    recordsCards
      ? (unwritten > 0
          // The host knows an answer never reached the ledger (0.587): the claim must not read complete.
          ? `_Recorded:_ every card answered on this turn should be here; ${String(unwritten)} could not be written down, and the notes under "How it ended" say which. `
          : '_Recorded:_ every card answered on this turn, with who answered it. ')
        // Three kinds, not two (0.599): the third raises no card, so nothing of it reaches the ledger.
        + 'Not recorded: a card no one answered because the run ended first, a call that ran in a mode that asks nothing, and a call covered by an earlier "Always" on this run, which raises no card.'
      : '_Not in the ledger:_ an approval that was allowed, what the card asked in full, who answered it and how. '
        + 'A call listed under Commands or File changes may or may not have been approved first; the record cannot tell.'
  ].join('\n')
}

function commandsSection(calls: readonly Call[]): string {
  const commands = calls.filter((call) => kindOf(call) === 'command')
  if (commands.length === 0) return 'No command is recorded in this turn.'
  return commands
    .map((call, at) => {
      const out: string[] = [`**Command ${String(at + 1)}** · ${callState(call)}${call.background === true ? ' · run in the background' : ''} · started ${call.startedAt}`]
      if (call.title !== undefined) out.push(`   The runtime described it as: ${oneLine(call.title)}`)
      out.push('', fenced(call.command ?? call.name, 'sh'))
      const words = outputWords(call.output)
      if (words === undefined) out.push('', '_No output is recorded for this command._')
      else if (words.length === 0) out.push('', '_The command printed nothing._')
      else out.push('', 'Output as recorded:', '', fenced(words))
      return out.join('\n')
    })
    .join('\n\n')
}

function changesSection(calls: readonly Call[]): string {
  const changes = calls.filter((call) => kindOf(call) === 'change')
  if (changes.length === 0) return 'No file change is recorded in this turn.'
  return changes
    .map((call, at) => {
      const paths = (call.command ?? call.name).split('\n').map((path) => path.trim()).filter((path) => path.length > 0)
      const out: string[] = [`**Change ${String(at + 1)}** · ${paths.map((path) => `\`${path}\``).join(', ')} · ${callState(call)}${call.status !== undefined && /on disk|from disk/.test(call.status) ? ' · seen on disk by Locust after the run, not reported by the runtime' : ''}`]
      const recorded = call.patch === undefined ? outputText(call.output) : undefined
      if (call.patch !== undefined) {
        out.push('', `Diff as recorded: +${String(call.patch.added)} −${String(call.patch.removed)}`)
        if (call.patch.truncated) {
          out.push(
            '',
            `_Too large to keep whole. The ledger holds only the first ${String(call.patch.text.length)} characters of this diff; the runtime reported +${String(call.patch.added)} −${String(call.patch.removed)} for the whole change._`
          )
        }
        out.push('', fenced(scrubbed(call.patch.text), 'diff'))
      } else if (recorded !== undefined && /too large/i.test(`${call.status ?? ''} ${recorded}`)) {
        out.push('', `No diff is recorded. The ledger's own words: ${oneLine(scrubbed(recorded))}`)
      } else {
        out.push('', '_No diff is recorded for this change._')
        if (recorded !== undefined && recorded.length > 0) out.push('', quoted(`Recorded with the call: ${scrubbed(recorded)}`))
      }
      return out.join('\n')
    })
    .join('\n\n')
}

function otherCallsSection(calls: readonly Call[]): string | undefined {
  const others = calls.filter((call) => kindOf(call) === 'other')
  if (others.length === 0) return undefined
  return others.map((call) => `- \`${oneLine(what(call))}\` · ${callState(call)}`).join('\n')
}

function endingSection(mission: RecoveredMission): string {
  const lines: string[] = []
  let terminal: NormalizedRuntimeEvent | undefined
  for (let at = mission.events.length - 1; at >= 0; at -= 1) {
    const event = mission.events[at]!
    if (event.type === 'run.completed' || event.type === 'run.failed' || event.type === 'run.cancelled') {
      terminal = event
      break
    }
  }
  if (terminal === undefined) {
    lines.push(
      mission.phase === 'failed'
        ? '**Failed before the runtime reported an ending.** Locust recorded a failure to start or keep the runtime, and the runtime itself recorded no completed, failed or stopped event.'
        : '**No ending is recorded.** The record stops without a completed, failed or stopped event, so how this turn ended is not known. '
          + 'A turn still running when the file was saved reads the same way.'
    )
  } else if (terminal.type === 'run.completed') {
    const model = terminal.payload.resolvedModel
    lines.push(`**Finished.** The runtime reported the run complete at ${terminal.occurredAt}.${model === undefined ? '' : ` It named the model it ran as \`${model}\`.`}`)
  } else if (terminal.type === 'run.failed') {
    lines.push(`**Failed** at ${terminal.occurredAt} (${terminal.payload.kind}).`)
    if (terminal.payload.message.trim().length > 0) lines.push('', quoted(terminal.payload.message))
  } else {
    lines.push(`**Stopped before it finished**, at ${terminal.occurredAt}.`)
  }
  for (const event of mission.events) {
    if (event.type === 'route.limit_detected') lines.push('', `The route reported a limit (${event.payload.kind}) at ${event.occurredAt}: ${oneLine(event.payload.message)}`)
  }
  for (const failure of mission.hostFailures) lines.push('', `Locust reported \`${failure.code}\` at ${failure.occurredAt}: ${oneLine(failure.message)}`)
  for (const event of mission.events) {
    if (event.type === 'adapter.diagnostic' && event.payload.code.startsWith('host.')) {
      lines.push('', `Note Locust wrote into the record (\`${event.payload.code}\`, ${event.occurredAt}): ${oneLine(event.payload.message)}`)
    }
  }
  for (const check of mission.editChecks) {
    lines.push('', `The person's check, \`${oneLine(check.command)}\`, after this turn: **${check.outcome}** (${check.occurredAt}).`)
    if (check.newLines.length > 0) lines.push('', fenced(scrubbed(check.newLines.join('\n'))))
  }
  return lines.join('\n')
}

function modelOf(mission: RecoveredMission): string {
  const completed = [...mission.events].reverse().find((event) => event.type === 'run.completed')
  const ran = completed !== undefined && completed.type === 'run.completed' ? completed.payload.resolvedModel : undefined
  return ran === undefined || ran === mission.metadata.model ? `\`${mission.metadata.model}\`` : `\`${mission.metadata.model}\`, which the runtime named \`${ran}\``
}

function runLine(mission: RecoveredMission): string {
  const { metadata } = mission
  const parts = [
    `${runtimeDisplayName(metadata.runtime)}`,
    `model ${modelOf(mission)}`,
    `mode ${metadata.mode ?? `${NOT_RECORDED} (this turn predates it)`}`,
    `sandbox ${metadata.sandbox}`
  ]
  if (metadata.cliVersion !== null) parts.push(`CLI ${metadata.cliVersion}`)
  return parts.join(' · ')
}

function integrity(missions: readonly RecoveredMission[], missingParent: string | undefined): string {
  const problems = missions.flatMap((mission) =>
    mission.issues.map((issue) => `\`${issue.code}\`: ${oneLine(issue.message)} (${issue.missionId ?? mission.metadata.missionId})`)
  )
  if (missingParent !== undefined) {
    problems.push(`An earlier turn, \`${missingParent}\`, is not in the ledger (deleted, in the trash, or unreadable). This record begins after it.`)
  }
  if (problems.length === 0) {
    return (
      '**record readable.** Every ledger file for this conversation was read and its event sequence has no gap. '
      + 'This is not tamper evidence: a record edited at rest, or a ledger file deleted, reads back the same.'
    )
  }
  return [
    `**record incomplete.** ${String(problems.length)} problem${problems.length === 1 ? ' was' : 's were'} found reading this conversation's ledger, so what follows may stop short of what happened:`,
    ...problems.map((problem) => `  - ${problem}`)
  ].join('\n')
}

function handoffsSection(mission: RecoveredMission, missions: readonly RecoveredMission[]): string {
  const lines: string[] = []
  const link = mission.metadata.continuesFrom
  if (link?.reason === 'route-switch') {
    const prior = missions.find((entry) => entry.metadata.missionId === link.missionId)
    const from = prior === undefined ? `a turn that is not in this record (\`${link.missionId}\`)` : `${runtimeDisplayName(prior.metadata.runtime)}, model ${modelOf(prior)}`
    lines.push(`- **Handed over** from ${from}, to ${runtimeDisplayName(mission.metadata.runtime)}, model ${modelOf(mission)}, at ${mission.metadata.createdAt}.`)
    const checkpoint = prior?.checkpoints.find((entry) => entry.epoch === link.checkpointEpoch)
    if (checkpoint === undefined) {
      lines.push(`  - It resumed from checkpoint ${String(link.checkpointEpoch)}, which is not in this record.`)
    } else {
      lines.push(
        `  - Resumed from checkpoint ${String(checkpoint.epoch)} (${checkpoint.reason}), recorded as ${checkpoint.resumeSafety}.`,
        checkpoint.unsettledActions.length === 0
          ? '  - No action was left unsettled at that checkpoint.'
          : `  - Left unsettled at that checkpoint: ${checkpoint.unsettledActions.map((action) => `\`${oneLine(action.name)}\``).join(', ')}.`
      )
    }
    lines.push(
      link.leftOut === undefined ? '  - What the hand-over brief left out to fit: none recorded.' : `  - The hand-over brief left out, to fit: ${link.leftOut.join(', ')}.`
    )
    if (link.leftOutByYou !== undefined && link.leftOutByYou.length > 0) lines.push(`  - The person chose to leave out of the brief: ${link.leftOutByYou.join(', ')}.`)
    lines.push('  - The brief the host wrote for the new runtime is held in the raw record, not repeated here.')
  }
  for (const peer of mission.peerLinks) {
    lines.push(
      `- **${peer.direction === 'received' ? 'Received a message from' : 'Posted a message to'} another teammate** (\`${peer.peerTeammateId}\`) at ${peer.occurredAt}. `
      + `The message is held in the workroom, not in this ledger (message \`${peer.messageId}\`).`
    )
  }
  return lines.length === 0 ? 'No hand-off is recorded in this turn.' : lines.join('\n')
}

/** Who is recorded as having started a turn, and the person's words where a person typed them. */
function askedSection(mission: RecoveredMission, teammate: string): { readonly heading: string; readonly body: string } {
  const { metadata } = mission
  const by = metadata.startedBy
  const link = metadata.continuesFrom
  const host = (origin: string): { heading: string; body: string } => ({
    heading: 'Asked',
    body: `_${origin} The prompt the ledger holds:_\n\n${quoted(metadata.prompt)}`
  })
  if (by?.kind === 'relay') return host(`Nobody typed this turn: Locust started it so ${teammate} could answer another teammate (automatic turn ${String(by.hop)}).`)
  if (by?.kind === 'routine') return host(`Nobody typed this turn: Locust replayed step ${String(by.step)} of a saved routine.`)
  if (by?.kind === 'resume') return host(`Nobody typed this turn: Locust picked the work back up from checkpoint ${String(by.epoch)} after an interruption.`)
  if (by?.kind === 'side') return host(`A side question (number ${String(by.question)}) asked on a fork of the conversation; the conversation itself never saw it.`)
  if (by?.kind === 'carry-on') return host(`Nobody typed this turn: ${runtimeDisplayName(by.from)} ran out of its plan, so Locust carried the work on.`)
  if (by?.kind === 'check-fix') return host(`Nobody typed this turn: the check \`${by.command}\` failed after the turn before, so Locust sent it back to fix (fix ${String(by.fix)} of ${String(by.of)}).`)
  if (by?.kind === 'terminal') {
    return {
      heading: 'The person said, in the runtime\'s own terminal',
      body: `_Locust did not run this exchange (number ${String(by.exchange)}); it was brought into the record afterwards._\n\n${quoted(metadata.prompt)}`
    }
  }
  if (link?.reason === 'route-switch') {
    const asked = handoffInstruction(metadata.prompt)
    if (asked !== undefined) return { heading: 'The person said', body: quoted(asked) }
    return {
      heading: 'Asked',
      body: '_Nobody typed this turn. Locust carried the work to another runtime on its own; the brief it wrote is held in the raw record._'
    }
  }
  return { heading: 'The person said', body: quoted(metadata.prompt) }
}

function turnSection(mission: RecoveredMission, at: number, source: RecordSource, teammate: string): string {
  const calls = callsIn(mission.events)
  const { metadata } = mission
  const asked = askedSection(mission, teammate)
  const said = saidIn(mission.events)
  const other = otherCallsSection(calls)
  const out: string[] = [
    `## Turn ${String(at + 1)}`,
    '',
    `${runLine(mission)}`,
    `Started ${metadata.createdAt} · last recorded ${mission.lastUpdatedAt} · \`${metadata.missionId}\``
  ]
  const link = metadata.continuesFrom
  if (link?.edited === true) {
    out.push(
      '',
      `_This turn is an edit of an earlier message: it follows \`${link.missionId}\` in place of the turn that first followed it. That earlier version is still in the ledger and is not part of this file._`
    )
  }
  out.push('', `### ${asked.heading}`, '', asked.body)
  out.push('', `### ${teammate} said`, '', said.length > 0 ? quoted(said) : '_Nothing was said in this turn._')
  out.push('', '### Approvals and refusals', '', approvalsSection(calls, mission.events, mission))
  out.push('', '### Commands', '', commandsSection(calls))
  out.push('', '### File changes', '', changesSection(calls))
  if (other !== undefined) out.push('', '### Other tool calls', '', other)
  out.push('', '### Hand-offs', '', handoffsSection(mission, source.missions))
  out.push('', '### How it ended', '', endingSection(mission))
  return out.join('\n')
}

/** The whole conversation, as Markdown. */
export function missionRecordMarkdown(source: RecordSource): string {
  const { missions } = source
  const teammate = source.teammate ?? 'The teammate'
  const first = missions[0]
  const last = missions[missions.length - 1]
  const out: string[] = [`# Record of a conversation with ${source.teammate ?? 'a teammate'}`, '']
  if (first === undefined || last === undefined) {
    out.push('The ledger holds no turn of this conversation, so there is nothing to record.')
    return `${out.join('\n')}\n`
  }
  const ended =
    last.phase === 'interrupted'
      ? `no ending is recorded; the last thing recorded is at ${last.lastUpdatedAt}`
      : `${last.lastUpdatedAt} (${last.phase})`
  piecesReplaced = 0
  // The turns are built first: the header counts what they replaced.
  const turns = missions.map((mission, at) => turnSection(mission, at, source, teammate))
  out.push(
    `- **Teammate:** ${source.teammate ?? `${NOT_RECORDED}: the roster no longer names them`}`,
    `- **Folder:** ${source.folder === undefined ? `${NOT_RECORDED}. The ledger holds only the workspace id \`${first.metadata.workspaceId}\`.` : `${source.folder} (where the teammate works now; the ledger holds only the workspace id \`${first.metadata.workspaceId}\`)`}`,
    `- **Started:** ${first.metadata.createdAt}`,
    `- **Ended:** ${ended}`,
    `- **Turns:** ${String(missions.length)}`,
    `- **Locust version:** ${source.locustVersion}, the build that wrote this file. The ledger does not record which build ran each turn.`,
    `- **Saved:** ${source.savedAt}`,
    `- **The record's own check:** ${integrity(missions, source.missingParent)}`,
    piecesReplaced === 0
      ? '- **Secret-shaped text:** none found.'
      : `- **Secret-shaped text:** ${String(piecesReplaced)} ${piecesReplaced === 1 ? 'piece' : 'pieces'} replaced in this file. The raw record keeps the original.`,
    '',
    '## Who answered',
    ''
  )
  missions.forEach((mission, at) => {
    const previous = missions[at - 1]
    if (previous !== undefined && previous.metadata.runtime !== mission.metadata.runtime) {
      out.push(`- ↪ **Handed over** from ${runtimeDisplayName(previous.metadata.runtime)} to ${runtimeDisplayName(mission.metadata.runtime)}.`)
    }
    out.push(`- **Turn ${String(at + 1)}** · ${runLine(mission)}`)
  })
  out.push('')
  turns.forEach((turn) => out.push(turn, ''))
  return `${collapseOutsideFences(out.join('\n')).trimEnd()}\n`
}

/** Runs of blank lines become one -- outside fenced blocks only: a command's output is kept as recorded (2026-10-10 sweep). */
export function collapseOutsideFences(text: string): string {
  const kept: string[] = []
  let fence: string | undefined
  let blanks = 0
  for (const line of text.split('\n')) {
    const marker = /^(`{3,}|~{3,})/.exec(line)?.[1]
    if (fence === undefined && marker !== undefined) fence = marker
    else if (fence !== undefined && marker !== undefined && line.startsWith(fence) && line.trim() === line.trim().slice(0, marker.length)) fence = undefined
    if (fence === undefined && marker === undefined && line.trim().length === 0) {
      blanks += 1
      if (blanks > 1) continue
    } else blanks = 0
    kept.push(line)
  }
  return kept.join('\n')
}

/** The events exactly as the ledger holds them, for the person who wants to check the Markdown against them. */
export function rawRecordJson(missions: readonly RecoveredMission[], locustVersion: string, savedAt: string): string {
  return `${JSON.stringify({ warning: 'This file is the ledger as recorded, including any secret-shaped text the Markdown replaced.', writtenBy: `Locust ${locustVersion}`, savedAt, missions }, null, 2)}\n`
}

/**
 * The turns of a conversation, oldest first, from any turn in it: a reply
 * exported alone brings its parents. Bounded by what it has already seen, so a
 * hand-edited cycle cannot spin. A parent the ledger no longer holds ends the
 * walk and is named, never skipped over.
 */
export async function recordTurns(
  getMission: (missionId: string) => Promise<RecoveredMission | undefined>,
  missionId: string
): Promise<{ readonly missions: readonly RecoveredMission[]; readonly missingParent?: string }> {
  const seen = new Set<string>()
  const newestFirst: RecoveredMission[] = []
  let next: string | undefined = missionId
  let missingParent: string | undefined
  while (next !== undefined && !seen.has(next)) {
    seen.add(next)
    const mission: RecoveredMission | undefined = await getMission(next)
    if (mission === undefined) {
      if (newestFirst.length > 0) missingParent = next
      break
    }
    newestFirst.push(mission)
    next = mission.metadata.continuesFrom?.missionId
  }
  return { missions: newestFirst.reverse(), ...(missingParent === undefined ? {} : { missingParent }) }
}

/** `Locust record - <teammate> - <date>.md`, with nothing a file name cannot hold. */
export function recordFileName(teammate: string | undefined, now: Date): string {
  const who = (teammate ?? 'conversation').replace(/[\\/:*?"<>|\u0000-\u001f]/g, '').replace(/\s+/g, ' ').trim().slice(0, 60) || 'conversation'
  const pad = (value: number): string => String(value).padStart(2, '0')
  return `Locust record - ${who} - ${String(now.getFullYear())}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}.md`
}
