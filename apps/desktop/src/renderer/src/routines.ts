import type { PublicRecoveredMission, PublicRoutine, PublicTeammate, RoutineHandOff, RoutineImportPreview, TeammateRoute } from '../../shared/ipc.js'
import { proposedHandOffs } from '../../shared/chain-proposal.js'
import type { RoutineInput } from '../../shared/routine-inputs.js'
import type { MissionRuntimeId } from '@teammate/runtime-adapters'
import { splitAttachments } from '../../shared/attachments.js'
import { nextRunAfter, scheduleBase, scheduleLabel } from '../../shared/routine-schedule.js'
import { conversationTurns, typedPrompt } from './missionView.js'
import { routeModelName } from './routeName.js'
import { runtimeDisplayName } from '../../shared/runtimes.js'

/**
 * Turning a finished conversation into a routine a teammate can replay.
 *
 * A teammate cannot watch a person work outside this app -- Locust sees what a
 * runtime reports inside a mission, not an editor or a terminal. What it CAN
 * learn from is work it did WITH the person, and every conversation is already
 * a durable record of that. So a routine is the words a PERSON typed on each
 * turn, in order.
 *
 * Which is why host-written turns are dropped rather than saved: a relayed
 * reply, a resume, and a routine step all carry a briefing the host wrote for
 * a runtime, not something anyone typed. Replaying those would replay the
 * app's own words back at itself.
 */

export const MAX_ROUTINE_STEPS = 12

export interface RoutineDraft {
  readonly name: string
  readonly steps: readonly string[]
  readonly learnedFrom: readonly string[]
  /** True when the conversation had more turns than a routine may hold, and the draft was cut. */
  readonly truncated: boolean
}

/** A name from the first step: enough to recognise, short enough to sit in a list. */
export function draftName(step: string): string {
  const said = step.replace(/\s+/gu, ' ').trim()
  if (said.length <= 60) return said
  const cut = said.slice(0, 60)
  const space = cut.lastIndexOf(' ')
  return `${(space > 24 ? cut.slice(0, space) : cut).trimEnd()}...`
}

/**
 * The draft for one conversation, or undefined when there is nothing a person
 * typed in it -- an exchange made entirely of relayed turns is a real thing to
 * read and not a routine to replay.
 */
export function routineDraft(
  mission: PublicRecoveredMission,
  byId: ReadonlyMap<string, PublicRecoveredMission>
): RoutineDraft | undefined {
  const turns = conversationTurns(mission, byId)
  const said: { readonly missionId: string; readonly prompt: string; readonly phase: string | undefined }[] = []
  for (const turn of turns) {
    const held = byId.get(turn.missionId)
    // Host-written turns carry a briefing, not a person's words. A turn typed
    // in the runtime's own terminal (0.391) is the person's, but it can be a
    // terminal command ("/review") that a replay here would not run the same
    // way, so it is not offered as a step either.
    if (held?.startedBy !== undefined) continue
    // A turn on another runtime was STARTED with the host's briefing; the
    // person's words are the part of it they typed (`typedPrompt`). Read raw,
    // the briefing itself was offered as a routine step.
    const prompt = (held === undefined ? turn.prompt : typedPrompt(held, byId)).trim()
    if (prompt.length === 0) continue
    said.push({ missionId: turn.missionId, prompt, phase: held?.phase })
  }
  /*
   * The turns that WORKED, where there are any.
   *
   * A routine replays what a conversation did, and a turn the person stopped
   * or that never finished is not something it did: the 0.271 recheck found
   * "Save as routine" proposing a cancelled prompt as a step. Completed turns
   * first; failing those, anything not cancelled, so a conversation whose
   * every turn failed on a quota can still be saved and replayed later.
   */
  const completed = said.filter((turn) => turn.phase === 'completed')
  const typed = completed.length > 0 ? completed : said.filter((turn) => turn.phase !== 'cancelled')
  const first = typed[0]
  if (first === undefined) return undefined
  const kept = typed.slice(0, MAX_ROUTINE_STEPS)
  return {
    // Named by what the person typed, not by the host's attachment line (L17).
    name: draftName(splitAttachments(first.prompt).text),
    steps: kept.map((turn) => turn.prompt),
    learnedFrom: kept.map((turn) => turn.missionId),
    truncated: typed.length > kept.length
  }
}

/** What the sidebar says under a teammate replaying a routine. */
export function routineStepLabel(progress: { readonly step: number; readonly of: number }): string {
  return `routine · step ${String(progress.step)} of ${String(progress.of)}`
}

/**
 * A running routine step, where the conversation it runs in is named: its
 * header says "routine Morning check, step 2 of 3" in place of "running".
 *
 * Only the compact sidebar said which step a routine was on; the conversation
 * the routine was running in said "running" like any other. Colin, asked
 * where the progress belongs: "maybe put in convo hub for teammate".
 * Undefined for a run no routine started.
 */
