// "Use a free model" lands on one that has answered here (0.517).
//
//   node _tools/drive-a-free-model-that-answered.mjs [--packaged <exe>] [--tag <name>]
//
// On 2026-09-30 the catalogue's first free model answered "Endpoint is
// unavailable" all day while others answered, and every "Use a free model"
// landed on it. A profile whose ledger holds one finished run on a free
// model that is NOT the catalogue's first: Home's "Use a free model" must
// put the chat box on that one. Sends nothing.

import { createHash } from 'node:crypto'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, recordRoot, say, scratchRepository, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'
// Not the catalogue's first free model (that has been Ling, then Muse).
const ANSWERED = process.env.LOCUST_ANSWERED_MODEL ?? 'opencode/mimo-v2.6-flash-free'

const root = localPath(new URL('..', import.meta.url))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-free-answered-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-free-answered-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const missionId = 'mission_5e000000-0000-4000-8000-000100000000'
const runId = 'run_5e0001'
const at = new Date(Date.now() - 3_600_000).toISOString()
await ledger.createMission({
  missionId, runId, prompt: 'Summarize the README in two sentences',
  runtime: 'opencode', model: ANSWERED, requestedRouteId: 'opencode', resolvedRouteId: 'opencode-account:default', cliVersion: null,
  workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at
})
// OpenCode's own events, as the ledger's tests write them.
const event = (sequence, type, payload) => ({ id: `event_${String(sequence)}`, runId, missionId, sequence, occurredAt: at, sourceAdapter: 'opencode', type, payload: { ...payload, evidence: { redacted: true } } })
await ledger.appendEvents(missionId, [
  event(1, 'message.delta', { itemId: 'answer', operation: 'append', text: 'A small web shop with a Node backend.', final: true }),
  event(2, 'run.completed', { usage: { inputTokens: 900, outputTokens: 40 }, process: { exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 2, inputDeliveryFailed: false, outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at, finishedAt: at } })
])
await ledger.flush()

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `free-model-that-answered-${tag}`,
  port: 9835,
  workspace,
  profilePath,
  sendsNothing: true,
  outPath: join(recordRoot('a-free-model-that-answered-2026-10-01'), tag),
  seed: { schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 260)}`}`)
}
const chip = `[...document.querySelectorAll('.lc-control')].find((b) => b.getAttribute('aria-haspopup') === 'listbox')?.innerText.replace(/\\s+/g, ' ').trim() ?? ''`
// The model's words as the picker shows them: "mimo-v2.6-flash" -> /mimo.*v2\.6.*flash/i.
const named = new RegExp(ANSWERED.replace(/^opencode\//, '').replace(/-free$/, '').split('-').map((word) => word.replace(/[.*+?^$()|[\]\\{}]/g, '\\$&')).join('.*'), 'i')

try {
  await drive.capture('launch', () => drive.ready())
  const home = JSON.parse(String(await drive.capture('Home, on the default route', () => drive.evaluate(`(async () => {
    for (let i = 0; i < 60 && ![...document.querySelectorAll('button')].some((b) => b.innerText.trim() === 'Use a free model'); i += 1) await new Promise((r) => setTimeout(r, 250))
    return JSON.stringify({ chip: ${chip}, button: [...document.querySelectorAll('button')].some((b) => b.innerText.trim() === 'Use a free model') })
  })()`))))
  check('Home offers "Use a free model"', home.button === true, JSON.stringify(home))
  const freed = JSON.parse(String(await drive.capture('Use a free model pressed', () => drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Use a free model')?.click()
    await new Promise((r) => setTimeout(r, 900))
    return JSON.stringify({ chip: ${chip} })
  })()`))))
  check(`pressed, the chat box is on the free model that answered here (${ANSWERED})`, /OpenCode/.test(freed.chip) && named.test(freed.chip), freed.chip)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. One finished run on ${ANSWERED}; nothing sent.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
