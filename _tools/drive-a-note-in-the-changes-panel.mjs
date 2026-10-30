// A note on a line in the Changes panel, sent to the teammate (0.776).
//
//   node _tools/drive-a-note-in-the-changes-panel.mjs [--packaged <exe>] [--tag <name>]
//
// Notes on a change worked in the conversation's own diffs since 0.376; the Changes panel (0.732), the folder's
// whole change beside the conversation, offered no "+" at all. The folder here has an edit not yet committed
// (app.js's port 3000 -> 3001) and a conversation with Mira on the free OpenCode model (seeded, so nothing is
// spent until she acts on the note). Changes opens the panel; the drive hovers the changed line with the mouse,
// presses its "+", types a note with key events, adds it with Enter, and sends one short message. Mira must act
// on it, and the note must be drawn under the message with its place and line.

import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdtemp, readFile, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, FREE_ROUTE, recordRoot, say, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')

const git = (cwd, ...args) => execFileSync('git', args, { cwd, encoding: 'utf8', windowsHide: true })
const workspace = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-panel-note-ws-'))
git(workspace, 'init', '-q', '-b', 'main')
git(workspace, 'config', 'user.email', 'drive@locust.test')
git(workspace, 'config', 'user.name', 'Locust drive')
await writeFile(join(workspace, 'app.js'), "const http = require('http')\n\nconst port = 3000\n\nhttp.createServer((req, res) => res.end('ok')).listen(port)\n", 'utf8')
git(workspace, 'add', '.')
git(workspace, 'commit', '-q', '-m', 'A small server')
// The change the panel shows: not committed yet.
await writeFile(join(workspace, 'app.js'), "const http = require('http')\n\nconst port = 3001\n\nhttp.createServer((req, res) => res.end('ok')).listen(port)\n", 'utf8')

const root = localPath(new URL('..', import.meta.url))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-panel-note-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const MIRA = { teammateId: 'tm_mira000000000000000000', name: 'Mira', hue: 'violet', role: 'Custom', roleTitle: 'Developer', createdAt: '2026-10-15T01:00:00.000Z', route: FREE_ROUTE }
const missionId = 'mission_5e000000-0000-4000-8000-000776000000'
const runId = 'run_5e0776'
const at = new Date(Date.now() - 600_000).toISOString()
await ledger.createMission({
  missionId, runId, prompt: 'In app.js, change the port from 3000 to 3001. Change nothing else.',
  runtime: 'opencode', model: FREE_ROUTE.model, requestedRouteId: 'opencode', resolvedRouteId: `opencode:${FREE_ROUTE.model}`, cliVersion: null,
  workspaceId, sandbox: 'workspace-write', mode: 'accept-edits', executionPolicyVersion: 1, createdAt: at
})
const process_ = { exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 2, inputDeliveryFailed: false, outputLimitExceeded: false, oversizedRecordsDropped: 0, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at, finishedAt: at }
const event = (sequence, type, payload) => ({ id: `event_${String(sequence)}`, runId, missionId, sequence, occurredAt: at, sourceAdapter: 'opencode', type, payload: { ...payload, evidence: { redacted: true } } })
await ledger.appendEvents(missionId, [
  event(1, 'message.delta', { itemId: 'answer', operation: 'append', text: 'Changed the port in app.js to 3001.', final: true }),
  event(2, 'run.completed', { runtimeTerminal: 'completed', process: process_ })
])
await ledger.flush?.()

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `panel-note-${tag}`,
  port: 9777,
  workspace,
  profilePath,
  outPath: join(recordRoot('a-note-in-the-changes-panel-2026-10-15'), tag),
  seed: { schemaVersion: 1, teammates: [MIRA], missionOwners: { [missionId]: MIRA.teammateId }, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 500)}`}`)
}
const send = (text) => drive.evaluate(`(async () => {
  const field = document.querySelector('form.command-dock textarea')
  const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
  setter.call(field, ${JSON.stringify(text)})
  field.dispatchEvent(new Event('input', { bubbles: true }))
  for (let i = 0; i < 120; i += 1) {
    await new Promise((r) => setTimeout(r, 250))
    const button = document.querySelector('button[aria-label="Send"]')
    if (button && !button.disabled) { button.click(); break }
  }
  for (let i = 0; i < 900; i += 1) {
    await new Promise((r) => setTimeout(r, 400))
    if (i > 8 && !document.querySelector('button[aria-label^="Stop the running"]')) return 'ended'
  }
  return 'timed out'
})()`)
const LINE = `[...document.querySelectorAll('.lc-changes .lc-diff__row.is-add')].find((row) => /3001/.test(row.innerText))`

