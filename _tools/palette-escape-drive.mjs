// The command palette closes on Escape from anywhere.
//
//   node _tools/palette-escape-drive.mjs
//
// Grok, pass 11: Ctrl+K opened it, Escape left it open, changing screen
// closed it. The Escape handler sat on the palette's own element and only
// heard the key when focus was inside. It listens at the document now, the
// way the plus menu does. This opens the palette the way their harness did --
// a window keydown -- presses Escape at the document, and reads whether the
// palette is still there.
//
// No provider run; costs nothing.

import '../_tools/scratch-root.mjs'

import { spawn } from 'node:child_process'
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { localPath } from './local-path.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const PORT = 9493

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

const workspace = await mkdtemp(join(tmpdir(), 'locust-palette-'))
const profile = await mkdtemp(join(tmpdir(), 'locust-palette-profile-'))
await mkdir(workspace, { recursive: true })
await writeFile(join(workspace, 'README.md'), 'Scratch workspace.\n', 'utf8')
await writeFile(join(profile, 'teammates.json'), JSON.stringify({ schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, autoMode: false } }))

const child = spawn(ELECTRON, [APP_DIR, `--remote-debugging-port=${PORT}`, `--user-data-dir=${profile}`], {
  cwd: workspace,
  env: { ...process.env },
  stdio: ['ignore', 'pipe', 'pipe']
})
const appOutput = []
child.stdout.on('data', (d) => appOutput.push(String(d)))
child.stderr.on('data', (d) => appOutput.push(String(d)))

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
  await sleep(4000)

  say('1. Ctrl+K opens the palette; Escape at the document closes it')
  const result = await cdp.eval(`(async () => {
    const present = () => !!document.querySelector('.lc-palette, [class*="palette"]')
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', code: 'KeyK', ctrlKey: true, bubbles: true }))
    await new Promise(r => setTimeout(r, 400))
    const opened = present()
    // Focus somewhere that is NOT the palette, the way a person who clicked
    // away would be, then press the key the rest of the app trained.
    if (document.body) document.body.focus()
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', code: 'Escape', bubbles: true }))
    await new Promise(r => setTimeout(r, 400))
    const afterEscape = present()
    return JSON.stringify({ opened, afterEscape })
  })()`)
  say(`   ${result}`)
  const seen = JSON.parse(result)
  check('the palette opened on Ctrl+K', seen.opened === true, result)
  check('and closed on Escape without focus inside it', seen.afterEscape === false, result)
} finally {
  child.kill()
  await sleep(500)
  await rm(profile, { recursive: true, force: true }).catch(() => undefined)
  await rm(workspace, { recursive: true, force: true }).catch(() => undefined)
}
console.error(failures === 0 ? 'PALETTE ESCAPE DRIVE PASSED' : `${String(failures)} check(s) failed`)
process.exitCode = failures === 0 ? 0 : 1
