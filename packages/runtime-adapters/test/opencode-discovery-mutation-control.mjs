// Controls for version-selected OpenCode discovery. All probes are mocked.
// Run from runtime-adapters: node test/opencode-discovery-mutation-control.mjs
import { spawnSync } from 'node:child_process'
import { randomUUID } from 'node:crypto'
import { existsSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)))
const SOURCE = join(ROOT, 'src', 'discovery.ts')
const TESTS = ['test/opencode-2-discovery-uses-its-acp-command.test.ts', 'src/probes-report-as-they-run.test.ts']
const VITEST = join(ROOT, '..', '..', 'node_modules', 'vitest', 'vitest.mjs')
const REPORT = join(tmpdir(), `locust-opencode-discovery-${process.pid}-${randomUUID()}.json`)
const MUTATIONS = [
  {
    name: 'the version-first probe serialises all runtime discovery',
    from: 'new Promise((resolve) => setTimeout(resolve, stagger)),',
    to: 'running.then(() => undefined),',
    expect: 'does not make the total the sum of every probe'
  },
  {
    name: 'a future major is guessed to use ACP',
    from: 'parseRuntimeVersion(version.version)?.major === 2',
    to: 'Number(parseRuntimeVersion(version.version)?.major) >= 2',
    expect: 'shares one exact-major selector without assigning ACP to unknown, V1 or future versions'
  },
  {
    name: 'V1 is incorrectly promoted to ACP',
    from: 'parseRuntimeVersion(version.version)?.major === 2',
    to: 'Number(parseRuntimeVersion(version.version)?.major) >= 1',
    expect: 'keeps the V1 help, model-list readiness and free-model parsing'
  },
  {
    name: 'the V2 help probe asks the removed V1 command',
    from: '"capabilities", ["acp", "--help"]',
    to: '"capabilities", ["run", "--help"]',
    expect: 'checks V2 ACP help without launching V1 probes or spending a turn'
  },
  {
    name: 'any successful help is accepted as proof of ACP',
    from: '/\\bagent[\\s-]*client[\\s-]*protocol\\b/i.test(`${capability.result.stdout}\\n${capability.result.stderr}`)',
    to: 'true',
    expect: 'refuses V2 without safe ACP capability evidence: old run help'
  },
  {
    name: 'failed and timed out help still establishes readiness',
    from: 'const advertised = succeeded(capability)',
    to: 'const advertised = capability.result !== undefined',
    expect: 'refuses V2 without safe ACP capability evidence: timed out help'
  },
  {
    name: 'ACP capabilities inherit V1 required flags',
    from: 'const requiredFeatures: readonly RuntimeFeature[] = ["acp-stdio"];',
    to: 'const requiredFeatures: readonly RuntimeFeature[] = OPENCODE_REQUIRED_FEATURES;',
    expect: 'checks V2 ACP help without launching V1 probes or spending a turn'
  },
  {
    name: 'V2 discovery creates CLI model hints',
    from: '    requiredFeatures,\n    diagnostics,\n  };\n}',
    to: '    requiredFeatures,\n    diagnostics,\n    modelHints: { aliases: [], efforts: [] },\n  };\n}',
    expect: 'leaves V2 model choices to the ACP session instead of inventing CLI hints'
  },
  {
    name: 'V2 skips its dedicated branch when old help was cached',
    from: 'if (version !== undefined && openCodeUsesAcp(version)) {',
    to: 'if (remembered === undefined && version !== undefined && openCodeUsesAcp(version)) {',
    expect: 'ignores legacy cached V2 help and never saves ACP help under a V1 cache key'
  },
  {
    name: 'V2 stderr is not read for ACP capability evidence',
    from: '/\\bagent[\\s-]*client[\\s-]*protocol\\b/i.test(`${capability.result.stdout}\\n${capability.result.stderr}`)',
    to: '/\\bagent[\\s-]*client[\\s-]*protocol\\b/i.test(capability.result.stdout)',
    expect: 'accepts ACP help printed on stderr with the spaced protocol name'
  }
]

function run() {
  rmSync(REPORT, { force: true })
  const result = spawnSync(process.execPath, [VITEST, 'run', ...TESTS, '--maxWorkers=2', '--reporter=json', '--outputFile', REPORT], { cwd: ROOT, encoding: 'utf8', timeout: 60_000 })
  if (result.error || !existsSync(REPORT)) return { valid: false, assertions: [], passed: false }
  const report = JSON.parse(readFileSync(REPORT, 'utf8'))
  const files = report.testResults ?? []
  const assertions = files.flatMap(file => file.assertionResults ?? [])
  return {
    valid: (report.numRuntimeErrorTestSuites ?? 0) === 0 && files.length === TESTS.length && assertions.length > 0
      && (report.numPendingTests ?? 0) === 0 && (report.numTodoTests ?? 0) === 0,
    assertions,
    passed: result.status === 0 && report.success === true
  }
}

const original = readFileSync(SOURCE, 'utf8')
let failures = 0
try {
  const baseline = run()
  if (!baseline.valid || !baseline.passed) throw new Error('Discovery baseline failed or did not collect every test')
  console.error(`Discovery baseline: ${baseline.assertions.length} tests passing`)
  for (const mutation of MUTATIONS) {
    if (!original.includes(mutation.from)) throw new Error(`Missing mutation anchor: ${mutation.name}`)
    if (!baseline.assertions.some(assertion => assertion.title === mutation.expect)) throw new Error(`Missing named test: ${mutation.expect}`)
    writeFileSync(SOURCE, original.replace(mutation.from, mutation.to), 'utf8')
    const result = run()
    writeFileSync(SOURCE, original, 'utf8')
    const caught = result.valid && result.assertions.length === baseline.assertions.length
      && result.assertions.some(assertion => assertion.title === mutation.expect && assertion.status === 'failed')
    if (!caught) failures += 1
    console.error(`[${caught ? 'CAUGHT' : 'INVALID OR SURVIVED'}] ${mutation.name}`)
  }
} finally {
  writeFileSync(SOURCE, original, 'utf8')
  rmSync(REPORT, { force: true })
}
console.error(`${MUTATIONS.length - failures}/${MUTATIONS.length} discovery mutation controls caught`)
process.exitCode = failures === 0 ? 0 : 1
