// A COMMIT MESSAGE WRITTEN FROM THE CHANGES (0.771), the way a person meets it: a conversation whose teammate changed
// the folder; Commit in its header, its message drafted from what was asked; Write it from the changes -- "Wren is
// writing it…" -- and the box holds a message about what changed: a subject line of at most 72 characters, about the
// total, then a body. The run that wrote it is not a conversation in the sidebar, and nothing is committed: the drive
// never presses Commit.
//
//   node _tools/drive-a-commit-message-is-written-from-the-changes.mjs [--packaged <exe>]
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
  name: 'a-commit-message-is-written-from-the-changes', port: 9662, workspace, profilePath,
  outPath: join(recordRoot('a-commit-message-is-written-2026-10-13'), packaged === undefined ? 'local' : 'packaged'),
  ...(packaged === undefined ? {} : { packaged }),
  seed: { schemaVersion: 1, teammates: [WREN], missionOwners: { [MISSION.missionId]: WREN.teammateId }, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } }
})

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 500)}`}`)
}
const dialog = async () => JSON.parse(String(await drive.evaluate(`JSON.stringify((() => {
  const box = document.querySelector('#lc-commit-message')
  const write = [...document.querySelectorAll('.lc-commit__labelrow .lc-commit__link')][0]
  return {
    open: document.querySelector('.lc-commit') !== null,
    message: box?.value ?? null,
    write: write?.textContent.trim() ?? null,
    said: [...document.querySelectorAll('.lc-commit__said--refused')].map((one) => one.textContent.trim()).join(' '),
    rows: [...document.querySelectorAll('button.lc-conv')].map((one) => one.innerText.replace(/\\s+/g, ' ').trim())
  }
})())`)))

try {
  await drive.ready()
  await drive.resize(1215, 860)
  say(`  conversation: ${String(await drive.evaluate(`(() => { const row = [...document.querySelectorAll('button.lc-conv')].find((b) => b.innerText.includes('invoice total')); if (!row) return 'no row'; row.click(); return 'opened' })()`))}`)
  await sleep(2000)
  say(`  commit: ${String(await drive.evaluate(`(() => { const button = [...document.querySelectorAll('button')].find((one) => one.innerText.trim() === 'Commit' && !one.closest('.lc-commit')); if (!button) return 'no commit button'; button.click(); return 'opened' })()`))}`)
  let before = await dialog()
  for (let tries = 0; tries < 20 && (before.message === null || before.write === null); tries += 1) {
    await sleep(500)
    before = await dialog()
  }
  await drive.capture('Commit, drafted from what was asked', () => JSON.stringify(before))
  check('Commit opens with a message from what was asked, and offers to write it from the changes', before.open && typeof before.message === 'string' && before.message.startsWith('The invoice total is wrong') && before.write === 'Write it from the changes', JSON.stringify(before))

  await drive.evaluate(`[...document.querySelectorAll('.lc-commit__labelrow .lc-commit__link')][0]?.click()`)
  await sleep(600)
  const writing = await dialog()
  await drive.capture('Wren writing it from the changes', () => JSON.stringify(writing))
  check('it says who is writing it while they do', writing.write === 'Wren is writing it…', JSON.stringify(writing))

  let after = writing
  for (let second = 0; second < 180 && after.write !== 'Write it from the changes'; second += 1) {
    await sleep(1000)
    after = await dialog()
  }
  await drive.capture('The message, written from the changes', () => JSON.stringify(after))
  const [subject = '', ...rest] = String(after.message ?? '').split('\n')
  check('the box holds a message about what changed: a subject line of at most 72 characters, and a body', after.said === '' && after.message !== before.message && subject.length > 0 && subject.length <= 72 && /total|sum|add|subtract|invoice/i.test(String(after.message)) && rest.join('\n').trim().length > 0, JSON.stringify(after))
  check('the run that wrote it is not a conversation in the sidebar', after.rows.length === 1 && !after.rows.some((row) => /commit message/i.test(row)), JSON.stringify(after.rows))
  check('nothing is committed until Commit is pressed', (await git(['rev-list', '--count', 'HEAD'], workspace)).trim() === commitsBefore && /total\.js/.test(await git(['status', '--porcelain'], workspace)))
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. A scratch repository with an uncommitted fix; Wren on opencode/space-bunny-free.`, extra: `Checks failed: ${String(failures)}` })
  process.exit(failures === 0 ? 0 : 1)
}
