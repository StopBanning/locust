import { resolve } from 'node:path'

import { describe, expect, it } from 'vitest'

/**
 * A RED GATE SAYS WHY, AND RETRIES ONLY A TIMEOUT (2026-10-05).
 *
 * The gate went red once on four tests that pass alone, and its log kept
 * `grep "×|FAIL"`: the name of each failed test and not the line under it,
 * so a timeout and an assertion looked alike and the reason was gone. The
 * gate now reads vitest's log (_tools/gate-support.mjs), says why every test
 * failed, and runs the failed files once more if -- and only if -- EVERY
 * failure was a timeout.
 *
 * The second half is the dangerous one: a retry that is too willing turns a
 * real failure into a green gate, and the main agent commits on green. So
 * what these pin is mostly what is NOT retried. The logs are the shapes
 * vitest 4 really prints (a timed-out test whose cleanup then fails appears
 * twice under one header; tests that share one hook error are headers in a
 * row and the error once), cut down.
 *
 * The JSON reporter is not used for this because it cannot say: it writes a
 * timeout as `Error: STACK_TRACE_ERROR` and a stack.
 */

interface Row {
  pid: number
  ppid: number
  name: string
  path: string
  cmd: string
  started: string | null
}
interface GateSupport {
  MIN_FREE_BYTES: number
  retryDecision(text: string): { retry: boolean; files: string[]; reason: string }
  failureReport(suite: string, text: string, logPath: string, digestOnly?: boolean): string[]
  processesFromPs(output: string, now?: number): Row[]
  probePosix(): { freeVirtualBytes?: number | undefined; processes: Row[] }
  otherWork(processes: Row[], repository: string, ownPid: number): { vitest: Row[]; builders: Row[]; locusts: Row[] }
  assess(input: { probe: { freeVirtualBytes?: number | undefined; freePhysicalBytes?: number | undefined; processes: Row[] }; repository: string; ownPid: number; scratch: string; entries?: number | undefined; platformName: string }): {
    lines: string[]
    refusals: string[]
    roots: Row[]
    stops: string[]
  }
}
const support = (await import(new URL('../../../../_tools/gate-support.mjs', import.meta.url).href)) as GateSupport

const BAR = '⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯'
const HINT = 'If this is a long-running test, pass a timeout value as the last argument or configure it globally with "testTimeout".'
const timeout = (header: string, ms: number) => ` FAIL  ${header}\nError: Test timed out in ${String(ms)}ms.\n${HINT}\n ❯ ${header.split(' > ')[0]}:12:3\n`
const summary = (tests: number, files: number) => `\n Test Files  ${String(files)} failed | 40 passed (${String(files + 40)})\n      Tests  ${String(tests)} failed | 900 passed (${String(tests + 900)})\n   Duration  172.17s\n`
const PASSED_LINE = ' ✓ src/main/fine.test.ts (3 tests) 12ms'
const failed = (count: number, ...blocks: string[]) => `${PASSED_LINE}\n\n⎯⎯⎯⎯⎯⎯⎯ Failed Tests ${String(count)} ⎯⎯⎯⎯⎯⎯⎯\n\n${blocks.join(`\n${BAR}[x/y]⎯\n\n`)}\n${BAR}[y/y]⎯\n\n`

// The gate of 10-04, as its log read.
const INCIDENT = `${failed(
  5,
  timeout('src/main/a-cloud-session-is-read-in-its-own-worktree.test.ts > reading a cloud session > brought in but no transcript saved', 15_000),
  timeout('src/main/a-conversation-can-be-named.test.ts > bounds, because this file has a size cliff > still lets an existing one be renamed at the cap', 60_000),
  timeout('src/main/a-long-history-says-what-it-lists.test.ts > a long history > past 2,000, says how many the ledger keeps and how many are listed', 120_000),
  timeout('src/main/a-tidy-pass-only-suggests.test.ts > the reader, on a reply written the way models write > says, in the conversation, how many lines it could not read', 5_000),
  timeout('src/main/a-tidy-pass-only-suggests.test.ts > the reader, on a reply written the way models write > says so even when nothing at all could be read', 5_000)
)}${summary(5, 4)}`

