// What _tools/gate.sh asks a program to do, because bash is the wrong place for it.
//
//   node _tools/gate-support.mjs quiet <repository> <scratch folder>
//     Is the machine quiet enough to trust a run? Prints what it found, one
//     `GATE quiet:` line each. Exit 0 when it is; 75 (EX_TEMPFAIL, "try again
//     later") when it is not: under 2 GB of free virtual memory, or another
//     vitest already running anywhere on the machine. A check that cannot be
//     made (no PowerShell, an unreadable process table) is said and does not
//     refuse: the gate is not to be held up by its own instrument.
//
//   node _tools/gate-support.mjs failures <suite> <vitest log> [digest]
//     Why did a suite go red? One `GATE:` line per failed test with the reason
//     vitest gave -- the timeout or the assertion, which the old
//     `grep "×|FAIL"` threw away -- then vitest's own failure section (left
//     out when the word `digest` is added). Always exits 0: it explains, it
//     does not decide.
//
//   node _tools/gate-support.mjs retry-files <vitest log>
//     The files worth running once more. Printed one per line, exit 0, ONLY
//     when every failed test failed by timing out (a cleanup that then hit a
//     locked folder is the timeout's own aftermath, since the abandoned test
//     body is still writing into it). Anything else -- an assertion, a file that
//     would not load, an unhandled error, a count that does not add up, a log
//     that could not be read -- prints nothing, says why on stderr and exits 1.
//
// Why a log and not vitest's JSON report: in vitest 4 the JSON reporter writes
// a timeout as `Error: STACK_TRACE_ERROR` and a stack, with the words "timed
// out" nowhere in it. The default report is the one place the reason is.

import { execFileSync } from 'node:child_process'
import { readFileSync, readdirSync, realpathSync } from 'node:fs'
import { platform } from 'node:os'
import { resolve, sep } from 'node:path'
import { pathToFileURL } from 'node:url'

const GIB = 1024 ** 3
export const MIN_FREE_BYTES = 2 * GIB
export const REFUSED = 75
const REPORT_LINES = 140
const LISTED_FAILURES = 30
// The scratch folder holds a temp folder per test and every gate's logs; past
// this many entries it is worth saying so (11,000 leftover folders once timed
// out the fs tests).
const CROWDED_SCRATCH = 3_000

const ESCAPE = String.fromCharCode(27)
const plain = (text) => text.replaceAll('\r', '').replace(new RegExp(`${ESCAPE}\\[[0-9;]*[A-Za-z]`, 'g'), '')

// ---------------------------------------------------------------------------
// The log: what failed, and why.

const TIMEOUT = /^(?:\w+)?Error: (?:Test|Hook) timed out in \d+ms/
// A folder or file another program was holding when cleanup came for it.
const LOCKED = /^(?:\w+)?Error: (?:EBUSY|EPERM|ENOTEMPTY|EACCES)\b/
// A bare `Error: ...` as well as `AssertionError: ...`, at the start of the line.
const ERROR_LINE = /^[\w$.]*(?:Error|Exception)\b/
const SEPARATOR = /^⎯{3,}/
const SECTION = /^⎯{3,} (?:Failed Tests|Failed Suites|Unhandled)/

const countIn = (summary, word) => {
  const found = new RegExp(`(\\d+) ${word}`).exec(summary)
  return found === null ? 0 : Number(found[1])
}

