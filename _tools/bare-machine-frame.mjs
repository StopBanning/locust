// What the first screen looks like to somebody who has nothing installed.
//
//   node _tools/bare-machine-frame.mjs
//
// Every frame anybody has captured of the first screen was taken on THIS
// machine, which has all six coding agents on it and reads "6 ready". The
// person Colin is designing for -- "a new user without tech savvyness ... able
// to just use the software off rip" -- sees a different screen entirely, and
// nobody has looked at it.
//
// So this launches the PACKAGED build with a PATH that has no node, npm or any
// CLI on it, and with APPDATA and LOCALAPPDATA pointed at empty folders so the
// inferred roots find nothing either -- the same isolation
// `install-without-node-drive.mjs` uses -- and captures the frame plus every
// word on it.
//
// It presses NOTHING. No install runs, no provider runs, nothing is spent.

import '../_tools/scratch-root.mjs'

import { spawn } from 'node:child_process'
import { existsSync } from 'node:fs'
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { localPath } from './local-path.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const EXE = join(APP_DIR, 'release', 'win-unpacked', 'Locust.exe')
const PORT = 9506

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

if (!existsSync(EXE)) {
  say(`no packaged build at ${EXE}; run the package step first`)
  process.exit(1)
}

const root = await mkdtemp(join(tmpdir(), 'locust-bare-'))
const workspace = join(root, 'workspace')
const profile = join(root, 'profile')
const appData = join(root, 'AppData', 'Roaming')
const localAppData = join(root, 'AppData', 'Local')
for (const dir of [workspace, profile, appData, localAppData]) await mkdir(dir, { recursive: true })
await writeFile(join(workspace, 'README.md'), 'Scratch workspace.\n', 'utf8')
await writeFile(
  join(profile, 'teammates.json'),
  JSON.stringify({ schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, autoMode: false } })
)

const barePath = ['C:\\Windows\\System32', 'C:\\Windows', 'C:\\Windows\\System32\\Wbem'].join(';')
const child = spawn(EXE, [`--remote-debugging-port=${String(PORT)}`, `--user-data-dir=${profile}`], {
  cwd: workspace,
  env: { ...process.env, PATH: barePath, Path: barePath, APPDATA: appData, LOCALAPPDATA: localAppData, NPM_CONFIG_PREFIX: '' },
  stdio: ['ignore', 'pipe', 'pipe']
})

try {
  let page
  for (let attempt = 0; attempt < 90 && page === undefined; attempt += 1) {
    await sleep(500)
    if (child.exitCode !== null) throw new Error(`app exited ${String(child.exitCode)}`)
    try {
      const list = await (await fetch(`http://127.0.0.1:${String(PORT)}/json/list`)).json()
      page = list.find((t) => t.type === 'page' && t.webSocketDebuggerUrl && !t.url.includes('#splash'))
    } catch { /* not up yet */ }
  }
  if (page === undefined) throw new Error('no renderer target')
  const socket = new WebSocket(page.webSocketDebuggerUrl)
  await new Promise((resolve, reject) => {
    socket.addEventListener('open', resolve, { once: true })
    socket.addEventListener('error', reject, { once: true })
  })
  const cdp = new Cdp(socket)
  await cdp.send('Runtime.enable')

  // Discovery has to finish saying "checking" before the screen means
  // anything; four sweeps is what the app itself gives up after.
  await cdp.eval(`(async () => {
    for (let i = 0; i < 300; i += 1) {
      const note = document.querySelector('.lc-agenthead__note')
      if (note && !/checking/i.test(note.innerText)) return note.innerText
      await new Promise(r => setTimeout(r, 500))
    }
    return 'still checking'
  })()`)
  await sleep(2000)

  const read = JSON.parse(await cdp.eval(`(() => {
    const clean = (s) => (s || '').replace(new RegExp('[' + String.fromCharCode(32, 9, 13, 10) + ']+', 'g'), ' ').trim()
    const rows = [...document.querySelectorAll('.lc-runtimecell')].map(cell => ({
      name: clean((cell.querySelector('.lc-runtimecell__name') || {}).innerText),
      need: clean((cell.querySelector('.lc-runtimecell__need') || {}).innerText),
      action: clean(([...cell.querySelectorAll('button')][0] || {}).innerText),
      onramp: cell.className.includes('is-onramp')
    }))
    return JSON.stringify({
      headNote: clean((document.querySelector('.lc-agenthead__note') || {}).innerText),
      rows,
      // Every word on the screen, in order, because the question the design
      // agent is being asked is about what a person READS here.
      body: clean(document.body.innerText),
      // Can they type at all? The composer is the thing they came for.
      composer: (() => {
        const field = document.querySelector('form.command-dock textarea')
        const send = document.querySelector('form.command-dock .send-button')
        return field ? { placeholder: field.getAttribute('placeholder'), sendDisabled: send ? send.disabled : null } : null
      })()
    })
  })()`))

  say(`head note: ${read.headNote}`)
  say('')
  for (const row of read.rows) {
    say(`  ${row.onramp ? '>' : ' '} ${row.name.padEnd(16)} ${row.need.padEnd(34)} ${row.action}`)
  }
  say('')
  say(`composer: ${JSON.stringify(read.composer)}`)
  say('')
  say(`everything on the screen, in order:\n${read.body}`)

  const shot = await cdp.send('Page.captureScreenshot', { format: 'png' })
  const out = new URL('../docs/chain-measure/bare-machine-first-screen-2026-09-19.png', import.meta.url)
  await writeFile(out, Buffer.from(shot.result.data, 'base64'))
  say('')
  say(`frame ${out.pathname.slice(1)}`)
} finally {
  child.kill()
  await sleep(1000)
  await rm(root, { recursive: true, force: true }).catch(() => undefined)
}