// The same shapes as vitest prints them when it colours its output (it does, even into a pipe, when it
// does not think an agent is reading), byte for byte from a real run, cut down.
const ESC = String.fromCharCode(27)
const COLOURED = [
  `${ESC}[31m⎯⎯⎯⎯⎯⎯⎯${ESC}[39m${ESC}[1m${ESC}[41m Failed Tests 1 ${ESC}[49m${ESC}[22m${ESC}[31m⎯⎯⎯⎯⎯⎯⎯${ESC}[39m`,
  '',
  `${ESC}[41m${ESC}[1m FAIL ${ESC}[22m${ESC}[49m src/main/slow.test.ts${ESC}[2m > ${ESC}[22ma loop${ESC}[2m > ${ESC}[22mgoes on and on`,
  `${ESC}[31m${ESC}[1mError${ESC}[22m: Test timed out in 100ms.`,
  `${HINT}${ESC}[39m`,
  `${ESC}[36m ${ESC}[2m❯${ESC}[22m src/main/slow.test.ts:${ESC}[2m7:3${ESC}[22m${ESC}[39m`,
  '',
  `${ESC}[31m${ESC}[2m⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯[1/1]⎯${ESC}[22m${ESC}[39m`,
  '',
  `${ESC}[2m Test Files ${ESC}[22m ${ESC}[1m${ESC}[31m1 failed${ESC}[39m${ESC}[22m${ESC}[90m (1)${ESC}[39m`,
  `${ESC}[2m      Tests ${ESC}[22m ${ESC}[1m${ESC}[31m1 failed${ESC}[39m${ESC}[22m${ESC}[2m | ${ESC}[22m${ESC}[1m${ESC}[32m1 passed${ESC}[39m${ESC}[22m${ESC}[90m (2)${ESC}[39m`
].join(String.fromCharCode(10))

const ASSERTION = ` FAIL  src/main/wrong.test.ts > a sum > adds\nAssertionError: expected 3 to be 4 // Object.is equality\n ❯ src/main/wrong.test.ts:7:19\n`

describe('what is retried', () => {
  it('a gate whose every failure was a timeout is worth one more go, of those files and no others', () => {
    const decision = support.retryDecision(INCIDENT)
    expect(decision.retry).toBe(true)
    expect(decision.files).toEqual([
      'src/main/a-cloud-session-is-read-in-its-own-worktree.test.ts',
      'src/main/a-conversation-can-be-named.test.ts',
      'src/main/a-long-history-says-what-it-lists.test.ts',
      'src/main/a-tidy-pass-only-suggests.test.ts'
    ])
  })

  it('a hook that timed out, with the same words under every test it failed, is a timeout', () => {
    // Three tests, one error: vitest prints the headers in a row and the error once.
    const log = `${failed(3, ' FAIL  src/main/a.test.ts > one\n FAIL  src/main/a.test.ts > two\n FAIL  src/main/a.test.ts > three\nError: Hook timed out in 10000ms.\nIf this is a long-running hook, pass a timeout value as the last argument or configure it globally with "hookTimeout".\n')}${summary(3, 1)}`
    expect(support.retryDecision(log)).toMatchObject({ retry: true, files: ['src/main/a.test.ts'] })
  })

  it('a timeout whose cleanup then hit a locked folder is still the timeout -- the abandoned test was still writing', () => {
    // Printed twice under one header; the second is a consequence of the first.
    const log = `${failed(2, timeout('src/main/slow.test.ts > a loop > goes on', 120_000), ` FAIL  src/main/slow.test.ts > a loop > goes on\nError: ENOTEMPTY: directory not empty, rmdir 'x'\n`)}${summary(1, 1)}`
    expect(support.retryDecision(log)).toMatchObject({ retry: true, files: ['src/main/slow.test.ts'] })
  })
  it('a log vitest coloured is read the same: the colour codes are not part of the words', () => {
    expect(support.retryDecision(COLOURED)).toMatchObject({ retry: true, files: ['src/main/slow.test.ts'] })
  })
})

