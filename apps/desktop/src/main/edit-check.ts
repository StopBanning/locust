import { spawn } from 'node:child_process'

import { releaseProcessTree } from '@teammate/runtime-adapters'

/**
 * A3.3: the person's check, run after a teammate's turn changed files.
 *
 * The command is the PERSON's, set in Settings for this folder -- never read
 * from the project, because a repository that could name a command Locust
 * runs after every turn is the hole 0.341 closed by another door. It runs in
 * the teammate's own folder (its worktree when it has one), bounded in time
 * and output, and what comes back is only what is NEW since the last check
 * there: a failure that was already failing is said once as "still failing
 * the same way", not re-listed as the teammate's doing.
 *
 * Nothing is sent to the teammate from here. The thread shows the result
 * and a person decides whether it goes back (Colin, 2026-09-25: the button,
 * not Claude Code's automatic Stop-hook loop, which can spend turns).
 */
export interface EditCheckResult {
  readonly command: string
  readonly outcome: 'passed' | 'failed' | 'timed-out' | 'could-not-run'
  /** Lines of output that were not in the last check in this folder. */
  readonly newLines: readonly string[]
  /** Failed, and nothing in the failure is new since the last check. */
  readonly unchanged: boolean
  /** No earlier check in this folder to compare with. */
  readonly first: boolean
}

export interface CheckRun {
  readonly exitCode: number | null
  readonly output: string
  readonly timedOut: boolean
  readonly error?: string
}

const CHECK_TIMEOUT_MS = 180_000
const CHECK_OUTPUT_BYTES = 256 * 1024
const MAX_NEW_LINES = 30
const MAX_LINE = 300

export function runCheckCommand(command: string, cwd: string, timeoutMs = CHECK_TIMEOUT_MS): Promise<CheckRun> {
  return new Promise((resolve) => {
    let output = ''
    let timedOut = false
    let settled = false
    // Declared before done(): a spawn that throws at once reaches done() before the timer exists (2026-10-10 sweep).
    let timer: ReturnType<typeof setTimeout> | undefined
    const done = (run: CheckRun): void => {
      if (settled) return
      settled = true
      if (timer !== undefined) clearTimeout(timer)
      resolve(run)
    }
    let child: ReturnType<typeof spawn>
    try {
      // Through the shell: it is a command line the person typed, pipes and all.
      child = spawn(command, { cwd, shell: true, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] })
    } catch (error) {
      done({ exitCode: null, output: '', timedOut: false, error: error instanceof Error ? error.message : 'The check could not be started.' })
      return
    }
    const take = (chunk: Buffer): void => {
      if (output.length < CHECK_OUTPUT_BYTES) output += chunk.toString('utf8')
    }
    child.stdout?.on('data', take)
    child.stderr?.on('data', take)
    child.on('error', (error) => done({ exitCode: null, output, timedOut, error: error.message }))
    child.on('close', (code) => done({ exitCode: code, output, timedOut }))
    timer = setTimeout(() => {
      timedOut = true
      // The whole tree: a test runner's workers outlive its shell.
      void releaseProcessTree(child.pid).then((released) => {
        if (!released) child.kill()
      })
    }, timeoutMs)
  })
}

/**
 * The check's output with the folder's own path read as "." (0.810), however
 * the check spelled it: as Windows writes it, with forward slashes, as Git
 * Bash does (/c/...), escaped inside JSON, or as a file:// address, which
 * Node's test runner prints. A comparison's copies each have a path of their own, so one failure
 * read as a different one in each; and a judge reads these lines, which are
 * never to carry a copy's path or the person's.
 */
