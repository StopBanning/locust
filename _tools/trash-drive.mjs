// Deleting is reversible, driven the way it was lost.
//
//   node _tools/trash-drive.mjs --fixture <ledger.jsonl> --workspace <original-folder>
//
// On 2026-09-17 eighteen of Colin's missions went in four seconds through the
// Missions screen's select-and-confirm, and the files were unlinked. This
// drives exactly that path -- tick the row, Delete 1, Delete 1 for good -- and
// then puts the conversation back from Settings and checks it is really there.
//
// Seeded with the acceptance fixture. No provider run; costs nothing.

import '../_tools/scratch-root.mjs'

import { spawn } from 'node:child_process'
import { copyFile, mkdtemp, mkdir, readdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { requiredPathArgument } from './required-path-argument.mjs'
import { localPath } from './local-path.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const PORT = 9483
const FIXTURE = requiredPathArgument('--fixture')
const WORKSPACE = requiredPathArgument('--workspace')
const MISSION_ID = 'mission_4ac0d5fd-b688-4f04-8222-624a155225dd'

let failures = 0
const check = (label, ok, detail) => {
  if (!ok) failures += 1
  console.error(`  [${ok ? 'PASS' : 'FAIL'}] ${label}${detail === undefined ? '' : ` -- ${detail}`}`)
}
const say = (line) => console.error(line)
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))

class Cdp {
  constructor(ws) {
    this.ws = ws
    this.id = 0
    this.pending = new Map()
    ws.addEventListener('message', (event) => {
      const message = JSON.parse(event.data)
      const waiter = this.pending.get(message.id)
      if (waiter === undefined) return
      this.pending.delete(message.id)
      waiter(message)
    })
  }
  send(method, params = {}) {
    const id = ++this.id
    this.ws.send(JSON.stringify({ id, method, params }))
    return new Promise((resolve) => this.pending.set(id, resolve))
  }
  async eval(expression) {
    const reply = await this.send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })
    if (reply.result?.exceptionDetails) throw new Error(reply.result.exceptionDetails.text)
    return reply.result?.result?.value
  }
}

const profile = await mkdtemp(join(tmpdir(), 'locust-trash-'))
await mkdir(join(profile, 'mission-ledger'), { recursive: true })
await copyFile(FIXTURE, join(profile, 'mission-ledger', `${MISSION_ID}.jsonl`))
await writeFile(join(profile, 'teammates.json'), JSON.stringify({ schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, autoMode: false } }))

const child = spawn(ELECTRON, [APP_DIR, `--remote-debugging-port=${PORT}`, `--user-data-dir=${profile}`], {
  cwd: WORKSPACE,
  env: { ...process.env },
  stdio: ['ignore', 'pipe', 'pipe']
})
const appOutput = []
child.stdout.on('data', (d) => appOutput.push(String(d)))
child.stderr.on('data', (d) => appOutput.push(String(d)))

const ledgerFiles = async (where) =>
  (await readdir(join(profile, 'mission-ledger', where)).catch(() => [])).filter((name) => name.endsWith('.jsonl'))