describe('what is never retried', () => {
  // Why it was not retried -- and that nothing was: no files come back with a refusal.
  const refusal = (log: string): string => {
    const decision = support.retryDecision(log)
    expect(decision.retry).toBe(false)
    expect(decision.files).toEqual([])
    return decision.reason
  }

  it('an assertion', () => {
    expect(refusal(`${failed(1, ASSERTION)}${summary(1, 1)}`)).toMatch(/wrong\.test\.ts > a sum > adds failed, and not by timing out \(assertion\)/)
  })

  it('a timeout in the same run as an assertion: the assertion decides', () => {
    expect(refusal(`${failed(2, timeout('src/main/slow.test.ts > a loop > goes on', 60_000), ASSERTION)}${summary(2, 2)}`)).toMatch(/not by timing out \(assertion\)/)
  })

  it('a cleanup that hit a locked folder with no timeout beside it', () => {
    expect(refusal(`${failed(1, ` FAIL  src/main/a.test.ts > one\nError: EBUSY: resource busy or locked, rmdir 'x'\n`)}${summary(1, 1)}`)).toMatch(/\(locked\)/)
  })

  it('any other error, even one that says "timed out" somewhere in its message', () => {
    expect(refusal(`${failed(1, ` FAIL  src/main/a.test.ts > one\nError: the connector timed out in 3ms\n`)}${summary(1, 1)}`)).toMatch(/\(error\)/)
  })

  it('a failure with no error line vitest printed, which is not known to be a timeout', () => {
    expect(refusal(`${failed(1, ' FAIL  src/main/a.test.ts > one\n ❯ src/main/a.test.ts:3:3\n')}${summary(1, 1)}`)).toMatch(/\(unknown\)/)
  })

  it('a file that would not load, which is not one test timing out', () => {
    expect(refusal(`\n⎯⎯⎯⎯⎯⎯ Failed Suites 1 ⎯⎯⎯⎯⎯⎯⎯\n\n FAIL  src/main/a.test.ts [ src/main/a.test.ts ]\nError: Test timed out in 5000ms.\n${HINT}\n\n${BAR}[1/1]⎯\n${summary(1, 1)}`)).toMatch(/whole file failed/)
  })

  it('timeouts beside an unhandled error', () => {
    expect(refusal(`${failed(1, timeout('src/main/a.test.ts > one', 5_000))}\n⎯⎯⎯⎯⎯⎯ Unhandled Errors ⎯⎯⎯⎯⎯⎯\n\nVitest caught 1 unhandled error during the test run.\n${summary(1, 1)}`)).toMatch(/unhandled/)
  })

  it('a count that does not add up: vitest says three failed and two could be read', () => {
    expect(refusal(`${failed(2, timeout('src/main/a.test.ts > one', 5_000), timeout('src/main/b.test.ts > two', 5_000))}${summary(3, 2)}`)).toMatch(/counted 3 failed test\(s\) but 2 were read/)
  })

  it('a file count that does not add up', () => {
    expect(refusal(`${failed(2, timeout('src/main/a.test.ts > one', 5_000), timeout('src/main/a.test.ts > two', 5_000))}${summary(2, 2)}`)).toMatch(/counted 2 failed file\(s\) but 1 were read/)
  })

  it('a run that never printed its summary: it did not finish', () => {
    expect(refusal(failed(1, timeout('src/main/a.test.ts > one', 5_000)))).toMatch(/no vitest summary/)
  })

  it('a log with no failure in it', () => {
    expect(refusal(`${PASSED_LINE}\n Test Files  41 passed (41)\n      Tests  900 passed (900)\n`)).toMatch(/no failed test could be read/)
  })

  it('a file name that could not be put on a command line as one word', () => {
    expect(refusal(`${failed(1, timeout('src/main/a file.test.ts > one', 5_000))}${summary(1, 1)}`)).toMatch(/command line/)
  })
})

