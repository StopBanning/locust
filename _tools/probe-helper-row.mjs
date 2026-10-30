// Does a runtime's own helper get its own row?
//
//   node _tools/probe-helper-row.mjs
//
// Seeds a profile with one finished mission whose ledger holds a Claude
// Code `Task` tool call (the sub-agent launcher) and an OpenCode-style
// `task` still open, launches the BUILT app, opens the mission and reads
// the activity fold: its summary must say helpers were asked, and each
// helper row must say what it was asked and whether it reported back. No
// runtime is spent: the ledger is the fixture.

import { spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { localPath } from './local-path.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const NPM_DIR = join(homedir(), 'AppData', 'Roaming', 'npm')
const PORT = 9295
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const say = (line) => console.error(line)
let failures = 0
const check = (label, ok, detail = '') => {
  if (ok) say(`  [PASS] ${label}${detail ? ` -- ${detail}` : ''}`)
  else { failures += 1; say(`  [FAIL] ${label}${detail ? ` -- ${detail}` : ''}`) }
}
try {
  const already = await fetch(`http://127.0.0.1:${String(PORT)}/json/list`, { signal: AbortSignal.timeout(1500) })
  if (already.ok) { say(`  [FAIL] something is already debugging on port ${String(PORT)}`); process.exit(1) }
} catch { /* free */ }

const workspace = await mkdtemp(join(tmpdir(), 'locust-helper-ws-'))
await writeFile(join(workspace, 'README.md'), '# scratch\n', 'utf8')
const WORKSPACE_ID = `ws_${createHash('sha256').update(resolve(workspace), 'utf8').digest('hex').slice(0, 32)}`
const profile = await mkdtemp(join(tmpdir(), 'locust-helper-'))
const LEDGER_DIR = join(profile, 'mission-ledger')
await mkdir(LEDGER_DIR, { recursive: true })
const T0 = '2026-09-05T05:00:00.000Z'
const T1 = '2026-09-05T05:01:00.000Z'
await writeFile(join(profile, 'teammates.json'), JSON.stringify({
  schemaVersion: 1,
  teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: T0, route: { runtime: 'claude', model: 'account-default', mode: 'ask' } }],
  missionOwners: { mission_helper: 'tm_wren' },
  settings: { swarm: false, relay: false, relayHopCap: 6, memoryMode: 'off' }
}))
const PROCESS = {
  exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 6, inputDeliveryFailed: false,
  outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: T0, finishedAt: T1
}
const evidence = { redacted: true }
const missionId = 'mission_helper'
const runId = 'run_helper'
const metadata = {
  missionId, runId, prompt: 'Find the flaky tests.', runtime: 'claude', model: 'claude-sonnet-5', requestedRouteId: 'claude', resolvedRouteId: 'claude-account:default',
  cliVersion: '2.1.0', workspaceId: WORKSPACE_ID, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: T0
}
const events = [
  { type: 'run.started', occurredAt: T0, payload: { runtimeThreadId: 'thread-1', evidence } },
  { type: 'tool.started', occurredAt: T0, payload: { itemId: 't1', toolKind: 'tool_use', name: 'Task', command: 'Search the tests for flaky cases', phase: 'started', evidence } },
  { type: 'tool.completed', occurredAt: T1, payload: { itemId: 't1', toolKind: 'tool_use', name: 'Task', command: 'Search the tests for flaky cases', phase: 'completed', evidence } },
  { type: 'tool.started', occurredAt: T1, payload: { itemId: 't2', toolKind: 'tool_use', name: 'Read', command: 'src/status.ts', phase: 'started', evidence } },
  { type: 'tool.completed', occurredAt: T1, payload: { itemId: 't2', toolKind: 'tool_use', name: 'Read', command: 'src/status.ts', phase: 'completed', evidence } },
  { type: 'message.delta', occurredAt: T1, payload: { itemId: 'answer', operation: 'replace', text: 'Two tests are flaky; the helper found them.', final: true, evidence } },
  { type: 'run.completed', occurredAt: T1, payload: { process: PROCESS, evidence } }
]
const lines = [JSON.stringify({ schemaVersion: 13, recordType: 'mission.created', ledgerSequence: 1, occurredAt: T0, metadata })]
events.forEach((event, index) => {
  lines.push(JSON.stringify({
    schemaVersion: 13, recordType: 'mission.event', ledgerSequence: index + 2, occurredAt: event.occurredAt,
    event: { ...event, id: `${missionId}:${String(index + 1)}`, runId, missionId, sequence: index + 1, sourceAdapter: 'claude' }
  }))
})
await writeFile(join(LEDGER_DIR, `${missionId}.jsonl`), `${lines.join('\n')}\n`, 'utf8')

