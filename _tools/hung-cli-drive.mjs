// A CLI that hangs on its version probe must not end the first hour.
//
//   node _tools/hung-cli-drive.mjs
//
// Grok, pass 11, 2026-09-18: five shims on PATH named for the coding CLIs,
// each sleeping for an hour. Every row read CHECKING, every Install button
// was gone, and the composer asked for a sign-in beside a row saying no
// account was needed. Nothing on the screen could change.
//
// This puts hanging `.cmd` shims on a PATH that has nothing else, points
// APPDATA and LOCALAPPDATA at empty folders so the machine's real CLIs are
// not found through the inferred roots, starts the app, and waits past the
// three re-checks (15 s each, after a first one at 15 s). Then it reads the
// OpenCode row and the composer's placeholder.
//
// No provider run; costs nothing. About 90 seconds.

import '../_tools/scratch-root.mjs'

import { spawn, spawnSync } from 'node:child_process'
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { localPath } from './local-path.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const PORT = 9491
const GIVE_UP_AFTER_MS = 120_000

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

const root = await mkdtemp(join(tmpdir(), 'locust-hung-'))
const workspace = join(root, 'workspace')
const profile = join(root, 'profile')
const shims = join(root, 'shims')
const appData = join(root, 'AppData', 'Roaming')
const localAppData = join(root, 'AppData', 'Local')
for (const dir of [workspace, profile, shims, appData, localAppData]) await mkdir(dir, { recursive: true })
await writeFile(join(workspace, 'README.md'), 'Scratch workspace.\n', 'utf8')
await writeFile(
  join(profile, 'teammates.json'),
  JSON.stringify({ schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, autoMode: false } })
)
// A shim that never answers. `timeout` needs stdin it does not have under a
// spawn, so it is a ping to nowhere that takes an hour instead.
for (const name of ['opencode', 'codex', 'claude', 'copilot', 'cursor-agent']) {
  await writeFile(join(shims, `${name}.cmd`), '@echo off\r\nping -n 3600 127.0.0.1 >nul\r\n', 'utf8')
}

const barePath = [shims, 'C:\\Windows\\System32', 'C:\\Windows'].join(';')
const child = spawn(ELECTRON, [APP_DIR, `--remote-debugging-port=${PORT}`, `--user-data-dir=${profile}`], {
  cwd: workspace,
  env: { ...process.env, PATH: barePath, Path: barePath, APPDATA: appData, LOCALAPPDATA: localAppData },
  stdio: ['ignore', 'pipe', 'pipe']
})
const appOutput = []
child.stdout.on('data', (d) => appOutput.push(String(d)))
child.stderr.on('data', (d) => appOutput.push(String(d)))

const readScreen = (cdp) => cdp.eval(`(() => {
  const clean = (s) => (s || '').replace(new RegExp('[' + String.fromCharCode(32, 9, 13, 10) + ']+', 'g'), ' ').trim()
  const cells = [...document.querySelectorAll('.lc-runtimecell')].map(c => clean(c.innerText))
  const box = document.querySelector('.lc-composer textarea, textarea')
  return JSON.stringify({
    opencode: cells.find(c => /OpenCode/i.test(c)) || null,
    head: (document.querySelector('.lc-agenthead__note') || { innerText: '' }).innerText.trim(),
    placeholder: box ? box.getAttribute('placeholder') : null,
    checkAgain: !!([...document.querySelectorAll('button')].find(b => /Check again/.test(b.innerText))),
    // The red card and the no-Node sentence: neither belongs on a screen
    // whose rows are still CHECKING (Colin, 2026-09-19, with the frame).
    redCard: !!document.querySelector('.lc-composer .lc-notice'),
    npmNote: [...document.querySelectorAll('.lc-installnote')].some(n => /copy of npm it carries/.test(n.innerText))
  })
})()`)