/** The failed tests of one vitest log, one entry per test however many errors it printed. */
export function readLog(text) {
  const lines = plain(text).split('\n')
  const start = lines.findIndex((line) => SECTION.test(line))
  const end = lines.findIndex((line, index) => index > start && /^\s+Test Files\s/.test(line))
  const section = start < 0 ? [] : lines.slice(start, end < 0 ? lines.length : end)

  // Two shapes vitest prints, both read here. A test that timed out and then
  // failed its cleanup appears twice under one header, so errors are gathered
  // per header. And tests that fail on ONE shared error (a hook) are printed as
  // several headers in a row and then the error once, so an error belongs to
  // every header still waiting for one when it arrives.
  const tests = new Map()
  let waiting = []
  for (const line of section) {
    const header = /^ FAIL  (.+)$/.exec(line)
    if (header !== null) {
      const name = header[1].trim()
      const entry = tests.get(name) ?? { name, file: name.split(' > ')[0].replace(/ \[.*$/, ''), suiteLevel: / \[ /.test(name), errors: [] }
      tests.set(name, entry)
      waiting.push(entry)
    } else if (SEPARATOR.test(line)) {
      waiting = []
    } else if (ERROR_LINE.test(line)) {
      for (const entry of waiting) entry.errors.push(line)
    }
  }

  const summaryOf = (label) => lines.find((line) => new RegExp(`^\\s+${label}\\s`).test(line)) ?? ''
  const testsLine = summaryOf('Tests')
  const filesLine = summaryOf('Test Files')
  return {
    section,
    tests: [...tests.values()].map((entry) => ({ ...entry, kind: kindOf(entry.errors) })),
    failedTests: countIn(testsLine, 'failed'),
    failedFiles: countIn(filesLine, 'failed'),
    hasSummary: testsLine !== '',
    unhandled: lines.some((line) => /^⎯{3,} Unhandled (?:Errors|Rejection)/.test(line) || /^Vitest caught \d+ unhandled/.test(line)),
    lines
  }
}

function kindOf(errors) {
  if (errors.length === 0) return 'unknown'
  if (errors.some((error) => TIMEOUT.test(error)) && errors.every((error) => TIMEOUT.test(error) || LOCKED.test(error))) return 'timeout'
  if (errors.every((error) => LOCKED.test(error))) return 'locked'
  if (errors.some((error) => /^AssertionError\b/.test(error))) return 'assertion'
  return 'error'
}

/** Whether a red run is worth one more go, and which files to give it. */
export function retryDecision(text) {
  const log = readLog(text)
  const no = (reason) => ({ retry: false, files: [], reason })
  if (!log.hasSummary) return no('the log has no vitest summary, so the run did not finish normally')
  if (log.tests.length === 0) return no('no failed test could be read out of the log')
  if (log.unhandled) return no('vitest reported an unhandled error, which is not a timeout')
  const other = log.tests.find((test) => test.kind !== 'timeout')
  if (other !== undefined) return no(`${other.name} failed, and not by timing out (${other.kind})`)
  if (log.tests.some((test) => test.suiteLevel)) return no('a whole file failed, which is not a timeout in one test')
  const files = [...new Set(log.tests.map((test) => test.file))]
  if (log.tests.length !== log.failedTests) return no(`vitest counted ${String(log.failedTests)} failed test(s) but ${String(log.tests.length)} were read`)
  if (files.length !== log.failedFiles) return no(`vitest counted ${String(log.failedFiles)} failed file(s) but ${String(files.length)} were read`)
  if (files.some((file) => !/^[\w@./+-]+\.test\.tsx?$/.test(file))) return no('a failed file has a name this will not pass on a command line')
  return { retry: true, files, reason: '' }
}

/** The lines `failures` prints: a digest a grep for GATE shows, then (unless digestOnly) vitest's own section. */
export function failureReport(suite, text, logPath, digestOnly = false) {
  const log = readLog(text)
  const out = []
  if (log.tests.length === 0) {
    out.push(`GATE: ${suite}: no failed test could be read from the log; its last lines:`)
    out.push(...log.lines.filter((line) => line.trim() !== '').slice(-40))
    return out
  }
  const timeouts = log.tests.filter((test) => test.kind === 'timeout').length
  out.push(`GATE: ${suite}: ${String(log.tests.length)} failed test(s) in ${String(new Set(log.tests.map((test) => test.file)).size)} file(s), ${String(timeouts)} of them timeouts:`)
  for (const test of log.tests.slice(0, LISTED_FAILURES)) {
    const [first, ...rest] = test.errors
    const aftermath = rest.length === 0 ? '' : ` (then: ${rest.map((error) => error.slice(0, 90)).join('; ')})`
    out.push(`GATE:   [${test.kind}] ${test.name} -- ${(first ?? 'no error line').slice(0, 200)}${aftermath}`)
  }
  if (log.tests.length > LISTED_FAILURES) out.push(`GATE:   ... and ${String(log.tests.length - LISTED_FAILURES)} more`)
  if (log.unhandled) out.push(`GATE: ${suite}: vitest also reported an unhandled error (in the section below)`)
  if (digestOnly) return out
  out.push('GATE: vitest\'s own failure section:')
  out.push(...log.section.slice(0, REPORT_LINES))
  if (log.section.length > REPORT_LINES) out.push(`... ${String(log.section.length - REPORT_LINES)} more lines in ${logPath}`)
  return out
}

// ---------------------------------------------------------------------------
// The machine: is it quiet?

const WINDOWS_PROBE = `
$ErrorActionPreference = 'Stop'
$os = Get-CimInstance Win32_OperatingSystem
$rows = Get-CimInstance Win32_Process | ForEach-Object {
  [pscustomobject]@{
    pid = [int]$_.ProcessId
    ppid = [int]$_.ParentProcessId
    name = $_.Name
    path = $_.ExecutablePath
    cmd = $_.CommandLine
    started = if ($_.CreationDate) { $_.CreationDate.ToUniversalTime().ToString('o') } else { $null }
  }
}
[pscustomobject]@{ freeVirtualKB = [double]$os.FreeVirtualMemory; freePhysicalKB = [double]$os.FreePhysicalMemory; processes = @($rows) } | ConvertTo-Json -Compress -Depth 4
`

function probeWindows() {
  const encoded = Buffer.from(WINDOWS_PROBE, 'utf16le').toString('base64')
  const output = execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-EncodedCommand', encoded], {
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
    timeout: 60_000,
    windowsHide: true,
    stdio: ['ignore', 'pipe', 'pipe']
  })
  const probe = JSON.parse(output)
  return {
    freeVirtualBytes: probe.freeVirtualKB * 1024,
    freePhysicalBytes: probe.freePhysicalKB * 1024,
    processes: probe.processes.map((row) => ({ ...row, name: row.name ?? '', cmd: row.cmd ?? '', path: row.path ?? '' }))
  }
}

