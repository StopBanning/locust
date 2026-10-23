import { execFile } from 'node:child_process'
import { randomBytes } from 'node:crypto'
import { homedir } from 'node:os'
import { join } from 'node:path'

import { copyLineChanges, type CopyBringIn } from './compare-copies.js'
import { createRoutineCopies, type RoutineCopies } from './routine-copy.js'

/**
 * SCOUT PREPARES THE FIX (0.828). Duet's edge is that the work is ready when you look; Locust's is for code. When the
 * folder's check starts failing and the person has turned it on, the teammate they picked fixes it in a copy of the
 * folder -- never the folder itself -- and Locust runs the check there again. The card then says the fix is ready,
 * how many files it changed, and that the check passes; or, when it could not, what was missing.
 *
 * WHERE IT WORKS. The copy is a routine's (routine-copy.ts): the folder as it is, uncommitted work included, without
 * git's own store, its dependencies linked rather than copied. A git repository of its own is made in it, with no
 * remote, so git never walks up out of the copy into a repository above it. Locust's command guard refuses
 * `git push`, `git remote` and `gh` in any folder under SCOUT_FIX_ROOT (resources/locust-command-guard.mjs), and the
 * prompt says so. Nothing in the copy reaches the person's folder until they press Apply.
 *
 * LIMITS. One fix at a time, at most SCOUT_FIX_TURNS turns, and SCOUT_FIX_MS in all: past it the turn is stopped and
 * the card says so.
 */
export const SCOUT_FIX_ROOT_ENV = 'LOCUST_SCOUT_FIX_ROOT'
/** Where fixes are prepared: `~/.locust/scout-fixes`, or LOCUST_SCOUT_FIX_ROOT for a drive's own profile. */
export function scoutFixRoot(env: NodeJS.ProcessEnv = process.env): string {
  const set = env[SCOUT_FIX_ROOT_ENV]?.trim()
  return set !== undefined && set.length > 0 ? set : join(homedir(), '.locust', 'scout-fixes')
}
export const SCOUT_FIX_TURNS = 2
export const SCOUT_FIX_MS = 30 * 60_000
/** A fix's copy is named so, and the command guard knows a fix's folder by it. */
export const SCOUT_FIX_PREFIX = 'rt_scoutfix'
export const newFixId = (): string => `${SCOUT_FIX_PREFIX}${randomBytes(6).toString('hex')}`
const isFixId = (fixId: string): boolean => new RegExp(`^${SCOUT_FIX_PREFIX}[a-f0-9]{12}$`).test(fixId)

const indent = (lines: readonly string[]): string[] => (lines.length === 0 ? ['    (it printed nothing)'] : lines.map((line) => `    ${line}`))

/** The first turn: what Scout found, where the teammate is, and what it may not do. */
export function scoutFixPrompt(command: string, lines: readonly string[]): string {
  return [
    'Scout found this folder’s check failing and asked you to prepare a fix before the person looks. You are working in a copy of their folder: nothing you change reaches it until they press Apply, and opening a pull request stays their own step.',
    '',
    `The check \`${command}\` fails. What it printed:`,
    '',
    ...indent(lines),
    '',
    'Fix the cause, then say in one or two sentences what you changed. Do not weaken, skip or delete tests, or change the check itself, to make it pass. Do not push, add a remote, or use gh: none of that is allowed here. If you cannot fix it from what is here, change nothing and say exactly what is missing. Locust runs the check again after your turn.'
  ].join('\n')
}

/** A later turn: the check still fails after the last one. */
export function scoutFixAgainPrompt(command: string, lines: readonly string[], turn: number, of: number): string {
  return [
    `The check \`${command}\` still fails after your change (try ${String(turn)} of ${String(of)}). What it printed:`,
    '',
    ...indent(lines),
    '',
    'Fix what is left, the same way: no weakened tests, no change to the check, no push, no gh. If you cannot, change nothing more and say what is missing.'
  ].join('\n')
}

export type ScoutFixOutcome =
  | { readonly kind: 'ready'; readonly files: readonly string[]; readonly added?: number; readonly removed?: number; readonly missionId: string }
  | { readonly kind: 'unfixed'; readonly why: string; readonly missionId?: string }

/** How a fix's turn ended, as its record says. */
export interface FixTurnEnd {
  readonly phase: 'completed' | 'failed' | 'cancelled' | 'other'
  /** What the turn said last. */
  readonly said?: string
  /** Why it failed, in the runtime's words. */
  readonly failed?: string
  /** It ended by asking the person something. */
  readonly asked?: boolean
}