try {
  let page
  for (let attempt = 0; attempt < 90 && page === undefined; attempt += 1) {
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

  say('1. the first minute: CHECKING is honest while discovery is still asking')
  // The rows are drawn once the first sweep answers, which with five hung
  // probes is the probe timeout plus the stagger: wait for them, not for a
  // number of seconds.
  let early = JSON.parse(await readScreen(cdp))
  for (let i = 0; i < 40 && early.opencode === null; i += 1) {
    await sleep(500)
    early = JSON.parse(await readScreen(cdp))
  }
  say(`   ${JSON.stringify(early)}`)
  check('the head note says checking, once, with a count', /checking \d+ on this machine/.test(early.head ?? ''), early.head)
  check('and offers no Install yet', early.checkAgain === false)
  check('the composer does not ask for a sign-in nothing has asked for', !/sign in/.test(early.placeholder ?? ''), early.placeholder)
  check('and while the rows are still being asked it says checking, not install or not-answering', /Checking the AI agents/.test(early.placeholder ?? ''), early.placeholder)
  check('no red card while the rows are still checking', early.redCard === false)
  check('no no-Node sentence while there is no Install to be about', early.npmNote === false)

  say('2. after discovery has asked three more times')
  const startedAt = Date.now()
  let late = early
  while (Date.now() - startedAt < GIVE_UP_AFTER_MS) {
    await sleep(5_000)
    late = JSON.parse(await readScreen(cdp))
    if (late.checkAgain) break
  }
  const gaveUpAfterMs = Date.now() - startedAt
  say(`   gave up after ${String(Math.round(gaveUpAfterMs / 1000))}s`)
  /*
   * FOUR SWEEPS, not two. Fable's pass 1 timed the shims: the rows gave up
   * after the second real sweep, because the counter counted answers and two
   * of them came from the host's ten-second cache. Sweeps are fifteen
   * seconds apart, so four of them cannot complete inside forty seconds of
   * the first; the tooltip's "four times" is measured here, not assumed.
   */
  check('the rows gave up after four sweeps, not two (at least 40s in)', gaveUpAfterMs >= 40_000, `${String(Math.round(gaveUpAfterMs / 1000))}s`)
  say(`   ${Math.round((Date.now() - startedAt) / 1000)}s: ${JSON.stringify(late)}`)
  check('the row says what was tried', /did not answer its version check/.test(late.opencode ?? ''), late.opencode)
  check('and Check again is on the screen -- not Install, which it already is', late.checkAgain === true)
  check('the composer still does not ask for a sign-in', !/sign in/.test(late.placeholder ?? ''), late.placeholder)
  // Now something IS standing -- a runtime that is here and never answered
  // -- and the rows ARE the register for that. 0.185.0 drew the red card
  // here too; Grok's pass 14 and Fable's pass 1 both read it from the
  // stranger's seat: five rows saying not answering with Check again beside
  // each, a box that already cannot send, and a card telling them to
  // install what is installed. The card stays for a sign-in only (0.189.0).
  check('no red card: the rows and Check again are the repair', late.redCard === false)
  check('the composer says installed but not answering, and points at Check again', /installed but not answering/.test(late.placeholder ?? ''), late.placeholder)

  const shot = await cdp.send('Page.captureScreenshot', { format: 'png' })
  const out = new URL('../docs/chain-measure/hung-cli-2026-09-18.png', import.meta.url)
  await writeFile(out, Buffer.from(shot.result.data, 'base64'))
  say(`   screenshot ${out.pathname.slice(1)}`)
} finally {
  child.kill()
  await sleep(800)
  // The probes are killed on their timeout, but the ping each shim started
  // is a grandchild and outlives cmd.exe. Fifty of them were found running
  // after the first two attempts of this drive, holding the shim folder.
  // Counted BEFORE the cleanup: with the runner killing the tree on timeout,
  // this should be zero; before that fix it was five per sweep.
  const listed = spawnSync('tasklist', [], { encoding: 'utf8', windowsHide: true }).stdout || ''
  const leaked = listed.split(String.fromCharCode(10)).filter((l) => /^ping\.exe/i.test(l)).length
  say(`   hung probes still running at exit: ${String(leaked)}`)
  check('a timed-out probe takes its tree with it', leaked === 0, String(leaked))
  spawnSync('taskkill', ['/F', '/IM', 'ping.exe'], { stdio: 'ignore', windowsHide: true })
  await rm(root, { recursive: true, force: true }).catch(() => undefined)
}
console.error(failures === 0 ? 'HUNG CLI DRIVE PASSED' : `${String(failures)} check(s) failed`)
process.exitCode = failures === 0 ? 0 : 1
