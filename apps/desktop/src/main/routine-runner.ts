import type { MissionRuntimeId } from '@teammate/runtime-adapters'
import type { RecoveredMissionPhase } from '@teammate/mission-store'

import type { CodexMissionStartResponse, CodexMissionUpdate, MissionMode, PublicRoutine, RoutineHandOff, RoutineRunResponse, RoutineStaged, TeammateRoute } from '../shared/ipc.js'
import { handOffPrompt, verdictOf } from '../shared/hand-off.js'
import { isDue, missedSlot, scheduleBase } from '../shared/routine-schedule.js'
import { arrivalNote } from './routine-file-watch.js'
import type { FileArrivals } from './routine-file-watch.js'
import { hookNote, type HookBatch } from './routine-hooks.js'
import type { MissionPeerContext } from './workroom-briefing.js'
import { randomUUID } from 'node:crypto'
import type { RoutineExecution, RoutineRecoveryRequest, RoutineRecoveryResponse } from '../shared/routine-recovery.js'
import type { RoutineStore } from './routine-store.js'
import type { RoutineCopies } from './routine-copy.js'
import { hostReadsEventsOf } from '../shared/runtimes.js'
import { stepTooLongNotice } from '../shared/step-budget.js'
import { resolveValues, substituteSteps } from '../shared/routine-inputs.js'
import type { RoutineValues } from '../shared/routine-inputs.js'

/**
 * Replays a routine: step 1 starts as a new mission for the teammate, and each
 * later step starts as a follow-up of the same conversation once the step
 * before it COMPLETED. Built the way the relay is built -- the host starts
 * runs through the same service a person's do, each recorded as started by
 * the routine (`startedBy: { kind: 'routine', routineId, step }`), and the
 * thread that is being replayed hears every notice.
 *
 * Nothing skips ahead: a step that failed, was stopped, or was interrupted
 * ends the routine there, and the thread says which step and why. Continuing
 * past a failure would build step 3 on work step 2 never did.
 *
 * A routine with a schedule also starts on its own: the host ticks the
 * runner once a minute and a routine whose next run has passed starts
 * exactly as if the person pressed Run -- same path, same record, same
 * "one routine per teammate at a time". A teammate in the middle of a
 * person's run is skipped and tried on the next tick, never queued behind
 * them; a scheduled start that FAILS is held off for an hour rather than
 * retried every minute against a runtime that is down.
 */

export interface RoutineRunnerOptions {
  readonly hooks?: { ready(id: string): Promise<HookBatch | undefined>; fired(id: string, batch: HookBatch): Promise<void>; restore?(id: string, batch: HookBatch): Promise<void> }
  /** W7: the host picker vouches for folder values at dispatch, after the final store read. */
  readonly folderChosen?: (path: string) => boolean
  readonly workspaceId: string
  /**
   * The folder a routine was made in: its own record, or for one saved
   * before that was recorded, the folder of the mission it was learned
   * from. Undefined when neither says; such a routine runs as before (M15).
   */
  readonly homeOf?: (routine: PublicRoutine) => Promise<string | undefined>
  readonly routines: Pick<RoutineStore, 'get' | 'list' | 'recordRun' | 'saveProgress' | 'clearProgress' | 'abandon' | 'keepSchedule'> & {
    /** A slot that passed before this process opened. Absent in tests that do not exercise misses. */
    recordMiss?(routineId: string, dueAt: string, recordedAt: string): Promise<void>
  }
  /**
   * When this process opened. A slot at or before this is missed, not started.
   * Absent means the process has been open for the whole clock the test uses,
   * so a due routine still starts.
   */
  readonly openedAt?: Date
  /** Whether the teammate has a live run of anyone's. A scheduled routine waits for it to end. */
  readonly teammateBusy?: (teammateId: string) => Promise<boolean>
  readonly peerContextFor: (teammateId: string) => Promise<MissionPeerContext | undefined>
  readonly start: (input: {
    readonly prompt: string
    readonly runtime: MissionRuntimeId
    readonly mode: MissionMode
    readonly model: string | undefined
    /** The level the routine was taught with; absent where the route reports none. */
    readonly effort: string | undefined
    readonly account?: string
    readonly peer: MissionPeerContext
    readonly followUpOf: string | undefined
    readonly startedBy: { readonly kind: 'routine'; readonly routineId: string; readonly step: number }
    /** The routine's copy, when it works in one (0.533): the step runs there, not in the folder. */
    readonly cwd?: string
  }) => Promise<CodexMissionStartResponse>
  readonly assignOwner: (teammateId: string, missionId: string) => Promise<void>
  /** How a finished mission ended, from the durable record. Undefined when the ledger cannot say. */
  readonly phaseOf: (missionId: string) => Promise<RecoveredMissionPhase | undefined>
  /**
   * Whether that mission is still RUNNING somewhere in this process.
   *
   * The ledger cannot answer it. `phaseFor` reads the events and returns
   * `interrupted` when it finds no terminal one -- which is exactly what a
   * mission that has not finished yet looks like. So "still going" and
   * "abandoned mid-flight" are the same record, and only the live set can
   * tell them apart.
   */
  readonly isLive?: (missionId: string) => boolean
  /** New files for routines that run "on a new file" (0.522, routine-file-watch.ts). */
  readonly arrivals?: Pick<FileArrivals, 'ready' | 'fired'>
  /**
   * Whether that turn ended by ASKING the person something.
   *
   * A turn that ends on a decision block completes perfectly normally -- exit
   * 0, a receipt, phase 'completed' -- so the runner read it as success and
   * started the next step. The person's answer then arrived at a teammate
   * already working on step 2 and was refused as RUN_ALREADY_ACTIVE, so the
   * routine carried on without the answer it had asked for. Verified
   * 2026-09-08 from a report by a Cursor teammate reading this source.
   *
   * Absent, or a read that fails, means "cannot tell" and now holds for review.
   * Counting an unreadable answer as completion would bypass recovery policy.
   */
  readonly askedAQuestion?: (missionId: string) => Promise<boolean>
  /** A teammate's route now, for a step handed to them (0.435). Absent: only the routine's own teammate can take a step. */
  readonly routeOf?: (teammateId: string) => Promise<TeammateRoute | undefined>
  /** A finished step's answer, for the teammate the next step goes to and for a checker's verdict (0.435). */
  readonly replyOf?: (missionId: string) => Promise<string | undefined>
  readonly notify: (update: CodexMissionUpdate) => void
  /**
   * A routine that works in a copy (0.533, routine-copy.ts): the copy each run
   * makes, and the folder it is made from -- the one the window is in.
   */
  readonly copies?: Pick<RoutineCopies, 'make' | 'path' | 'source' | 'changes' | 'discard'>
  readonly folderNow?: () => string
  /**
   * A standing goal's check (0.534): the folder's own check command, run in
   * `cwd` -- the folder, or the routine's copy. Undefined when the folder has
   * no command set.
   */
  readonly goalCheck?: (cwd: string) => Promise<GoalCheck | undefined>
}