export function routineStepPhrase(
  startedBy: { readonly kind: string; readonly routineId?: string; readonly step?: number } | undefined,
  routines: readonly { readonly routineId: string; readonly name: string; readonly steps: readonly string[] }[]
): string | undefined {
  if (startedBy?.kind !== 'routine' || startedBy.routineId === undefined || startedBy.step === undefined) return undefined
  const routine = routines.find((entry) => entry.routineId === startedBy.routineId)
  return routine === undefined
    ? `routine, step ${String(startedBy.step)}`
    : `routine ${routine.name}, step ${String(startedBy.step)} of ${String(routine.steps.length)}`
}

/**
 * Who does a hand-off chain's steps, in order (0.435): "Wren → Atlas → Sable
 * (checks)". Consecutive steps of one teammate read once; a checker always
 * reads, marked. Undefined for a routine one teammate runs throughout, whose
 * card names that teammate as it always has.
 */
export function routineChain(
  routine: Pick<PublicRoutine, 'teammateId' | 'handOffs' | 'steps'>,
  team: readonly { readonly teammateId: string; readonly name: string; readonly route?: { readonly runtime: string; readonly model: string } }[],
  /**
   * Each teammate with the model their steps run on (0.493): a step runs on
   * its teammate's route at the time, and Grok's 0.489 pass could not tell
   * which model a routine would use from its card.
   */
  withModels = false
): string | undefined {
  const handOffs = routine.handOffs
  if (handOffs === undefined || !handOffs.some((entry) => entry.teammateId !== undefined || entry.check === true)) {
    if (!withModels) return undefined
    return routineRunner(routine.teammateId, team)
  }
  const name = (id: string): string => team.find((teammate) => teammate.teammateId === id)?.name ?? 'someone removed'
  const steps: { readonly owner: string; readonly check: boolean }[] = []
  let last: string | undefined
  routine.steps.forEach((_, index) => {
    const entry = handOffs[index] ?? {}
    const owner = entry.teammateId ?? routine.teammateId
    if (owner === last && entry.check !== true) return
    steps.push({ owner, check: entry.check === true })
    last = owner
  })
  const link = (step: { readonly owner: string; readonly check: boolean }, label: string): string => (step.check ? `${label} (checks)` : label)
  if (!withModels) return steps.map((step) => link(step, name(step.owner))).join(' → ')
  /*
   * ONE MODEL, NAMED ONCE (0.636). A chain whose teammates all run the same
   * tool and model said it at every link -- "Wren (Muse Spark 1.3 Contributor
   * Free) → Atlas (Muse Spark ...) → Sable (Muse Sp..." -- and the card's line
   * cut off before "(checks)", the one word that says who decides (seen in a
   * hand-off chain run in Locust itself, 2026-10-05). The same model by two
   * tools is two routes, so they still read per teammate.
   */
  const routes = steps.map((step) => team.find((teammate) => teammate.teammateId === step.owner)?.route)
  const first = routes[0]
  const shared = first !== undefined && routes.every((route) => route !== undefined && route.runtime === first.runtime && route.model === first.model)
  if (shared && steps.length > 1) {
    const model = first.model === 'account-default' ? `${runtimeDisplayName(first.runtime as MissionRuntimeId)} default` : routeModelName(first.runtime, first.model)
    return `${steps.map((step) => link(step, name(step.owner))).join(' → ')}, ${steps.length === 2 ? 'both' : 'all'} on ${model}`
  }
  return steps.map((step) => link(step, routineRunner(step.owner, team))).join(' → ')
}

/** A teammate and the model their routine steps run on now: "Wren (Grok 4.7)". */
export function routineRunner(
  teammateId: string,
  team: readonly { readonly teammateId: string; readonly name: string; readonly route?: { readonly runtime: string; readonly model: string } }[]
): string {
  const teammate = team.find((entry) => entry.teammateId === teammateId)
  if (teammate === undefined) return 'someone removed'
  const route = teammate.route
  return route === undefined ? teammate.name : `${teammate.name} (${route.model === 'account-default' ? `${runtimeDisplayName(route.runtime as MissionRuntimeId)} default` : routeModelName(route.runtime, route.model)})`
}

/**
 * What a routine's card says about its last run. Never "never run" dressed up
 * as a time: a routine that has not run says so.
 */
export function routineRunSummary(routine: Pick<PublicRoutine, 'runs' | 'steps' | 'execution'>): string {
  const steps = `${String(routine.steps.length)} step${routine.steps.length === 1 ? '' : 's'}`
  if (routine.execution !== undefined) return `${steps} · ${routine.execution.status === 'abandoned' ? 'last attempt abandoned' : routine.execution.status === 'running' ? 'in progress' : 'waiting for review'} · ${String(routine.runs)} completed runs`
  if (routine.runs === 0) return `${steps} · not run yet`
  return `${steps} · run ${String(routine.runs)} time${routine.runs === 1 ? '' : 's'}`
}

/**
 * A scheduled routine on a teammate in Approve each (0.591, the PRD's U4):
 * fired while nobody is at the window, its run stops on the first card and
 * waits there. Said on the row, where the schedule is set, instead of being
 * found the next morning under "waiting on you". Undefined for every other
 * mode, and for a routine that only runs when pressed (someone is there).
 */
