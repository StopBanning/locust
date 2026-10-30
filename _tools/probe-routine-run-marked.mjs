// Can two sidebar rows with one title be told apart without opening them?
//
//   node _tools/probe-routine-run-marked.mjs [--packaged <exe>] [--tag <name>]
//
// The 0.271 design recheck, finding 2: running a saved routine added a row
// with the same title as the conversation it was saved from, and the rows
// "do not reliably identify ... the routine". Seeds Wren's conversation and
// the routine's replay of it, same words, and reads the two rows. SPENDS
// NOTHING: both runs are written to the ledger, none is started.

import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

import { localPath, say, scratchRepository, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const OUT = join(localPath(new URL('../docs/beta-fixes-2026-09-23/', import.meta.url)), `routine-run-marked-${tag}`)
await mkdir(OUT, { recursive: true })

const workspace = await scratchRepository('locust-routine-mark-ws-')
const WORKSPACE_ID = `ws_${createHash('sha256').update(resolve(workspace), 'utf8').digest('hex').slice(0, 32)}`
const profile = await mkdtemp(join(tmpdir(), 'locust-routine-mark-'))
await mkdir(join(profile, 'mission-ledger'), { recursive: true })

const PROMPT = 'Read the README and list the TODOs'
const ROUTE = { runtime: 'codex', model: 'account-default', mode: 'ask' }
const RUNS = [
  { id: 'mission_person', at: '2026-09-23T05:00:00.000Z' },
  { id: 'mission_replay', at: '2026-09-23T06:00:00.000Z', startedBy: { kind: 'routine', routineId: 'rt_morning', step: 1 } }
]
for (const run of RUNS) {
  const end = new Date(Date.parse(run.at) + 20_000).toISOString()
  const proc = {
    exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 3, inputDeliveryFailed: false,
    outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: run.at, finishedAt: end
  }
  const metadata = {
    missionId: run.id, runId: `run_${run.id}`, prompt: PROMPT,
    runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default',
    cliVersion: '0.153.0', workspaceId: WORKSPACE_ID, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: run.at,
    ...(run.startedBy === undefined ? {} : { startedBy: run.startedBy })
  }
  const events = [
    { type: 'run.started', occurredAt: run.at, payload: { runtimeThreadId: 'thread-1', evidence: { redacted: true } } },
    { type: 'message.delta', occurredAt: end, payload: { itemId: 'answer', operation: 'append', text: 'Two TODOs.', final: true, evidence: { redacted: true } } },
    { type: 'run.completed', occurredAt: end, payload: { process: proc, evidence: { redacted: true } } }
  ]
  const lines = [JSON.stringify({ schemaVersion: 13, recordType: 'mission.created', ledgerSequence: 1, occurredAt: run.at, metadata })]
  events.forEach((event, at) => {
    lines.push(JSON.stringify({
      schemaVersion: 13, recordType: 'mission.event', ledgerSequence: at + 2, occurredAt: event.occurredAt,
      event: { ...event, id: `${run.id}:${String(at + 1)}`, runId: `run_${run.id}`, missionId: run.id, sequence: at + 1, sourceAdapter: 'codex' }
    }))
  })
  await writeFile(join(profile, 'mission-ledger', `${run.id}.jsonl`), `${lines.join('\n')}\n`, 'utf8')
}
await writeFile(
  join(profile, 'teammates.json'),
  JSON.stringify({
    schemaVersion: 1,
    teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-09-05T01:00:00.000Z' }],
    missionOwners: { mission_person: 'tm_wren', mission_replay: 'tm_wren' },
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false }
  }),
  'utf8'
)
await writeFile(
  join(profile, 'routines.json'),
  JSON.stringify({
    schemaVersion: 1,
    routines: [{ routineId: 'rt_morning', name: 'Morning check', teammateId: 'tm_wren', route: ROUTE, steps: [PROMPT], learnedFrom: ['mission_person'], createdAt: '2026-09-23T05:30:00.000Z', runs: 1, lastRunAt: '2026-09-23T06:00:20.000Z' }]
  }),
  'utf8'
)

const drive = await startDrive({
  name: `routine-run-marked-${tag}`,
  port: 9425,
  workspace,
  profilePath: profile,
  sendsNothing: true,
  outPath: OUT,
  ...(packaged === undefined ? {} : { packaged })
})

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}

try {
  await drive.ready()
  await drive.resize(1215, 800)
  const rows = JSON.parse(await drive.capture('the two rows, same words', () => drive.evaluate(`(async () => {
    for (let i = 0; i < 40 && document.querySelectorAll('.lc-conv').length < 2; i += 1) await new Promise((r) => setTimeout(r, 250))
    return JSON.stringify([...document.querySelectorAll('.lc-conv')].map((row) => ({
      text: row.innerText.replace(/\\s+/g, ' ').trim(),
      hover: row.getAttribute('title') ?? '',
      mark: row.querySelector('.lc-conv__routine')?.getAttribute('aria-label') ?? '',
      face: row.querySelector('[data-teammate]')?.getAttribute('aria-label') ?? ''
    })))
  })()`)))
  say(`rows: ${JSON.stringify(rows)}`)
  const marked = rows.filter((row) => row.mark.length > 0)
  check('both conversations are listed', rows.length === 2, String(rows.length))
  check("exactly one carries the routine's mark, naming the routine", marked.length === 1 && marked[0].mark === 'From the routine Morning check', JSON.stringify(marked.map((row) => row.mark)))
  check('the two rows read differently on hover', rows.length === 2 && rows[0].hover !== rows[1].hover, rows.map((row) => row.hover).join(' | '))
  check("each face names its teammate", rows.every((row) => row.face === 'Wren'), rows.map((row) => row.face).join(', '))
  say(failures === 0 ? '\nROUTINE RUN MARKED PASSED' : `\nROUTINE RUN MARKED: ${String(failures)} FAILED`)
} catch (error) {
  say(`probe failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: "Wren's conversation and the routine's replay of it, with the same words, as the sidebar lists them. Nothing was sent." })
}