export interface ScoutFixDeps {
  readonly root?: string
  readonly copies?: RoutineCopies
  /** Runs the folder's check in `cwd`: its command, whether it passed, and the end of what it printed. */
  readonly runCheck: (command: string, cwd: string) => Promise<{ readonly passed: boolean; readonly lines: readonly string[] }>
  /** Starts a fix's turn in its copy, on the teammate's own route, in Edit. */
  readonly start: (input: { readonly fixId: string; readonly teammateId: string; readonly prompt: string; readonly cwd: string; readonly followUpOf?: string }) => Promise<{ readonly missionId: string; readonly runId: string } | { readonly refused: string }>
  readonly stop: (runId: string) => void
  /** How a turn that has ended ended. */
  readonly endOf: (missionId: string) => Promise<FixTurnEnd>
  /** git, for the copy's own repository and its line counts. */
  readonly git?: (args: readonly string[], cwd: string) => Promise<string>
  readonly later?: (task: () => void, ms: number) => unknown
  readonly cancel?: (handle: unknown) => void
  readonly now?: () => number
  readonly fixMs?: number
}

const defaultGit = (args: readonly string[], cwd: string): Promise<string> => new Promise((resolve, reject) => {
  execFile('git', [...args], { cwd, windowsHide: true, timeout: 30_000, maxBuffer: 8 * 1024 * 1024, env: { ...process.env, GIT_TERMINAL_PROMPT: '0' } }, (error, stdout, stderr) => {
    if (error !== null) reject(Object.assign(error, { stdout: String(stdout), stderr: String(stderr) }))
    else resolve(String(stdout))
  })
})

