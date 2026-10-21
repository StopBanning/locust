// Focused controls for the generated OpenCode 2 command guard. No agent or
// network is used. Each control must fail its named assertion, never collection,
// a timeout, or a changed test inventory. The existing V1 code is never mutated.
import { spawnSync } from 'node:child_process'
import { randomUUID } from 'node:crypto'
import { existsSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)))
const SOURCE = join(ROOT, 'src/main/command-guard.ts')
const TEST = 'src/main/an-opencode-2-teammate-keeps-the-command-guard.test.ts'
const VITEST = join(ROOT, '../../node_modules/vitest/vitest.mjs')
const REPORT = join(tmpdir(), `locust-opencode-2-guard-${randomUUID()}.json`)
const DENY = 'denies a browser termination before either a bash tool or a shell starts and permits harmless commands'
const ENVELOPE = 'passes the hook envelope and Locust parent to the same guard without running the requested command'
const OUTPUT = 'bounds guard output and kills an overflowing child instead of accumulating it'
const MUTATIONS = [
  ['the native V2 shell tool bypasses the guard', "event?.tool === 'shell' || event?.tool === 'bash' ? event.input?.command : undefined", "event?.tool === 'bash' ? event.input?.command : undefined", DENY],
  ['the V2 shell command is dropped', 'check(event?.command)', 'check(undefined)', DENY],
  ['an explicit refusal is ignored', 'if (reason !== undefined) throw new Error(reason)', 'if (false) throw new Error(reason)', DENY],
  ['the wrong input envelope is handed to the guard', 'tool_input: { command }', "tool_input: { command: '' }", DENY],
  ['the guard receives the wrong Locust parent', 'LOCUST_GUARD_PARENT: String(parent)', "LOCUST_GUARD_PARENT: '0'", ENVELOPE],
  ['the hook guard loses Electron node mode', "ELECTRON_RUN_AS_NODE: '1'", "ELECTRON_RUN_AS_NODE: '0'", ENVELOPE],
  ['the guard output bound is removed', 'if (bytes > 64 * 1024)', 'if (false)', OUTPUT],
  ['an overflowing child is not stopped', 'try { child?.kill() } catch {}', 'try { void child } catch {}', OUTPUT],
  ['the guard deadline fires before fifteen seconds', 'finish(undefined, true), 15000)', 'finish(undefined, true), 14000)', 'bounds a stalled guard to fifteen seconds and releases the child once'],
  ['a spawn failure retains the timer', "child.once('error', () => finish(undefined, true))", "child.once('error', () => resolve(undefined))", 'fails open promptly when the guard cannot start or returns malformed output']
]

function run() {
  rmSync(REPORT, { force: true })
  const result = spawnSync(process.execPath, [VITEST, 'run', TEST, '--maxWorkers=1', '--reporter=json', `--outputFile=${REPORT}`], { cwd: ROOT, encoding: 'utf8', timeout: 90_000, maxBuffer: 5 * 1024 * 1024, shell: false })
  if (result.error !== undefined || result.signal !== null || !existsSync(REPORT)) throw new Error(`Test runner failed: ${result.error?.message ?? result.stderr}`)
  const report = JSON.parse(readFileSync(REPORT, 'utf8'))
  const assertions = (report.testResults ?? []).flatMap((file) => file.assertionResults ?? [])
  if (assertions.length === 0 || assertions.length !== report.numTotalTests || (report.numPendingTests ?? 0) > 0 || assertions.some((item) => !['passed', 'failed'].includes(item.status))) throw new Error('Missing, skipped, or uncollected assertion inventory.')
  const failed = assertions.filter((item) => item.status === 'failed')
  if ((result.status === 0) !== (failed.length === 0)) throw new Error('Runner exit disagrees with assertion results.')
  if (failed.some((item) => item.failureMessages?.some((message) => /Test timed out|Failed to resolve|Transform failed/.test(message)))) throw new Error('An infrastructure failure or timeout is not a mutation caught.')
  return { failed, identities: assertions.map((item) => item.fullName).sort() }
}

const original = readFileSync(SOURCE, 'utf8')
const start = original.indexOf('export function openCode2CommandGuardPlugin(')
if (start < 0) throw new Error('V2 generator source not found.')
const before = original.slice(0, start)
const implementation = original.slice(start)
let caught = 0
try {
  const baseline = run()
  if (baseline.failed.length !== 0) throw new Error('Baseline is not green.')
  for (const [name, from, to, expected] of MUTATIONS) {
    if (implementation.split(from).length !== 2) throw new Error(`Expected one V2 mutation anchor for ${name}.`)
    let result
    try {
      writeFileSync(SOURCE, before + implementation.replace(from, to), 'utf8')
      result = run()
    } finally {
      writeFileSync(SOURCE, original, 'utf8')
    }
    if (JSON.stringify(result.identities) !== JSON.stringify(baseline.identities)) throw new Error(`Changed assertion inventory: ${name}`)
    if (!result.failed.some((item) => item.title === expected && item.failureMessages?.length > 0)) throw new Error(`SURVIVED: ${name}`)
    console.error(`CAUGHT: ${name}`)
    caught += 1
  }
  console.error(`${caught}/${MUTATIONS.length} mutation controls caught; zero skipped or surviving.`)
} finally {
  writeFileSync(SOURCE, original, 'utf8')
  rmSync(REPORT, { force: true })
}