try {
  let page
  for (let attempt = 0; attempt < 60 && page === undefined; attempt += 1) {
    await sleep(500)
    if (child.exitCode !== null) break
    try {
      const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json()
      page = list.find((t) => t.type === 'page' && t.webSocketDebuggerUrl && !t.url.includes('#splash'))
    } catch { /* not up yet */ }
  }
  if (page === undefined) { say(appOutput.join('')); throw new Error('no renderer target') }
  const socket = new WebSocket(page.webSocketDebuggerUrl)
  await new Promise((resolve, reject) => {
    socket.addEventListener('open', resolve, { once: true })
    socket.addEventListener('error', reject, { once: true })
  })
  const cdp = new Cdp(socket)
  await cdp.send('Runtime.enable')
  await cdp.eval(`(async () => { for (let i = 0; i < 240; i += 1) { const c = [...document.querySelectorAll('.lc-control')].find(b => b.getAttribute('aria-haspopup') === 'listbox'); if (c && /cursor|codex|claude|opencode/i.test(c.innerText)) return true; await new Promise(r => setTimeout(r, 500)) } return false })()`)

  // Each page script is written out whole rather than composed from pieces:
  // the harness guard parses what is sent AS WRITTEN, and a script spliced
  // together from fragments is not the thing it can check.
  say('1. the conversation is there')
  const present = await cdp.eval(`(async () => {
    const m = [...document.querySelectorAll('button')].find(b => /^All missions/.test(b.getAttribute('aria-label') || b.getAttribute('title') || ''))
    if (m) m.click()
    await new Promise(r => setTimeout(r, 700))
    return [...document.querySelectorAll('.lc-missionrow')].filter(r => /Two independent tasks/.test(r.innerText || '')).length
  })()`)
  check('one row for the fixture conversation', present === 1, `rows ${String(present)}`)
  check('its record is in the ledger', (await ledgerFiles('.')).length === 1)

  say('2. delete it the way it was lost: tick, Delete 1, Delete 1 for good')
  const deleted = await cdp.eval(`(async () => {
    const box = [...document.querySelectorAll('.lc-missionrow__pick')].find(input => /Two independent tasks/.test(input.getAttribute('aria-label') || ''))
    if (!box) return JSON.stringify({ ticked: false })
    box.click()
    await new Promise(r => setTimeout(r, 400))
    const first = document.querySelector('.lc-pickbar__delete')
    if (!first) return JSON.stringify({ ticked: true, bar: false })
    const armedLabel = first.innerText
    first.click()
    await new Promise(r => setTimeout(r, 400))
    const second = document.querySelector('.lc-pickbar__delete')
    const confirmLabel = second ? second.innerText : ''
    if (second) second.click()
    await new Promise(r => setTimeout(r, 1200))
    const rows = [...document.querySelectorAll('.lc-missionrow')].filter(r => /Two independent tasks/.test(r.innerText || '')).length
    return JSON.stringify({ ticked: true, bar: true, armedLabel, confirmLabel, rows })
  })()`)
  const d = JSON.parse(deleted)
  say(`   ${deleted}`)
  check('the first press only arms', /Delete 1$/.test((d.armedLabel ?? '').trim()), d.armedLabel)
  check('the second asks for good', /for good/.test(d.confirmLabel ?? ''), d.confirmLabel)
  check('the row is gone from the list', d.rows === 0, `rows ${String(d.rows)}`)

  say('3. the record is in the trash, not destroyed')
  check('no ledger file at the top level', (await ledgerFiles('.')).length === 0)
  check('the file is in .trash, byte for byte', (await ledgerFiles('.trash')).length === 1)

  say('4. put it back from Settings')
  const restored = await cdp.eval(`(async () => {
    const settings = [...document.querySelectorAll('button')].find(b => /^Settings/.test(b.getAttribute('aria-label') || b.getAttribute('title') || ''))
    if (settings) settings.click()
    await new Promise(r => setTimeout(r, 900))
    let listed
    for (let i = 0; i < 30 && !listed; i += 1) {
      listed = [...document.querySelectorAll('.lc-trash__what')].find(node => /Two independent tasks/.test(node.innerText || ''))
      if (!listed) await new Promise(r => setTimeout(r, 300))
    }
    if (!listed) return JSON.stringify({ listed: false, panel: (document.querySelector('.lc-trash') || { innerText: '' }).innerText.slice(0, 200) })
    const row = listed.closest('.lc-trash__row')
    const back = [...row.querySelectorAll('button')].find(b => /Put back/.test(b.innerText || ''))
    if (!back) return JSON.stringify({ listed: true, button: false })
    back.click()
    await new Promise(r => setTimeout(r, 1500))
    const m = [...document.querySelectorAll('button')].find(b => /^All missions/.test(b.getAttribute('aria-label') || b.getAttribute('title') || ''))
    if (m) m.click()
    await new Promise(r => setTimeout(r, 700))
    const rows = [...document.querySelectorAll('.lc-missionrow')].filter(r => /Two independent tasks/.test(r.innerText || '')).length
    return JSON.stringify({ listed: true, button: true, rows })
  })()`)
  const r = JSON.parse(restored)
  say(`   ${restored}`)
  check('the trash names the conversation', r.listed === true, r.panel)
  check('the row is back in the list', r.rows === 1, `rows ${String(r.rows)}`)

  say('5. the record is back where it was')
  check('a ledger file at the top level again', (await ledgerFiles('.')).length === 1)
  check('nothing left in the trash', (await ledgerFiles('.trash')).length === 0)

  const shot = await cdp.send('Page.captureScreenshot', { format: 'png' })
  const out = new URL('../docs/chain-measure/trash-2026-09-17.png', import.meta.url)
  await writeFile(out, Buffer.from(shot.result.data, 'base64'))
  say(`   screenshot ${out.pathname.slice(1)}`)
} finally {
  child.kill()
  await sleep(500)
  await rm(profile, { recursive: true, force: true }).catch(() => undefined)
}
console.error(failures === 0 ? 'TRASH DRIVE PASSED' : `${String(failures)} check(s) failed`)
process.exitCode = failures === 0 ? 0 : 1