// How long a process has run, as `ps -o etime` writes it: [[dd-]hh:]mm:ss on Linux and macOS alike. The column used
// to be `etimes` (plain seconds), which only Linux's ps knows: macOS's refused the whole probe, so on the Mac the gate
// said "could not read the machine's state" and never checked for another vitest (2026-10-31).
const ELAPSED = /^(?:(?:(\d+)-)?(\d+):)?(\d+):(\d+)$/

/** The rows of `ps -eo pid=,ppid=,etime=,args=`, each one's start worked back from how long it has run. */
export function processesFromPs(output, now = Date.now()) {
  return output
    .split('\n')
    .map((line) => /^\s*(\d+)\s+(\d+)\s+(\S+)\s+(.*)$/.exec(line))
    .filter((found) => found !== null)
    .map(([, pid, ppid, elapsed, cmd]) => {
      const parts = ELAPSED.exec(elapsed)
      const seconds = parts === null ? undefined : ((Number(parts[1] ?? 0) * 24 + Number(parts[2] ?? 0)) * 60 + Number(parts[3])) * 60 + Number(parts[4])
      const started = seconds === undefined ? null : new Date(now - seconds * 1000).toISOString()
      return { pid: Number(pid), ppid: Number(ppid), name: cmd.split(' ')[0].split('/').pop(), path: cmd.split(' ')[0], cmd, started }
    })
}

export function probePosix() {
  const output = execFileSync('ps', ['-eo', 'pid=,ppid=,etime=,args='], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024, timeout: 30_000 })
  const processes = processesFromPs(output)
  let freeVirtualBytes
  try {
    const meminfo = readFileSync('/proc/meminfo', 'utf8')
    const kb = (key) => Number(new RegExp(`^${key}:\\s+(\\d+) kB`, 'm').exec(meminfo)?.[1] ?? Number.NaN)
    freeVirtualBytes = (kb('MemAvailable') + kb('SwapFree')) * 1024
  } catch {
    // Not Linux (macOS reports "free" in a way that makes 2 GB a false alarm): not enforced.
  }
  return { freeVirtualBytes, freePhysicalBytes: undefined, processes }
}