try {
  await drive.ready()
  await drive.resize(1440, 900)
  const opened = String(await drive.capture('the conversation, then Changes', () => drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('.lc-convrow button.lc-conv')].find((r) => /port/i.test(r.innerText))?.click()
    await new Promise((r) => setTimeout(r, 1500))
    ;[...document.querySelectorAll('button')].find((b) => (b.getAttribute('aria-label') || b.innerText.trim()) === 'Changes')?.click()
    for (let i = 0; i < 40 && !(${LINE}); i += 1) await new Promise((r) => setTimeout(r, 250))
    const line = ${LINE}
    if (!line) return 'no added line in the panel: ' + (document.querySelector('.lc-changes')?.innerText ?? 'no panel').replace(/\\s+/g, ' ').slice(0, 300)
    line.scrollIntoView({ block: 'center' })
    await new Promise((r) => setTimeout(r, 300))
    const box = line.getBoundingClientRect()
    return JSON.stringify({ x: Math.round(box.left + box.width / 2), y: Math.round(box.top + box.height / 2) })
  })()`)))
  check('the Changes panel shows the folder’s change to app.js', opened.startsWith('{'), opened)
  if (!opened.startsWith('{')) throw new Error('no line to note on')
  const spot = JSON.parse(opened)
  await drive.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: spot.x, y: spot.y })
  await sleep(400)
  const shown = String(await drive.capture('hovering the changed line in the panel shows its "+"', () => drive.evaluate(`(() => {
    const plus = (${LINE})?.querySelector('.lc-diff__noteadd')
    return plus ? getComputedStyle(plus).opacity + ' || ' + plus.getAttribute('aria-label') : 'no plus'
  })()`)))
  check('in the panel, the "+" shows on hover, naming its line', /^1 \|\| Add a note on app\.js, line 3$/.test(shown), shown)
  if (!/^1 /.test(shown)) throw new Error('no "+" to press')
  const plusAt = JSON.parse(String(await drive.evaluate(`(() => {
    const box = (${LINE}).querySelector('.lc-diff__noteadd').getBoundingClientRect()
    return JSON.stringify({ x: Math.round(box.left + box.width / 2), y: Math.round(box.top + box.height / 2) })
  })()`)))
  for (const type of ['mousePressed', 'mouseReleased']) await drive.send('Input.dispatchMouseEvent', { type, x: plusAt.x, y: plusAt.y, button: 'left', clickCount: 1 })
  await sleep(400)
  const editor = String(await drive.evaluate(`(() => { const field = document.querySelector('.lc-changes .lc-diff__noteedit textarea'); return field === null ? 'no editor' : 'focused: ' + String(document.activeElement === field) + ' || ' + (field.getAttribute('placeholder') ?? '') })()`))
  check('the "+" opens a note under the line, focused, saying who it goes to', /^focused: true/.test(editor) && /Mira/.test(editor), editor)
  const NOTE = 'Read the port from the PORT environment variable, and fall back to 3001.'
  for (const character of NOTE) await drive.send('Input.insertText', { text: character })
  await drive.send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13, text: String.fromCharCode(13) })
  await drive.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13 })
  await sleep(500)
  const kept = JSON.parse(String(await drive.capture('the note under its line in the panel, and its tile in the chat box', () => drive.evaluate(`(() => JSON.stringify({
    note: document.querySelector('.lc-changes .lc-diff__notetext')?.textContent ?? '',
    tile: document.querySelector('.lc-notestile')?.innerText.trim() ?? ''
  }))()`))))
  check('the note sits under its line and the chat box holds "1 note on the changes"', kept.note === NOTE && kept.tile === '1 note on the changes', JSON.stringify(kept))

  const answered = await send('One note on the change.')
  const after = await readFile(join(workspace, 'app.js'), 'utf8')
  const sent = String(await drive.capture('Mira, after the note', () => drive.evaluate(`(() => JSON.stringify([...document.querySelectorAll('.lc-sentnote')].map((row) => ({
    place: row.querySelector('.lc-sentnote__place')?.textContent ?? '', text: row.querySelector('.lc-sentnote__text')?.textContent ?? ''
  }))))()`)))
  const drawn = JSON.parse(sent)
  check('the note went with the message: drawn under it with its place and words', drawn.length === 1 && drawn[0].place === 'app.js:3' && drawn[0].text === NOTE, sent)
  check('Mira acted on the note', answered === 'ended' && /process\.env\.PORT/.test(after), after.split('\n').filter((line) => /port/i.test(line)).join(' / '))
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. An uncommitted change in the folder; a note on it from the Changes panel, sent to Mira on ${FREE_ROUTE.model}.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