const child = spawn(ELECTRON, [APP_DIR, `--remote-debugging-port=${String(PORT)}`, `--user-data-dir=${profile}`], {
  cwd: workspace,
  env: { ...process.env, PATH: `${NPM_DIR};${process.env.PATH ?? ''}` },
  stdio: ['ignore', 'pipe', 'pipe']
})
child.stdout.on('data', () => undefined)
child.stderr.on('data', () => undefined)
try {
  let page
  for (let i = 0; i < 80 && page === undefined; i += 1) {
    await sleep(500)
    if (child.exitCode !== null) throw new Error(`app exited ${String(child.exitCode)}`)
    try {
      const list = await (await fetch(`http://127.0.0.1:${String(PORT)}/json/list`)).json()
      page = list.find((t) => t.type === 'page' && t.webSocketDebuggerUrl && !t.url.includes('#splash'))
    } catch { /* not up */ }
  }
  if (page === undefined) throw new Error('renderer never came up')
  const socket = new WebSocket(page.webSocketDebuggerUrl)
  await new Promise((res, rej) => { socket.addEventListener('open', res, { once: true }); socket.addEventListener('error', rej, { once: true }) })
  let id = 0
  const pending = new Map()
  socket.addEventListener('message', (event) => {
    const message = JSON.parse(event.data)
    const waiter = pending.get(message.id)
    if (waiter) { pending.delete(message.id); waiter(message) }
  })
  const evaluate = (expression) => new Promise((resolve_) => {
    const next = ++id
    const gaveUp = setTimeout(() => { if (pending.delete(next)) resolve_(undefined) }, 60_000)
    pending.set(next, (message) => {
      clearTimeout(gaveUp)
      const thrown = message.result?.exceptionDetails
      if (thrown !== undefined) say(`  eval threw: ${thrown.exception?.description ?? ''}`.slice(0, 200))
      resolve_(message.result?.result?.value)
    })
    socket.send(JSON.stringify({ id: next, method: 'Runtime.evaluate', params: { expression, awaitPromise: true, returnByValue: true } }))
  })

  say('1. the app starts on the seeded profile')
  const ready = await evaluate(`(async () => {
    for (let i = 0; i < 240; i += 1) {
      const field = document.querySelector('form.command-dock textarea')
      if (field && !/Checking local runtimes/.test(field.placeholder)) return true
      await new Promise(r => setTimeout(r, 250))
    }
    return false
  })()`)
  check('discovery finished', ready === true)

  say('2. open the mission and read the activity fold')
  const fold = await evaluate(`(async () => {
    const row = [...document.querySelectorAll('.lc-teammate__mission, .lc-row')].find(r => /flaky tests/i.test(r.innerText))
    if (!row) return JSON.stringify({ error: 'no mission row' })
    row.click()
    for (let i = 0; i < 40; i += 1) {
      await new Promise(r => setTimeout(r, 250))
      if (document.querySelector('.lc-activity')) break
    }
    const button = document.querySelector('.lc-activity')
    if (!button) return JSON.stringify({ error: 'no activity fold' })
    const summary = button.innerText.replace(/\\s+/g, ' ').trim()
    // Only when it is not already open. Since 0.49.0 a finished turn's fold
    // opens itself, so an unconditional click CLOSES it and every row below
    // then reads as absent -- a harness reporting a bare screen at an app
    // that is fine.
    if (button.getAttribute('aria-expanded') !== 'true') button.click()
    await new Promise(r => setTimeout(r, 300))
    const rows = [...document.querySelectorAll('.lc-filerow')].map(r => ({ helper: r.classList.contains('is-helper'), text: r.innerText.replace(/\\s+/g, ' ').trim() }))
    return JSON.stringify({ summary, rows })
  })()`)
  let f = {}
  try { f = JSON.parse(String(fold)) } catch { /* below */ }
  say(`       ${String(fold).slice(0, 300)}`)
  check('the summary counts the helper apart from tool calls', /asked 1 subagent/.test(String(f.summary)) && /1 tool call/.test(String(f.summary)), String(f.summary))
  const helperRow = (f.rows ?? []).find((r) => r.helper)
  check('the helper row says what it was asked and that it reported back', helperRow !== undefined && /Search the tests for flaky cases/.test(helperRow.text) && /helper/.test(helperRow.text) && /reported back/.test(helperRow.text), JSON.stringify(helperRow))
  check('the Read stays an ordinary tool row', (f.rows ?? []).some((r) => !r.helper && /status[.]ts/.test(r.text)))
} catch (error) {
  failures += 1
  say(`  [FAIL] ${error instanceof Error ? error.message : String(error)}`)
} finally {
  try { child.kill() } catch { /* gone */ }
  await sleep(1500)
  await rm(profile, { recursive: true, force: true }).catch(() => undefined)
  await rm(workspace, { recursive: true, force: true }).catch(() => undefined)
}
if (failures > 0) { say(`\n${String(failures)} HELPER ROW PROBE FAILURE(S)`); process.exit(1) }
say('\nHELPER ROW PROBE PASSED')