const lower = (path) => path.replaceAll('\\', '/').toLowerCase()
const gb = (bytes) => `${(bytes / GIB).toFixed(1)} GB`
const ago = (iso) => {
  if (iso === null || iso === undefined) return 'unknown age'
  const minutes = Math.max(0, Math.round((Date.now() - Date.parse(iso)) / 60_000))
  return minutes < 90 ? `${String(minutes)} min` : `${(minutes / 60).toFixed(1)} h`
}

/** The processes that make up other work on this machine, from a process table. */
export function otherWork(processes, repository, ownPid) {
  const byPid = new Map(processes.map((row) => [row.pid, row]))
  const own = new Set()
  for (let row = byPid.get(ownPid); row !== undefined && !own.has(row.pid); row = byPid.get(row.ppid)) own.add(row.pid)

  const repo = lower(repository)
  const here = (row) => lower(row.cmd).includes(repo) || lower(row.path).startsWith(repo)
  const isNode = (row) => /^node(\.exe)?$/i.test(row.name)
  const others = processes.filter((row) => !own.has(row.pid))
  return {
    vitest: others.filter((row) => isNode(row) && /vitest/i.test(row.cmd)),
    builders: others.filter((row) => here(row) && /electron-builder|app-builder/i.test(`${row.cmd} ${row.path}`)),
    locusts: others.filter((row) => /^Locust(\.exe)?$/i.test(row.name) && lower(row.path).startsWith(repo))
  }
}

