// A Word document and a deck a teammate hands over open as what they say (0.517).
//
//   node _tools/drive-a-word-file-reads.mjs [--packaged <exe>] [--tag <name>]
//
// Product ideas, round four: the .docx or .pptx a teammate makes is the
// deliverable for someone who is not a programmer, and pressing one said
// "Locust does not open that kind of file here". The workspace holds the
// python-docx and python-pptx fixtures (apps/desktop/test/documents); a
// seeded reply hands both over. Each must open in the viewer with its words:
// the report's headings, list and table; the deck's slides in order. Sends
// nothing.

import { createHash } from 'node:crypto'
import { copyFile, mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'

const root = localPath(new URL('..', import.meta.url))
const adapters = await import(pathToFileURL(join(root, 'packages', 'runtime-adapters', 'dist', 'index.js')).href)
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-word-file-ws-')
await copyFile(join(root, 'apps', 'desktop', 'test', 'documents', 'report-python-docx.docx'), join(workspace, 'quarterly-report.docx'))
await copyFile(join(root, 'apps', 'desktop', 'test', 'documents', 'deck-python-pptx.pptx'), join(workspace, 'launch-plan.pptx'))
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-word-file-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const TEAMMATE = { teammateId: 'tm_penny0000000000000000', name: 'Penny', hue: 'clay', role: 'Custom', roleTitle: 'Reports', createdAt: '2026-09-05T05:00:00.000Z' }
const missionId = 'mission_5e000000-0000-4000-8000-000200000000'
const runId = 'run_5e0002'
const at = new Date(Date.now() - 600_000).toISOString()
const reply = 'Both are in the project folder.\n\n<locust-file>\nquarterly-report.docx :: the quarterly report\nlaunch-plan.pptx :: the launch deck\nnever-written.md :: the notes\n</locust-file>'
await ledger.createMission({
  missionId, runId, prompt: 'Write the quarterly report as a Word document and the launch plan as a deck',
  runtime: 'codex', model: 'account-default', requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: null,
  workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at
})
let tick = 0
const normalizer = adapters.createCodexEventNormalizer({ runId, missionId, requestedRouteId: 'codex', resolvedRouteId: 'codex-account:default', cliVersion: '0.156.1', now: () => new Date(Date.parse(at) + (tick++) * 5000) })
await ledger.appendEvents(missionId, [
  ...normalizer.accept({ sequence: 1, raw: JSON.stringify({ type: 'thread.started', thread_id: `thread_${missionId}` }) }),
  ...normalizer.accept({ sequence: 2, raw: JSON.stringify({ type: 'item.completed', item: { id: 'answer', type: 'agent_message', text: reply } }) }),
  ...normalizer.accept({ sequence: 3, raw: JSON.stringify({ type: 'turn.completed', usage: { input_tokens: 3000, cached_input_tokens: 0, output_tokens: 300 } }) }),
  ...normalizer.finish({ exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 3, cancelled: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, startedAt: at, finishedAt: new Date(Date.parse(at) + 20_000).toISOString() })
])
await ledger.flush?.()

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `word-file-reads-${tag}`,
  port: 9836,
  workspace,
  profilePath,
  sendsNothing: true,
  outPath: join(recordRoot('a-word-file-reads-2026-10-01'), tag),
  seed: { schemaVersion: 1, teammates: [TEAMMATE], missionOwners: { [missionId]: TEAMMATE.teammateId }, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 400)}`}`)
}
const openFile = (pattern) => `(async () => {
  document.querySelector('.lc-viewer .lc-viewer__close')?.click()
  await new Promise((r) => setTimeout(r, 400))
  const button = [...document.querySelectorAll('button')].find((b) => ${pattern}.test(b.innerText))
  if (!button) return JSON.stringify({ found: false })
  button.click()
  for (let i = 0; i < 20 && !document.querySelector('.lc-viewer .lc-docview'); i += 1) await new Promise((r) => setTimeout(r, 250))
  await new Promise((r) => setTimeout(r, 600))
  const view = document.querySelector('.lc-viewer .lc-docview')
  return JSON.stringify({
    found: true,
    text: view?.innerText.replace(/\\s+/g, ' ').trim() ?? '',
    headings: [...(view?.querySelectorAll('.lc-heading') ?? [])].map((h) => h.innerText.trim()),
    slides: [...(view?.querySelectorAll('.lc-docview__slide') ?? [])].map((h) => h.innerText.replace(/\\s+/g, ' ').trim()),
    items: [...(view?.querySelectorAll('.lc-docview__item') ?? [])].map((p) => p.innerText.trim()),
    cells: [...(view?.querySelectorAll('.lc-table th, .lc-table td') ?? [])].map((c) => c.innerText.trim()),
    refused: view ? '' : (document.querySelector('.lc-viewer')?.innerText ?? '') + ' ' + [...document.querySelectorAll('[role="status"], .lc-notice')].map((el) => el.innerText).join(' / '),
    overflow: document.documentElement.scrollWidth - document.documentElement.clientWidth
  })
})()`

