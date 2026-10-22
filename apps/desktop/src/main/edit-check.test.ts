import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'

import { comparableLines, createEditCheck, runCheckCommand, withoutFolder } from './edit-check.js'
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

  it('keeps the whole of a failure, and the check before it, for a fix that follows (0.812)', async () => {
    const check = scripted([
      { exitCode: 0, output: 'ok 4 tests\n', timedOut: false },
      { exitCode: 1, output: 'FAIL a.test.ts\nFAIL b.test.ts\n', timedOut: false },
      { exitCode: 1, output: 'FAIL b.test.ts\n', timedOut: false }
    ])
    const passed = await check.after('C:/work')
    expect(passed).not.toHaveProperty('lines')
    expect(await check.after('C:/work')).toMatchObject({ lines: ['FAIL a.test.ts', 'FAIL b.test.ts'], before: ['ok 4 tests'] })
    // Nothing new, so nothing to show -- but a fix after a fix needs to know what is left.
    expect(await check.after('C:/work')).toMatchObject({ unchanged: true, newLines: [], lines: ['FAIL b.test.ts'], before: ['FAIL a.test.ts', 'FAIL b.test.ts'] })
  })

  it('keeps each folder to itself', async () => {
    const check = scripted([{ exitCode: 1, output: 'broken\n', timedOut: false }])
    await check.after('C:/work/a')
    expect(await check.after('C:/work/b')).toMatchObject({ first: true, newLines: ['broken'] })
  })

  // JUDGES SEE THE CHECK (0.810): a comparison's judge waits for each column's check before it reads the column.
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

  it('says its command as it starts, before it has run, and only when it runs (0.811)', async () => {
    const said: string[] = []
    let finish: ((run: CheckRun) => void) | undefined
    const check = createEditCheck({ commandFor: async () => 'npm test', run: () => new Promise<CheckRun>((done) => { finish = done }) })
    const first = check.after('C:/copies/cmp_y-a', 'mission-a', (command) => said.push(command))
    while (finish === undefined) await new Promise((done) => setTimeout(done, 1))
    expect(said).toEqual(['npm test'])
    // A second turn in the same folder while the first is checked runs no check, so it says none started.
    expect(await check.after('C:/copies/cmp_y-a', 'mission-b', (command) => said.push(`again ${command}`))).toBeUndefined()
    finish({ exitCode: 0, output: '', timedOut: false })
    expect(await first).toMatchObject({ outcome: 'passed' })
    expect(said).toEqual(['npm test'])
    // No command for the folder: nothing starts and nothing is said.
    const none = createEditCheck({ commandFor: async () => undefined, run: async () => { throw new Error('must not run') } })
    await none.after('C:/work', 'mission-c', (command) => said.push(command))
    expect(said).toEqual(['npm test'])
    // A window that throws on the word does not stop the check.
    const loud = createEditCheck({ commandFor: async () => 'npm test', run: async () => ({ exitCode: 0, output: '', timedOut: false }) })
    expect(await loud.after('C:/work', 'mission-d', () => { throw new Error('window gone') })).toMatchObject({ outcome: 'passed' })
  })

  it('is found by a judge even when no command is set, as no check at all', async () => {
    const check = createEditCheck({ commandFor: async () => undefined, run: async () => { throw new Error('must not run') } })
    void check.after('C:/copies/cmp_x-c', 'mission-c')
    expect(await check.afterTurn('mission-c')).toBeUndefined()
  })

  it('reads the folder\'s own path as ".", however the check spelled it, so each copy fails in the same words (0.810)', async () => {
    const copy = String.raw`C:\Users\someone\.locust\compare\cmp_1a2b\a`
    const printed = [
      'not ok 1 - adds two numbers',
      "  location: 'file:///C:/Users/someone/.locust/compare/cmp_1a2b/a/sum.test.mjs:3:1'",
      String.raw`Error: Cannot find module 'c:\users\someone\.locust\compare\cmp_1a2b\a\node_modules\vitest'`,
      'sh: /c/Users/someone/.locust/compare/cmp_1a2b/a/node_modules/.bin/vitest: No such file or directory',
      String.raw`beside it, C:\Users\someone\.locust\compare\cmp_1a2b\ab\x.mjs is another folder`
    ].join('\n')
    expect(withoutFolder(printed, copy, 'win32').split('\n')).toEqual([
      'not ok 1 - adds two numbers',
      "  location: './sum.test.mjs:3:1'",
      String.raw`Error: Cannot find module '.\node_modules\vitest'`,
      'sh: ./node_modules/.bin/vitest: No such file or directory',
      String.raw`beside it, C:\Users\someone\.locust\compare\cmp_1a2b\ab\x.mjs is another folder`
    ])
    // Escaped inside JSON, as a reporter writes a path into it.
    expect(withoutFolder(String.raw`{"file":"C:\\Users\\someone\\.locust\\compare\\cmp_1a2b\\a\\sum.test.mjs"}`, copy, 'win32')).toBe(String.raw`{"file":".\\sum.test.mjs"}`)
    expect(withoutFolder('at file:///home/me/My%20Work/t.mjs and /home/me/My Work/t.mjs', '/home/me/My Work/', 'linux')).toBe('at ./t.mjs and ./t.mjs')
    // Case matters off Windows, and a root too short to be a folder is left alone.
    expect(withoutFolder('/Home/me/t.mjs', '/home/me', 'linux')).toBe('/Home/me/t.mjs')
    expect(withoutFolder('/t.mjs', '/', 'linux')).toBe('/t.mjs')
    // Two copies, one failure: the same words in each.
    const failing = (cwd: string): CheckRun => ({ exitCode: 1, timedOut: false, output: `not ok 1\n  location: 'file:///${cwd.replace(/\\/g, '/')}/sum.test.mjs:3:1'` })
    const check = createEditCheck({ commandFor: async () => 'node --test', run: async (_command, cwd) => failing(cwd) })
    const [a, b] = await Promise.all([check.after(String.raw`C:\copies\a`), check.after(String.raw`C:\copies\b`)])
    expect(a?.newLines).toEqual(['not ok 1', "location: './sum.test.mjs:3:1'"])
    expect(b?.newLines).toEqual(a?.newLines)
  })

  it('drops colour codes and timings from what it compares', () => {
    expect(comparableLines('\u001b[31mFAIL\u001b[39m x\n  Duration 3.21s\nTime: 12ms\n12 ms\nok\nok\n')).toEqual(['FAIL x', 'ok'])
  })

  it('keeps the name of a test that ends in its time, and drops Node\'s total (0.810)', () => {
    // As Node's test runner prints a failure: every figure here differs from one copy, and one run, to the next.
    expect(comparableLines('✖ adds two numbers (1.6709ms)\nℹ tests 1\nℹ duration_ms 76.857\n# duration_ms 12.5\n✓ src/a.test.ts (3 tests) 12ms\n')).toEqual(['✖ adds two numbers', 'ℹ tests 1', '✓ src/a.test.ts (3 tests)'])
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
