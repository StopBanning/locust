// FIND IN A CONVERSATION (0.769), the way a person meets it: a finished conversation open, Ctrl+F, a word typed --
// every place it is marked, the one it is on more strongly, "1 of N" -- Enter and Shift+Enter step through, a phrase
// that crosses a code span is found as one, a word that is not there says so, and Esc closes it all. Ctrl+F where no
// conversation is on screen opens nothing.
//
//   node _tools/drive-a-conversation-can-be-searched.mjs [--packaged <exe>]
//
// Spends nothing: the conversation is seeded into the drive's own ledger, as a finished Codex turn's events.

import { createHash } from 'node:crypto'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const root = localPath(new URL('..', import.meta.url))
const adapters = await import(pathToFileURL(join(root, 'packages', 'runtime-adapters', 'dist', 'index.js')).href)
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)

const workspace = await scratchRepository('locust-drive-find-ws-')
const profilePath = await mkdtemp(join(tmpdir(), 'locust-drive-find-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const WREN = { teammateId: 'tm_aaaaaaaaaaaaaaaaaaaaaaa1', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-08-10T09:00:00.000Z' }

const ANSWER = [
  'The deploy failed on the staging server: one migration timed out.',
  '',
  'Run `npm test` again before you deploy, then deploy to staging first.',
  '',
  'Once staging is green, the production deploy is one command.'
].join('\n')
const MISSION = { missionId: 'mission_30000000-0000-4000-8000-000000000001', runId: 'run_300001', prompt: 'Why did the deploy fail last night?' }

{
  const at = new Date(Date.now() - 300_000).toISOString()
  await ledger.createMission({
    missionId: MISSION.missionId, runId: MISSION.runId, prompt: MISSION.prompt,
    runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: null,
    workspaceId, sandbox: 'workspace-write', executionPolicyVersion: 1, createdAt: at
  })
  let tick = 0
  const normalizer = adapters.createCodexEventNormalizer({ runId: MISSION.runId, missionId: MISSION.missionId, requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: '0.162.1', now: () => new Date(Date.parse(at) + (tick++) * 1000) })
  const records = [
    { type: 'thread.started', thread_id: `thread_${MISSION.runId}` },
    { type: 'turn.started' },
    { type: 'item.completed', item: { id: 'answer', type: 'agent_message', text: ANSWER } },
    { type: 'turn.completed', usage: { input_tokens: 2000, cached_input_tokens: 0, output_tokens: 80 } }
  ]
  const events = [
    ...records.flatMap((record, i) => normalizer.accept({ sequence: i + 1, raw: JSON.stringify(record) })),
    ...normalizer.finish({ exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: records.length, cancelled: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, startedAt: at, finishedAt: new Date(Date.parse(at) + 12_000).toISOString() })
  ]
  await ledger.appendEvents(MISSION.missionId, events)
}
say('seeded one finished conversation about a failed deploy')

const drive = await startDrive({
  name: 'a-conversation-can-be-searched', port: 9648, workspace, profilePath, sendsNothing: true,
  outPath: join(recordRoot('a-conversation-can-be-searched-2026-10-13'), packaged === undefined ? 'local' : 'packaged'),
  ...(packaged === undefined ? {} : { packaged }),
  seed: { schemaVersion: 1, teammates: [WREN], missionOwners: { [MISSION.missionId]: WREN.teammateId }, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } }
})

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 400)}`}`)
}
const ctrlF = () => drive.evaluate(`(() => { window.dispatchEvent(new KeyboardEvent('keydown', { key: 'f', ctrlKey: true, bubbles: true })); return true })()`)
const type = (words) => drive.evaluate(`(() => {
  const box = document.querySelector('input[aria-label="Find in this conversation"]')
  if (!box) return 'no find box'
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(box, ${JSON.stringify(words)})
  box.dispatchEvent(new Event('input', { bubbles: true }))
  return 'typed'
})()`)
const key = (name, shift = false) => drive.evaluate(`(() => {
  const box = document.querySelector('input[aria-label="Find in this conversation"]')
  if (!box) return 'no find box'
  box.dispatchEvent(new KeyboardEvent('keydown', { key: ${JSON.stringify(name)}, shiftKey: ${shift ? 'true' : 'false'}, bubbles: true }))
  return 'pressed'
})()`)
const state = async () => JSON.parse(String(await drive.evaluate(`JSON.stringify({
  bar: document.querySelector('.lc-threadfind') !== null,
  focused: document.activeElement?.getAttribute('aria-label') ?? document.activeElement?.tagName ?? '',
  count: (document.querySelector('.lc-threadfind__count')?.textContent ?? '').trim(),
  marked: CSS.highlights.get('lc-find')?.size ?? 0,
  now: CSS.highlights.get('lc-find-now')?.size ?? 0,
  nowText: (() => { const range = [...(CSS.highlights.get('lc-find-now') ?? [])][0]; return range === undefined ? '' : range.toString() })()
})`)))