// The folder a vitest process came from: the first path on its command line
// that reaches vitest through a node_modules. npx's own launcher names no such
// path (vitest is only its argument), and is said as what it is.
const repositoryOf = (row) => {
  const cmd = row.cmd.replaceAll('\\', '/')
  const found = /([A-Za-z]:\/[^\s"]*?)\/node_modules\/[^\s"]*vitest/i.exec(cmd) ?? /(\/[^\s"]*?)\/node_modules\/[^\s"]*vitest/i.exec(cmd)
  return found === null ? 'an npx launch' : found[1].replaceAll('/', sep)
}

function describeVitest(rows) {
  const byRepository = new Map()
  for (const row of rows) {
    const entry = byRepository.get(repositoryOf(row)) ?? { count: 0, oldest: row.started, pid: row.pid }
    entry.count += 1
    if (row.started !== null && (entry.oldest === null || row.started < entry.oldest)) entry.oldest = row.started
    byRepository.set(repositoryOf(row), entry)
  }
  return [...byRepository].map(([repository, entry]) => `${String(entry.count)} process(es) in ${repository}, the oldest up ${ago(entry.oldest)} (pid ${String(entry.pid)} among them)`)
}

/**
 * What a reading of the machine says: the lines to print, and what, if
 * anything, means the gate should not start. Pure, so the arithmetic is tested
 * (a-red-gate-says-why-and-retries-only-a-timeout.test.ts) without a machine.
 */
export function assess({ probe, repository, ownPid, scratch, entries, platformName }) {
  const lines = []
  const memoryKnown = probe.freeVirtualBytes !== undefined && !Number.isNaN(probe.freeVirtualBytes)
  if (memoryKnown) {
    const physical = probe.freePhysicalBytes === undefined ? '' : `; free physical memory ${gb(probe.freePhysicalBytes)}`
    lines.push(`free virtual memory ${gb(probe.freeVirtualBytes)} (the gate refuses under ${gb(MIN_FREE_BYTES)})${physical}`)
  } else {
    lines.push(`free memory is not read on ${platformName}; not enforced`)
  }

  const work = otherWork(probe.processes, resolve(repository), ownPid)
  lines.push(work.vitest.length === 0 ? 'vitest: none running' : `vitest: ${describeVitest(work.vitest).join('; ')}`)
  lines.push(
    work.builders.length === 0
      ? 'electron-builder under this repository: none'
      : `electron-builder under this repository: ${String(work.builders.length)} process(es), the oldest up ${ago(work.builders.map((row) => row.started).sort()[0])}`
  )
  lines.push(
    work.locusts.length === 0
      ? 'Locust.exe under this repository: none'
      : `Locust.exe under this repository: ${String(work.locusts.length)} process(es) (pid ${work.locusts.map((row) => String(row.pid)).join(', ')})`
  )
  lines.push(
    entries === undefined
      ? `the scratch folder could not be read (${scratch})`
      : `${entries.toLocaleString('en-US')} entries in the scratch folder (${scratch})${entries >= CROWDED_SCRATCH ? '; a folder this full slows every test that makes a temp folder -- clear the old locust-* ones' : ''}`
  )

  const refusals = []
  if (memoryKnown && probe.freeVirtualBytes < MIN_FREE_BYTES) {
    refusals.push(`only ${gb(probe.freeVirtualBytes)} of virtual memory is free (needs ${gb(MIN_FREE_BYTES)})`)
  }
  if (work.vitest.length > 0) refusals.push('another vitest is running, and two suites at once is how timeouts happen')
  // Kill a whole tree from its root: a worker's pid would leave its parent running.
  const roots = work.vitest.filter((row) => !work.vitest.some((other) => other.pid === row.ppid))
  // How to stop one, in this system's own words: taskkill takes the tree; elsewhere the workers are the root's children.
  const stops = roots.slice(0, 3).map((row) => (platformName === 'win32' ? `taskkill /PID ${String(row.pid)} /T /F` : `pkill -P ${String(row.pid)}; kill ${String(row.pid)}`))
  return { lines, refusals, roots, stops }
}

export function quiet(repository, scratch, log = console.log) {
  const say = (line) => log(`GATE quiet: ${line}`)
  let probe
  try {
    probe = platform() === 'win32' ? probeWindows() : probePosix()
  } catch (error) {
    say(`could not read the machine's state (${String(error instanceof Error ? error.message.split('\n')[0] : error)}); not refusing on what could not be checked`)
    return 0
  }
  let entries
  try {
    entries = readdirSync(scratch).length
  } catch {
    // Said as "could not be read" in the lines.
  }
  const { lines, refusals, stops } = assess({ probe, repository, ownPid: process.pid, scratch, entries, platformName: platform() })
  for (const line of lines) say(line)
  if (refusals.length > 0) {
    log(`GATE REFUSED: ${refusals.join('; ')}. Nothing was run. Wait for the other work to finish and run the gate again (exit ${String(REFUSED)}).`)
    if (stops.length > 0) log(`GATE REFUSED: if one of those is yours and stuck: ${stops.join('   ')}`)
    return REFUSED
  }
  say('quiet enough; going ahead')
  return 0
}

// ---------------------------------------------------------------------------

function main(argv) {
  const [command, ...args] = argv
  if (command === 'quiet' && args.length === 2) return quiet(args[0], args[1])
  if (command === 'failures' && (args.length === 2 || (args.length === 3 && args[2] === 'digest'))) {
    let text = ''
    try {
      text = readFileSync(args[1], 'utf8')
    } catch (error) {
      console.log(`GATE: ${args[0]}: the log could not be read (${String(error instanceof Error ? error.message : error)})`)
      return 0
    }
    for (const line of failureReport(args[0], text, args[1], args.length === 3)) console.log(line)
    return 0
  }
  if (command === 'retry-files' && args.length === 1) {
    let decision
    try {
      decision = retryDecision(readFileSync(args[0], 'utf8'))
    } catch (error) {
      decision = { retry: false, files: [], reason: `the log could not be read (${String(error instanceof Error ? error.message : error)})` }
    }
    if (!decision.retry) {
      console.error(`GATE: not retried: ${decision.reason}`)
      return 1
    }
    for (const file of decision.files) console.log(file)
    return 0
  }
  console.error('usage: gate-support.mjs quiet <repository> <scratch> | failures <suite> <log> [digest] | retry-files <log>')
  return 2
}

// Run as a program, not imported by a test. Compared by real path: on a Mac the temp folder is /var/folders/..., a link
// to /private/var/folders/... where import.meta.url points, and the plain path never matched, so from such a folder
// every command exited 0 having said nothing -- the quiet check passed unasked, and an empty retry list reran the
// whole suite after an assertion failure (2026-10-31, the self-test).
if (process.argv[1] !== undefined && import.meta.url === pathToFileURL(realpathSync(resolve(process.argv[1]))).href) {
  process.exitCode = main(process.argv.slice(2))
}
