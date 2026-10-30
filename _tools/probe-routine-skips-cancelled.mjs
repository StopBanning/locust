// Does "Save as routine" propose only the turns that worked?
//
//   node _tools/probe-routine-skips-cancelled.mjs [--packaged <exe>] [--tag <name>]
//
// The 0.271 design recheck: Save as routine proposed a CANCELLED prompt as a
// step. A three-turn conversation is seeded -- the first turn stopped, the
// next two finished -- and the routine is saved through the row's visible
// menu. The draft should hold the two that finished. SPENDS NOTHING.

import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

import { localPath, say, scratchRepository, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const OUT = join(localPath(new URL('../docs/beta-fixes-2026-09-23/', import.meta.url)), `routine-skips-cancelled-${tag}`)
await mkdir(OUT, { recursive: true })

const workspace = await scratchRepository('locust-routine-cancel-ws-')
const WORKSPACE_ID = `ws_${createHash('sha256').update(resolve(workspace), 'utf8').digest('hex').slice(0, 32)}`
const profile = await mkdtemp(join(tmpdir(), 'locust-routine-cancel-'))
await mkdir(join(profile, 'mission-ledger'), { recursive: true })

const TURNS = [
  { id: 'mission_turn1', prompt: 'Summarise the whole repository history', ends: 'run.cancelled', at: '2026-09-23T05:00:00.000Z' },
  { id: 'mission_turn2', prompt: 'Summarise what changed today', ends: 'run.completed', at: '2026-09-23T05:02:00.000Z' },
  { id: 'mission_turn3', prompt: 'List the open TODOs', ends: 'run.completed', at: '2026-09-23T05:04:00.000Z' }
]
for (const [index, turn] of TURNS.entries()) {
  const end = new Date(Date.parse(turn.at) + 20_000).toISOString()
  const proc = {
    exitCode: turn.ends === 'run.completed' ? 0 : null, signal: null, stderr: '', stderrTruncated: false, recordCount: 3, inputDeliveryFailed: false,
    outputLimitExceeded: false, forcedTerminationAttempted: turn.ends !== 'run.completed', terminationUnconfirmed: false, startedAt: turn.at, finishedAt: end
  }
  const metadata = {
    missionId: turn.id, runId: `run_${turn.id}`, prompt: turn.prompt,
    runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default',
    cliVersion: '0.153.0', workspaceId: WORKSPACE_ID, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: turn.at,
    ...(index === 0 ? {} : { continuesFrom: { missionId: TURNS[index - 1].id, checkpointEpoch: 1, reason: 'follow-up' } })
  }
  const events = [
    { type: 'run.started', occurredAt: turn.at, payload: { runtimeThreadId: 'thread-1', evidence: { redacted: true } } },
    ...(turn.ends === 'run.completed'
      ? [{ type: 'message.delta', occurredAt: end, payload: { itemId: 'answer', operation: 'append', text: 'Done.', final: true, evidence: { redacted: true } } }]
      : []),
    { type: turn.ends, occurredAt: end, payload: { process: proc, evidence: { redacted: true } } }
  ]
  const lines = [JSON.stringify({ schemaVersion: 13, recordType: 'mission.created', ledgerSequence: 1, occurredAt: turn.at, metadata })]
  events.forEach((event, at) => {
    lines.push(JSON.stringify({
      schemaVersion: 13, recordType: 'mission.event', ledgerSequence: at + 2, occurredAt: event.occurredAt,
      event: { ...event, id: `${turn.id}:${String(at + 1)}`, runId: `run_${turn.id}`, missionId: turn.id, sequence: at + 1, sourceAdapter: 'codex' }
    }))
  })
  await writeFile(join(profile, 'mission-ledger', `${turn.id}.jsonl`), `${lines.join('\n')}\n`, 'utf8')
}
await writeFile(
  join(profile, 'teammates.json'),
  JSON.stringify({
    schemaVersion: 1,
    teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-09-05T01:00:00.000Z' }],
    missionOwners: { mission_turn1: 'tm_wren', mission_turn2: 'tm_wren', mission_turn3: 'tm_wren' },
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false }
  }),
  'utf8'
)

const drive = await startDrive({
  name: `routine-skips-cancelled-${tag}`,
  port: 9424,
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
  const steps = JSON.parse(await drive.capture('Save as routine, from the row menu', () => drive.evaluate(`(async () => {
    // .lc-conv is the sidebar's conversation row; its context menu is the
    // row menu (Sidebar.tsx), where the item reads "Save conversation as
    // routine".
    let row
    for (let i = 0; i < 40 && !row; i += 1) {
      await new Promise((r) => setTimeout(r, 250))
      row = document.querySelector('.lc-conv')
    }
    if (!row) return JSON.stringify({ error: 'no conversation row in the sidebar' })
    const box = row.getBoundingClientRect()
    row.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, clientX: box.left + 20, clientY: box.top + 8 }))
    await new Promise((r) => setTimeout(r, 500))
    const item = [...document.querySelectorAll('[role=menu] button, [role=menuitem]')].find((b) => /Save (conversation )?as routine/i.test(b.innerText))
    if (!item) return JSON.stringify({ error: 'no Save as routine item; menu: ' + [...document.querySelectorAll('[role=menu] button, [role=menuitem]')].map((b) => b.innerText.trim()).join(' / ') })
    if (item.disabled) return JSON.stringify({ error: 'disabled: ' + (item.getAttribute('title') ?? '') })
    item.click()
    for (let i = 0; i < 20 && !document.querySelector('.lc-routinestep__text'); i += 1) await new Promise((r) => setTimeout(r, 200))
    return JSON.stringify({ steps: [...document.querySelectorAll('.lc-routinestep__text')].map((field) => field.value) })
  })()`)))
  say(`draft: ${JSON.stringify(steps)}`)
  const drafted = steps.steps ?? []
  check('the draft opened with steps', drafted.length > 0, steps.error)
  check('the turn that was stopped is not a step', !drafted.includes(TURNS[0].prompt), JSON.stringify(drafted))
  check('the two that finished are, in order', drafted.join(' | ') === `${TURNS[1].prompt} | ${TURNS[2].prompt}`, JSON.stringify(drafted))
  say(failures === 0 ? '\nROUTINE SKIPS CANCELLED PASSED' : `\nROUTINE SKIPS CANCELLED: ${String(failures)} FAILED`)
} catch (error) {
  say(`probe failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: 'A seeded conversation whose first turn was stopped, saved as a routine. Nothing was sent.' })
}
