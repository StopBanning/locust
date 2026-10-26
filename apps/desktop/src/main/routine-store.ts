import { randomUUID } from 'node:crypto'
import { constants as fsConstants } from 'node:fs'
import { mkdir, open, readFile, unlink } from 'node:fs/promises'
import { isAbsolute, join } from 'node:path'

import { MAX_GOAL_TRIES } from '../shared/ipc.js'
import type { PublicRoutine, RoutineGoal, RoutineHandOff, RoutineHistoryEntry, RoutineStaged, TeammateRoute } from '../shared/ipc.js'
import { slotPassedDuring, validSchedule } from '../shared/routine-schedule.js'
import { inputsRefusal, keptInputs, placeholdersIn, undeclaredPlaceholders, validInputs } from '../shared/routine-inputs.js'
import type { RoutineInput } from '../shared/routine-inputs.js'

/** Every shape a schedule may take, said when one does not read (0.522). */
const SCHEDULE_REFUSAL = 'The schedule is not one Locust can keep: every 1 to 168 hours, daily or on set days at HH:MM, once at a date and time, on a new file in a folder inside the project, or when a link is called.'
import { isTeammateRoute, safeId } from './teammate-store.js'
import type { RoutineExecution } from '../shared/routine-recovery.js'
import { routineAwaitsReview } from '../shared/routine-recovery.js'
import { STEP_BUDGET } from '../shared/step-budget.js'
import { replaceFile } from './replace-file.js'

/**
 * Routines: conversations a person saved as steps a teammate can replay.
 *
 * Kept beside the roster in the profile (`routines.json`), written the way
 * the roster is -- validate everything, write-and-rename, one writer at a
 * time -- because a routine is a person's own words and will be sent to a
 * runtime again later, unread. Anything that would not round-trip cleanly is
 * refused at the door rather than repaired: a step silently rewritten is a
 * step the person did not ask for.
 */

const SCHEMA_VERSION = 1 as const
const MAX_FILE_BYTES = 4 * 1024 * 1024
/** The file exists and cannot be read. Not "no routines": nothing is written over it. */
export const ROUTINES_UNREADABLE = 'ROUTINES_UNREADABLE'
export const MAX_ROUTINES = 64
export const MAX_STEPS = 12
/**
 * What a stored routine may hold when it is READ -- 20,000, as it always was,
 * so a routine saved before 0.316 stays readable. A step is SAVED only up to
 * STEP_BUDGET (A5.1): the message a step becomes can carry about 8,000, and
 * one saved longer never started.
 */
export const MAX_STEP_LENGTH = 20_000

/** Why these steps cannot be saved, or undefined when they can. */
export function stepsTooLongToSave(steps: readonly string[]): string | undefined {
  const index = steps.findIndex((step) => step.length > STEP_BUDGET)
  if (index < 0) return undefined
  return `Step ${String(index + 1)} is ${steps[index]!.length.toLocaleString('en-US')} characters; a step can be at most ${STEP_BUDGET.toLocaleString('en-US')}. Shorten it and save again.`
}
export const MAX_ROUTINE_NAME_LENGTH = 80
export const MAX_LEARNED_FROM = 64