export function withoutFolder(output: string, cwd: string, platform: NodeJS.Platform = process.platform): string {
  const root = cwd.replace(/[\\/]+$/, '')
  if (root.length < 3) return output
  const forward = root.replace(/\\/g, '/')
  const slash = forward.startsWith('/') ? '' : '/'
  const spellings = new Set([`file://${slash}${encodeURI(forward)}`, `file://${slash}${forward}`, root, root.replace(/\\/g, '\\\\'), forward])
  const drive = /^([A-Za-z]):\//.exec(forward)
  if (drive !== null) spellings.add(`/${drive[1]!.toLowerCase()}${forward.slice(2)}`)
  // Longest first, so an address is read whole rather than the path inside it; and never part of a longer name.
  const alternatives = [...spellings].sort((left, right) => right.length - left.length).map((spelling) => spelling.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'))
  return output.replace(new RegExp(`(?<![\\w.\\\\/-])(?:${alternatives.join('|')})(?![\\w.-])`, platform === 'win32' ? 'gi' : 'g'), '.')
}

/**
 * The check's output as lines worth comparing. Colour codes and timings are
 * dropped -- "Duration 3.21s" differs on every run and says nothing about
 * what broke -- and the rest is compared exactly.
 */
export function comparableLines(output: string): readonly string[] {
  const seen = new Set<string>()
  const lines: string[] = []
  for (const raw of output.replace(/\u001b\[[0-9;]*[A-Za-z]/g, '').split(/\r?\n/)) {
    // A time that ends a line differs on every run: it is dropped, and the line kept when more than a word for the
    // time is left (0.810) -- Node's test runner ends each test's line with one, "✖ adds (1.67ms)", naming the test.
    const line = raw.replace(/\s+/g, ' ').trim().replace(/\s*\(?\b\d+(?:\.\d+)?\s?(?:ms|s|sec|seconds)\)?$/i, '')
    if (line.length === 0 || /^(?:time|took|duration|elapsed|done in|finished in|in)\s*:?$/i.test(line)) continue
    if (/^(?:start at|duration|time:|finished in|ran \d+ tests? in)\b/i.test(line)) continue
    // Node's test runner's own total, "ℹ duration_ms 76.857": a different figure in every copy and every run.
    if (/^\W*duration_ms\b/i.test(line)) continue
    if (seen.has(line)) continue
    seen.add(line)
    lines.push(line)
  }
  return lines
}

/** The newest turns' checks kept in memory for a judge (0.810); an older one is read from its mission's record. */
const KEPT_TURN_CHECKS = 200

export function createEditCheck(options: {
  /** This folder's command, read fresh each time, or undefined for none. */
  readonly commandFor: () => Promise<string | undefined>
  readonly run?: (command: string, cwd: string) => Promise<CheckRun>
}) {
  const run = options.run ?? ((command: string, cwd: string) => runCheckCommand(command, cwd))
  const last = new Map<string, ReadonlySet<string>>()
  const busy = new Set<string>()
  /*
   * JUDGES SEE THE CHECK (0.810). A comparison's columns each run the check in their own copy after their turn, and a
   * judge asked to read them waits for it first: in the API benchmark of 10/21 every judge gave a refactor that
   * crashed its own tests full marks, and one said the test run "cannot be verified from the material".
   */
  const byTurn = new Map<string, Promise<EditCheckResult | undefined>>()
  const check = async (cwd: string, onStart?: (command: string) => void): Promise<EditCheckResult | undefined> => {
    const command = await options.commandFor().catch(() => undefined)
    if (command === undefined || command.length === 0) return undefined
    // One at a time per folder: two at once would each see the other's files.
    if (busy.has(cwd)) return undefined
    busy.add(cwd)
    try {
      // Said as it starts (0.811): a check can take minutes, and a card that says nothing until then reads as none.
      try {
        onStart?.(command)
      } catch {
        // The window's word is not the check's: it runs all the same.
      }
      const result = await run(command, cwd)
      const lines = comparableLines(withoutFolder(result.output, cwd))
      const before = last.get(cwd)
      last.set(cwd, new Set(lines))
      const first = before === undefined
      const bounded = (list: readonly string[]) => list.slice(-MAX_NEW_LINES).map((line) => (line.length > MAX_LINE ? `${line.slice(0, MAX_LINE - 1)}…` : line))
      if (result.error !== undefined) return { command, outcome: 'could-not-run', newLines: bounded([withoutFolder(result.error, cwd)]), unchanged: false, first }
      if (result.timedOut) return { command, outcome: 'timed-out', newLines: [], unchanged: false, first }
      if (result.exitCode === 0) return { command, outcome: 'passed', newLines: [], unchanged: false, first }
      const fresh = first ? lines : lines.filter((line) => !before.has(line))
      return { command, outcome: 'failed', newLines: bounded(fresh), unchanged: !first && fresh.length === 0, first }
    } finally {
      busy.delete(cwd)
    }
  }
  return {
    /** The check after a turn. Called before the turn's slot is released, so a reader that waits for that finds it. */
    after(cwd: string, missionId?: string, onStart?: (command: string) => void): Promise<EditCheckResult | undefined> {
      const job = check(cwd, onStart)
      if (missionId !== undefined) {
        byTurn.delete(missionId)
        byTurn.set(missionId, job)
        if (byTurn.size > KEPT_TURN_CHECKS) {
          const oldest = byTurn.keys().next()
          if (oldest.done !== true) byTurn.delete(oldest.value)
        }
      }
      return job
    },
    /**
     * The check after this turn, waited for while it runs, at most `timeoutMs`: undefined when none ran after it
     * here, 'running' when it is still going at the end.
     */
    async afterTurn(missionId: string, timeoutMs = CHECK_TIMEOUT_MS + 10_000): Promise<EditCheckResult | 'running' | undefined> {
      const job = byTurn.get(missionId)
      if (job === undefined) return undefined
      let timer: ReturnType<typeof setTimeout> | undefined
      const late = new Promise<'running'>((done) => {
        timer = setTimeout(() => done('running'), timeoutMs)
      })
      try {
        return await Promise.race([job.catch(() => undefined), late])
      } finally {
        if (timer !== undefined) clearTimeout(timer)
      }
    }
  }
}