try {
  await drive.ready()
  await drive.resize(1215, 860)
  // Ctrl+F with no conversation on screen: nothing opens.
  await ctrlF()
  await sleep(500)
  check('Ctrl+F with no conversation on screen opens nothing', !(await state()).bar)

  const opened = String(await drive.evaluate(`(() => {
    const row = [...document.querySelectorAll('button.lc-conv')].find((b) => b.innerText.includes('deploy fail'))
    if (!row) return 'no conversation row'
    row.click()
    return 'opened'
  })()`))
  say(`  conversation: ${opened}`)
  await sleep(1800)
  const words = Number(await drive.evaluate(`((document.querySelector('.lc-thread__column')?.innerText ?? '').match(/deploy/gi) ?? []).length`))

  await ctrlF()
  await sleep(700)
  const open = await state()
  check('Ctrl+F opens the find bar, the cursor in it', open.bar && open.focused === 'Find in this conversation', JSON.stringify(open))

  say(`  type deploy: ${String(await type('deploy'))}`)
  await sleep(700)
  const found = await state()
  await drive.capture('"deploy", marked everywhere, the first one strongly', () => drive.evaluate(`(document.querySelector('.lc-threadfind')?.innerText ?? '').replace(/\\s+/g, ' ')`))
  check('every "deploy" on the page is marked, and the count says so', words >= 5 && found.marked === words && found.count === `1 of ${String(words)}`, JSON.stringify({ words, ...found }))
  check('the one it is on is marked on its own, in any case', found.now === 1 && /^deploy$/i.test(found.nowText), JSON.stringify(found))

  await key('Enter')
  await sleep(300)
  const next = await state()
  await key('Enter', true)
  await sleep(300)
  const back = await state()
  check('Enter steps to the next, Shift+Enter back', next.count === `2 of ${String(words)}` && back.count === `1 of ${String(words)}`, JSON.stringify({ next: next.count, back: back.count }))

  await type('run npm test again')
  await sleep(700)
  const across = await state()
  check('a phrase that crosses a code span is found as one', across.marked === 1 && across.count === '1 of 1' && across.nowText.toLowerCase() === 'run npm test again', JSON.stringify(across))

  await type('kubernetes')
  await sleep(700)
  const none = await state()
  await drive.capture('a word that is not there', () => drive.evaluate(`(document.querySelector('.lc-threadfind')?.innerText ?? '').replace(/\\s+/g, ' ')`))
  check('a word that is not there says No matches, and marks nothing', none.count === 'No matches' && none.marked === 0, JSON.stringify(none))

  await type('staging')
  await sleep(600)
  await key('Escape')
  await sleep(500)
  const closed = await state()
  check('Esc closes it and takes every mark away', !closed.bar && closed.marked === 0 && closed.now === 0, JSON.stringify(closed))
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. One finished, seeded conversation about a failed deploy.`, extra: `Checks failed: ${String(failures)}` })
  process.exit(failures === 0 ? 0 : 1)
}