export interface RoutineStore {
  list(): Promise<readonly PublicRoutine[]>
  get(routineId: unknown): Promise<PublicRoutine | undefined>
  create(input: {
    readonly name: unknown
    readonly teammateId: unknown
    readonly route: unknown
    readonly steps: unknown
    readonly learnedFrom: unknown
    readonly schedule?: unknown
    /** The folder it is made in (M15). */
    readonly workspaceId?: unknown
    /** Who takes each step (0.435). */
    readonly handOffs?: unknown
    /** Works in a copy, kept or discarded by the person (0.533). */
    readonly inCopy?: unknown
    /** Keep going until the check passes (0.534). */
    readonly untilCheck?: unknown
    /** What it asks for when it runs (W7). */
    readonly inputs?: unknown
  }): Promise<PublicRoutine>
  /** Corrections: the name, the steps, who takes them, and the schedule (`null` clears it). The teammate, route and provenance stay. */
  update(input: { readonly routineId: unknown; readonly name: unknown; readonly steps: unknown; readonly schedule?: unknown; readonly handOffs?: unknown; readonly mode?: unknown; readonly inCopy?: unknown; readonly untilCheck?: unknown; readonly inputs?: unknown }): Promise<PublicRoutine>
  remove(routineId: unknown): Promise<void>
  /**
   * Count reconciled final completion and clear its matching progress in one write
   * -- with, for a routine that works in a copy, what it changed there (0.533).
   */
  recordRun(routineId: unknown, attemptId: string, staged?: RoutineStaged, failed?: string): Promise<void>
  /** A copy's changes, kept or discarded: the routine stops waiting on them (0.533). */
  settleStaged(routineId: string, attemptId: string): Promise<void>
  saveProgress(routineId: string, progress: RoutineExecution, expectedAttemptId: string | null): Promise<void>
  abandon(routineId: string, attemptId: string): Promise<void>
  /**
   * A held attempt, reviewed and put down, with the schedule kept (0.392).
   * Counted from when the attempt STARTED, so a routine that is due every
   * four hours does not start again the moment it is cleared; not counted as
   * a completed run, because it was not one.
   */
  /** Put the held attempt down and keep the routine, its clock started at `keptAt` (the decision). */
  keepSchedule(routineId: string, attemptId: string, keptAt: string): Promise<void>
  clearProgress(routineId: string, attemptId: string): Promise<void>
  /**
   * A time that passed while Locust was closed. Not a completed run: `runs`
   * stays, and the clock moves to that slot so the next tick does not start it.
   */
  recordMiss(routineId: string, dueAt: string, recordedAt: string): Promise<void>
  recordHookLoss(routineId: string, count: number, at: string): Promise<void>
  /**
   * Pause or resume its schedule (0.705). Resuming records when, so the clock
   * counts from then. Answers the routine as saved, or undefined if it is gone.
   */
  setPaused(routineId: unknown, paused: unknown, at: string): Promise<PublicRoutine | undefined>
  /** Drop every routine of a teammate who is gone; their steps had nobody to run them. */
  removeForTeammate(teammateId: unknown): Promise<void>
}

interface StoredFile {
  readonly schemaVersion: typeof SCHEMA_VERSION
  readonly routines: readonly PublicRoutine[]
}

const EMPTY: StoredFile = { schemaVersion: SCHEMA_VERSION, routines: [] }

/** A name the UI can show back: one line, no control characters, bounded. */
export function validRoutineName(value: unknown): value is string {
  if (typeof value !== 'string') return false
  const trimmed = value.trim()
  if (trimmed.length === 0 || trimmed.length > MAX_ROUTINE_NAME_LENGTH) return false
  // eslint-disable-next-line no-control-regex
  return !/[\u0000-\u001f\u007f]/.test(trimmed)
}

/**
 * Steps are what a person typed: any printable text, newlines included, each
 * bounded, at least one and not more than a person could reasonably review.
 * An empty step is refused: it would start a run with nothing to do.
 */
export function validSteps(value: unknown): value is readonly string[] {
  if (!Array.isArray(value) || value.length === 0 || value.length > MAX_STEPS) return false
  return value.every(
    (step) =>
      typeof step === 'string'
      && step.trim().length > 0
      && step.length <= MAX_STEP_LENGTH
      // eslint-disable-next-line no-control-regex
      && !/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/.test(step)
  )
}

const validWorkspaceId = (value: unknown): value is string => typeof value === 'string' && /^ws_[a-z0-9]{1,64}$/.test(value)

function validLearnedFrom(value: unknown): value is readonly string[] {
  return Array.isArray(value) && value.length <= MAX_LEARNED_FROM && value.every((id) => safeId(id))
}

/**
 * Who takes each step (0.435): one entry per step, each naming a teammate or
 * nobody (the routine's own), and at most marking it a checker. An entry list
 * that does not line up with the steps is not one this can read.
 */
export function validHandOffs(value: unknown, steps: number): value is readonly RoutineHandOff[] {
  return Array.isArray(value) && value.length === steps && value.every((entry) =>
    typeof entry === 'object' && entry !== null && !Array.isArray(entry)
    && Object.keys(entry).every((key) => key === 'teammateId' || key === 'check')
    && ((entry as RoutineHandOff).teammateId === undefined || safeId((entry as RoutineHandOff).teammateId))
    && ((entry as RoutineHandOff).check === undefined || (entry as RoutineHandOff).check === true))
}

/** Stored only when some step goes to another teammate or checks. */
const keptHandOffs = (handOffs: readonly RoutineHandOff[], owner: string): readonly RoutineHandOff[] | undefined => {
  const cleaned = handOffs.map((entry) => ({
    ...(entry.teammateId === undefined || entry.teammateId === owner ? {} : { teammateId: entry.teammateId }),
    ...(entry.check === true ? { check: true as const } : {})
  }))
  return cleaned.some((entry) => entry.teammateId !== undefined || entry.check === true) ? cleaned : undefined
}

