// An older comparison shows every column's answer, and Home leaves it (0.569).
//
//   node _tools/drive-an-old-comparison-shows-every-answer.mjs [--packaged <exe>] [--tag <name>]
//
// Colin, 2026-10-03, on a day-old three-way compare (an arcade game):
// "disappeared from all models except claude" -- the first column answered,
// the other two read "No answer was recorded", their ledgers whole on disk.
// History sends the newest twenty turns whole and older ones as rows; opening
// the comparison read only the turn it was opened through. And "you cant
// click to home from the compare screen": Home cleared the teammate and the
// conversation, never the comparison, and one started with nobody's teammate
// stayed on screen.
//
// Seeded: a three-column comparison with no teammate, then 22 newer turns, so
// the comparison's turns arrive without their events. Opening it must show
// all three answers; Home must then show the home screen. Sends nothing.

import { createHash } from 'node:crypto'
import { mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'

const root = localPath(new URL('..', import.meta.url))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-old-compare-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-old-compare-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const COLUMNS = [
  { slot: 'a', runtime: 'opencode', model: 'opencode/mimo-v2.6-flash-free', label: 'Mimo V2.6 Flash', answer: 'MIMO-ANSWER: dodge the locusts with the arrow keys.' },
  { slot: 'b', runtime: 'opencode', model: 'opencode/ling-3.0-flash-fin-free', label: 'Ling 3.0 Flash', answer: 'LING-ANSWER: a canvas game with a score.' },
  { slot: 'c', runtime: 'opencode', model: 'opencode/muse-spark-1.3-contributor-free', label: 'Muse Spark 1.3', answer: 'MUSE-ANSWER: falling locusts, one life.' }
]
const PROMPT = 'make a small browser arcade game where you dodge falling locusts'
let n = 0
async function turn(prompt, route, text, createdAt) {
  n += 1
  const missionId = `mission_5e000000-0000-4000-8000-0007${String(n).padStart(8, '0')}`
  const runId = `run_5e0007${String(n)}`
  await ledger.createMission({
    missionId, runId, prompt,
    runtime: route.runtime, model: route.model, requestedRouteId: 'opencode', resolvedRouteId: 'opencode-account:default', cliVersion: null,
    workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt
  })
  const ended = new Date(Date.parse(createdAt) + 20_000).toISOString()
  const event = (sequence, type, occurredAt, payload) => ({ id: `event_${String(n)}_${String(sequence)}`, runId, missionId, sequence, occurredAt, sourceAdapter: 'opencode', type, payload: { ...payload, evidence: { redacted: true } } })
  await ledger.appendEvents(missionId, [
    event(1, 'message.delta', createdAt, { itemId: 'answer', operation: 'append', text, final: true }),
    event(2, 'run.completed', ended, { usage: { inputTokens: 900, outputTokens: 200 }, process: { exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 2, inputDeliveryFailed: false, outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: createdAt, finishedAt: ended } })
  ])
  return missionId
}
const day = Date.now() - 86_400_000
const slots = []
for (const column of COLUMNS) {
  const missionId = await turn(PROMPT, column, column.answer, new Date(day).toISOString())
  slots.push({ slot: column.slot, route: { runtime: column.runtime, model: column.model, label: column.label }, missionIds: [missionId] })
}
// Newer than it: the twenty-two turns that push the comparison out of the history sent whole.
for (let i = 0; i < 22; i += 1) await turn(`Newer question ${String(i + 1)}`, COLUMNS[0], `Newer answer ${String(i + 1)}.`, new Date(day + (i + 1) * 600_000).toISOString())
await ledger.flush?.()
await writeFile(join(profilePath, 'compares.json'), JSON.stringify({ schemaVersion: 1, compares: [{ compareId: 'cmp_arcade', prompt: PROMPT, createdAt: new Date(day).toISOString(), slots }] }), 'utf8')

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `old-compare-${tag}`,
  port: 9890,
  workspace,
  profilePath,
  sendsNothing: true,
  outPath: join(recordRoot('an-old-comparison-shows-every-answer-2026-10-03'), tag),
  seed: { schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 500)}`}`)
}

try {
  await drive.capture('launch', () => drive.ready())
  await drive.resize(1440, 900)
  const opened = JSON.parse(String(await drive.capture('the day-old comparison, opened', () => drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('.lc-convrow button.lc-conv')].find((r) => /arcade/i.test(r.innerText))?.click()
    await new Promise((r) => setTimeout(r, 1500))
    ;[...document.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Open the comparison')?.click()
    for (let i = 0; i < 20 && !document.querySelector('.lc-compare'); i += 1) await new Promise((r) => setTimeout(r, 250))
    await new Promise((r) => setTimeout(r, 2500))
    return JSON.stringify({
      compare: !!document.querySelector('.lc-compare'),
      cells: [...document.querySelectorAll('.lc-compare__cell')].map((c) => c.innerText.replace(/\\s+/g, ' ').trim().slice(0, 80))
    })
  })()`))))
  check('the comparison opens', opened.compare, JSON.stringify(opened))
  for (const column of COLUMNS) {
    const word = column.answer.split(':')[0]
    check(`${column.label}'s answer is shown`, opened.cells.some((cell) => cell.includes(word)), JSON.stringify(opened.cells))
  }
  check('no column says no answer was recorded', !opened.cells.some((cell) => /No answer was recorded/.test(cell)), JSON.stringify(opened.cells))
  const controls = JSON.parse(String(await drive.capture('comparison controls', () => drive.evaluate(`JSON.stringify({
    title: document.querySelector('.lc-control__versus')?.closest('button')?.title
  })`))))
  check('the route hover names every compared model', COLUMNS.every((column) => controls.title?.includes(column.label)), controls.title)
  const home = JSON.parse(String(await drive.capture('Home, from the comparison', () => drive.evaluate(`(async () => {
    document.querySelector('.lc-brand__lockup')?.click()
    await new Promise((r) => setTimeout(r, 1500))
    return JSON.stringify({ compare: !!document.querySelector('.lc-compare'), cover: !!document.querySelector('.lc-cover') })
  })()`))))
  check('Home leaves the comparison for the home screen', !home.compare && home.cover, JSON.stringify(home))
  await sleep(200)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. A day-old three-way comparison with no teammate, behind 22 newer turns; nothing sent.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
