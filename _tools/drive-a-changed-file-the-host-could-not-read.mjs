// A file the runtime named but the host could not read: what does the row say (0.597)?
//
//   node _tools/drive-a-changed-file-the-host-could-not-read.mjs [--packaged <exe>] [--tag <name>]
//
// Two seeded Codex turns in one profile, no model call, nothing spent. Codex's
// file_change names a path and sends no diff, so its row depends on the host's
// disk observation. In a dot-folder workspace (Colin's is ~/.claude) the host
// sees the file change by name and never reads it (0.489). Before 0.597 the
// host wrote no event for that case and the row read "Codex CLI did not report
// the change" -- the first turn here, seeded as that host left it. Since 0.597
// the host writes the observation with the status `reported by the runtime,
// changed on disk` -- the second turn, seeded as the new host leaves it -- and
// the row reads "changed · seen on disk" beside the runtime's own word.
//
// The host side (that codex-mission.ts now emits the pair for such a path) is
// a unit test; this drive is the renderer on the built app, on the events the
// host writes. A live Codex run would spend his account; a live Antigravity
// run was measured 10/04 to carry its own diff on today's CLI, so neither can
// show this case for free.

import { createHash } from 'node:crypto'
import { mkdir, mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, openAllStepsScript, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'
const OUT = join(recordRoot('beta-fixes-2026-09-24'), `a-changed-file-the-host-could-not-read-${tag}`)
await mkdir(OUT, { recursive: true })

const root = localPath(new URL('..', import.meta.url))
const adapters = await import(pathToFileURL(join(root, 'packages', 'runtime-adapters', 'dist', 'index.js')).href)
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)