export function createScoutFixes(deps: ScoutFixDeps) {
  const root = deps.root ?? scoutFixRoot()
  const copies = deps.copies ?? createRoutineCopies(root)
  const git = deps.git ?? defaultGit
  const later = deps.later ?? ((task: () => void, ms: number) => setTimeout(task, ms))
  const cancel = deps.cancel ?? ((handle: unknown) => clearTimeout(handle as NodeJS.Timeout))
  const now = deps.now ?? Date.now
  /** A turn being waited on, by its mission. */
  const waiting = new Map<string, () => void>()
  /** Turns that ended before they were waited on: a start answers, and its run may end before the next line. */
  const endedEarly = new Set<string>()
  /** The fix under way: its run, and how to end it early. */
  let running: FixRun | undefined

  /** Until the turn ends, or the time left runs out: whether it ended in time. */
  const untilEnded = (missionId: string, ms: number): Promise<boolean> => new Promise((resolve) => {
    if (endedEarly.delete(missionId)) { resolve(true); return }
    let done = false
    const finish = (ended: boolean): void => {
      if (done) return
      done = true
      waiting.delete(missionId)
      running?.wake.delete(stopNow)
      cancel(timer)
      resolve(ended)
    }
    const stopNow = (): void => { finish(false) }
    const timer = later(() => { finish(false) }, Math.max(0, ms))
    waiting.set(missionId, () => { finish(true) })
    running?.wake.add(stopNow)
  })

  const prepare = async (input: { readonly fixId: string; readonly teammateId: string; readonly name: string; readonly command: string; readonly folder: string }): Promise<ScoutFixOutcome> => {
    if (!isFixId(input.fixId)) return { kind: 'unfixed', why: 'That fix cannot be named so.' }
    if (running !== undefined) return { kind: 'unfixed', why: 'Scout is preparing another fix.' }
    const fix: FixRun = { fixId: input.fixId, wake: new Set() }
    running = fix
    const deadline = now() + (deps.fixMs ?? SCOUT_FIX_MS)
    try {
      let copy: string
      try {
        copy = await copies.make(input.fixId, input.folder)
      } catch (error) {
        return { kind: 'unfixed', why: error instanceof Error ? error.message : 'The folder could not be copied.' }
      }
      // Its own repository, with no remote: git never walks up out of the copy.
      await git(['init', '-q'], copy).catch(() => undefined)
      // The check fails here too, or there is nothing to fix: a check that passes in a fresh copy reads something
      // outside the folder, or failed once by chance.
      let check = await deps.runCheck(input.command, copy)
      if (check.passed) return { kind: 'unfixed', why: `The check passes in a fresh copy of the folder, so there was nothing for ${input.name} to fix. It may depend on something outside the folder, or have failed once by chance.` }
      let last: string | undefined
      for (let turn = 1; turn <= SCOUT_FIX_TURNS; turn += 1) {
        if (fix.stopped !== undefined) return { kind: 'unfixed', why: fix.stopped, ...(last === undefined ? {} : { missionId: last }) }
        const prompt = turn === 1 ? scoutFixPrompt(input.command, check.lines) : scoutFixAgainPrompt(input.command, check.lines, turn, SCOUT_FIX_TURNS)
        const started = await deps.start({ fixId: input.fixId, teammateId: input.teammateId, prompt, cwd: copy, ...(last === undefined ? {} : { followUpOf: last }) })
          .catch((error: unknown) => ({ refused: error instanceof Error ? error.message : 'The turn could not start.' }))
        if ('refused' in started) return { kind: 'unfixed', why: `${input.name} could not start: ${started.refused}`, ...(last === undefined ? {} : { missionId: last }) }
        last = started.missionId
        fix.runId = started.runId
        const inTime = await untilEnded(started.missionId, deadline - now())
        if (!inTime) {
          try { deps.stop(started.runId) } catch { /* it has ended already */ }
          return { kind: 'unfixed', why: fix.stopped ?? `It ran past ${String(Math.round((deps.fixMs ?? SCOUT_FIX_MS) / 60_000))} minutes, so Locust stopped it.`, missionId: last }
        }
        const end = await deps.endOf(started.missionId).catch((): FixTurnEnd => ({ phase: 'other' }))
        const words = end.said === undefined ? undefined : clip(end.said, 240)
        if (end.phase === 'failed') return { kind: 'unfixed', why: `${input.name}’s turn failed${end.failed === undefined ? '.' : `: ${clip(end.failed, 200)}`}`, missionId: last }
        if (end.phase !== 'completed') return { kind: 'unfixed', why: fix.stopped ?? `${input.name}’s turn was stopped.`, missionId: last }
        if (end.asked === true) return { kind: 'unfixed', why: `${input.name} asked something instead${words === undefined ? '.' : `: “${words}”`}`, missionId: last }
        const changes = await copies.changes(input.fixId)
        if (changes.changed.length + changes.deleted.length === 0) {
          return { kind: 'unfixed', why: `${input.name} changed nothing${words === undefined ? '.' : `, and said: “${words}”`}`, missionId: last }
        }
        check = await deps.runCheck(input.command, copy)
        if (check.passed) {
          const files = [...changes.changed, ...changes.deleted].sort()
          const counted = await lineCounts(input.fixId, input.folder)
          return { kind: 'ready', files, ...counted, missionId: last }
        }
      }
      const first = check.lines.find((line) => line.trim().length > 0)
      return { kind: 'unfixed', why: `The check still fails after ${String(SCOUT_FIX_TURNS)} tries${first === undefined ? '.' : `: ${clip(first, 160)}`}`, ...(last === undefined ? {} : { missionId: last }) }
    } finally {
      if (running === fix) running = undefined
    }
  }

  /** "+a -b" the way git counts it, or nothing when git cannot: never a guessed number. */
  const lineCounts = async (fixId: string, folder: string): Promise<{ added?: number; removed?: number }> => {
    const counted = await copyLineChanges({ name: fixId, folder, root, runGit: git }).catch(() => undefined)
    return counted?.added === undefined ? {} : { added: counted.added, removed: counted.removed ?? 0 }
  }

  return {
    prepare,
    /** Into the person's folder, beside their own edits; refused, file by file, where they changed the same files since. */
    apply: async (fixId: string): Promise<CopyBringIn | { readonly kind: 'gone' }> => {
      if (!isFixId(fixId)) return { kind: 'gone' }
      if ((await copies.source(fixId).catch(() => undefined)) === undefined) return { kind: 'gone' }
      const brought = await copies.keep(fixId).catch(() => undefined)
      if (brought === undefined) return { kind: 'gone' }
      if (brought.kind === 'brought') await copies.discard(fixId).catch(() => undefined)
      return brought
    },
    discard: async (fixId: string): Promise<void> => {
      if (isFixId(fixId)) await copies.discard(fixId).catch(() => undefined)
    },
    /** Scout turned off, or Locust closing: the fix under way stops, and says why. */
    stop: (why: string): void => {
      if (running === undefined) return
      running.stopped = why
      if (running.runId !== undefined) {
        try { deps.stop(running.runId) } catch { /* it has ended already */ }
      }
      for (const wake of [...running.wake]) wake()
    },
    busy: (): boolean => running !== undefined,
    /** Every run's end: a fix's turn is waited on here. */
    onRunEnded: async (mission: { readonly missionId: string }): Promise<void> => {
      const wake = waiting.get(mission.missionId)
      if (wake !== undefined) wake()
      else if (running !== undefined) {
        endedEarly.add(mission.missionId)
        if (endedEarly.size > 20) endedEarly.delete(endedEarly.values().next().value as string)
      }
    }
  }
}

export type ScoutFixes = ReturnType<typeof createScoutFixes>

/** The fix under way: its run, why it was stopped early, and the waits to end when it is. */
interface FixRun {
  readonly fixId: string
  runId?: string
  stopped?: string
  readonly wake: Set<() => void>
}

function clip(text: string, max: number): string {
  const flat = text.replace(/\s+/g, ' ').trim()
  return flat.length <= max ? flat : `${flat.slice(0, max - 1).trimEnd()}…`
}
