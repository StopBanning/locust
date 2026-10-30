// A command that reaches programs it did not start says so on its row (0.578).
//
//   node _tools/drive-a-command-that-reaches.mjs [--packaged <exe>] [--tag <name>]
//
// The arena run, 2026-10-03: Opus ran `taskkill //F //IM python.exe` and the
// row said `exit 0`. This seeds one finished Codex turn that ran that command
// and an ordinary one (nothing is run for real), opens it in the built app,
// opens the fold, and reads both rows: the badge on the first, none on the
// second, and its sentence on hover. Sends nothing.

import { mkdir, mkdtemp } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, openAllStepsScript, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'
const OUT = join(recordRoot('a-command-that-reaches-2026-10-04'), `a-command-that-reaches-${tag}`)
await mkdir(OUT, { recursive: true })

const REACHES = 'taskkill //F //IM python.exe'
const ORDINARY = 'python -m pytest -q'
// 0.586: Windows' older kill, one more spelling the row names.
const OLDER = 'tskill python'

const root = localPath(new URL('..', import.meta.url))
const adapters = await import(pathToFileURL(join(root, 'packages', 'runtime-adapters', 'dist', 'index.js')).href)
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-reach-ws-')
const profilePath = await mkdtemp(join(tmpdir(), 'locust-drive-reach-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const WREN = { teammateId: 'tm_aaaaaaaaaaaaaaaaaaaaaaa1', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-08-10T09:00:00.000Z' }
const missionId = 'mission_10000000-0000-4000-8000-000000000001'
const runId = 'run_100001'
const at = new Date(Date.now() - 120_000).toISOString()
await ledger.createMission({
  missionId, runId, prompt: 'The tests hang. Clear out the stuck Python and run them again.',
  runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: null,
  workspaceId, sandbox: 'workspace-write', executionPolicyVersion: 1, createdAt: at
})
let tick = 0
const normalizer = adapters.createCodexEventNormalizer({ runId, missionId, requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: '0.156.1', now: () => new Date(Date.parse(at) + (tick++) * 1000) })
const command = (id, text, output) => [
  { type: 'item.started', item: { id, type: 'command_execution', command: text, status: 'in_progress' } },
  { type: 'item.completed', item: { id, type: 'command_execution', command: text, status: 'completed', exit_code: 0, aggregated_output: output } }
]
const records = [
  { type: 'thread.started', thread_id: 'thread_reach' },
  { type: 'turn.started' },
  ...command('c1', REACHES, 'SUCCESS: The process "python.exe" with PID 4120 has been terminated.\n'),
  ...command('c2', ORDINARY, '4 passed in 0.31s\n'),
  ...command('c3', OLDER, ''),
  { type: 'item.completed', item: { id: 'answer', type: 'agent_message', text: 'Stopped the stuck Python and the tests pass now.' } },
  { type: 'turn.completed', usage: { input_tokens: 3000, cached_input_tokens: 0, output_tokens: 200 } }
]
await ledger.appendEvents(missionId, [
  ...records.flatMap((record, i) => normalizer.accept({ sequence: i + 1, raw: JSON.stringify(record) })),
  ...normalizer.finish({ exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: records.length, cancelled: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, startedAt: at, finishedAt: new Date(Date.parse(at) + 20_000).toISOString() })
])
say('seeded one finished turn with two commands')

const drive = await startDrive({
  name: `a-command-that-reaches-${tag}`, port: 9793, workspace, profilePath, outPath: OUT, sendsNothing: true,
  ...(packaged === undefined ? {} : { packaged }),
  seed: { schemaVersion: 1, teammates: [WREN], missionOwners: { [missionId]: WREN.teammateId }, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}
try {
  await drive.ready()
  await drive.resize(1440, 900)
  await sleep(2500)
  const opened = String(await drive.evaluate(`(() => {
    const row = document.querySelector('button.lc-conv')
    if (!row) return 'no conversation row'
    row.click()
    return 'opened'
  })()`))
  check('the conversation opens', opened === 'opened', opened)
  await sleep(1500)
  await drive.evaluate(openAllStepsScript())
  await drive.evaluate(`(() => { for (const b of document.querySelectorAll('.lc-thread .lc-activity__head[aria-expanded="false"], .lc-thread button[aria-expanded="false"].lc-activity__toggle')) b.click(); return true })()`)
  await sleep(600)
  const rows = JSON.parse(String(await drive.evaluate(`JSON.stringify([...document.querySelectorAll('.lc-thread .lc-filerow.is-shell')].map((row) => ({
    text: row.innerText.replace(/\\s+/g, ' ').trim(),
    badge: row.querySelector('.lc-shellbadge.is-reach')?.textContent ?? null,
    title: row.querySelector('.lc-shellbadge.is-reach')?.getAttribute('title') ?? null,
    colour: (() => { const b = row.querySelector('.lc-shellbadge.is-reach'); return b ? getComputedStyle(b).color : null })(),
    width: row.scrollWidth - row.clientWidth
  })))`)))
  say(`  rows: ${JSON.stringify(rows)}`)
  const reaching = rows.find((row) => row.text.includes('taskkill'))
  const ordinary = rows.find((row) => row.text.includes('pytest'))
  check('both command rows are on the page', reaching !== undefined && ordinary !== undefined, `${String(rows.length)} rows`)
  check('the taskkill row says what it reached', reaching?.badge === 'stops every python.exe', reaching?.badge ?? 'no badge')
  check('its sentence is on hover', reaching?.title === 'Stops every python.exe on this computer, not only the ones this run started.', reaching?.title ?? 'no title')
  check('the pytest row says nothing more', ordinary !== undefined && ordinary.badge === null, ordinary?.badge ?? 'none')
  const older = rows.find((row) => row.text.includes('tskill'))
  check('0.586: the tskill row says what it reached', older?.badge === 'stops every python', older?.badge ?? 'no badge')
  check('the badge is drawn in amber, not the row colour', reaching?.colour !== null && reaching?.colour !== undefined && !/rgb\((2[0-4]\d|25[0-5]), (2[0-4]\d|25[0-5]), (2[0-4]\d|25[0-5])\)/.test(reaching.colour), reaching?.colour ?? 'none')
  check('the row does not overflow', reaching !== undefined && reaching.width <= 1, `${String(reaching?.width)} px`)
  for (const [w, h] of [[1440, 900], [900, 700]]) {
    await drive.resize(w, h)
    await sleep(500)
    await drive.capture(`reach at ${String(w)}`, async () => `${String(w)}x${String(h)}`)
  }
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. One seeded Codex turn: ${REACHES}, ${ORDINARY} and ${OLDER}. Nothing sent, nothing run.` })
  say(failures === 0 ? 'PASSED' : `FAILED (${String(failures)})`)
  process.exitCode = failures === 0 ? 0 : 1
}