describe('what a red gate says', () => {
  it('names every failed test with the reason vitest gave, as lines a grep for GATE shows', () => {
    const lines = support.failureReport('desktop', `${failed(2, timeout('src/main/slow.test.ts > a loop > goes on', 60_000), ASSERTION)}${summary(2, 2)}`, '/logs/d.log')
    expect(lines[0]).toBe('GATE: desktop: 2 failed test(s) in 2 file(s), 1 of them timeouts:')
    expect(lines).toContain('GATE:   [timeout] src/main/slow.test.ts > a loop > goes on -- Error: Test timed out in 60000ms.')
    expect(lines).toContain('GATE:   [assertion] src/main/wrong.test.ts > a sum > adds -- AssertionError: expected 3 to be 4 // Object.is equality')
    // And vitest's own section, with the code frames, for the detail.
    expect(lines.join('\n')).toContain('⎯⎯⎯⎯⎯⎯⎯ Failed Tests 2 ⎯⎯⎯⎯⎯⎯⎯')
    expect(lines.join('\n')).toContain(' ❯ src/main/wrong.test.ts:7:19')
  })

  it('says a timeout and the cleanup that followed it as one failure', () => {
    const log = `${failed(2, timeout('src/main/slow.test.ts > a loop > goes on', 120_000), ` FAIL  src/main/slow.test.ts > a loop > goes on\nError: ENOTEMPTY: directory not empty, rmdir 'x'\n`)}${summary(1, 1)}`
    const lines = support.failureReport('desktop', log, '/logs/d.log', true)
    expect(lines).toEqual(['GATE: desktop: 1 failed test(s) in 1 file(s), 1 of them timeouts:', "GATE:   [timeout] src/main/slow.test.ts > a loop > goes on -- Error: Test timed out in 120000ms. (then: Error: ENOTEMPTY: directory not empty, rmdir 'x')"])
  })

  it('digest only leaves vitest\'s own section out, which is what a retried gate prints', () => {
    const lines = support.failureReport('desktop', INCIDENT, '/logs/d.log', true)
    expect(lines.some((line) => line.includes('Failed Tests'))).toBe(false)
    expect(lines.filter((line) => line.startsWith('GATE:   [timeout]'))).toHaveLength(5)
  })

  it('says a coloured log in plain words, with no colour code left in a line', () => {
    const lines = support.failureReport('desktop', COLOURED, '/logs/d.log')
    expect(lines).toContain('GATE:   [timeout] src/main/slow.test.ts > a loop > goes on and on -- Error: Test timed out in 100ms.')
    expect(lines.join(String.fromCharCode(10))).not.toContain(ESC)
  })

  it('caps a long report and says where the rest is', () => {
    const many = Array.from({ length: 60 }, (_, index) => timeout(`src/main/f${String(index)}.test.ts > t`, 5_000))
    const lines = support.failureReport('desktop', `${failed(60, ...many)}${summary(60, 60)}`, '/logs/d.log')
    expect(lines.some((line) => line === 'GATE:   ... and 30 more')).toBe(true)
    expect(lines.some((line) => /^\.\.\. \d+ more lines in \/logs\/d\.log$/.test(line))).toBe(true)
  })

  it('says what it can of a log it cannot read a failure out of, rather than nothing', () => {
    const lines = support.failureReport('adapters', 'Error: spawn npx ENOENT\n', '/logs/ra.log')
    expect(lines[0]).toBe('GATE: adapters: no failed test could be read from the log; its last lines:')
    expect(lines).toContain('Error: spawn npx ENOENT')
  })
})

