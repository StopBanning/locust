// A long turn keeps what it said, read back from its record (0.627).
//
//   node _tools/drive-a-long-turn-keeps-what-it-said.mjs [--packaged <exe>] [--tag <name>]
//
// Colin, 2026-10-05, of a Claude Code / Sonnet 5.5 teammate 35 minutes in: "complete radio silence and one
// stacked bar for 40 min?" Locust drew only the latest 500 of a turn's events, live and from the record, so
// a long run's messages fell off the screen. This seeds one finished Claude Code turn shaped like his -- a
// message, 40 calls, a second message, 50 more calls, over 900 events -- opens it, and checks both messages
// are drawn, in order, with no "kept in its record, not drawn" line (it fits the window). Sends nothing.

import { createHash } from 'node:crypto'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'
const FIRST = 'Setting up the worktree and reading how the check is wired.'
const SECOND = 'Install finished. Building the app next, then the timing drive.'

const root = localPath(new URL('..', import.meta.url))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-long-turn-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-long-turn-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const missionId = 'mission_10000000-0000-4000-8000-000000000627'
const runId = 'run_long_turn'
const at = new Date(Date.now() - 3_600_000).toISOString()
await ledger.createMission({
  missionId, runId, prompt: 'have fun', runtime: 'claude', model: 'claude-sonnet-5-5', requestedRouteId: 'claude',
  resolvedRouteId: 'claude-account:default', cliVersion: null, workspaceId, sandbox: 'workspace-write', executionPolicyVersion: 1, createdAt: at
})
let sequence = 0
const event = (type, payload) => {
  sequence += 1
  return { id: `event_${String(sequence)}`, runId, missionId, sequence, occurredAt: at, sourceAdapter: 'claude', type, payload: { ...payload, evidence: { redacted: true } } }
}
const call = (n) => {
  const itemId = `toolu_${String(n)}`
  return [
    event('tool.started', { itemId, toolKind: 'tool_use', name: 'Bash', phase: 'started' }),
    event('tool.started', { itemId, toolKind: 'tool_use', name: 'Bash', command: `echo ${String(n)}`, phase: 'started' }),
    event('tool.completed', { itemId, toolKind: 'tool_use', name: 'Bash', command: `echo ${String(n)}`, exitCode: 0, phase: 'completed' }),
    event('tool.completed', { itemId, toolKind: 'tool_use', name: 'Bash', command: `echo ${String(n)}`, exitCode: 0, phase: 'completed' }),
    ...Array.from({ length: 6 }, (_, at) => event('tool.started', { itemId: `${itemId}_${String(at)}`, toolKind: 'tool_use', name: 'Read', phase: 'started' })),
    ...Array.from({ length: 6 }, (_, at) => event('tool.completed', { itemId: `${itemId}_${String(at)}`, toolKind: 'tool_use', name: 'Read', phase: 'completed' }))
  ]
}
const events = [
  event('run.started', { runtimeThreadId: 'session-long-turn' }),
  ...call(1),
  event('message.delta', { itemId: 'm1', operation: 'append', text: FIRST, final: true }),
  ...Array.from({ length: 40 }, (_, n) => call(2 + n)).flat(),
  event('message.delta', { itemId: 'm2', operation: 'append', text: SECOND, final: true }),
  ...Array.from({ length: 50 }, (_, n) => call(42 + n)).flat()
]
events.push(event('run.completed', { usage: { inputTokens: 900, outputTokens: 90 }, process: { exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: events.length, inputDeliveryFailed: false, outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at, finishedAt: at } }))
await ledger.appendEvents(missionId, events)
await ledger.flush()
say(`seeded one finished turn of ${String(events.length)} events`)

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `a-long-turn-keeps-what-it-said-${tag}`, port: 9927, workspace, profilePath, sendsNothing: true,
  outPath: join(recordRoot('a-long-turn-keeps-what-it-said-2026-10-05'), tag),
  seed: { schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 400)}`}`)
}
try {
  await drive.ready()
  await drive.resize(1200, 780)
  await sleep(2500)
  const seen = JSON.parse(String(await drive.capture('the long turn, opened', () => drive.evaluate(`(async () => {
    let row
    for (let i = 0; i < 40 && !row; i += 1) {
      row = [...document.querySelectorAll('.lc-conv')].find((one) => /have fun/.test(one.textContent))
      if (!row) await new Promise((r) => setTimeout(r, 500))
    }
    row?.click()
    for (let i = 0; i < 40 && !document.querySelector('.lc-thread .lc-agentline__body'); i += 1) await new Promise((r) => setTimeout(r, 250))
    await new Promise((r) => setTimeout(r, 1500))
    const thread = document.querySelector('.lc-thread')?.innerText ?? ''
    return JSON.stringify({ opened: Boolean(row), first: thread.indexOf(${JSON.stringify(FIRST)}), second: thread.indexOf(${JSON.stringify(SECOND)}), trimmed: thread.includes('kept in its record, not drawn') })
  })()`))))
  check('the conversation opens', seen.opened === true, JSON.stringify(seen))
  check('its first message is drawn', seen.first >= 0, JSON.stringify(seen))
  check('its second message is drawn, after the first', seen.second > seen.first && seen.first >= 0, JSON.stringify(seen))
  check('no "kept in its record" line: the turn fits the window', seen.trimmed === false, JSON.stringify(seen))
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. One finished ${String(events.length)}-event turn, opened from its record.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
