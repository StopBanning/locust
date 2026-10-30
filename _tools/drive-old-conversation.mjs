// Does an older conversation open with its replies (H3)?
//
//   node _tools/drive-old-conversation.mjs [--packaged <exe>] [--tag <name>]
//
// History sends the newest twenty missions whole and every other one as a
// row, and nothing fetched the rest: an older conversation opened on the
// person's words alone. The ledger is seeded, with the mission store's own
// writer, with 22 finished missions; the oldest is a two-turn conversation.
// It is opened from the sidebar, and both of its replies must be in the
// thread. Sends nothing.

import { mkdir, mkdtemp } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, say, scratchRepository, sleep, startDrive, recordRoot } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag')
const outPath = tag === undefined ? undefined : join(recordRoot('beta-fixes-2026-09-24'), `old-conversation-${tag}`)
if (outPath !== undefined) await mkdir(outPath, { recursive: true })

const adapters = await import(pathToFileURL(join(localPath(new URL('..', import.meta.url)), 'packages', 'runtime-adapters', 'dist', 'index.js')).href)
const store = await import(pathToFileURL(join(localPath(new URL('..', import.meta.url)), 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-oldconv-ws-')
const profilePath = await mkdtemp(join(tmpdir(), 'locust-drive-oldconv-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })

// 22 missions, oldest first; the first two are one conversation.
const base = Date.parse('2026-09-20T09:00:00.000Z')
for (let n = 0; n < 22; n += 1) {
  const missionId = `mission_seed_${String(n).padStart(2, '0')}`
  const runId = `run_seed_${String(n).padStart(2, '0')}`
  const at = new Date(base + n * 60_000).toISOString()
  await ledger.createMission({
    missionId, runId,
    prompt: n === 0 ? 'OLDEST QUESTION: what does the build do?' : n === 1 ? 'OLDEST FOLLOW-UP: and the tests?' : `Question number ${String(n)}.`,
    runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: null,
    workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at,
    ...(n === 1 ? { continuesFrom: { missionId: 'mission_seed_00', checkpointEpoch: 1, reason: 'follow-up' } } : {})
  })
  const reply = n === 0 ? 'OLDEST REPLY: it compiles the app.' : n === 1 ? 'OLDEST FOLLOW-UP REPLY: they run with pnpm test.' : `Reply number ${String(n)}.`
  // Real events, made the way a run makes them: Codex's own records through its normalizer.
  const normalizer = adapters.createCodexEventNormalizer({ runId, missionId, requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: '0.156.1', now: () => new Date(at) })
  await ledger.appendEvents(missionId, [
    ...normalizer.accept({ sequence: 1, raw: JSON.stringify({ type: 'thread.started', thread_id: `thread_${missionId}` }) }),
    ...normalizer.accept({ sequence: 2, raw: JSON.stringify({ type: 'item.completed', item: { id: 'answer', type: 'agent_message', text: reply } }) }),
    ...normalizer.accept({ sequence: 3, raw: JSON.stringify({ type: 'turn.completed' }) }),
    ...normalizer.finish({ exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 3, cancelled: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, startedAt: at, finishedAt: at })
  ])
}
say(`seeded 22 missions in ${workspaceId}`)

const drive = await startDrive({
  name: 'old-conversation',
  port: 9557,
  workspace,
  profilePath,
  sendsNothing: true,
  ...(packaged === undefined ? {} : { packaged }),
  ...(outPath === undefined ? {} : { outPath }),
  seed: { schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}
try {
  await drive.capture('launch: 22 missions in the ledger', () => drive.ready())
  await drive.resize(1215, 800)
  await sleep(2000)
  await drive.capture('open the oldest conversation from the sidebar', () => drive.evaluate(`(async () => {
    for (let i = 0; i < 40; i += 1) {
      const row = [...document.querySelectorAll('button, a, [role="button"], li')].find((el) => /OLDEST (QUESTION|FOLLOW-UP)/.test(el.textContent ?? '') && el.getBoundingClientRect().height > 0 && el.getBoundingClientRect().height < 120)
      if (row) { row.click(); await new Promise((r) => setTimeout(r, 2500)); return 'opened' }
      await new Promise((r) => setTimeout(r, 250))
    }
    return 'no row'
  })()`))
  const thread = String(await drive.evaluate(`document.querySelector('main')?.innerText ?? document.body.innerText`))
  check('the oldest conversation shows its first reply', thread.includes('OLDEST REPLY: it compiles the app.'))
  check('and its follow-up reply', thread.includes('OLDEST FOLLOW-UP REPLY: they run with pnpm test.'))
  check('and both of the person’s messages', thread.includes('OLDEST QUESTION') && thread.includes('OLDEST FOLLOW-UP: and the tests?'))
  say(failures === 0 ? '\nOLD CONVERSATION PASSED' : `\nOLD CONVERSATION: ${String(failures)} FAILED`)
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'whatever pnpm build last wrote to out/'}. 22 finished missions seeded; the oldest, a two-turn conversation, opened from the sidebar.` })
}