describe('whether the machine is quiet', () => {
  const REPO = resolve('/work/locust')
  const row = (pid: number, ppid: number, name: string, cmd: string, path = ''): Row => ({ pid, ppid, name, path, cmd, started: null })
  const table = [
    row(10, 1, 'explorer.exe', ''),
    // The shell the gate was started from has "vitest" in its own command line. It is not a vitest.
    row(100, 10, 'bash.exe', 'bash -c "npx vitest run"'),
    // And this one IS a vitest, and the gate's own ancestor: not "another".
    row(200, 100, 'node.exe', 'node C:\\npm\\node_modules\\npm\\bin\\npx-cli.js vitest run'),
    row(300, 200, 'bash.exe', 'bash _tools/gate.sh'),
    row(400, 300, 'node.exe', 'node _tools/gate-support.mjs quiet'),
    // Another run: its main process and one worker.
    row(500, 1, 'node.exe', 'node C:\\other\\node_modules\\.pnpm\\vitest@4.1.11\\node_modules\\vitest\\vitest.mjs run'),
    row(501, 500, 'node.exe', 'node --require C:\\other\\node_modules\\.pnpm\\vitest@4.1.11\\node_modules\\vitest\\suppress-warnings.cjs worker'),
    // The packaged app built in this repository, and the installed one, which is not.
    row(600, 1, 'Locust.exe', '', resolve(REPO, 'apps/desktop/release/win-unpacked/Locust.exe')),
    row(601, 1, 'Locust.exe', '', 'C:\\Users\\a\\AppData\\Local\\Programs\\Locust\\Locust.exe'),
    row(700, 1, 'node.exe', `node ${resolve(REPO, 'node_modules/electron-builder/cli.js')}`),
    row(701, 1, 'node.exe', 'node C:\\elsewhere\\node_modules\\electron-builder\\cli.js')
  ]

  it('finds another vitest, a packaged Locust and a build under this repository, and not the gate\'s own shell, ancestors or the installed app', () => {
    const work = support.otherWork(table, REPO, 400)
    expect(work.vitest.map((found) => found.pid)).toEqual([500, 501])
    expect(work.locusts.map((found) => found.pid)).toEqual([600])
    expect(work.builders.map((found) => found.pid)).toEqual([700])
  })

  const quietTable = table.filter((found) => ![500, 501].includes(found.pid))
  const reading = (freeVirtualBytes: number | undefined, processes: Row[]) =>
    support.assess({ probe: { freeVirtualBytes, processes }, repository: REPO, ownPid: 400, scratch: '/scratch', entries: 1200, platformName: 'win32' })

  it('goes ahead on a quiet machine, and says what it found', () => {
    const { lines, refusals } = reading(8 * 1024 ** 3, quietTable)
    expect(refusals).toEqual([])
    expect(lines).toContain('free virtual memory 8.0 GB (the gate refuses under 2.0 GB)')
    expect(lines).toContain('vitest: none running')
    expect(lines).toContain('1,200 entries in the scratch folder (/scratch)')
  })

  it('refuses under 2 GB of free virtual memory, and not at 2 GB', () => {
    expect(reading(support.MIN_FREE_BYTES - 1, quietTable).refusals).toHaveLength(1)
    expect(reading(support.MIN_FREE_BYTES, quietTable).refusals).toEqual([])
  })

  it('refuses while another vitest is running, and offers the root of its tree to stop, not a worker', () => {
    const { refusals, roots, stops } = reading(8 * 1024 ** 3, table)
    expect(refusals).toEqual(['another vitest is running, and two suites at once is how timeouts happen'])
    expect(roots.map((found) => found.pid)).toEqual([500])
    expect(stops).toEqual(['taskkill /PID 500 /T /F'])
  })

  it('says how to stop it in a Mac\'s own words, where there is no taskkill', () => {
    const { stops } = support.assess({ probe: { freeVirtualBytes: undefined, processes: table }, repository: REPO, ownPid: 400, scratch: '/scratch', entries: 1200, platformName: 'darwin' })
    expect(stops).toEqual(['pkill -P 500; kill 500'])
  })

  // macOS's ps knows no `etimes`, and refusing that one column made the whole probe fail, so the Mac's gate never
  // looked for another vitest (2026-10-31). `etime` is [[dd-]hh:]mm:ss on both systems.
  it('reads a Mac\'s process table, how long each has run included, and finds another vitest in it', () => {
    const now = Date.parse('2026-10-31T22:00:00.000Z')
    const vitest = '/Users/a/work/other/node_modules/.pnpm/vitest@4.1.11/node_modules/vitest'
    const rows = support.processesFromPs(
      [
        '    1     0 01-22:18:27 /sbin/launchd',
        `  900     1    1:02:03 /opt/homebrew/opt/node@24/bin/node ${vitest}/vitest.mjs run`,
        `  901   900       00:05 /opt/homebrew/opt/node@24/bin/node ${vitest}/dist/workers/forks.js`,
        '  902     1  not-a-time /usr/bin/odd',
        ''
      ].join('\n'),
      now
    )
    expect(rows.map((found) => [found.pid, found.ppid, found.name, found.started])).toEqual([
      [1, 0, 'launchd', '2026-10-29T23:41:33.000Z'],
      [900, 1, 'node', '2026-10-31T20:57:57.000Z'],
      [901, 900, 'node', '2026-10-31T21:59:55.000Z'],
      [902, 1, 'odd', null]
    ])
    expect(rows[1]?.cmd).toBe(`/opt/homebrew/opt/node@24/bin/node ${vitest}/vitest.mjs run`)
    expect(support.otherWork(rows, '/Users/a/work/locust', 400).vitest.map((found) => found.pid)).toEqual([900, 901])
  })

  it.runIf(process.platform !== 'win32')('reads this machine\'s own process table, this test among it', () => {
    const own = support.probePosix().processes.find((found) => found.pid === process.pid)
    expect(own?.started).toEqual(expect.any(String))
    expect(Date.parse(own?.started ?? '')).toBeLessThanOrEqual(Date.now())
  })

  it('says both reasons when there are two', () => {
    expect(reading(1024 ** 3, table).refusals).toHaveLength(2)
  })

  it('does not refuse on memory it could not read, and says so', () => {
    const { lines, refusals } = reading(undefined, quietTable)
    expect(refusals).toEqual([])
    expect(lines[0]).toBe('free memory is not read on win32; not enforced')
  })

  it('says when the scratch folder has grown crowded', () => {
    const { lines } = support.assess({ probe: { freeVirtualBytes: 8 * 1024 ** 3, processes: quietTable }, repository: REPO, ownPid: 400, scratch: '/scratch', entries: 11_000, platformName: 'win32' })
    expect(lines.some((line) => line.startsWith('11,000 entries') && line.includes('clear the old locust-* ones'))).toBe(true)
  })
})