try {
  await drive.capture('launch', () => drive.ready())
  await drive.resize(1440, 900)
  await drive.evaluate(`(async () => {
    const row = [...document.querySelectorAll('.lc-convrow button.lc-conv')].find((r) => /quarterly report/i.test(r.innerText))
    row?.click()
    await new Promise((r) => setTimeout(r, 1500))
  })()`)
  await sleep(500)
  const word = JSON.parse(String(await drive.capture('the Word document, opened', () => drive.evaluate(openFile('/quarterly-report\\.docx/')))))
  check('the reply hands over the Word document as a button', word.found === true, JSON.stringify(word).slice(0, 200))
  check('it opens as its words: title and headings', JSON.stringify(word.headings) === JSON.stringify(['Quarterly Report', 'Highlights', 'By region']), JSON.stringify(word.headings) + ' ' + word.refused)
  check('its list items and its table', word.items.length === 3 && word.cells.join(' ') === 'Region Q2 Q3 North 120 150 South 90 95', JSON.stringify({ items: word.items, cells: word.cells }))
  check('characters a model writes are shown as written, not as markup', /see <the table> & \*notes\*\./.test(word.text), word.text.slice(0, 200))
  const deck = JSON.parse(String(await drive.capture('the deck, opened', () => drive.evaluate(openFile('/launch-plan\\.pptx/')))))
  check('the deck opens as its slides, numbered and titled, in order', JSON.stringify(deck.slides) === JSON.stringify(['Slide 1 Launch Plan', 'Slide 2 What ships', 'Slide 3 Numbers']), JSON.stringify(deck.slides) + ' ' + deck.refused)
  check('with each slide\'s points and table', deck.items.join(' | ') === 'Weekday routines | Monday to Friday by default | Word and PowerPoint previews' && deck.cells.join(' ') === 'Metric Value Users 1,200', JSON.stringify({ items: deck.items, cells: deck.cells }))
  // A file the teammate named but never wrote (0.524): the refusal says which, and why.
  const refused = String(await drive.capture('a file never written, pressed', () => drive.evaluate(`(async () => {
    document.querySelector('.lc-viewer .lc-viewer__close')?.click()
    await new Promise((r) => setTimeout(r, 400))
    ;[...document.querySelectorAll('button')].find((b) => /never-written\\.md/.test(b.innerText))?.click()
    await new Promise((r) => setTimeout(r, 1200))
    return document.querySelector('.lc-viewer--refused')?.innerText.replace(/\\s+/g, ' ').trim() ?? 'no refusal'
  })()`)))
  check('a file never written: the refusal names it, and says why', /^Could not open never-written\.md/i.test(refused) && /did not write it/.test(refused), refused)
  await drive.resize(1120, 760)
  await sleep(800)
  const narrow = JSON.parse(String(await drive.capture('the deck at 1120', () => drive.evaluate(openFile('/launch-plan\\.pptx/')))))
  check('at 1120 nothing scrolls sideways', narrow.overflow <= 0, String(narrow.overflow))
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. A seeded reply hands over a .docx and a .pptx; nothing sent.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