export function routineWaitsForYou(routine: Pick<PublicRoutine, 'route' | 'schedule'>): string | undefined {
  if (routine.schedule === undefined || routine.route.mode !== 'approve-each') return undefined
  return 'Runs in Approve each: fired while you are away, it waits for you on its first card. Ask or Accept edits runs through.'
}

const pad = (value: number): string => String(value).padStart(2, '0')
const clock = (at: Date): string => `${pad(at.getHours())}:${pad(at.getMinutes())}`
const sameDay = (a: Date, b: Date): boolean =>
  a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate()
const WEEKDAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'] as const

/**
 * What a routine's card says about its schedule: the rule and the next run,
 * in the person's own clock. Undefined for a routine that only runs when
 * pressed, so the card says nothing rather than "never".
 */
/** "Missed, 08:00 today" — the slot that passed while Locust was closed. */
export function missedLine(missedAt: string, now: Date): string | undefined {
  const at = new Date(missedAt)
  if (Number.isNaN(at.getTime())) return undefined
  const when = clock(at)
  if (sameDay(at, now)) return `Missed, ${when} today`
  const yesterday = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1)
  if (sameDay(at, yesterday)) return `Missed, ${when} yesterday`
  return `Missed, ${WEEKDAYS[at.getDay()] ?? ''} ${when}`
}

export function routineScheduleSummary(
  routine: Pick<PublicRoutine, 'schedule' | 'lastRunAt' | 'createdAt' | 'execution' | 'missedAt' | 'paused' | 'resumedAt'>,
  now: Date
): string | undefined {
  if (routine.schedule === undefined) return undefined
  // Paused (0.705): said on the chip itself, the one fact a person scans the row for.
  if (routine.paused === true && (routine.execution === undefined || routine.execution.status === 'abandoned')) return `${scheduleLabel(routine.schedule)} · paused`
  if (routine.execution !== undefined && routine.execution.status !== 'abandoned') return `${scheduleLabel(routine.schedule)} · ${routine.execution.status === 'running' ? 'in progress' : 'held for review; no automatic retry'}`
  if (routine.missedAt !== undefined) {
    const missed = missedLine(routine.missedAt, now)
    if (missed !== undefined) return missed
  }
  const next = nextRunAfter(routine.schedule, scheduleBase(routine), now)
  // A watcher has no next time: it is waiting for a file (0.522).
  if (routine.schedule.kind === 'files') return `${scheduleLabel(routine.schedule)} · watching`
  // A once that has run: said, so the card does not promise a run that will not come.
  if (next === undefined) return `${scheduleLabel(routine.schedule)} · done`
  const when =
    next.getTime() <= now.getTime()
      ? 'due now'
      : sameDay(next, now)
        ? `next ${clock(next)}`
        : sameDay(next, new Date(now.getTime() + 24 * 3_600_000))
          ? `next tomorrow ${clock(next)}`
          : `next ${WEEKDAYS[next.getDay()] ?? ''} ${clock(next)}`
  return `${scheduleLabel(routine.schedule)} · ${when}`
}

/**
 * A chain template, ready for the editor (2026-10-05): the draft a routine
 * dialog opens on, with a teammate proposed for each step's role, the first
 * step's teammate running it, the checking step marked, and -- for the chains
 * that change files -- the run set to change them, in a copy that waits for
 * Keep or Discard. Only a proposal: the dialog shows all of it and the person
 * changes what they like before saving.
 *
 * `fallback` is the route a teammate with none of their own would replay on.
 */
export function chainDraftFrom(
  preview: Pick<RoutineImportPreview, 'name' | 'steps' | 'inputs' | 'handOffRoles' | 'handOffChecks' | 'handOffOnAnotherAgent' | 'changesFiles'>,
  team: readonly PublicTeammate[],
  fallback: TeammateRoute
): {
  readonly teammateId: string | undefined
  readonly name: string
  readonly steps: readonly string[]
  readonly inputs: readonly RoutineInput[]
  readonly handOffs: readonly RoutineHandOff[]
  /** The role the template named for each step, for the editor to show beside who was proposed. */
  readonly stepRoles: readonly (string | undefined)[]
  readonly route?: TeammateRoute
  readonly inCopy?: true
  readonly forceMode?: 'accept-edits'
} {
  const { owner, handOffs } = proposedHandOffs(preview.steps.map((_, at) => ({ role: preview.handOffRoles[at], check: preview.handOffChecks[at], another: preview.handOffOnAnotherAgent?.[at] })), team)
  const teammate = team.find((entry) => entry.teammateId === owner)
  const base = teammate === undefined ? undefined : (teammate.route ?? fallback)
  const changes = preview.changesFiles === true
  return {
    teammateId: owner,
    name: preview.name,
    steps: preview.steps,
    inputs: preview.inputs,
    handOffs,
    stepRoles: preview.handOffRoles,
    ...(base === undefined ? {} : { route: changes ? { ...base, mode: 'accept-edits' as const } : base }),
    ...(changes ? { inCopy: true as const, forceMode: 'accept-edits' as const } : {})
  }
}