/** A standing goal the store will keep: 1 to MAX_GOAL_TRIES fixes (0.534). */
export function validGoal(value: unknown): value is RoutineGoal {
  if (typeof value !== 'object' || value === null) return false
  const tries = (value as { readonly tries?: unknown }).tries
  return typeof tries === 'number' && Number.isInteger(tries) && tries >= 1 && tries <= MAX_GOAL_TRIES
}

/** What a finished copy run left, as the store will keep it (0.533). */
export function validStaged(value: unknown): value is RoutineStaged {
  if (typeof value !== 'object' || value === null) return false
  const item = value as Record<string, unknown>
  const paths = (list: unknown): boolean => Array.isArray(list) && list.length <= 2_000 && list.every((path) => typeof path === 'string' && path.length > 0 && path.length <= 1_024)
  return safeId(item.attemptId)
    && typeof item.finishedAt === 'string' && !Number.isNaN(Date.parse(item.finishedAt))
    && typeof item.folder === 'string' && item.folder.length > 0 && item.folder.length <= 4_096
    && paths(item.changed) && paths(item.deleted)
}

const HISTORY_CAP = 30

function parsedHistory(value: unknown): readonly RoutineHistoryEntry[] | undefined {
  if (!Array.isArray(value)) return undefined
  const entries: RoutineHistoryEntry[] = []
  for (const item of value.slice(-HISTORY_CAP)) {
    if (typeof item !== 'object' || item === null) continue
    const record = item as Record<string, unknown>
    if (record.kind !== 'missed') continue
    if (typeof record.dueAt !== 'string' || Number.isNaN(Date.parse(record.dueAt))) continue
    if (typeof record.recordedAt !== 'string' || Number.isNaN(Date.parse(record.recordedAt))) continue
    entries.push({ kind: 'missed', dueAt: record.dueAt, recordedAt: record.recordedAt, ...(Number.isSafeInteger(record.hookCalls) && Number(record.hookCalls) > 0 ? { hookCalls: Number(record.hookCalls) } : {}) })
  }
  return entries.length === 0 ? undefined : entries
}

function parsedInstant(value: unknown): string | undefined {
  return typeof value === 'string' && !Number.isNaN(Date.parse(value)) ? value : undefined
}

export function parsedRoutine(value: unknown): PublicRoutine | undefined {
  if (typeof value !== 'object' || value === null) return undefined
  const record = value as Record<string, unknown>
  if (!safeId(record.routineId) || !safeId(record.teammateId)) return undefined
  if (!validRoutineName(record.name) || !validSteps(record.steps) || !isTeammateRoute(record.route)) return undefined
  if (!validLearnedFrom(record.learnedFrom)) return undefined
  if (record.inputs !== undefined && inputsRefusal(record.inputs, record.steps) !== undefined) return undefined
  if (typeof record.createdAt !== 'string' || Number.isNaN(Date.parse(record.createdAt))) return undefined
  if (typeof record.runs !== 'number' || !Number.isSafeInteger(record.runs) || record.runs < 0) return undefined
  if (record.lastRunAt !== undefined && (typeof record.lastRunAt !== 'string' || Number.isNaN(Date.parse(record.lastRunAt)))) {
    return undefined
  }
  // A schedule that does not read is dropped, not the routine: the steps
  // are the person's words and outrank a malformed timer.
  const schedule = validSchedule(record.schedule) ? record.schedule : undefined
  const missedAt = parsedInstant(record.missedAt)
  const resumedAt = parsedInstant(record.resumedAt)
  const history = parsedHistory(record.history)
  const route: TeammateRoute = {
    runtime: record.route.runtime,
    model: record.route.model,
    mode: record.route.mode,
    // Carried explicitly. This object is rebuilt field by field, so anything
    // added to TeammateRoute and not named here is dropped in silence -- the
    // shape that once lost a row's effort levels and drew a control with none
    // under a detail line promising five.
    ...(record.route.effort === undefined ? {} : { effort: record.route.effort })
  }
  return {
    routineId: record.routineId,
    name: record.name.trim(),
    teammateId: record.teammateId,
    route,
    steps: [...record.steps],
    learnedFrom: [...record.learnedFrom],
    createdAt: record.createdAt,
    runs: record.runs,
    ...(record.lastRunAt === undefined ? {} : { lastRunAt: record.lastRunAt }),
    ...(schedule === undefined ? {} : { schedule }),
    ...(record.execution === undefined ? {} : { execution: parsedExecution(record.execution, record.steps, route) }),
    // Named, like every field here: this object is rebuilt field by field (M15).
    ...(validWorkspaceId(record.workspaceId) ? { workspaceId: record.workspaceId } : {}),
    // Hand-offs that do not read are dropped, not the routine (0.435).
    ...(validHandOffs(record.handOffs, record.steps.length) ? { handOffs: record.handOffs.map((entry) => ({ ...entry })) } : {}),
    // A copy's waiting changes that do not read are dropped, not the routine (0.533).
    ...(record.inCopy === true ? { inCopy: true as const } : {}),
    // A goal that does not read is dropped, not the routine (0.534).
    ...(validGoal(record.untilCheck) ? { untilCheck: { tries: record.untilCheck.tries } } : {}),
    ...(validStaged(record.staged) ? { staged: { ...record.staged, changed: [...record.staged.changed], deleted: [...record.staged.deleted] } } : {}),
    ...(validFailed(record.lastFailed) ? { lastFailed: record.lastFailed } : {}),
    ...(missedAt === undefined ? {} : { missedAt }),
    // Paused only by a literal true (0.705): anything else goes on its own, as before.
    ...(record.paused === true ? { paused: true as const } : {}),
    ...(resumedAt === undefined ? {} : { resumedAt }),
    ...(history === undefined ? {} : { history }),
    // Validated above: a corrupt declaration must never become a run without inputs.
    ...(validInputs(record.inputs) && record.inputs.length > 0 ? { inputs: keptInputs(record.inputs) } : {})
  }
}

