// Run only while this worktree has exclusive ownership of the source files.
// Every control must fail its named assertion, with the same collected tests;
// a timeout, skipped test, collection failure or runner error never counts.
// No installed agent, provider, socket or account is used by these fixtures.
import { spawnSync } from 'node:child_process'
import { randomUUID } from 'node:crypto'
import { existsSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)))
const VITEST = join(ROOT, '../../node_modules/vitest/vitest.mjs')
const REPORT = join(tmpdir(), `locust-opencode-2-desktop-${randomUUID()}.json`)
const TESTS = [
  'src/main/opencode-2-models-come-from-its-acp-session.test.ts',
  'src/main/opencode-2-runs-over-acp-with-its-permissions.test.ts',
  'src/main/an-opencode-2-mode-has-native-permission-rules.test.ts'
]
const MISSION = 'src/main/codex-mission.ts'
const CATALOG = 'src/main/model-catalog.ts'
const PROBE = 'src/main/opencode-acp-models.ts'
const LAUNCH = 'src/main/opencode-2-launch.ts'
const ROUTING = 'starts ACP instead of serve or run, passes the native guard and sets mode and model before its prompt'
const MODE = 'uses an owned ACP mode and a directory plugin, without unsupported V1 flags'
const MODE_RULES = 'denies edits and subagents in Ask while preserving reading commands and refusing unsafe git variants'
const CATALOG_READ = 'opens an empty no-prompt session and lists only its advertised models, including free grouped rows'
const ACP_SCOPE = { start: '} else if (opencodeAcp) {', end: '} else if (opencodeServes) {' }

const MUTATIONS = [
  { name: 'the ACP handshake serialises independent model probes', file: CATALOG,
    from: '? readOpenCodeAcpModels(opencode, { spawn: options.spawn,', to: '? await readOpenCodeAcpModels(opencode, { spawn: options.spawn,',
    expected: 'starts independent catalog probes together instead of serialising their deadlines' },
  { name: 'Ask enables repository configuration', file: LAUNCH,
    from: "OPENCODE_CONFIG_PROJECT_DISABLE: '1'", to: "OPENCODE_CONFIG_PROJECT_DISABLE: '0'", expected: MODE },
  { name: 'imported sessions inherit permissions', file: MISSION,
    from: 'modeChanged || protocolChanged || unverifiedAcpSession', to: 'modeChanged || protocolChanged || false',
    expected: 'briefs a new session for imported-permissions instead of loading a foreign permission history' },
  { name: 'a protocol change borrows the old session', file: MISSION,
    from: 'modeChanged || protocolChanged || unverifiedAcpSession', to: 'modeChanged || false || unverifiedAcpSession',
    expected: 'briefs a new session for version-change instead of loading a foreign permission history' },
  { name: 'a cold side session loses its context', file: MISSION,
    from: '(side !== undefined && !opencodeAcp) ? attached', to: 'side !== undefined ? attached',
    expected: 'asks a side question in a freshly briefed ACP session without appending to the parent' },
  { name: 'V2 is routed to the old serve transport', file: MISSION,
    from: "const opencodeAcp = runtime === 'opencode' && cloud === undefined && openCodeUsesAcp(chosen.version)",
    to: "const opencodeAcp = runtime === 'opencode' && cloud === undefined && false", expected: ROUTING },
  { name: 'V1 is routed into ACP', file: MISSION,
    from: "const opencodeAcp = runtime === 'opencode' && cloud === undefined && openCodeUsesAcp(chosen.version)",
    to: "const opencodeAcp = runtime === 'opencode' && cloud === undefined && (openCodeUsesAcp(chosen.version) || chosen.version?.version.startsWith('1.') === true)",
    expected: 'retains the V1 run route and its original hook when no serve bridge is available' },
  { name: 'V2 notifications use the old normalizer', file: MISSION,
    from: "? createAcpEventNormalizer({ ...normalizerContext, runtime: 'opencode', ...(resumeThreadId === undefined ? {} : { sessionId: resumeThreadId }) })",
    to: '? createOpenCodeEventNormalizer(normalizerContext)', expected: ROUTING },
  { name: 'the V2 guard directory is omitted', file: LAUNCH,
    from: '...(options.guardPlugin === undefined ? {} : { plugins: [options.guardPlugin] }),', to: '...{},', expected: MODE },
  { name: 'native per-turn permission rules are dropped', file: LAUNCH,
    from: 'Object.entries(config.permission)', to: 'Object.entries({})', expected: MODE_RULES },
  { name: 'the required model is not selected', file: MISSION, scope: ACP_SCOPE,
    from: '...(chosenModel === undefined ? {} : { model: chosenModel }),', to: '...{},', expected: ROUTING },
  { name: 'the required mode is not selected', file: MISSION, scope: ACP_SCOPE,
    from: 'mode: OPENCODE_2_MODE,', to: '...{},', expected: ROUTING },
  { name: 'Approve each permissions bypass the card handler', file: MISSION, scope: ACP_SCOPE,
    from: 'return acpAnswerFor(await handler(acpPermissionRequest(asked, runCwd)))', to: "return 'allow_once'", expected: "answers an Approve each card with the person's approve-once decision" },
  { name: 'V2 resurrects stale CLI model hints', file: CATALOG,
    from: 'if (openCodeUsesAcp(opencode?.version)) return []', to: 'if (false) return []', expected: 'keeps the V1 CLI list unchanged and never offers its stale hints for V2' },
  { name: 'the no-prompt catalog claims filesystem and terminal access', file: PROBE,
    from: 'clientCapabilities: { fs: { readTextFile: false, writeTextFile: false }, terminal: false }',
    to: 'clientCapabilities: { fs: { readTextFile: true, writeTextFile: true }, terminal: true }', expected: CATALOG_READ },
  { name: 'catalog stdout closure is ignored', file: PROBE,
    from: 'child.onStdoutClose?.(() => { if (active && !sessionAnswered) fail() })', to: 'void child', expected: 'stops immediately when close arrives while a request is pending' },
  { name: 'catalog cleanup leaves the owned process alive', file: PROBE,
    from: 'try { child?.kill() } catch', to: 'try { void child } catch', expected: CATALOG_READ },
  { name: 'duplicate advertised model IDs are emitted twice', file: PROBE,
    from: 'if (seen.has(model.value)) return []', to: 'if (false) return []', expected: CATALOG_READ },
  { name: 'Ask can load unrelated plugins', file: LAUNCH,
    from: "{ action: 'integration.use', resource: 'plugin:*', effect: 'deny' },", to: "{ action: 'integration.use', resource: 'plugin:*', effect: 'allow' },", expected: MODE },
  { name: 'default path denial overwrites its specific grants', file: LAUNCH,
    from: "[...entries.filter(([path]) => path === '*'), ...entries.filter(([path]) => path !== '*')]",
    to: "[...entries.filter(([path]) => path !== '*'), ...entries.filter(([path]) => path === '*')]", expected: 'grants references and worktree git metadata after the default denial, without granting the shared root' }
]

