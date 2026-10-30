// QA-2026-09-29 round 2, R1: a long conversation reopens with every reply.
//
//   node _tools/drive-a-long-conversation-reopens-whole.mjs [--packaged <exe>] [--tag <name>] [--turns 30]
//
// One conversation of 30 turns, written with the mission store's own writer
// and Codex's own normalizer (as everyday-ledger.mjs does). History sends the
// newest 20 missions with their events; the conversation is the newest in its
// folder, so the window adopts it at launch. Opening it from the sidebar must
// show all 30 replies -- before 0.466 it showed turns 11 to 30 only. Sends
// nothing.

import { mkdir, mkdtemp } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'
const TURNS = Number(arg('--turns') ?? '30')
const OUT = join(recordRoot('a-long-conversation-2026-09-29'), `a-long-conversation-${tag}`)
await mkdir(OUT, { recursive: true })

const root = localPath(new URL('..', import.meta.url))
const adapters = await import(pathToFileURL(join(root, 'packages', 'runtime-adapters', 'dist', 'index.js')).href)
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-long-ws-')
const profilePath = await mkdtemp(join(tmpdir(), 'locust-drive-long-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const WREN = { teammateId: 'tm_aaaaaaaaaaaaaaaaaaaaaaa1', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-08-10T09:00:00.000Z' }
const missionOwners = {}
const now = Date.now()
let previous
for (let t = 0; t < TURNS; t += 1) {
  const n = t + 1
  const missionId = `mission_10000000-0000-4000-8000-${String(n).padStart(12, '0')}`
  const runId = `run_10${String(n).padStart(4, '0')}`
  const at = new Date(now - (TURNS - t) * 120_000).toISOString()
  await ledger.createMission({
    missionId, runId, prompt: `Question number ${String(n)}`,
    runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: null,
    workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at,
    ...(previous === undefined ? {} : { continuesFrom: { missionId: previous, checkpointEpoch: 1, reason: 'follow-up' } })
  })
  let tick = 0
  const normalizer = adapters.createCodexEventNormalizer({ runId, missionId, requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: '0.156.1', now: () => new Date(Date.parse(at) + (tick++) * 1000) })
  await ledger.appendEvents(missionId, [
    ...normalizer.accept({ sequence: 1, raw: JSON.stringify({ type: 'thread.started', thread_id: 'thread_long' }) }),
    ...normalizer.accept({ sequence: 2, raw: JSON.stringify({ type: 'item.completed', item: { id: 'answer', type: 'agent_message', text: `Reply-${String(n)}-done.` } }) }),
    ...normalizer.accept({ sequence: 3, raw: JSON.stringify({ type: 'turn.completed', usage: { input_tokens: 3000, cached_input_tokens: 0, output_tokens: 200 } }) }),
    ...normalizer.finish({ exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 3, cancelled: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, startedAt: at, finishedAt: new Date(Date.parse(at) + 20_000).toISOString() })
  ])
  missionOwners[missionId] = WREN.teammateId
  previous = missionId
}
say(`seeded one conversation of ${String(TURNS)} turns`)

const drive = await startDrive({
  name: `a-long-conversation-${tag}`, port: 9791, workspace, profilePath, outPath: OUT, sendsNothing: true,
  ...(packaged === undefined ? {} : { packaged }),
  seed: { schemaVersion: 1, teammates: [WREN], missionOwners, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}
const repliesShown = `(() => {
  const text = document.querySelector('.lc-thread')?.innerText ?? ''
  return JSON.stringify([...text.matchAll(/Reply-(\\d+)-done/g)].map((m) => Number(m[1])))
})()`
try {
  await drive.ready()
  await drive.resize(1440, 900)
  await sleep(2500)
  const clicked = String(await drive.evaluate(`(() => {
    const row = [...document.querySelectorAll('.lc-convrow')].find((r) => /Question number/.test(r.innerText))
    ;(row?.querySelector('.lc-conv') ?? row)?.click()
    return row === undefined ? 'no row' : row.innerText.replace(/\\s+/g, ' ')
  })()`))
  say(`  opened: ${clicked}`)
  await sleep(2500)
  const shown = JSON.parse(String(await drive.capture('the conversation, opened from the sidebar after launch', () => drive.evaluate(repliesShown))))
  const unique = [...new Set(shown)].sort((a, b) => a - b)
  const missing = Array.from({ length: TURNS }, (_, i) => i + 1).filter((n) => !unique.includes(n))
  check(`all ${String(TURNS)} replies are shown`, missing.length === 0, missing.length === 0 ? `${String(unique.length)} replies` : `missing ${missing.join(', ')}`)
  // The thread's own scrolling box: the nearest one taller inside than out.
  await drive.evaluate(`(() => { let box = document.querySelector('.lc-thread'); while (box && box.scrollHeight <= box.clientHeight + 1) box = box.parentElement; box?.scrollTo(0, 0) })()`)
  await sleep(500)
  await drive.capture('the top of the conversation', () => drive.evaluate(repliesShown))
} finally {
  await drive.finish({ intro: 'One conversation of ' + String(TURNS) + ' turns, opened from the sidebar after launch (QA round 2, R1).' })
}
say('')
say(`record: ${OUT}`)
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exitCode = failures === 0 ? 0 : 1