const workspace = await scratchRepository('locust-drive-unread-ws-')
const profilePath = await mkdtemp(join(tmpdir(), 'locust-drive-unread-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const WREN = { teammateId: 'tm_aaaaaaaaaaaaaaaaaaaaaaa1', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-08-10T09:00:00.000Z' }
const NOTES = `${workspace.replace(/\\/g, '/')}/NOTES.md`

/** One finished Codex turn that changed NOTES.md through a file_change (a path, no diff). */
async function seedTurn({ missionId, runId, prompt, startedAgoMs, observed }) {
  const at = new Date(Date.now() - startedAgoMs).toISOString()
  await ledger.createMission({
    missionId, runId, prompt,
    runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: null,
    workspaceId, sandbox: 'workspace-write', executionPolicyVersion: 1, createdAt: at
  })
  let tick = 0
  const normalizer = adapters.createCodexEventNormalizer({ runId, missionId, requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: '0.156.1', now: () => new Date(Date.parse(at) + (tick++) * 1000) })
  const records = [
    { type: 'thread.started', thread_id: `thread_${runId}` },
    { type: 'turn.started' },
    { type: 'item.started', item: { id: 'fc1', type: 'file_change', status: 'in_progress', changes: [{ path: NOTES, kind: 'update' }] } },
    { type: 'item.completed', item: { id: 'fc1', type: 'file_change', status: 'completed', changes: [{ path: NOTES, kind: 'update' }] } },
    { type: 'item.completed', item: { id: 'answer', type: 'agent_message', text: 'Added the passphrase line to NOTES.md.' } },
    { type: 'turn.completed', usage: { input_tokens: 2000, cached_input_tokens: 0, output_tokens: 80 } }
  ]
  const events = [
    ...records.flatMap((record, i) => normalizer.accept({ sequence: i + 1, raw: JSON.stringify(record) })),
    ...normalizer.finish({ exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: records.length, cancelled: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, startedAt: at, finishedAt: new Date(Date.parse(at) + 12_000).toISOString() })
  ]
  if (observed) {
    // What the 0.597 host writes for a named path it saw change and could not read: the pair, no patch.
    let sequence = Math.max(...events.map((event) => event.sequence)) + 1
    const base = { runId, missionId, occurredAt: new Date(Date.parse(at) + 13_000).toISOString(), sourceAdapter: 'codex' }
    const payload = { itemId: 'disk-observed-1', toolKind: 'observed_edit', name: 'edit', command: 'NOTES.md', status: 'reported by the runtime, changed on disk', evidence: { redacted: true } }
    events.push({ ...base, id: `${runId}:disk:${String(sequence)}`, sequence, type: 'tool.started', payload: { ...payload, phase: 'started' } })
    sequence += 1
    events.push({ ...base, id: `${runId}:disk:${String(sequence)}`, sequence, type: 'tool.completed', payload: { ...payload, phase: 'completed' } })
  }
  await ledger.appendEvents(missionId, events)
}

const BEFORE = { missionId: 'mission_20000000-0000-4000-8000-000000000001', runId: 'run_200001', prompt: 'Before: add the passphrase line to NOTES.md' }
const AFTER = { missionId: 'mission_20000000-0000-4000-8000-000000000002', runId: 'run_200002', prompt: 'After: add the passphrase line to NOTES.md' }
await seedTurn({ ...BEFORE, startedAgoMs: 600_000, observed: false })
await seedTurn({ ...AFTER, startedAgoMs: 120_000, observed: true })
say('seeded two finished Codex turns: one as the old host left it, one with the 0.597 observation')

const drive = await startDrive({
  name: `a-changed-file-the-host-could-not-read-${tag}`, port: 9794, workspace, profilePath, outPath: OUT, sendsNothing: true,
  ...(packaged === undefined ? {} : { packaged }),
  seed: { schemaVersion: 1, teammates: [WREN], missionOwners: { [BEFORE.missionId]: WREN.teammateId, [AFTER.missionId]: WREN.teammateId }, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } }
})

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}
const rowsOf = async (word) => {
  const opened = String(await drive.evaluate(`(() => {
    const row = [...document.querySelectorAll('button.lc-conv')].find((b) => b.innerText.includes(${JSON.stringify(word)}))
    if (!row) return 'no conversation row for ' + ${JSON.stringify(word)}
    row.click()
    return 'opened'
  })()`))
  if (opened !== 'opened') return { opened, rows: [] }
  await sleep(1500)
  await drive.evaluate(openAllStepsScript())
  await drive.evaluate(`(() => { for (const b of document.querySelectorAll('.lc-thread .lc-activity__head[aria-expanded="false"], .lc-thread button[aria-expanded="false"].lc-activity__toggle')) b.click(); return true })()`)
  await sleep(600)
  const rows = JSON.parse(String(await drive.evaluate(`JSON.stringify([...document.querySelectorAll('.lc-thread .lc-filerow')].map((row) => row.innerText.replace(/\\s+/g, ' ').trim()).filter((text) => /NOTES\\.md/.test(text)))`)))
  return { opened, rows }
}
try {
  await drive.ready()
  await drive.resize(1440, 900)
  await sleep(2500)
  const before = await rowsOf('Before:')
  await drive.capture('before 0.597: the host wrote nothing for the unread file', async () => JSON.stringify(before.rows))
  check('the old turn opens and has a NOTES.md row', before.opened === 'opened' && before.rows.length > 0, before.opened + ' ' + JSON.stringify(before.rows).slice(0, 200))
  check('CONTROL: as the old host left it, the row says the runtime did not report the change', before.rows.some((text) => /did not report the change/.test(text)), before.rows.join(' || '))
  const after = await rowsOf('After:')
  await drive.capture('0.597: the host saw it change and said so', async () => JSON.stringify(after.rows))
  check('the new turn opens and has a NOTES.md row', after.opened === 'opened' && after.rows.length > 0, after.opened + ' ' + JSON.stringify(after.rows).slice(0, 200))
  check('the row says the host saw it change', after.rows.some((text) => /changed · seen on disk/.test(text)), after.rows.join(' || '))
  check('and no row says the runtime did not report it', !after.rows.some((text) => /did not report the change/.test(text)), after.rows.join(' || '))
  // Codex's word for a file_change is "changed", which the status says already: once, not "changed changed".
  check('and reads "NOTES.md changed · seen on disk", the word once', after.rows.every((text) => /^NOTES\.md changed · seen on disk$/.test(text)), after.rows.join(' || '))
  say(failures === 0 ? '\nA CHANGED FILE THE HOST COULD NOT READ PASSED' : `\nA CHANGED FILE THE HOST COULD NOT READ: ${String(failures)} FAILED`)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'whatever pnpm build last wrote to out/'}. Two seeded Codex turns that changed NOTES.md by name: one as the pre-0.597 host left it, one with the 0.597 observation of a file it could not read.` })
}