function run() {
  rmSync(REPORT, { force: true })
  const result = spawnSync(process.execPath, [VITEST, 'run', ...TESTS, '--maxWorkers=1', '--reporter=json', `--outputFile=${REPORT}`], {
    cwd: ROOT, encoding: 'utf8', timeout: 90_000, maxBuffer: 8 * 1024 * 1024, shell: false
  })
  if (result.error !== undefined || result.signal !== null || !existsSync(REPORT)) throw new Error(`Test runner failed: ${result.error?.message ?? result.stderr}`)
  const report = JSON.parse(readFileSync(REPORT, 'utf8'))
  const files = report.testResults ?? []
  const assertions = files.flatMap((file) => file.assertionResults ?? [])
  if (files.length !== TESTS.length || files.some((file) => !Array.isArray(file.assertionResults) || file.assertionResults.length === 0)
    || assertions.length === 0 || assertions.length !== report.numTotalTests || (report.numPendingTests ?? 0) !== 0
    || (report.numTodoTests ?? 0) !== 0 || (report.numRuntimeErrorTestSuites ?? 0) !== 0
    || assertions.some((item) => !['passed', 'failed'].includes(item.status))) throw new Error('Missing, skipped, or uncollected assertion inventory.')
  const failed = assertions.filter((item) => item.status === 'failed')
  if ((result.status === 0) !== (failed.length === 0)) throw new Error('Runner exit disagrees with assertion results.')
  const infrastructure = /Test timed out|Hook timed out|Failed to resolve|Transform failed|Cannot find module|Failed Suites|Unhandled Rejection|Unhandled Error|SyntaxError|ReferenceError|TypeError:/i
  if (failed.some((item) => item.failureMessages?.some((message) => infrastructure.test(message)))
    || /Unhandled Rejection|Unhandled Error|Failed Suites/.test(`${result.stdout}\n${result.stderr}`)) throw new Error(`An infrastructure failure or timeout is not a mutation caught. ${failed.map((item) => `${item.title}: ${item.failureMessages?.join(' ').slice(0, 1200)}`).join('\n')}`)
  return { failed, identities: assertions.map((item) => `${item.fullName}`).sort() }
}

function changed(original, mutation) {
  let start = 0
  let end = original.length
  if (mutation.scope !== undefined) {
    start = original.indexOf(mutation.scope.start)
    if (start < 0 || original.indexOf(mutation.scope.start, start + 1) >= 0) throw new Error(`Expected one scope start: ${mutation.name}`)
    end = original.indexOf(mutation.scope.end, start)
    if (end < 0) throw new Error(`Missing scope end: ${mutation.name}`)
  }
  const scope = original.slice(start, end)
  if (scope.split(mutation.from).length !== 2) throw new Error(`Expected one mutation anchor: ${mutation.name}`)
  return original.slice(0, start) + scope.replace(mutation.from, mutation.to) + original.slice(end)
}

const originals = new Map([...new Set(MUTATIONS.map((mutation) => mutation.file))].map((file) => [file, readFileSync(join(ROOT, file))]))
const restore = () => { for (const [file, bytes] of originals) writeFileSync(join(ROOT, file), bytes) }
let caught = 0
try {
  // Validate every anchor before the first mutation changes a source file.
  for (const mutation of MUTATIONS) changed(originals.get(mutation.file).toString('utf8'), mutation)
  const baseline = run()
  if (baseline.failed.length !== 0) throw new Error('Baseline is not green.')
  console.error(`BASELINE: ${baseline.identities.length} assertions passed with no skips.`)
  for (const mutation of MUTATIONS) {
    let result
    try {
      writeFileSync(join(ROOT, mutation.file), changed(originals.get(mutation.file).toString('utf8'), mutation), 'utf8')
      result = run()
    } finally { restore() }
    if (JSON.stringify(result.identities) !== JSON.stringify(baseline.identities)) throw new Error(`Changed assertion inventory: ${mutation.name}`)
    const named = result.failed.find((item) => item.title === mutation.expected)
    if (named === undefined || !named.failureMessages?.some((message) => /AssertionError|expected|expect\(/i.test(message))) throw new Error(`SURVIVED or named assertion did not fail: ${mutation.name}`)
    console.error(`CAUGHT: ${mutation.name}`)
    caught += 1
  }
  console.error(`${caught}/${MUTATIONS.length} mutation controls caught; zero skipped or surviving.`)
} finally {
  restore()
  rmSync(REPORT, { force: true })
}
