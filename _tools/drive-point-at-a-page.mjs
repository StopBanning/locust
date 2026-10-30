// Point at a part of a page, and ask about it (0.484).
//
//   node _tools/drive-point-at-a-page.mjs [--packaged <exe>]
//
// A conversation whose turn made index.html is seeded (OpenCode's record
// shapes, through Locust's own normalizer, as drive-first-impressions does). The page
// opens running in the viewer; the target button is pressed; a REAL mouse
// click (CDP Input) lands on the page's heading. The chat box must then hold
// a quote naming index.html, the heading's place and its code, with a picture
// of it attached -- and the page's own click handler must not have run. Then
// pointing is started and stopped: nothing may be added. Sends nothing.

import { createHash } from 'node:crypto'
import { mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, git, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const packaged = process.argv.includes('--packaged') ? process.argv[process.argv.indexOf('--packaged') + 1] : undefined
const root = join(localPath(new URL('..', import.meta.url)))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const adapters = await import(pathToFileURL(join(root, 'packages', 'runtime-adapters', 'dist', 'index.js')).href)

const workspace = await scratchRepository('locust-drive-point-ws-', 'A coffee shop page.\n')
await writeFile(join(workspace, 'index.html'), [
  '<!doctype html>',
  '<html><head><meta charset="utf-8"><style>body{margin:0;font-family:sans-serif;background:#f6f3ee;color:#2b2a28} section.hero{padding:40px} h1.title{margin:0;font-size:40px;background:#dfe8e2;padding:12px 16px}</style></head>',
  '<body><main><section class="hero"><h1 class="title" id="headline">Fresh bread daily</h1><p>Open 7 to 3.</p></section></main>',
  '<script>',
  "  const h1 = document.getElementById('headline')",
  "  h1.addEventListener('click', () => parent.postMessage({ pointProbe: 'the page acted on the click' }, '*'))",
  "  const tell = () => { const r = h1.getBoundingClientRect(); parent.postMessage({ pointProbe: { x: r.left + r.width / 2, y: r.top + r.height / 2 } }, '*') }",
  "  addEventListener('load', tell)",
  '</script></body></html>',
  ''
].join('\n'), 'utf8')
await git(['add', '-A'], workspace)
await git(['commit', '-q', '-m', 'page'], workspace)

const profilePath = await mkdtemp(join(tmpdir(), 'locust-drive-point-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const at = '2026-09-29T09:00:00.000Z'
const ended = '2026-09-29T09:00:42.000Z'
// OpenCode's record shapes (`opencode run --format json`), as drive-first-impressions seeds them:
// a write carries the page's text, so the turn shows it as a new file with an Open button.
const pageText = await (await import('node:fs/promises')).readFile(join(workspace, 'index.html'), 'utf8')
await ledger.createMission({
  missionId: 'mission_page', runId: 'run_page', prompt: 'Make a landing page for a coffee shop.',
  runtime: 'opencode', model: 'opencode/ling-3.0-flash-fin-free', requestedRouteId: 'opencode', resolvedRouteId: 'opencode-account:default', cliVersion: '1.18.27',
  workspaceId, sandbox: 'workspace-write', mode: 'accept-edits', executionPolicyVersion: 1, createdAt: at
})
let clock = Date.parse(at)
let n = 0
const id = (prefix) => `${prefix}_page${String((n += 1)).padStart(4, '0')}`
const message = id('msg')
const part = (type, body) => ({ type, timestamp: (clock += 700), sessionID: 'ses_page', part: { id: id('prt'), messageID: message, sessionID: 'ses_page', ...body } })
const records = [
  part('step_start', { type: 'step-start' }),
  part('tool_use', { type: 'tool', tool: 'write', callID: id('call'), state: { status: 'completed', input: { filePath: join(workspace, 'index.html'), content: pageText }, output: 'Wrote file successfully.', metadata: { filepath: join(workspace, 'index.html'), exists: false }, title: 'index.html', time: { start: clock, end: clock + 40 } } }),
  part('text', { type: 'text', text: 'Made `index.html`: a headline, the hours, and a soft green band.', time: { start: clock, end: clock + 4 } }),
  part('step_finish', { type: 'step-finish', reason: 'stop', tokens: { total: 9300, input: 9000, output: 300, reasoning: 0, cache: { write: 0, read: 0 } }, cost: 0 })
]
const normalizer = adapters.createOpenCodeEventNormalizer({ runId: 'run_page', missionId: 'mission_page', cliVersion: '1.18.27', now: () => new Date(at) })
const events = records.flatMap((record, index) => normalizer.accept({ sequence: index + 1, raw: JSON.stringify(record) }))
await ledger.appendEvents('mission_page', [...events, ...normalizer.finish({ exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: records.length, cancelled: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, startedAt: at, finishedAt: ended })])

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: 'point-at-a-page', port: 9761, workspace, profilePath, sendsNothing: true,
  seed: { schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}
const mouse = async (x, y) => {
  await drive.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y })
  await sleep(120)
  await drive.send('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', clickCount: 1 })
  await drive.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', clickCount: 1 })
}
const composer = `JSON.stringify({ text: document.querySelector('form.command-dock textarea')?.value ?? '', tiles: [...document.querySelectorAll('.lc-attached__tile')].map((t) => t.innerText.trim()), hint: document.querySelector('.lc-viewer__pointhint')?.innerText ?? null })`