/** What a standing goal's check said (0.534). */
export interface GoalCheck {
  readonly command: string
  readonly passed: boolean
  /** The end of its output, for the teammate asked to fix it and for the person. */
  readonly tail: readonly string[]
}

/** The fix a standing goal asks for, quoting what the check said (0.534). */
export function goalFixPrompt(check: GoalCheck, fix: number, of: number): string {
  const tail = check.tail.length === 0 ? '(it printed nothing)' : check.tail.join('\n')
  return `The check \`${check.command}\` failed after the work above (fix ${String(fix)} of ${String(of)}). The end of its output:\n\n${tail}\n\nFix what it reports in this folder, then say in one or two sentences what you changed.`
}

export interface RoutineProgress {
  readonly routineId: string
  readonly name: string
  readonly teammateId: string
  /** The step now running, counting from 1. */
  readonly step: number
  readonly of: number
  readonly missionId: string
  readonly runId: string
}

export interface RoutineRunner {
  reconcile(): Promise<void>
  recover(request: RoutineRecoveryRequest): Promise<RoutineRecoveryResponse>
  /**
   * `arrived`: the new files a watching routine runs for (0.522), named to step 1.
   * `values` (W7): what a person entered for the routine's inputs. Absent, nobody
   * was asked -- a run on its own -- and the defaults stand.
   */
  run(routineId: string, arrived?: RoutineArrival, values?: RoutineValues): Promise<RoutineRunResponse>
  /** Any run ending, however it ended. Only a routine's own current step moves it. */
  onRunEnded(mission: { readonly missionId: string }): Promise<void>
  running(): readonly RoutineProgress[]
  /**
   * Start every scheduled routine whose next run has passed, as of `now`.
   * Returns the ids started. Safe to call every minute: a routine that is
   * running, or whose teammate is busy, or that failed to start within the
   * last hour, is skipped this tick.
   */
  tick(now: Date): Promise<readonly string[]>
}

/**
 * Whether a refused start means "no room right now" rather than "this is
 * broken". Matched on the host's own sentence, which names the cap.
 */
function isAtCapacity(message: string): boolean {
  return /missions can run at once/i.test(message)
}

/** How long a scheduled routine waits after a start that failed before it is tried again. */
export const SCHEDULE_HOLD_OFF_MS = 3_600_000
type RoutineArrival = { readonly folder: string; readonly files: readonly string[] } | { readonly kind: 'hook'; readonly batch: HookBatch }

/**
 * THE ROUTINE AS THIS RUN WILL SAY IT (W7): every `{{key}}` in its steps
 * replaced by the value settled for it, in main, as text, before any prompt is
 * assembled. The run then keeps THESE steps -- they are what its record holds
 * and what a recovery replays -- so a later edit, or a changed default, never
 * changes a run that has begun. Refused here, before anything starts, when a
 * required input has no value or a step ends up empty or too long to send.
 */
export function fillInputs(routine: PublicRoutine, given: RoutineValues | undefined, folderChosen?: (path: string) => boolean):
  { readonly ok: true; readonly routine: PublicRoutine } | { readonly ok: false; readonly message: string } {
  const inputs = routine.inputs
  if (inputs === undefined || inputs.length === 0) {
    if (given !== undefined && Object.keys(given).length > 0) return { ok: false, message: 'This routine does not ask for any values.' }
    return { ok: true, routine }
  }
  const settled = resolveValues(inputs, given)
  if (!settled.ok) return settled
  for (const input of inputs) {
    const path = settled.values[input.key]
    if (input.kind === 'folder' && path && folderChosen?.(path) !== true) return { ok: false, message: `Choose "${input.label}" with the folder picker before running.` }
  }
  const steps = substituteSteps(routine.steps, settled.values)
  for (const [at, step] of steps.entries()) {
    if (step.trim().length === 0) return { ok: false, message: `Step ${String(at + 1)} is empty once the values are filled in. Nothing was started.` }
    const tooLong = stepTooLongNotice(step)
    if (tooLong !== undefined) return { ok: false, message: `Step ${String(at + 1)} is too long once the values are filled in: ${tooLong} Nothing was started.` }
  }
  return { ok: true, routine: { ...routine, steps } }
}

/**
 * Runtimes the routine runner can start: the ones whose events this host
 * reads through a process it owns. Antigravity is excluded by that same
 * rule, which is the right reason -- it is driven through another service
 * and records no starter.
 *
 * ASKED, NOT LISTED. This was a hand-written set of five ids, and a set
 * like that is how Muse Code came to be pickable in the composer and
 * refused by the mission ledger on the same build. A routine would have
 * been refused here with "Routines cannot run on muse yet", which is a
 * sentence about a list nobody had updated rather than about the runtime.
 */
const canStartRoutine = (runtime: MissionRuntimeId): boolean => hostReadsEventsOf(runtime)

function phaseWords(phase: RecoveredMissionPhase | undefined): string {
  switch (phase) {
    case 'failed':
      return 'that run failed'
    case 'cancelled':
      return 'that run was stopped'
    case 'interrupted':
      return 'that run was interrupted'
    case 'completed':
      return 'that run completed'
    default:
      return 'the record of that run could not be read'
  }
}

/** Who takes a step of a hand-off chain (0.435): its named teammate, or the routine's own. */
const ownerOf = (routine: { readonly teammateId: string; readonly handOffs?: readonly RoutineHandOff[] }, step: number): string =>
  routine.handOffs?.[step - 1]?.teammateId ?? routine.teammateId
/** What a step's notice adds when it was handed on (0.435): who has it, with whose answer, and whether they check. */
const handedTo = (to: string, from: string | undefined, check: boolean): string =>
  from === undefined
    ? check ? `, ${to} checks it` : ''
    : check ? `, handed to ${to} with ${from}'s answer to check` : `, handed to ${to} with ${from}'s answer`

/** Why a step cannot run: its teammate was removed (R12). Nothing has started when this is said. */
function removedStepSaying(routine: { readonly steps: readonly unknown[]; readonly handOffs?: readonly RoutineHandOff[] }, step: number): string {
  const role = routine.handOffs?.[step - 1]?.check === true ? ', the step that checks the work,' : ''
  return `Step ${String(step)} of ${String(routine.steps.length)}${role} was handed to a teammate who has since been removed, so nothing was started. Edit the routine to choose who takes it.`
}

/** Whether that step is the checker, whose approval the run needs. */
const checks = (routine: { readonly handOffs?: readonly RoutineHandOff[] }, step: number): boolean =>
  routine.handOffs?.[step - 1]?.check === true

