import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'

import { comparableLines, createEditCheck, runCheckCommand } from './edit-check.js'
import type { CheckRun } from './edit-check.js'

/** A3.3: what a check says after a teammate's turn changed files. */
describe('the check after an edit', () => {
  const scripted = (runs: CheckRun[]) => {
    let at = 0
    return createEditCheck({ commandFor: async () => 'npm test', run: async () => runs[Math.min(at++, runs.length - 1)]! })
  }

  it('says nothing at all when no command is set for the folder', async () => {
    const check = createEditCheck({ commandFor: async () => undefined, run: async () => { throw new Error('must not run') } })
    expect(await check.after('C:/work')).toBeUndefined()
  })

  it('names only what is new since the last check, and says when a failure is the same one', async () => {
    const check = scripted([
      { exitCode: 1, output: 'FAIL a.test.ts\n  expected 1 to be 2\nDuration 3.2s\n', timedOut: false },
      { exitCode: 1, output: 'FAIL a.test.ts\n  expected 1 to be 2\nFAIL b.test.ts\n  cannot read x\nDuration 4.1s\n', timedOut: false },
      { exitCode: 1, output: 'FAIL a.test.ts\n  expected 1 to be 2\nFAIL b.test.ts\n  cannot read x\nDuration 2.0s\n', timedOut: false },
      { exitCode: 0, output: 'all passed\n', timedOut: false }
    ])
    expect(await check.after('C:/work')).toMatchObject({ outcome: 'failed', first: true, newLines: ['FAIL a.test.ts', 'expected 1 to be 2'] })
    expect(await check.after('C:/work')).toMatchObject({ outcome: 'failed', first: false, unchanged: false, newLines: ['FAIL b.test.ts', 'cannot read x'] })
    // Only the timing moved: the same failure, not a new one.
    expect(await check.after('C:/work')).toMatchObject({ outcome: 'failed', unchanged: true, newLines: [] })
    expect(await check.after('C:/work')).toMatchObject({ outcome: 'passed' })
  })

  it('keeps each folder to itself', async () => {
    const check = scripted([{ exitCode: 1, output: 'broken\n', timedOut: false }])
    await check.after('C:/work/a')
    expect(await check.after('C:/work/b')).toMatchObject({ first: true, newLines: ['broken'] })
  })

  // JUDGES SEE THE CHECK (0.811): a comparison's judge waits for each column's check before it reads the column.
  it('lets a judge wait for the check after one turn, and says when none ran after it', async () => {
    let finish: ((run: CheckRun) => void) | undefined
    const check = createEditCheck({ commandFor: async () => 'npm test', run: () => new Promise<CheckRun>((done) => { finish = done }) })
    void check.after('C:/copies/cmp_x-a', 'mission-a')
    // Asked for before its command is even read: the judge finds it at once, and waits.
    const waited = check.afterTurn('mission-a')
    while (finish === undefined) await new Promise((done) => setTimeout(done, 1))
    finish({ exitCode: 1, output: 'FAIL totals.test.mjs\n  summarize is not a function\n', timedOut: false })
    expect(await waited).toMatchObject({ command: 'npm test', outcome: 'failed', newLines: ['FAIL totals.test.mjs', 'summarize is not a function'] })
    expect(await check.afterTurn('mission-b')).toBeUndefined()
  })

  it('says a check still running at the end of the wait is running, not passed', async () => {
    const check = createEditCheck({ commandFor: async () => 'npm test', run: () => new Promise<CheckRun>(() => undefined) })
    void check.after('C:/copies/cmp_x-b', 'mission-b')
    expect(await check.afterTurn('mission-b', 20)).toBe('running')
  })

  it('is found by a judge even when no command is set, as no check at all', async () => {
    const check = createEditCheck({ commandFor: async () => undefined, run: async () => { throw new Error('must not run') } })
    void check.after('C:/copies/cmp_x-c', 'mission-c')
    expect(await check.afterTurn('mission-c')).toBeUndefined()
  })

  it('drops colour codes and timings from what it compares', () => {
    expect(comparableLines('\u001b[31mFAIL\u001b[39m x\n  Duration 3.21s\nTime: 12ms\n12 ms\nok\nok\n')).toEqual(['FAIL x', 'ok'])
  })
})

describe('a check the shell cannot be handed (2026-10-10 sweep)', () => {
  it('says it could not run, rather than throwing', async () => {
    const run = await runCheckCommand('npm test\u0000', tmpdir())
    expect(run.exitCode).toBeNull()
    expect(run.error).toBeDefined()
  })
})

describe('running the command', () => {
  const roots: string[] = []
  afterEach(async () => {
    await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })))
  })

  it('runs it in the folder, through the shell, and reports its exit and output', async () => {
    const root = await mkdtemp(join(tmpdir(), 'locust-check-'))
    roots.push(root)
    const passed = await runCheckCommand('node -e "console.log(require(String.fromCharCode(112,97,116,104)).resolve(String.fromCharCode(46)))"', root)
    expect(passed.exitCode).toBe(0)
    expect(passed.output.trim().toLowerCase()).toContain(root.toLowerCase().split(/[\\/]/).at(-1)!)
    const failed = await runCheckCommand('node -e "console.error(String.fromCharCode(66)+String.fromCharCode(65)+String.fromCharCode(68)); process.exit(3)"', root)
    expect(failed.exitCode).toBe(3)
    expect(failed.output).toContain('BAD')
  }, 30_000)

  it('stops a check that runs past its time', async () => {
    const root = await mkdtemp(join(tmpdir(), 'locust-check-'))
    roots.push(root)
    const run = await runCheckCommand('node -e "setTimeout(() => {}, 60000)"', root, 1_500)
    expect(run.timedOut).toBe(true)
  }, 30_000)
})