try {
  await drive.capture('launch', () => drive.ready())
  const opened = JSON.parse(String(await drive.capture('the page, open and running', () => drive.evaluate(`(async () => {
    window.__point = []
    // The page also runs on its turn's card; only what the VIEWER's frame says is its place.
    window.__viewerAt = null
    window.addEventListener('message', (event) => {
      if (!event.data || event.data.pointProbe === undefined) return
      window.__point.push(event.data.pointProbe)
      const viewer = document.querySelector('.lc-viewer__page')
      if (viewer && event.source === viewer.contentWindow && typeof event.data.pointProbe === 'object') window.__viewerAt = event.data.pointProbe
    })
    for (let i = 0; i < 40; i += 1) {
      const row = [...document.querySelectorAll('button, a, [role="button"]')].find((el) => el.innerText.includes('Make a landing page'))
      if (row) { row.click(); break }
      await new Promise((r) => setTimeout(r, 250))
    }
    let open
    for (let i = 0; i < 40 && !open; i += 1) {
      await new Promise((r) => setTimeout(r, 250))
      open = [...document.querySelectorAll('button')].filter((b) => /^Open index\\.html/.test(b.innerText.trim())).pop()
        ?? document.querySelector('.lc-thread button[aria-label="Open index.html"]') ?? undefined
    }
    if (!open) return JSON.stringify({ open: false })
    open.click()
    for (let i = 0; i < 40 && !window.__viewerAt; i += 1) await new Promise((r) => setTimeout(r, 250))
    const frame = document.querySelector('.lc-viewer__page')?.getBoundingClientRect()
    const at = window.__viewerAt
    return JSON.stringify({ open: true, frame: frame ? { x: frame.left, y: frame.top } : null, at: at ?? null, target: !!document.querySelector('button[aria-label="Point at a part of the page to ask about it"]') })
  })()`))))
  check('index.html opens running, with a button to point at it', opened.open && opened.frame !== null && opened.at !== null && opened.target, JSON.stringify(opened))

  await drive.evaluate(`document.querySelector('button[aria-label="Point at a part of the page to ask about it"]').click()`)
  await sleep(600)
  const pointing = JSON.parse(String(await drive.capture('pointing: the hint says what to do', () => drive.evaluate(composer))))
  check('pointing says what to do', /Click a part of the page to ask about it/.test(pointing.hint ?? ''), JSON.stringify(pointing.hint))

  await mouse(Math.round(opened.frame.x + opened.at.x), Math.round(opened.frame.y + opened.at.y))
  let after = { text: '', tiles: [] }
  for (let i = 0; i < 40 && (after.text === '' || after.tiles.length === 0); i += 1) {
    await sleep(250)
    after = JSON.parse(String(await drive.evaluate(composer)))
  }
  await drive.capture('the heading, quoted in the chat box with its picture', () => drive.evaluate(composer))
  check('the chat box quotes the part: the file, its place, its code', /^> About this part of index\.html: `h1#headline`/.test(after.text) && /> <h1 class="title" id="headline">Fresh bread daily<\/h1>/.test(after.text), JSON.stringify(after.text.slice(0, 200)))
  check('with a picture of it attached', after.tiles.some((tile) => /page-part-\d+\.png/.test(tile)), JSON.stringify(after.tiles))
  const acted = await drive.evaluate(`window.__point.includes('the page acted on the click')`)
  check('and the page itself never saw the click', acted === false)
  check('pointing has stopped', after.hint === null, JSON.stringify(after.hint))

  // Started and stopped: nothing added.
  await drive.evaluate(`(() => { const box = document.querySelector('form.command-dock textarea'); const set = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set; set.call(box, ''); box.dispatchEvent(new Event('input', { bubbles: true })) })()`)
  await drive.evaluate(`document.querySelector('button[aria-label="Point at a part of the page to ask about it"]').click()`)
  await sleep(500)
  await drive.evaluate(`document.querySelector('button[aria-label="Stop pointing"]')?.click()`)
  await sleep(800)
  const stopped = JSON.parse(String(await drive.capture('pointing started, then stopped', () => drive.evaluate(composer))))
  check('stopped, it adds nothing and says nothing', stopped.text === '' && stopped.hint === null, JSON.stringify(stopped))
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. A seeded turn that made index.html; nothing sent.`, extra: `Checks failed: ${String(failures)}` })
}
if (failures > 0) process.exitCode = 1