export function createRoutineRunner(options: RoutineRunnerOptions): RoutineRunner {
  const active = new Map<string, RoutineProgress>()
  /** routineId -> epoch ms before which a scheduled start is not tried again. */
  const heldOff = new Map<string, number>()
  const changed = (): void => options.notify({ kind: 'routine-recovery-changed' })

  const hold = async (
    routine: PublicRoutine,
    execution: RoutineExecution,
    reason: string,
    canContinue = false,
    settledAtDispatch = false
  ): Promise<void> => {
    if (execution.status === 'held' && execution.reason === reason && execution.canContinue === canContinue) return
    await options.routines.saveProgress(
      routine.routineId,
      { ...execution, status: 'held', reason, canContinue, ...(settledAtDispatch ? { settledAtDispatch: true } : {}) },
      execution.attemptId
    )
    active.delete(routine.routineId)
    changed()
  }

  const nameOf = async (teammateId: string): Promise<string | undefined> =>
    (await options.peerContextFor(teammateId).catch(() => undefined))?.self.name

  /**
   * Whether a CHECKER step's answer approves (0.435). Anything but a clear
   * approval -- changes asked for, no verdict, an answer that cannot be read
   * -- is not one, and says which.
   */
  const checkerSaid = async (missionId: string, teammateId: string): Promise<{ readonly approved: boolean; readonly why: string }> => {
    const verdict = verdictOf(await options.replyOf?.(missionId).catch(() => undefined))
    const name = (await nameOf(teammateId)) ?? 'The checker'
    if (verdict?.approved === true) return { approved: true, why: `approved by ${name}` }
    return {
      approved: false,
      why: verdict === undefined
        ? `${name}, the checker, gave no verdict`
        // Without its own full stop: the notice adds one ("the code.." in the 0.437 frames).
        : `${name}, the checker, asked for changes${verdict.changes.length > 0 ? `: ${verdict.changes.replace(/[.!\s]+$/, '')}` : ''}`
    }
  }

  /**
   * A finished run. In a copy (0.533), what it changed there waits for the
   * person -- recorded in the same write that counts the run -- and a copy
   * that changed nothing is removed. Returns what is waiting, if anything.
   */
  const complete = async (routineId: string, execution: RoutineExecution, failed?: string): Promise<RoutineStaged | undefined> => {
    let staged: RoutineStaged | undefined
    if (execution.inCopy === true && options.copies !== undefined) {
      const changes = await options.copies.changes(routineId).catch(() => undefined)
      const folder = await options.copies.source(routineId).catch(() => undefined)
      if (changes !== undefined && folder !== undefined && changes.changed.length + changes.deleted.length > 0) {
        staged = { attemptId: execution.attemptId, finishedAt: new Date().toISOString(), folder, changed: changes.changed, deleted: changes.deleted }
      }
    }
    await options.routines.recordRun(routineId, execution.attemptId, staged, ...(failed === undefined ? [] : [failed]))
    if (execution.inCopy === true && staged === undefined) await options.copies?.discard(routineId).catch(() => undefined)
    active.delete(routineId)
    changed()
    return staged
  }

  /** "2 files changed in its copy" -- said in the run's own words when it finishes (0.533). */
  const waitingWords = (staged: RoutineStaged): string => {
    const count = staged.changed.length + staged.deleted.length
    return `${String(count)} file${count === 1 ? '' : 's'} changed in its copy, waiting for you under Routines: Keep writes ${count === 1 ? 'it' : 'them'} into the folder, Discard throws ${count === 1 ? 'it' : 'them'} away`
  }

  // Policy (c): ask. A separate runtime can outlive Electron, and a missing
  // receipt is not proof of no side effects. The cost is unattended routines
  // waiting indefinitely for review, so the hold is persisted on their cards.
  // Reconciliation is retried independently of lastRunAt, never the dispatch.
  const reconcile = async (): Promise<void> => {
    for (const routine of await options.routines.list()) {
      const execution = routine.execution
      if (execution === undefined || execution.status === 'abandoned' || active.has(routine.routineId)) continue
      /*
       * A refusal the dispatch already explained is not re-decided.
       *
       * Without this, every reconcile overwrote a precise reason with the
       * generic one below, because there is no mission to ask about -- so
       * the card blamed a restart for a route the runtime had refused
       * outright, and told the person to review external work that never
       * ran. See `settledAtDispatch`.
       */
      if (execution.status === 'held' && execution.settledAtDispatch === true) continue
      if (execution.workspaceId !== options.workspaceId) {
        await hold(routine, execution, 'Open the original workspace to reconcile this attempt. Nothing will be replayed.')
        continue
      }
      /*
       * A mission that is STILL RUNNING is not a mission to review.
       *
       * `phaseFor` returns `interrupted` for any mission with no terminal
       * event, and a run that is a second from finishing has none -- so
       * reconciling during that window held the routine with "Review
       * required: that run was interrupted", and nothing re-checked it. The
       * routine then sat at "waiting for review, 0 completed runs" forever,
       * with both its steps done. Measured 2026-09-11: the same routine
       * passed and failed on an unchanged build depending on whether the
       * terminal event landed first.
       */
      if (execution.missionId !== undefined && options.isLive?.(execution.missionId) === true) continue
      const phase = execution.missionId === undefined ? undefined : await options.phaseOf(execution.missionId).catch(() => undefined)
      const question = execution.missionId === undefined || options.askedAQuestion === undefined
        ? true : await options.askedAQuestion(execution.missionId).catch(() => true)
      const confirmed = phase === 'completed' && !question
      // A checker that finished while the app was closed is read like one
      // that finished while it was open: only an approval lets it count (0.435).
      const verdict = confirmed && execution.missionId !== undefined && checks(execution, execution.step)
        ? await checkerSaid(execution.missionId, ownerOf({ teammateId: routine.teammateId, ...(execution.handOffs === undefined ? {} : { handOffs: execution.handOffs }) }, execution.step))
        : undefined
      if (verdict !== undefined && !verdict.approved) {
        await hold(routine, execution, `${verdict.why}. The run does not count as done. Review the work, then run the routine again.`, false, true)
      } else if (confirmed && execution.step === execution.of) {
        await complete(routine.routineId, execution)
      } else {
        await hold(routine, execution, confirmed
          ? 'The saved step completed. Review the remaining steps before continuing.'
          : execution.missionId === undefined
            ? 'Dispatch outcome is uncertain: the app stopped before saving a mission receipt. Review external work; nothing will be replayed.'
            : `Review required: ${phaseWords(phase)}${question && phase === 'completed' ? ', but it asked a question or its answer could not be checked' : ''}. Nothing will be replayed.`, confirmed)
      }
    }
  }

  /**
   * `info` for how a routine is going -- a step started, handed on, the run
   * finished -- and `warning` (the default) when it stopped or waits for the
   * person. Every routine notice was amber, the colour of "this needs you",
   * even "finished, approved by Sable" (the 0.435 hand-off drive's pictures).
   */
  const notice = (progress: RoutineProgress, message: string, level: 'info' | 'warning' = 'warning'): void => {
    options.notify({ kind: 'relay-notice', runId: progress.runId, missionId: progress.missionId, message, level })
  }

  const startStep = async (
    routine: PublicRoutine,
    peer: MissionPeerContext,
    step: number,
    followUpOf: string | undefined,
    /** The step before, when this one goes to another teammate or checks it (0.435). */
    handedFrom?: { readonly missionId: string; readonly name: string },
    /** The new files a watching routine runs for, said before step 1's own words (0.522). */
    arrived?: RoutineArrival,
    /** A standing goal's fix turn (0.534): its own words, after the last step. */
    fix?: { readonly prompt: string; readonly goalTry: number }
  ): Promise<CodexMissionStartResponse> => {
    const saved = routine.steps[step - 1]
    const hook = (arrived !== undefined && 'kind' in arrived) || routine.execution?.hook === true
    const prompt = fix !== undefined ? fix.prompt : saved === undefined || arrived === undefined || step !== 1 ? saved
      : 'kind' in arrived ? hookNote(arrived.batch, saved) : `${arrivalNote(arrived.folder, arrived.files)}\n\n${saved}`
    if (prompt === undefined) {
      return { ok: false, error: { code: 'RUNTIME_START_FAILED', message: `Routine has no step ${String(step)}.` } }
    }
    /*
     * A STEP TOO LONG TO SEND IS REFUSED HERE, BEFORE ANYTHING IS RECORDED
     * (A5.1).
     *
     * It used to be written down as dispatching, handed to the mission
     * service, refused there ("Enter a mission between 1 and 8,000
     * characters."), and -- from step 2 on -- held as "Dispatch not confirmed
     * ... Review external work before proceeding": a person sent to check
     * for side effects of a step that provably never started. A routine
     * saved before 0.316 can still hold one (the store reads 20,000 and
     * saves 8,000), so the check stays here too.
     */
    const tooLong = stepTooLongNotice(prompt)
    if (tooLong !== undefined) {
      const message = `Step ${String(step)} is too long to send: ${tooLong} Shorten it in Edit, then run the routine again. Nothing was started.`
      const prior = routine.execution
      // Settled at dispatch: the reason is exact, and a later reconcile must
      // not trade it for "The saved step completed ... Continue", which would
      // only run into the same refusal (the 0.316 drive caught exactly that).
      if (step > 1 && prior !== undefined && prior.status !== 'abandoned') await hold(routine, prior, message, false, true)
      return { ok: false, error: { code: 'INVALID_PROMPT', message } }
    }
    /*
     * Approve-each replays now, and draws its cards.
     *
     * This used to refuse. The cards existed only on a second mission service
     * that the composer's own start reached and nothing else did; everything
     * here went through exec, where the mode fell through to `read-only`, so
     * a routine on a teammate saved to "approve each action" ran with no
     * cards and no writes and said neither. Refusing was the honest answer to
     * that. Since every Codex mode runs on one loop with the approval channel
     * attached, the honest answer is to run it: the routine stops at each
     * action and waits for the person, which is what the teammate was saved
     * to do.
     */
    /*
     * A STEP HANDED TO ANOTHER TEAMMATE (0.435) runs on THEIR route -- their
     * runtime, model and mode, as a message to them would -- and is given the
     * answer of the step before, quoted above the step's own words. Refused
     * here, before anything is recorded, when that teammate has no route this
     * host can run.
     */
    const owner = ownerOf(routine, step)
    /*
     * THE ROUTINE'S OWN TEAMMATE ON THEIR ROUTE NOW (0.493), as every other
     * step's teammate already was. Colin, 2026-09-30: Robin's routine ran Grok
     * 4.6 "even tho robin is assigned to 4.7, prob because 4.6 was assigned
     * when i made the routine". Runtime, model and effort are the teammate's
     * current ones; the MODE stays the one it was saved with -- a routine
     * saved read-only stays read-only, whatever the teammate is set to later.
     */
    const current = owner === routine.teammateId ? await options.routeOf?.(owner).catch(() => undefined) : undefined
    const route = owner === routine.teammateId
      ? current === undefined ? routine.route : { ...current, mode: routine.route.mode }
      : await options.routeOf?.(owner).catch(() => undefined)
    if (route === undefined || !canStartRoutine(route.runtime)) {
      const message = route === undefined
        ? `Step ${String(step)} goes to ${peer.self.name}, whose route could not be read. Nothing was started.`
        : `Step ${String(step)} goes to ${peer.self.name}, whose runtime routines cannot run on yet. Nothing was started.`
      const held = routine.execution
      if (step > 1 && held !== undefined && held.status !== 'abandoned') await hold(routine, held, message, false, true)
      return { ok: false, error: { code: 'INVALID_PROMPT', message } }
    }
    const sent = fix !== undefined || (handedFrom === undefined && !checks(routine, step))
      ? prompt
      : handOffPrompt({
          step: prompt,
          ...(handedFrom === undefined ? {} : { from: { name: handedFrom.name, answer: await options.replyOf?.(handedFrom.missionId).catch(() => undefined) } }),
          check: checks(routine, step),
          // A checker is told what the chain was for, not only what came last.
          ...(step > 1 && routine.steps[0] !== undefined ? { task: routine.steps[0] } : {})
        })
    const prior = routine.execution
    const intent: RoutineExecution = {
      ...(hook ? { hook: true as const } : {}),
      attemptId: prior?.status === 'abandoned' || prior === undefined ? randomUUID() : prior.attemptId,
      status: 'dispatching', step, of: routine.steps.length, steps: routine.steps, route: routine.route,
      ...(routine.handOffs === undefined ? {} : { handOffs: routine.handOffs }),
      ...(routine.inCopy === true ? { inCopy: true as const } : {}),
      ...(routine.untilCheck === undefined ? {} : { untilCheck: { tries: routine.untilCheck.tries } }),
      ...(fix === undefined ? {} : { goalTry: fix.goalTry }),
      workspaceId: options.workspaceId,
      ...(prior?.recovered === true ? { recovered: true } : {}),
      startedAt: prior?.status === 'abandoned' || prior === undefined ? new Date().toISOString() : prior.startedAt,
      updatedAt: new Date().toISOString(), ...(followUpOf === undefined ? {} : { followUpOf })
    }
    // Persist BEFORE spawn. If spawn succeeds but recording its id fails, the
    // intent remains uncertain; restart must not replay it as an unstarted step.
    await options.routines.saveProgress(routine.routineId, intent, prior?.status === 'abandoned' ? null : prior?.attemptId ?? null)
    if (arrived !== undefined && 'kind' in arrived) await options.hooks?.fired(routine.routineId, arrived.batch)
    let response: CodexMissionStartResponse
    try {
      response = await options.start({
        prompt: sent,
        runtime: route.runtime,
        mode: hook ? 'ask' : route.mode,
        model: route.model === 'account-default' ? undefined : route.model,
        // A routine replays the turns you typed, and how hard the model was
        // asked to think is part of how it ran: one saved at `high` that
        // replays at the runtime's default is not the same routine.
        effort: route.effort,
        ...(route.account === undefined ? {} : { account: route.account }),
        peer,
        followUpOf,
        startedBy: { kind: 'routine', routineId: routine.routineId, step },
        ...(routine.inCopy === true && options.copies !== undefined ? { cwd: options.copies.path(routine.routineId) } : {})
      })
      if (response.ok) {
        await options.routines.saveProgress(routine.routineId, { ...intent, status: 'running',
          missionId: response.data.missionId, runId: response.data.runId }, intent.attemptId)
      } else if (step === 1 && (prior === undefined || prior.status === 'abandoned')
        && ['INVALID_PROMPT', 'RUN_ALREADY_ACTIVE', 'CODEX_UNAVAILABLE', 'SPEND_LIMIT_REACHED', 'POLICY_REFUSED'].includes(response.error.code)) {
        // Only preflight refusals prove no dispatch. RUNTIME_START_FAILED and
        // PERSISTENCE_FAILED can also come from the spawn boundary; without an
        // id they remain uncertain, even though the service returned an error.
        await options.routines.clearProgress(routine.routineId, intent.attemptId)
        if (arrived !== undefined && 'kind' in arrived) await options.hooks?.restore?.(routine.routineId, arrived.batch)
      } else if (response.error.code === 'SPEND_LIMIT_REACHED') {
        // A later step at the teammate's monthly limit was refused before
        // anything was recorded, which PROVES it never started: said exactly,
        // never as "dispatch not confirmed ... review external work". It can
        // go on once the limit is raised or the month turns.
        await hold(routine, intent, `Step ${String(step)} was not started: ${response.error.message}`, true, true)
      } else {
        await hold(
          routine,
          intent,
          `Dispatch not confirmed for step ${String(step)}: ${response.error.message}. Review external work before proceeding.`,
          false,
          true
        )
      }
    } catch (error) {
      active.delete(routine.routineId)
      changed()
      throw error
    }
    return response
  }

  /** `said`: what the turn was actually asked, when it is not the saved step -- a goal's fix (0.534). */
  const announce = (routine: PublicRoutine, peer: MissionPeerContext, step: number, response: CodexMissionStartResponse & { ok: true }, said?: string): RoutineProgress => {
    const progress: RoutineProgress = {
      routineId: routine.routineId,
      name: routine.name,
      teammateId: peer.self.teammateId,
      step,
      of: routine.steps.length,
      missionId: response.data.missionId,
      runId: response.data.runId
    }
    active.set(routine.routineId, progress)
    options.notify({
      kind: 'mission-started',
      runId: response.data.runId,
      missionId: response.data.missionId,
      teammateId: peer.self.teammateId,
      prompt: said ?? routine.steps[step - 1] ?? '',
      data: response.data,
      startedBy: { kind: 'routine', routineId: routine.routineId, step }
    })
    return progress
  }

  const runner: RoutineRunner = {
    reconcile,
    async recover(request) {
      await reconcile()
      const routine = await options.routines.get(request.routineId)
      const execution = routine?.execution
      if (routine === undefined || execution?.status !== 'held' || execution.attemptId !== request.attemptId || execution.step !== request.step) {
        return { ok: false, error: { message: 'This recovery decision is stale. Reload routines and review the current attempt.' } }
      }
      if (request.decision === 'keep') {
        await options.routines.keepSchedule(routine.routineId, execution.attemptId, new Date().toISOString())
        changed()
        return { ok: true }
      }
      if (request.decision === 'abandon') {
        await options.routines.abandon(routine.routineId, execution.attemptId)
        // Put down: its copy goes with it (0.533). Nothing of it reached the folder.
        if (execution.inCopy === true) await options.copies?.discard(routine.routineId).catch(() => undefined)
        changed()
        return { ok: true }
      }
      if (!execution.canContinue || execution.missionId === undefined) return { ok: false, error: { message: 'The saved step is not confirmed complete. Review its mission and external work; it cannot be safely continued.' } }
      // Recovery uses the saved definition, not edits made four days later --
      // who takes each step included (0.435).
      const { handOffs: _current, inCopy: _now, untilCheck: _goalNow, ...base } = routine
      const saved = { ...base, steps: execution.steps, route: execution.route, ...(execution.handOffs === undefined ? {} : { handOffs: execution.handOffs }), ...(execution.inCopy === true ? { inCopy: true as const } : {}), ...(execution.untilCheck === undefined ? {} : { untilCheck: execution.untilCheck }), execution: { ...execution, recovered: true } }
      const next = execution.step + 1
      const owner = ownerOf(saved, next)
      const before = ownerOf(saved, execution.step)
      if (options.teammateBusy !== undefined && await options.teammateBusy(owner)) return { ok: false, error: { message: `${(await nameOf(owner)) ?? 'This teammate'} is busy. Wait for their current mission to finish.` } }
      const peer = await options.peerContextFor(owner)
      if (peer === undefined) return { ok: false, error: { message: removedStepSaying(saved, next) } }
      const handedFrom = owner !== before || checks(saved, next) ? { missionId: execution.missionId, name: (await nameOf(before)) ?? 'The teammate before' } : undefined
      const response = await startStep(saved, peer, next, owner === before ? execution.missionId : undefined, handedFrom)
      if (!response.ok) return { ok: false, error: { message: response.error.message } }
      await options.assignOwner(peer.self.teammateId, response.data.missionId).catch(() => undefined)
      const started = announce(saved, peer, next, response)
      if (handedFrom !== undefined) notice(started, `Routine "${saved.name}" · step ${String(next)} of ${String(saved.steps.length)}${handedTo(peer.self.name, handedFrom.name, checks(saved, next))}.`, 'info')
      changed()
      return { ok: true }
    },
    async run(routineId, arrived, values) {
      await reconcile()
      const stored = await options.routines.get(routineId)
      if (stored === undefined) {
        return { ok: false, error: { code: 'ROUTINE_REJECTED', message: 'That routine no longer exists.' } }
      }
      // What it asks for, settled before anything else is looked at (W7).
      const filled = fillInputs(stored, values, options.folderChosen)
      if (!filled.ok) return { ok: false, error: { code: 'ROUTINE_REJECTED', message: filled.message } }
      // A hook cannot run a folder check or make a writable copy. Its saved routine is otherwise unchanged.
      const routine: PublicRoutine = arrived !== undefined && 'kind' in arrived
        ? (({ inCopy: _copy, untilCheck: _goal, ...rest }) => ({ ...rest, route: { ...rest.route, mode: 'ask' as const } }))(filled.routine)
        : filled.routine
      if (routine.execution !== undefined && routine.execution.status !== 'abandoned') {
        return { ok: false, error: { code: 'ROUTINE_REJECTED', message: 'This routine is already running or waiting for review. Open its routine card before starting more work.' } }
      }
      if (!canStartRoutine(routine.route.runtime)) {
        return {
          ok: false,
          error: { code: 'ROUTINE_REJECTED', message: `Routines cannot run on ${routine.route.runtime} yet.` }
        }
      }
      const first = ownerOf(routine, 1)
      const already = [...active.values()].find((progress) => progress.teammateId === first)
      if (already !== undefined) {
        return {
          ok: false,
          error: {
            code: 'ROUTINE_REJECTED',
            message: `${already.name} is already running for this teammate (step ${String(already.step)} of ${String(already.of)}). One routine at a time.`
          }
        }
      }
      const peer = await options.peerContextFor(first)
      if (peer === undefined) {
        return { ok: false, error: { code: 'ROUTINE_REJECTED', message: 'The teammate this routine belongs to no longer exists.' } }
      }
      /*
       * EVERY STEP'S TEAMMATE, BEFORE ANY STEP RUNS (QA-2026-09-29 round 2,
       * R12). A chain whose checker had been removed ran its first steps --
       * real spend -- then held with "Review the remaining steps", naming
       * nobody. It is refused before anything starts, saying which step.
       */
      for (let step = 2; step <= routine.steps.length; step += 1) {
        const owner = ownerOf(routine, step)
        if (owner === first || await options.peerContextFor(owner) !== undefined) continue
        return { ok: false, error: { code: 'ROUTINE_REJECTED', message: removedStepSaying(routine, step) } }
      }
      /*
       * IN A COPY (0.533): a fresh copy of the folder for this run -- refused
       * while the last run's changes still wait for Keep or Discard, so two
       * runs' work never stacks up unseen.
       */
      if (routine.inCopy === true) {
        if (routine.staged !== undefined) {
          return { ok: false, error: { code: 'ROUTINE_REJECTED', message: `The last run's changes are waiting under Routines. Keep or Discard them, then run "${routine.name}" again.` } }
        }
        if (options.copies === undefined || options.folderNow === undefined) {
          return { ok: false, error: { code: 'ROUTINE_REJECTED', message: 'This routine works in a copy of the folder, which this window cannot make. Nothing was started.' } }
        }
        try {
          await options.copies.make(routine.routineId, options.folderNow())
        } catch (error) {
          return { ok: false, error: { code: 'ROUTINE_REJECTED', message: `${error instanceof Error ? error.message.replace(/^It answers in a copy of the folder, and/, 'It works in a copy of the folder, and') : 'A copy of the folder could not be made.'} Nothing was started.` } }
        }
      }
      const response = await startStep(routine, peer, 1, undefined, undefined, arrived)
      if (!response.ok) {
        if (routine.inCopy === true) await options.copies?.discard(routine.routineId).catch(() => undefined)
        return { ok: false, error: { code: 'ROUTINE_REJECTED', message: response.error.message } }
      }
      await options.assignOwner(peer.self.teammateId, response.data.missionId).catch(() => undefined)
      const progress = announce(routine, peer, 1, response)
      // What started it, said: the bubble shows the saved step, not the note the run was given (0.522).
      if (arrived !== undefined) {
        if ('kind' in arrived) notice(progress, `Started because a link was called. ${String(arrived.batch.count)} call${arrived.batch.count === 1 ? '' : 's'} folded into this Ask run.`, 'info')
        else {
          const named = arrived.files.map((name) => `${arrived.folder.replace(/\\/g, '/')}/${name}`).join(', ')
          notice(progress, `Started because ${arrived.files.length === 1 ? 'a new file' : `${String(arrived.files.length)} new files`} arrived: ${named}.`, 'info')
        }
      }
      if (routine.steps.length > 1) {
        notice(progress, `Routine "${routine.name}" · step 1 of ${String(routine.steps.length)}. Each next step starts when this one completes.`, 'info')
      }
      return { ok: true, data: { missionId: response.data.missionId, runId: response.data.runId } }
    },

    async onRunEnded(mission) {
      const progress = [...active.values()].find((held) => held.missionId === mission.missionId)
      if (progress === undefined) return
      const phase = await options.phaseOf(mission.missionId).catch(() => undefined)
      const stored = await options.routines.get(progress.routineId)
      const execution = stored?.execution
      if (stored === undefined) {
        active.delete(progress.routineId)
        return
      }
      if (execution === undefined || execution.missionId !== mission.missionId) return
      if (phase !== 'completed') {
        await hold(stored, execution, phaseWords(phase))
        notice(progress, `Routine "${progress.name}" stopped at step ${String(progress.step)} of ${String(progress.of)}: ${phaseWords(phase)}.`)
        return
      }
      // A step that ended by asking something did not finish, whatever its
      // exit code says. Stopping here leaves the decision card answerable: the
      // teammate is free, so the answer starts a run instead of being refused
      // as busy by the step this would otherwise have begun.
      if (options.askedAQuestion === undefined || (await options.askedAQuestion(mission.missionId).catch(() => true))) {
        await hold(stored, execution, 'The step asked you something. Review its mission before proceeding.')
        notice(
          progress,
          /*
           * WHAT RUNNING IT AGAIN WOULD ACTUALLY DO.
           *
           * Astra's acceptance pass, 2026-09-14, finding 1. This said
           * "Answer it, then run the routine again when you are ready" --
           * which reads as "carry on from here" and is not what happens. The
           * attempt is HELD with `canContinue: false`, Routines asks you to
           * acknowledge or abandon it, and a later Run starts at step one.
           *
           * She did not answer and rerun, so no duplicate side effect is
           * claimed -- and that is exactly why the wording matters: a person
           * recovering work that is not repeatable needs to know whether the
           * next action continues or repeats, BEFORE they press it. Durable
           * state that is correct does not excuse instructions that are not.
           */
          `Routine "${progress.name}" stopped at step ${String(progress.step)} of ${String(progress.of)}: it asked you something. Answer it in that mission, then deal with the held attempt under Routines — this attempt cannot be continued, so running the routine again starts from step 1.`
        )
        return
      }
      // A CHECKER step counts only if it approves (0.435); the hold is final,
      // so a restart cannot re-decide it into a completed run.
      let approval = ''
      if (execution.goalTry === undefined && checks(execution, progress.step)) {
        const verdict = await checkerSaid(mission.missionId, progress.teammateId)
        if (!verdict.approved) {
          await hold(stored, execution, `${verdict.why}. The run does not count as done. Review the work, then run the routine again.`, false, true)
          notice(progress, `Routine "${progress.name}" stopped at step ${String(progress.step)} of ${String(progress.of)}: ${verdict.why}.`)
          return
        }
        approval = `, ${verdict.why}`
      }
      /*
       * A STANDING GOAL (0.534): with its steps done, the folder's check runs --
       * in the copy, for a routine that works in one. Passed: done, and said.
       * Failed with fixes left: the same teammate is asked to fix what it says,
       * in the same conversation, and the check runs again when that ends.
       * Out of fixes: said, and not counted as done -- except that a copy's
       * changes still wait for Keep or Discard, since only the person can judge
       * whether work that does not pass is worth keeping.
       */
      if (progress.step >= progress.of && execution.hook !== true && execution.untilCheck !== undefined && options.goalCheck !== undefined) {
        const cwd = execution.inCopy === true && options.copies !== undefined ? options.copies.path(progress.routineId) : options.folderNow?.()
        const checked = cwd === undefined ? undefined : await options.goalCheck(cwd).catch(() => undefined)
        const fixes = execution.goalTry ?? 0
        const allowed = execution.untilCheck.tries
        const after = fixes === 0 ? '' : ` after ${String(fixes)} fix${fixes === 1 ? '' : 'es'}`
        if (checked === undefined) {
          const staged = await complete(progress.routineId, execution)
          notice(progress, `Routine "${progress.name}" finished its steps, but this folder has no check command, so it was not checked. Set one in Settings > Project folder.${staged === undefined ? '' : ` ${waitingWords(staged)}.`}`)
          return
        }
        if (checked.passed) {
          const staged = await complete(progress.routineId, execution)
          notice(progress, `Routine "${progress.name}" finished: the check \`${checked.command}\` passed${after}.${staged === undefined ? '' : ` ${waitingWords(staged)}.`}`, 'info')
          return
        }
        if (fixes < allowed) {
          const current = await options.routines.get(progress.routineId)
          const { handOffs: _now, inCopy: _copyNow, untilCheck: _goalNow, ...base } = current ?? stored
          const plan: PublicRoutine = { ...base, steps: execution.steps, route: execution.route, ...(execution.handOffs === undefined ? {} : { handOffs: execution.handOffs }), ...(execution.inCopy === true ? { inCopy: true as const } : {}), untilCheck: execution.untilCheck }
          const peer = await options.peerContextFor(progress.teammateId)
          if (peer === undefined) {
            await hold(stored, execution, 'The check failed, and the teammate who would fix it is gone.', false, true)
            return
          }
          // The fix turn is shown as what it was asked -- the check's own words -- not as the step again.
          const fixPrompt = goalFixPrompt(checked, fixes + 1, allowed)
          const response = await startStep(plan, peer, progress.of, mission.missionId, undefined, undefined, { prompt: fixPrompt, goalTry: fixes + 1 })
          if (!response.ok) {
            active.delete(progress.routineId)
            notice(progress, `Routine "${progress.name}": the check failed, and the fix could not start: ${response.error.message}`)
            return
          }
          await options.assignOwner(peer.self.teammateId, response.data.missionId).catch(() => undefined)
          // Said once, by the fix turn's own line ("failed, so Locust sent it back", kept with the turn), not again as
          // a notice that a reopened conversation would not have (0.814).
          announce(plan, peer, progress.of, response, fixPrompt)
          return
        }
        const said = `the check \`${checked.command}\` still fails after ${String(fixes)} fix${fixes === 1 ? '' : 'es'}${checked.tail.length === 0 ? '' : `: ${checked.tail.slice(-3).join(' / ')}`}`
        if (execution.inCopy === true) {
          const staged = await complete(progress.routineId, execution, `${said.charAt(0).toUpperCase()}${said.slice(1)}.`.slice(0, 600))
          notice(progress, `Routine "${progress.name}" stopped: ${said}.${staged === undefined ? '' : ` ${waitingWords(staged)} -- whether work that does not pass is worth keeping is yours to judge.`}`)
          return
        }
        await hold(stored, execution, `${said.charAt(0).toUpperCase()}${said.slice(1)}. The run does not count as done. Review the work, then run the routine again.`, false, true)
        notice(progress, `Routine "${progress.name}" stopped: ${said}. It does not count as done.`)
        return
      }
      if (progress.step >= progress.of) {
        const staged = await complete(progress.routineId, execution)
        notice(progress, `Routine "${progress.name}" finished: ${String(progress.of)} step${progress.of === 1 ? '' : 's'} completed${approval}.${staged === undefined ? execution.inCopy === true ? ' Nothing changed in its copy, so there is nothing to keep.' : '' : ` ${waitingWords(staged)}.`}`, 'info')
        return
      }
      /*
       * A RUN KEEPS THE STEPS IT STARTED WITH (0.493). This read the routine
       * as it stood NOW, so an edit made while a run was going changed steps
       * that had not started: Grok's 0.489 pass loosened a checker mid-run and
       * the run ended "APPROVED" on the new wording, with nothing saying so.
       * The attempt saved its plan when it began, as recovery already uses;
       * an edit applies from the next run, and the edit dialog says so.
       */
      const current = await options.routines.get(progress.routineId)
      const startedPlan = (held: PublicRoutine): PublicRoutine => {
        const { handOffs: _now, inCopy: _copyNow, untilCheck: _goalNow, ...base } = held
        return { ...base, steps: execution.steps, route: execution.route, ...(execution.handOffs === undefined ? {} : { handOffs: execution.handOffs }), ...(execution.inCopy === true ? { inCopy: true as const } : {}), ...(execution.untilCheck === undefined ? {} : { untilCheck: execution.untilCheck }) }
      }
      const routine = current === undefined ? undefined : startedPlan(current)
      const next = progress.step + 1
      const owner = routine === undefined ? undefined : ownerOf(routine, next)
      /*
       * HANDED TO A TEAMMATE WHO IS BUSY (0.435): held, not queued and not
       * refused -- the person continues it from the routine card once they
       * are free. Final, so a restart does not re-decide the reason away.
       */
      if (routine !== undefined && owner !== undefined && owner !== progress.teammateId && options.teammateBusy !== undefined
        && (await options.teammateBusy(owner).catch(() => true))) {
        const who = (await nameOf(owner)) ?? 'The next teammate'
        await hold(stored, execution, `Step ${String(next)} is ${who}'s, and ${who} is busy. Continue it here when they are free.`, true, true)
        notice(progress, `Routine "${progress.name}" is waiting after step ${String(progress.step)} of ${String(progress.of)}: step ${String(next)} is ${who}'s, who is busy. Continue it under Routines when they are free.`)
        return
      }
      const peer = routine === undefined || owner === undefined ? undefined : await options.peerContextFor(owner)
      if (routine === undefined || peer === undefined || progress.step >= routine.steps.length) {
        await hold(stored, execution, 'The routine or teammate changed while it was running.')
        notice(progress, `Routine "${progress.name}" stopped after step ${String(progress.step)}: the routine changed while it was running.`)
        return
      }
      // Another teammate's step starts a conversation of its own, given what
      // the step before answered; the same teammate's continues its own.
      const handedFrom = owner !== progress.teammateId || checks(routine, next)
        ? { missionId: mission.missionId, name: (await nameOf(progress.teammateId)) ?? 'The teammate before' }
        : undefined
      const response = await startStep(routine, peer, next, owner === progress.teammateId ? mission.missionId : undefined, handedFrom)
      if (!response.ok) {
        active.delete(progress.routineId)
        notice(progress, `Routine "${progress.name}" stopped before step ${String(next)} of ${String(routine.steps.length)}: ${response.error.message}`)
        return
      }
      await options.assignOwner(peer.self.teammateId, response.data.missionId).catch(() => undefined)
      const started = announce(routine, peer, next, response)
      notice(started, `Routine "${routine.name}" · step ${String(next)} of ${String(routine.steps.length)}${handedTo(peer.self.name, handedFrom?.name, checks(routine, next))}.`, 'info')
    },

    running() {
      return [...active.values()]
    },

    async tick(now) {
      // An end callback can fail on a transient disk error. Retry its durable
      // reconciliation too, rather than leaving this session stuck in RAM.
      for (const progress of [...active.values()]) {
        const phase = await options.phaseOf(progress.missionId).catch(() => undefined)
        // Same rule as reconcile: `interrupted` is also what a run that has
        // not written its terminal event yet looks like, so a live mission is
        // not one that ended.
        if (options.isLive?.(progress.missionId) === true) continue
        if (phase === 'completed' || phase === 'failed' || phase === 'cancelled' || phase === 'interrupted') await runner.onRunEnded({ missionId: progress.missionId })
      }
      await reconcile()
      const started: string[] = []
      const all = await options.routines.list().catch(() => [] as readonly PublicRoutine[])
      for (const routine of all) {
        if (routine.execution !== undefined && routine.execution.status !== 'abandoned') continue
        if (routine.schedule === undefined) continue
        // Paused (0.705): not started, and not missed either -- a pause is the person's choice.
        if (routine.paused === true) continue
        // Its last run's changes still wait for Keep or Discard (0.533): it does not run on
        // its own until they are settled. A watched folder's new files stay ready meanwhile.
        if (routine.staged !== undefined) continue
        // On a new file (0.522): due when the watcher has settled files for it and its hour is not full.
        const arrivedFiles = routine.schedule.kind === 'files' ? options.arrivals?.ready(routine.routineId, now.getTime()) ?? [] : []
        const hookBatch = routine.schedule.kind === 'hook' ? await options.hooks?.ready(routine.routineId) : undefined
        if (routine.schedule.kind === 'hook' ? hookBatch === undefined : routine.schedule.kind === 'files' ? arrivedFiles.length === 0 : !isDue(routine.schedule, scheduleBase(routine), now)) continue
        // M15: on its own, only in the folder it was made in. A routine made
        // for project A replayed its steps, in its write mode, in project B.
        const home = await (options.homeOf ?? (async (entry: PublicRoutine) => entry.workspaceId))(routine).catch(() => undefined)
        if (home !== undefined && home !== options.workspaceId) continue
        // No catch-up. A time that passed while Locust was closed is missed,
        // recorded, and not started. A time that passes while it is open still starts.
        if (routine.schedule.kind !== 'files' && routine.schedule.kind !== 'hook') {
          const slot = missedSlot(routine.schedule, scheduleBase(routine), options.openedAt ?? new Date(0), now)
          if (slot !== undefined) {
            await options.routines.recordMiss?.(routine.routineId, slot.toISOString(), now.toISOString())
            continue
          }
        }
        const until = heldOff.get(routine.routineId)
        if (until !== undefined && until > now.getTime()) continue
        heldOff.delete(routine.routineId)
        // Busy is a wait, not a failure: tried again next tick, no hold-off.
        const first = ownerOf(routine, 1)
        if ([...active.values()].some((progress) => progress.teammateId === first)) continue
        if (options.teammateBusy !== undefined && (await options.teammateBusy(first).catch(() => true))) continue
        const response = await this.run(routine.routineId, hookBatch !== undefined ? { kind: 'hook', batch: hookBatch } : routine.schedule.kind === 'files' ? { folder: routine.schedule.folder, files: arrivedFiles } : undefined)
        if (response.ok) {
          started.push(routine.routineId)
          if (routine.schedule.kind === 'files') options.arrivals?.fired(routine.routineId, arrivedFiles, now.getTime())
        } else if (isAtCapacity(response.error.message)) {
          /*
           * The workspace being full is a WAIT, exactly like the teammate
           * being busy two lines above -- and busy gets no hold-off.
           *
           * It used to fall to the branch below and wait an hour: the same
           * hold-off as a signed-out CLI, for a slot that may free in a
           * minute. And because `heldOff` is in memory, restarting cleared it
           * and retried at once -- so staying open was punished and quitting
           * was rewarded, which is the wrong way round (verified 2026-09-08
           * from a report by a Cursor teammate reading this source).
           *
           * Not announced either. Nobody needs telling that four missions are
           * running; they can see them.
           */
          continue
        } else {
          const latest = await options.routines.get(routine.routineId)
          if (latest?.execution !== undefined && latest.execution.status !== 'abandoned') continue
          const retryAt = now.getTime() + SCHEDULE_HOLD_OFF_MS
          heldOff.set(routine.routineId, retryAt)
          // Said, not just recorded. A scheduled routine is the one kind of
          // run nobody is watching start, so a refusal nobody is told about
          // is a routine that has quietly stopped happening.
          options.notify({
            kind: 'routine-blocked',
            routineId: routine.routineId,
            name: routine.name,
            message: response.error.message,
            retryAt: new Date(retryAt).toISOString()
          })
        }
      }
      return started
    }
  }
  // One transition at a time: duplicate end events, Run, recovery decisions
  // and timer ticks cannot dispatch the same step twice. Store writes also
  // compare attempt ids so stale decisions cannot acknowledge a newer run.
  let queue: Promise<unknown> = Promise.resolve()
  const serial = <T>(action: () => Promise<T>): Promise<T> => {
    const result = queue.then(action, action)
    queue = result.catch(() => undefined)
    return result
  }
  return {
    running: () => runner.running(),
    reconcile: () => serial(() => runner.reconcile()),
    run: (id, arrived, values) => serial(() => runner.run(id, arrived, values)),
    recover: (request) => serial(() => runner.recover(request)),
    onRunEnded: (mission) => serial(() => runner.onRunEnded(mission)),
    tick: (now) => serial(() => runner.tick(now))
  }
}
