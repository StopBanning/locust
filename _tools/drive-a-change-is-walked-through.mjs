// WALK ME THROUGH THIS CHANGE (0.771), the way a person meets it: a conversation whose teammate changed the folder;
// Changes in its header, every file in git's order; Walk me through it -- "Wren is walking through the change..." --
// and the panel shows the change as its author would in a review: a summary over the files, the files in the order
// to read them, a note on each. The run that walked through it is not a conversation in the sidebar; closing the
// walkthrough puts the panel back as it was; nothing in the folder changes.
//
//   node _tools/drive-a-change-is-walked-through.mjs [--packaged <exe>]
//
// Spends nothing: Wren is on OpenCode's free Space Bunny, and the folder is a scratch repository.

import { execFile } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const root = localPath(new URL('..', import.meta.url))
const adapters = await import(pathToFileURL(join(root, 'packages', 'runtime-adapters', 'dist', 'index.js')).href)
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const git = (args, cwd) => new Promise((resolve, reject) => {
  execFile('git', args, { cwd, windowsHide: true }, (error, stdout) => (error ? reject(error) : resolve(String(stdout))))
})

// A project with a history of its own, and a change waiting: the total subtracted, and now adds; a new test for it.
const workspace = await scratchRepository('locust-drive-commitdraft-ws-')
await mkdir(join(workspace, 'src'), { recursive: true })
await writeFile(join(workspace, 'src', 'total.js'), 'export const total = (lines) => lines.reduce((sum, line) => sum - line.amount, 0)\n', 'utf8')
await git(['add', '.'], workspace)
await git(['commit', '-q', '-m', 'Invoices list their lines in order'], workspace)
await writeFile(join(workspace, 'src', 'total.js'), 'export const total = (lines) => lines.reduce((sum, line) => sum + line.amount, 0)\n', 'utf8')
await mkdir(join(workspace, 'test'), { recursive: true })
await writeFile(join(workspace, 'test', 'total.test.js'), "import { total } from '../src/total.js'\n\nconsole.assert(total([{ amount: 2 }, { amount: 3 }]) === 5, 'the total adds its lines')\n", 'utf8')
const commitsBefore = (await git(['rev-list', '--count', 'HEAD'], workspace)).trim()

const profilePath = await mkdtemp(join(tmpdir(), 'locust-drive-commitdraft-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const WREN = { teammateId: 'tm_aaaaaaaaaaaaaaaaaaaaaaa1', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-08-10T09:00:00.000Z', route: { runtime: 'opencode', model: 'opencode/space-bunny-free', mode: 'accept-edits' } }
const MISSION = { missionId: 'mission_30000000-0000-4000-8000-000000000771', runId: 'run_300771', prompt: 'The invoice total is wrong: it comes out negative. Fix it and add a test.' }
{
  // OpenCode's own records, in the shape `opencode run --format json` prints them (as drive-first-impressions seeds).
  const at = new Date(Date.now() - 300_000).toISOString()
  await ledger.createMission({
    missionId: MISSION.missionId, runId: MISSION.runId, prompt: MISSION.prompt,
    runtime: 'opencode', model: 'opencode/space-bunny-free', requestedRouteId: 'opencode', resolvedRouteId: 'opencode-account:default', cliVersion: '1.18.27',
    workspaceId, sandbox: 'workspace-write', mode: 'accept-edits', executionPolicyVersion: 1, createdAt: at
  })
  let clock = Date.parse(at)
  let n = 0
  const id = (prefix) => `${prefix}_invoice${String((n += 1)).padStart(4, '0')}`
  const message = id('msg')
  const records = []
  const push = (type, part) => records.push({ type, timestamp: (clock += 700), sessionID: 'ses_invoice', part: { id: id('prt'), messageID: message, sessionID: 'ses_invoice', ...part } })
  push('step_start', { type: 'step-start' })
  push('text', { type: 'text', text: 'The total subtracted each line instead of adding it. src/total.js adds them now, and test/total.test.js checks that two lines come to 5.', time: { start: clock, end: clock + 4 } })
  push('step_finish', { type: 'step-finish', reason: 'stop', tokens: { total: 9080, input: 9000, output: 80, reasoning: 0, cache: { write: 0, read: 0 } }, cost: 0 })
  const normalizer = adapters.createOpenCodeEventNormalizer({ runId: MISSION.runId, missionId: MISSION.missionId, cliVersion: '1.18.27', now: () => new Date(at) })
  const events = records.flatMap((record, index) => normalizer.accept({ sequence: index + 1, raw: JSON.stringify(record) }))
  await ledger.appendEvents(MISSION.missionId, [
    ...events,
    ...normalizer.finish({ exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: records.length, cancelled: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, startedAt: at, finishedAt: new Date(Date.parse(at) + 12_000).toISOString() })
  ])
}
say('seeded one finished conversation that fixed the invoice total; its changes are not committed')

const drive = await startDrive({
  name: 'a-change-is-walked-through', port: 9664, workspace, profilePath,
  outPath: join(recordRoot('a-change-is-walked-through-2026-10-13'), packaged === undefined ? 'local' : 'packaged'),
  ...(packaged === undefined ? {} : { packaged }),
  seed: { schemaVersion: 1, teammates: [WREN], missionOwners: { [MISSION.missionId]: WREN.teammateId }, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } }
})

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 600)}`}`)
}
const panel = async () => JSON.parse(String(await drive.evaluate(`JSON.stringify((() => {
  const changes = document.querySelector('aside.lc-changes')
  const cards = [...document.querySelectorAll('.lc-changes__file')]
  return {
    open: changes !== null,
    walk: [...document.querySelectorAll('.lc-changes__walk')].map((one) => one.textContent.trim()).join(' '),
    said: (document.querySelector('.lc-changes__walksaid')?.textContent ?? '').trim(),
    who: (document.querySelector('.lc-changes__walkwho')?.textContent ?? '').trim(),
    summary: (document.querySelector('.lc-changes__walksummary')?.textContent ?? '').trim(),
    files: cards.map((card) => ({ path: card.getAttribute('data-path'), order: (card.querySelector('.lc-changes__order')?.textContent ?? '').trim(), note: (card.querySelector('.lc-changes__whytext')?.textContent ?? '').replace(/\\s+/g, ' ').trim() })),
    rows: [...document.querySelectorAll('button.lc-conv')].map((one) => one.innerText.replace(/\\s+/g, ' ').trim())
  }
})())`)))
const status = async () => git(['status', '--porcelain'], workspace)