/** A last failure is a line of the run's own words, bounded (0.536). */
function validFailed(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0 && value.length <= 600
}

function parsedExecution(value: unknown, steps: readonly string[], route: TeammateRoute): RoutineExecution {
  const item = (typeof value === 'object' && value !== null ? value : {}) as Record<string, unknown>
  if (safeId(item.attemptId) && ['dispatching', 'running', 'held', 'abandoned'].includes(String(item.status))
    && Number.isInteger(item.step) && Number(item.step) >= 1 && validSteps(item.steps)
    && item.of === item.steps.length && Number(item.step) <= Number(item.of) && isTeammateRoute(item.route) && safeId(item.workspaceId)
    && typeof item.startedAt === 'string' && !Number.isNaN(Date.parse(item.startedAt))
    && typeof item.updatedAt === 'string' && !Number.isNaN(Date.parse(item.updatedAt))
    && [item.missionId, item.runId, item.followUpOf].every((id) => id === undefined || safeId(id))
    && (item.reason === undefined || (typeof item.reason === 'string' && item.reason.length <= 4000))
    && (item.canContinue === undefined || typeof item.canContinue === 'boolean')
    && (item.recovered === undefined || typeof item.recovered === 'boolean')
    && (item.settledAtDispatch === undefined || typeof item.settledAtDispatch === 'boolean')
    && (item.inCopy === undefined || item.inCopy === true)
    && (item.untilCheck === undefined || validGoal(item.untilCheck))
    && (item.goalTry === undefined || (Number.isInteger(item.goalTry) && Number(item.goalTry) >= 1 && Number(item.goalTry) <= MAX_GOAL_TRIES))) {
    // A receipt's hand-offs that do not read are dropped, never the receipt (0.435).
    if (item.handOffs !== undefined && !validHandOffs(item.handOffs, (item.steps as readonly string[]).length)) {
      const { handOffs: _dropped, ...rest } = item
      return rest as unknown as RoutineExecution
    }
    return item as unknown as RoutineExecution
  }
  // Losing an invalid receipt must never turn uncertain side effects into a
  // routine eligible for automatic replay. Keep a visible, non-resumable hold.
  return { attemptId: 'invalid_receipt', status: 'held', step: 1, of: steps.length,
    startedAt: '1970-01-01T00:00:00.000Z', updatedAt: '1970-01-01T00:00:00.000Z', steps, route,
    workspaceId: 'unknown', reason: 'The saved execution receipt is invalid. Review external work before abandoning this attempt.', canContinue: false }
}

/**
 * A ROUTINE that does not parse is dropped rather than repaired. A FILE that
 * does not parse is unreadable -- it used to read as empty, "the same call
 * the roster store made", and the roster store's call was the bug: every
 * write reads first, so empty-on-failure is overwrite-on-failure.
 */
