// Do conversations still open whole when the history is not sent again?
//
//   node _tools/probe-history-kept.mjs [--packaged <exe>] [--tag <name>]
//
// Batch C #1: a history read now keeps back the events of every record the
// window already holds, and the window keeps its own copy (historyMerge.ts).
// If the window dropped a kept record's events, a conversation reopened
// after a few reads would show its turns with nothing in them. Seeds three
// finished conversations -- the first with two turns -- and opens each, then
// the first again (each first open is a history read), reading the answers
// every time. Then asks the host the way the window does, to see records
// come back kept. SPENDS NOTHING.

import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

import { localPath, say, scratchRepository, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const OUT = join(localPath(new URL('../docs/beta-fixes-2026-09-23/', import.meta.url)), `history-kept-${tag}`)
await mkdir(OUT, { recursive: true })

const workspace = await scratchRepository('locust-history-kept-ws-')
const WORKSPACE_ID = `ws_${createHash('sha256').update(resolve(workspace), 'utf8').digest('hex').slice(0, 32)}`
const profile = await mkdtemp(join(tmpdir(), 'locust-history-kept-'))
await mkdir(join(profile, 'mission-ledger'), { recursive: true })

const TURNS = [
  { id: 'mission_a1', prompt: 'Read the README and say what the project is', answer: 'ANSWER A1: a control plane for teammates.', at: '2026-09-23T05:00:00.000Z' },
  { id: 'mission_a2', prompt: 'And what does it need next', answer: 'ANSWER A2: a smaller history read.', at: '2026-09-23T05:02:00.000Z', after: 'mission_a1' },
  { id: 'mission_b', prompt: 'List the open TODOs in the repo', answer: 'ANSWER B: three TODOs.', at: '2026-09-23T05:04:00.000Z' },
  { id: 'mission_c', prompt: 'Summarise what changed this week', answer: 'ANSWER C: the title screen.', at: '2026-09-23T05:06:00.000Z' }
]
for (const turn of TURNS) {
  const end = new Date(Date.parse(turn.at) + 20_000).toISOString()
  const proc = {
    exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 3, inputDeliveryFailed: false,
    outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: turn.at, finishedAt: end
  }
  const metadata = {
    missionId: turn.id, runId: `run_${turn.id}`, prompt: turn.prompt,
    runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default',
    cliVersion: '0.153.0', workspaceId: WORKSPACE_ID, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: turn.at,
    ...(turn.after === undefined ? {} : { continuesFrom: { missionId: turn.after, checkpointEpoch: 1, reason: 'follow-up' } })
  }
  const events = [
    { type: 'run.started', occurredAt: turn.at, payload: { runtimeThreadId: 'thread-1', evidence: { redacted: true } } },
    { type: 'message.delta', occurredAt: end, payload: { itemId: 'answer', operation: 'append', text: turn.answer, final: true, evidence: { redacted: true } } },
    { type: 'run.completed', occurredAt: end, payload: { process: proc, evidence: { redacted: true } } }
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
    missionOwners: Object.fromEntries(TURNS.map((turn) => [turn.id, 'tm_wren'])),
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false }
  }),
  'utf8'
)

const drive = await startDrive({
  name: `history-kept-${tag}`,
  port: 9436,
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

/** Open the conversation whose row reads `words`, then read the thread's answers. */
const open = (words) => `(async () => {
  let row
  for (let i = 0; i < 40 && !row; i += 1) {
    await new Promise((r) => setTimeout(r, 250))
    row = [...document.querySelectorAll('.lc-conv')].find((node) => node.textContent.includes(${JSON.stringify(words)}))
  }
  if (!row) return JSON.stringify({ error: 'no row for ' + ${JSON.stringify(words)} })
  row.click()
  await new Promise((r) => setTimeout(r, 1800))
  const answers = [...document.querySelectorAll('.lc-thread .lc-agentline__body')].map((node) => node.textContent.trim())
  return JSON.stringify({ answers })
})()`

try {
  await drive.ready()
  await drive.resize(1215, 800)
  const steps = [
    ['the two-turn conversation', 'Read the README', ['ANSWER A1', 'ANSWER A2']],
    ['the second conversation', 'List the open TODOs', ['ANSWER B']],
    ['the third conversation', 'Summarise what changed', ['ANSWER C']],
    ['the two-turn conversation again, three history reads later', 'Read the README', ['ANSWER A1', 'ANSWER A2']]
  ]
  for (const [what, words, wanted] of steps) {
    const read = JSON.parse(await drive.capture(`open ${what}`, () => drive.evaluate(open(words))))
    const answers = read.answers ?? []
    check(`${what} shows every answer`, wanted.every((word) => answers.some((answer) => answer.startsWith(word))), read.error ?? answers.join(' | '))
  }
  const asked = JSON.parse(await drive.evaluate(`(async () => {
    const first = await window.desktop.getMissionHistory()
    if (!first.ok) return JSON.stringify({ error: 'history unavailable' })
    const held = {}
    for (const m of first.data.missions) if (m.digest !== undefined && m.events.length > 0) held[m.missionId] = m.digest
    const again = await window.desktop.getMissionHistory(held)
    if (!again.ok) return JSON.stringify({ error: 'history unavailable' })
    return JSON.stringify({
      whole: first.data.missions.filter((m) => m.events.length > 0).length,
      digests: Object.keys(held).length,
      kept: again.data.missions.filter((m) => m.eventsKept === true).length,
      bytes: [JSON.stringify(first).length, JSON.stringify(again).length]
    })
  })()`))
  say(`asked the host: ${JSON.stringify(asked)}`)
  check('the host keeps back every record the window says it holds', asked.digests > 0 && asked.kept === asked.digests, JSON.stringify(asked))
  say(failures === 0 ? '\nHISTORY KEPT PASSED' : `\nHISTORY KEPT: ${String(failures)} FAILED`)
} catch (error) {
  say(`probe failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: 'Three seeded conversations opened in turn and the first again, reading every answer; then the history asked for as the window does. Nothing was sent.' })
}