try {
  const before = await status()
  await drive.ready()
  await drive.resize(1215, 860)
  say(`  conversation: ${String(await drive.evaluate(`(() => { const row = [...document.querySelectorAll('button.lc-conv')].find((b) => b.innerText.includes('invoice total')); if (!row) return 'no row'; row.click(); return 'opened' })()`))}`)
  await sleep(2000)
  say(`  changes: ${String(await drive.evaluate(`(() => { const button = [...document.querySelectorAll('button')].find((one) => (one.getAttribute('aria-label') || one.innerText.trim()) === 'Changes'); if (!button) return 'no changes button'; button.click(); return 'opened' })()`))}`)
  let first = await panel()
  for (let tries = 0; tries < 20 && first.files.length < 2; tries += 1) {
    await sleep(500)
    first = await panel()
  }
  await drive.capture('Changes, in git\'s order', () => JSON.stringify(first))
  check('Changes lists both files and offers to walk through them', first.open && first.files.length === 2 && first.walk === 'Walk me through it', JSON.stringify(first))

  await drive.evaluate(`document.querySelector('.lc-changes__walk')?.click()`)
  await sleep(600)
  const asking = await panel()
  await drive.capture('Wren walking through the change', () => JSON.stringify(asking))
  check('it says who is walking through it while they do', asking.said === 'Wren is walking through the change…' && asking.walk === '', JSON.stringify(asking))

  let shown = asking
  for (let second = 0; second < 240 && shown.summary === '' && !(shown.said !== '' && shown.said !== asking.said); second += 1) {
    await sleep(1000)
    shown = await panel()
  }
  await drive.capture('The change, walked through', () => JSON.stringify(shown))
  const noted = shown.files.filter((file) => file.note.length > 0)
  check('a summary over the files, from Wren', shown.who === 'Wren’s walkthrough' && shown.summary.length > 20, JSON.stringify({ who: shown.who, summary: shown.summary, said: shown.said }))
  check('each file has its note, numbered in the order to read them, the first one first', noted.length >= 1 && shown.files[0]?.order === '1' && noted.every((file, index) => file.order === String(index + 1)), JSON.stringify(shown.files))
  check('the run that walked through it is not a conversation in the sidebar', shown.rows.length === 1, JSON.stringify(shown.rows))

  await drive.evaluate(`document.querySelector('.lc-changes__walkhead .lc-viewer__action')?.scrollIntoView({ block: 'nearest' })`)
  if (noted.length > 0) {
    await drive.evaluate(`document.querySelectorAll('.lc-changes__why')[${String(Math.min(1, noted.length - 1))}]?.scrollIntoView({ block: 'start' })`)
    await sleep(300)
    await drive.capture('A note on a file', () => JSON.stringify(shown.files))
  }
  await drive.evaluate(`document.querySelector('[aria-label="Close the walkthrough"]')?.click()`)
  await sleep(400)
  const closed = await panel()
  check('closing it puts the panel back as it was', closed.summary === '' && closed.files.every((file) => file.order === '') && closed.walk === 'Walk me through it', JSON.stringify(closed))
  check('nothing in the folder changed', (await status()) === before)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. A scratch repository with an uncommitted fix; Wren on opencode/space-bunny-free.`, extra: `Checks failed: ${String(failures)}` })
  process.exit(failures === 0 ? 0 : 1)
}