export function parsedFile(text: string): StoredFile {
  let value: unknown
  try {
    value = JSON.parse(text)
  } catch {
    throw new Error(ROUTINES_UNREADABLE)
  }
  if (typeof value !== 'object' || value === null) throw new Error(ROUTINES_UNREADABLE)
  const record = value as Record<string, unknown>
  if (record.schemaVersion !== SCHEMA_VERSION || !Array.isArray(record.routines)) throw new Error(ROUTINES_UNREADABLE)
  const routines: PublicRoutine[] = []
  const seen = new Set<string>()
  for (const entry of record.routines) {
    const routine = parsedRoutine(entry)
    if (routine === undefined || seen.has(routine.routineId)) continue
    seen.add(routine.routineId)
    routines.push(routine)
    if (routines.length >= MAX_ROUTINES) break
  }
  return { schemaVersion: SCHEMA_VERSION, routines }
}

export function createRoutineStore(options: { readonly rootDirectory: string }): RoutineStore {
  const rootDirectory = options.rootDirectory
  if (!isAbsolute(rootDirectory)) throw new Error('Routine store directory is invalid')
  const path = join(rootDirectory, 'routines.json')

  let queue: Promise<unknown> = Promise.resolve()
  const serialize = <T>(task: () => Promise<T>): Promise<T> => {
    const next = queue.then(task, task)
    queue = next.then(
      () => undefined,
      () => undefined
    )
    return next
  }

  const read = async (): Promise<StoredFile> => {
    let text: string
    try {
      text = await readFile(path, 'utf8')
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') return EMPTY
      throw new Error(ROUTINES_UNREADABLE)
    }
    if (Buffer.byteLength(text, 'utf8') > MAX_FILE_BYTES) throw new Error(ROUTINES_UNREADABLE)
    return parsedFile(text)
  }

  const write = async (file: StoredFile): Promise<void> => {
    // Progress includes a saved definition. Refuse growth before committing a
    // file our bounded reader would subsequently treat as empty.
    if (Buffer.byteLength(JSON.stringify(file, null, 2) + '\n', 'utf8') > MAX_FILE_BYTES) throw new Error('Routine store is full; reduce saved routines before starting more work.')
    await mkdir(rootDirectory, { recursive: true, mode: 0o700 })
    const temporary = `${path}.${randomUUID()}.tmp`
    const handle = await open(temporary, fsConstants.O_WRONLY | fsConstants.O_CREAT | fsConstants.O_EXCL, 0o600)
    try {
      await handle.writeFile(`${JSON.stringify(file, null, 2)}\n`, 'utf8')
      await handle.sync()
    } finally {
      await handle.close()
    }
    try {
      await replaceFile(temporary, path)
    } catch (error) {
      await unlink(temporary).catch(() => undefined)
      throw error
    }
  }

  return {
    list(): Promise<readonly PublicRoutine[]> {
      return serialize(async () => (await read()).routines)
    },

    get(routineId): Promise<PublicRoutine | undefined> {
      return serialize(async () => {
        if (!safeId(routineId)) return undefined
        return (await read()).routines.find((routine) => routine.routineId === routineId)
      })
    },

    create(input): Promise<PublicRoutine> {
      return serialize(async () => {
        if (!validRoutineName(input.name)) throw new Error('Routine name is invalid')
        if (!safeId(input.teammateId)) throw new Error('Teammate id is invalid')
        if (!isTeammateRoute(input.route)) throw new Error('Routine route is invalid')
        if (!validSteps(input.steps)) throw new Error(`Routine steps are invalid: 1 to ${String(MAX_STEPS)} non-empty steps`)
        const tooLongToSave = stepsTooLongToSave(input.steps)
        if (tooLongToSave !== undefined) throw new Error(tooLongToSave)
        if (!validLearnedFrom(input.learnedFrom)) throw new Error('Routine provenance is invalid')
        if (input.schedule !== undefined && !validSchedule(input.schedule)) {
          throw new Error(SCHEDULE_REFUSAL)
        }
        if (input.handOffs !== undefined && !validHandOffs(input.handOffs, input.steps.length)) throw new Error('Who takes each step does not line up with the steps')
        const handOffs = input.handOffs === undefined ? undefined : keptHandOffs(input.handOffs, input.teammateId)
        if (input.untilCheck !== undefined && !validGoal(input.untilCheck)) throw new Error('Routine goal is invalid')
        // Every {{key}} a step writes must be declared as an input (W7).
        const askedFor = input.inputs === undefined ? [] : input.inputs
        const inputsSaying = inputsRefusal(askedFor, input.steps)
        if (inputsSaying !== undefined) throw new Error(inputsSaying)
        const file = await read()
        if (file.routines.length >= MAX_ROUTINES) throw new Error('Too many routines')
        const routine: PublicRoutine = {
          routineId: `rt_${randomUUID().replace(/-/g, '').slice(0, 24)}`,
          name: input.name.trim(),
          teammateId: input.teammateId,
          route: {
            runtime: input.route.runtime,
            model: input.route.model,
            mode: input.route.mode,
            ...(input.route.effort === undefined ? {} : { effort: input.route.effort })
          },
          steps: [...input.steps],
          learnedFrom: [...input.learnedFrom],
          createdAt: new Date().toISOString(),
          runs: 0,
          ...(input.schedule === undefined ? {} : { schedule: input.schedule }),
          ...(validWorkspaceId(input.workspaceId) ? { workspaceId: input.workspaceId } : {}),
          ...(handOffs === undefined ? {} : { handOffs }),
          ...(input.inCopy === true ? { inCopy: true as const } : {}),
          ...(validGoal(input.untilCheck) ? { untilCheck: { tries: input.untilCheck.tries } } : {}),
          ...((askedFor as readonly RoutineInput[]).length > 0 ? { inputs: keptInputs(askedFor as readonly RoutineInput[]) } : {})
        }
        await write({ ...file, routines: [...file.routines, routine] })
        return routine
      })
    },

    update(input): Promise<PublicRoutine> {
      return serialize(async () => {
        if (!safeId(input.routineId)) throw new Error('Routine id is invalid')
        if (!validRoutineName(input.name)) throw new Error('Routine name is invalid')
        if (!validSteps(input.steps)) throw new Error(`Routine steps are invalid: 1 to ${String(MAX_STEPS)} non-empty steps`)
        const tooLongToSave = stepsTooLongToSave(input.steps)
        if (tooLongToSave !== undefined) throw new Error(tooLongToSave)
        const file = await read()
        const held = file.routines.find((routine) => routine.routineId === input.routineId)
        if (held === undefined) throw new Error('Routine not found')
        if (input.schedule !== undefined && input.schedule !== null && !validSchedule(input.schedule)) {
          throw new Error(SCHEDULE_REFUSAL)
        }
        if (input.handOffs !== undefined && !validHandOffs(input.handOffs, input.steps.length)) throw new Error('Who takes each step does not line up with the steps')
        // A new mode for its runs (0.530), checked as a whole route is on create.
        const route = input.mode === undefined ? held.route : { ...held.route, mode: input.mode }
        if (!isTeammateRoute(route)) throw new Error('Routine route is invalid')
        // In a copy or not (0.533): given, as given; not given, as it was.
        const { schedule: _held, handOffs: heldHandOffs, inCopy: heldInCopy, untilCheck: heldGoal, inputs: heldInputs, ...base } = { ...held, route }
        /*
         * WHAT IT ASKS FOR (W7): given, as given; null, nothing; not given, as it
         * was. Every {{key}} the steps now write must be declared -- except a
         * marker a routine saved before this already held, which stays the plain
         * text it always was, so editing its schedule is not refused over it.
         */
        if (input.inputs !== undefined && input.inputs !== null) {
          const saying = inputsRefusal(input.inputs)
          if (saying !== undefined) throw new Error(saying)
        }
        const nextInputs: readonly RoutineInput[] | undefined = input.inputs === undefined
          ? heldInputs
          : input.inputs === null || (input.inputs as readonly RoutineInput[]).length === 0 ? undefined : keptInputs(input.inputs as readonly RoutineInput[])
        const longstanding = new Set(heldInputs === undefined && nextInputs === undefined ? held.steps.flatMap(placeholdersIn) : [])
        const introduced = undeclaredPlaceholders(input.steps as readonly string[], nextInputs).filter((key) => !longstanding.has(key))
        if (introduced.length > 0) {
          const at = (input.steps as readonly string[]).findIndex((step) => placeholdersIn(step).includes(introduced[0] as string))
          throw new Error(`Step ${String(at + 1)} uses {{${introduced[0] as string}}}, which is not declared as an input. Add an input with the key ${introduced[0] as string}, or take the braces out.`)
        }
        const inCopy = input.inCopy === undefined ? heldInCopy === true : input.inCopy === true
        // A goal (0.534): given, as given; null, none; not given, as it was.
        if (input.untilCheck !== undefined && input.untilCheck !== null && !validGoal(input.untilCheck)) throw new Error('Routine goal is invalid')
        const goal = input.untilCheck === undefined ? heldGoal : input.untilCheck === null ? undefined : { tries: (input.untilCheck as RoutineGoal).tries }
        const rest = { ...base, ...(inCopy ? { inCopy: true as const } : {}), ...(goal === undefined ? {} : { untilCheck: goal }), ...(nextInputs === undefined ? {} : { inputs: nextInputs }) }
        // Given: as given. Not given: kept only while the steps still line up.
        const handOffs = input.handOffs !== undefined
          ? keptHandOffs(input.handOffs, held.teammateId)
          : heldHandOffs !== undefined && heldHandOffs.length === input.steps.length ? heldHandOffs : undefined
        const next: PublicRoutine = {
          ...rest,
          name: input.name.trim(),
          steps: [...input.steps],
          ...(handOffs === undefined ? {} : { handOffs }),
          ...(input.schedule === null
            ? {}
            : input.schedule === undefined
              ? (held.schedule === undefined ? {} : { schedule: held.schedule })
              : { schedule: input.schedule })
        }
        await write({
          ...file,
          routines: file.routines.map((routine) => (routine.routineId === next.routineId ? next : routine))
        })
        return next
      })
    },

    remove(routineId): Promise<void> {
      return serialize(async () => {
        if (!safeId(routineId)) throw new Error('Routine id is invalid')
        const file = await read()
        const held = file.routines.find((routine) => routine.routineId === routineId)
        if (held === undefined) return
        if (routineAwaitsReview(held)) {
          throw new Error('That routine is waiting for your review. Check what its last attempt did and abandon it first.')
        }
        await write({ ...file, routines: file.routines.filter((routine) => routine.routineId !== routineId) })
      })
    },

    recordRun(routineId, attemptId, staged, failed): Promise<void> {
      return serialize(async () => {
        if (!safeId(routineId)) return
        const file = await read()
        const held = file.routines.find((routine) => routine.routineId === routineId)
        if (held === undefined) return
        if (held.execution?.attemptId !== attemptId || held.execution.step !== held.execution.of
          || held.execution.missionId === undefined || !['running', 'held'].includes(held.execution.status)) {
          throw new Error('Routine completion receipt changed; reload before continuing.')
        }
        // Counting and clearing share ONE rename: a crash cannot count twice,
        // nor leave a completed routine looking like it only started step 1.
        if (staged !== undefined && !validStaged(staged)) throw new Error('Routine copy changes are invalid')
        const { execution: _execution, lastFailed: _lastFailed, missedAt: _missedAt, ...rest } = held
        const endedAt = new Date()
        // A clock time that passed while this run was going was dropped (0.690):
        // recorded as missed, so the card says so and offers Run now.
        const passed = held.schedule === undefined ? undefined : slotPassedDuring(held.schedule, held.execution.startedAt, endedAt)
        const missed = passed === undefined ? {} : {
          missedAt: passed.toISOString(),
          history: [...(held.history ?? []), { kind: 'missed', dueAt: passed.toISOString(), recordedAt: endedAt.toISOString() } satisfies RoutineHistoryEntry].slice(-HISTORY_CAP)
        }
        const next: PublicRoutine = { ...rest, runs: held.runs + 1, lastRunAt: endedAt.toISOString(), ...missed, ...(staged === undefined ? {} : { staged }), ...(validFailed(failed) ? { lastFailed: failed } : {}) }
        await write({
          ...file,
          routines: file.routines.map((routine) => (routine.routineId === next.routineId ? next : routine))
        })
      })
    },

    saveProgress(routineId, progress, expectedAttemptId) {
      return serialize(async () => {
        const file = await read()
        const held = file.routines.find((routine) => routine.routineId === routineId)
        const current = held?.execution
        if (held === undefined || (expectedAttemptId === null
          ? current !== undefined && current.status !== 'abandoned'
          : current?.attemptId !== expectedAttemptId)) throw new Error('Routine execution changed; reload before continuing.')
        if (parsedExecution(progress, held.steps, held.route).attemptId !== progress.attemptId) throw new Error('Invalid routine progress')
        await write({ ...file, routines: file.routines.map((routine) => routine.routineId === routineId ? { ...held, execution: progress } : routine) })
      })
    },

    settleStaged(routineId, attemptId) {
      return serialize(async () => {
        const file = await read()
        const held = file.routines.find((routine) => routine.routineId === routineId)
        if (held?.staged?.attemptId !== attemptId) throw new Error('Those changes were already kept or discarded; reload Routines.')
        const { staged: _staged, ...rest } = held
        await write({ ...file, routines: file.routines.map((routine) => routine.routineId === routineId ? rest : routine) })
      })
    },

    clearProgress(routineId, attemptId) {
      return serialize(async () => {
        const file = await read()
        const held = file.routines.find((routine) => routine.routineId === routineId)
        if (held?.execution?.attemptId !== attemptId) throw new Error('Routine execution changed.')
        const { execution: _execution, ...rest } = held
        await write({ ...file, routines: file.routines.map((routine) => routine.routineId === routineId ? rest : routine) })
      })
    },

    keepSchedule(routineId, attemptId, keptAt) {
      return serialize(async () => {
        const file = await read()
        const held = file.routines.find((routine) => routine.routineId === routineId)
        if (held?.execution?.attemptId !== attemptId || held.execution.status !== 'held') throw new Error('Routine execution changed; reload before continuing.')
        const { execution, ...rest } = held
        /*
         * THE CLOCK STARTS AT THE DECISION (0.404), as Abandon's does: an
         * acknowledgement is not permission to replay step 1 a minute later.
         * From the attempt's start, a routine held past its interval was due
         * the moment it was kept, and step 1 ran on the next tick -- shown only
         * after the click (the 0.402 beta retest).
         */
        const next: PublicRoutine = { ...rest, lastRunAt: keptAt }
        await write({ ...file, routines: file.routines.map((routine) => (routine.routineId === routineId ? next : routine)) })
      })
    },

    abandon(routineId, attemptId) {
      return serialize(async () => {
        const file = await read()
        const held = file.routines.find((routine) => routine.routineId === routineId)
        if (held?.execution?.attemptId !== attemptId || held.execution.status !== 'held') throw new Error('Routine execution changed; reload before continuing.')
        // Acknowledgement is not permission for the due scheduler to replay
        // step 1 a minute later. Pause the existing schedule in the same write.
        const { schedule: _schedule, ...rest } = held
        const next: PublicRoutine = { ...rest, execution: { ...held.execution, status: 'abandoned', canContinue: false,
          updatedAt: new Date().toISOString(), reason: 'Abandoned by you; schedule removed. This does not stop an external runtime or undo its work.' } }
        await write({ ...file, routines: file.routines.map((routine) => routine.routineId === routineId ? next : routine) })
      })
    },

    recordMiss(routineId, dueAt, recordedAt): Promise<void> {
      return serialize(async () => {
        if (!safeId(routineId) || parsedInstant(dueAt) === undefined || parsedInstant(recordedAt) === undefined) return
        const file = await read()
        const held = file.routines.find((routine) => routine.routineId === routineId)
        if (held === undefined) return
        if (held.execution !== undefined && held.execution.status !== 'abandoned') return
        const entry: RoutineHistoryEntry = { kind: 'missed', dueAt, recordedAt }
        const history = [...(held.history ?? []), entry].slice(-HISTORY_CAP)
        const next: PublicRoutine = { ...held, lastRunAt: dueAt, missedAt: dueAt, history }
        await write({
          ...file,
          routines: file.routines.map((routine) => (routine.routineId === routineId ? next : routine))
        })
      })
    },

    recordHookLoss(routineId, count, at): Promise<void> {
      return serialize(async () => {
        if (!safeId(routineId) || !Number.isSafeInteger(count) || count < 1 || parsedInstant(at) === undefined) return
        const file = await read()
        const held = file.routines.find((routine) => routine.routineId === routineId)
        if (held === undefined) return
        const entry: RoutineHistoryEntry = { kind: 'missed', dueAt: at, recordedAt: at, hookCalls: count }
        await write({ ...file, routines: file.routines.map((routine) => routine.routineId === routineId ? { ...held, history: [...(held.history ?? []), entry].slice(-HISTORY_CAP) } : routine) })
      })
    },

    setPaused(routineId, paused, at): Promise<PublicRoutine | undefined> {
      return serialize(async () => {
        if (!safeId(routineId) || typeof paused !== 'boolean' || parsedInstant(at) === undefined) return undefined
        const file = await read()
        const held = file.routines.find((routine) => routine.routineId === routineId)
        if (held === undefined) return undefined
        const { paused: _paused, ...rest } = held
        const next: PublicRoutine = paused ? { ...rest, paused: true } : { ...rest, resumedAt: at }
        await write({ ...file, routines: file.routines.map((routine) => (routine.routineId === routineId ? next : routine)) })
        return next
      })
    },

    removeForTeammate(teammateId): Promise<void> {
      return serialize(async () => {
        if (!safeId(teammateId)) return
        const file = await read()
        if (!file.routines.some((routine) => routine.teammateId === teammateId)) return
        await write({ ...file, routines: file.routines.filter((routine) => routine.teammateId !== teammateId) })
      })
    }
  }
}
