// The round trip Grok's pass 10 found missing: install a CLI with no Node on
// the machine, and then FIND and RUN what was installed.
//
//   node _tools/install-without-node-drive.mjs
//
// 0.178.0 measured the first half -- the bundled npm installs a package --
// and shipped. Grok pressed the button the next day: npm ran, said done, and
// Locust answered "opencode installed, but Locust still cannot find the
// command", because npm had chosen a prefix from the binary's location and
// nothing searched it. The install was verified; where it went was not.
//
// This drive runs the PACKAGED Locust.exe with a PATH that has no node, npm
// or opencode on it, and with APPDATA/LOCALAPPDATA pointed at empty folders
// so the machine's own installs cannot be found through the inferred roots
// either. Then it presses Install and waits for the runtime row to report a
// version -- which means the locator found the shim and the app's own Node
// ran it.
//
// Live: `npm install -g opencode-ai` really runs, into the temp profile. No
// provider run, no quota.

import '../_tools/scratch-root.mjs'

import { spawn } from 'node:child_process'
import { existsSync } from 'node:fs'
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { localPath } from './local-path.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const EXE = join(APP_DIR, 'release', 'win-unpacked', 'Locust.exe')
const PORT = 9489
const WAIT_FOR_INSTALL_MS = 4 * 60 * 1000

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

if (!existsSync(EXE)) {
  say(`no packaged build at ${EXE}; run the package step first`)
  process.exit(1)
}

const root = await mkdtemp(join(tmpdir(), 'locust-nonode-'))
// Its frames' folder: a record, kept out of the public repository; made here, so a fresh clone runs it.
await mkdir(new URL('../docs/chain-measure/', import.meta.url), { recursive: true })
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

// A machine with nothing on it. The real PATH is not inherited.
const barePath = ['C:\\Windows\\System32', 'C:\\Windows', 'C:\\Windows\\System32\\Wbem'].join(';')
// The first hour, timed (the to-market plan's phase-1 measurement): open, Install, a first answer.
const launchedAt = Date.now()
const timing = {}
const child = spawn(EXE, [`--remote-debugging-port=${PORT}`, `--user-data-dir=${profile}`], {
  cwd: workspace,
  env: {
    ...process.env,
    PATH: barePath,
    Path: barePath,
    APPDATA: appData,
    LOCALAPPDATA: localAppData,
    // Nothing should ask npm for its prefix on a machine that has none; if
    // something does, it must not find this machine's.
    NPM_CONFIG_PREFIX: ''
  },
  stdio: ['ignore', 'pipe', 'pipe']
})
const appOutput = []
child.stdout.on('data', (d) => appOutput.push(String(d)))
child.stderr.on('data', (d) => appOutput.push(String(d)))

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

  say('1. the first screen, on a machine with nothing on it')
  const first = await cdp.eval(`(async () => {
    for (let i = 0; i < 120; i += 1) {
      const button = document.querySelector('.lc-runtimecell__install.is-primary')
      if (button) break
      await new Promise(r => setTimeout(r, 500))
    }
    const button = document.querySelector('.lc-runtimecell__install.is-primary')
    const notes = [...document.querySelectorAll('.lc-installnote')].map(n => n.innerText.replace(new RegExp('[' + String.fromCharCode(32, 9, 13, 10) + ']+', 'g'), ' ').trim())
    const clean = (s) => (s || '').replace(new RegExp('[' + String.fromCharCode(32, 9, 13, 10) + ']+', 'g'), ' ').trim()
    return JSON.stringify({
      button: !!button,
      disabled: button ? button.disabled : null,
      title: button ? button.title : null,
      notes,
      // The design ruling of 2026-09-18, on a launch with nothing installed:
      // no red card, no "coming soon", no changelog banner.
      redCard: !!document.querySelector('.lc-composer .lc-notice'),
      comingSoon: /coming soon/i.test(clean(document.body.innerText)),
      banner: /is running[.] Here is what changed/.test(clean(document.body.innerText))
    })
  })()`)
  say(`   ${first}`)
  timing.firstScreen = Math.round((Date.now() - launchedAt) / 1000)
  const screen = JSON.parse(first)
  check('the free runtime has an Install button', screen.button === true, first)
  check('and it is enabled with no Node on the machine', screen.disabled === false, first)
  check('no sentence still says the buttons are below anything', !screen.notes.some((n) => /below/.test(n)) && !/below/.test(screen.title ?? ''), first)
  check('no red card with nothing installed: nothing has stopped', screen.redCard === false)
  check('no "coming soon" on the screen whose job is Install', screen.comingSoon === false)
  check('the changelog banner is held: a fresh profile has no previous version', screen.banner === false)

  // The frame a person who just downloaded Locust sees, before anything is
  // pressed: the one the design agent redrew.
  const before = await cdp.send('Page.captureScreenshot', { format: 'png' })
  await writeFile(new URL('../docs/chain-measure/first-screen-nothing-installed-2026-09-19.png', import.meta.url), Buffer.from(before.result.data, 'base64'))

  say('2. press Install and wait')
  await cdp.eval(`(() => { document.querySelector('.lc-runtimecell__install.is-primary').click(); return 'pressed' })()`)
  const startedAt = Date.now()
  let outcome
  // The screen WHILE the install runs: Colin thought some installs looked
  // bugged, not showing as installing (2026-09-19). Captured once, early.
  await sleep(2500)
  // The install's sentences wait for the press (0.514): read them now, while it runs.
  const duringNotes = String(await cdp.eval(`[...document.querySelectorAll('.lc-installnote')].map((n) => n.innerText.replace(/\\s+/g, ' ').trim()).join(' | ')`))
  check('while it installs, the screen says it uses the npm the app carries', /npm/i.test(duringNotes), duringNotes)
  const during = await cdp.send('Page.captureScreenshot', { format: 'png' })
  await writeFile(new URL('../docs/chain-measure/first-screen-installing-2026-09-19.png', import.meta.url), Buffer.from(during.result.data, 'base64'))
  while (Date.now() - startedAt < WAIT_FOR_INSTALL_MS) {
    await sleep(3000)
    const state = await cdp.eval(`(() => {
      const failed = document.querySelector('.lc-installnote--failed')
      const button = document.querySelector('.lc-runtimecell__install.is-primary')
      const cells = [...document.querySelectorAll('.lc-runtimecell')].map(c => c.innerText.replace(new RegExp('[' + String.fromCharCode(32, 9, 13, 10) + ']+', 'g'), ' ').trim())
      const opencode = cells.find(c => /OpenCode/i.test(c)) || null
      const status = [...document.querySelectorAll('.lc-installnote, .lc-runtimecell__tag')].map(n => n.innerText.trim()).join(' | ')
      return JSON.stringify({ failed: failed ? failed.innerText.slice(0, 300) : null, installing: !!(button && button.disabled), opencode, status })
    })()`)
    const seen = JSON.parse(state)
    const seconds = Math.round((Date.now() - startedAt) / 1000)
    if (seen.failed !== null) { outcome = { kind: 'failed', seen, seconds }; break }
    // The row reports a version only when the locator found the shim AND the
    // probe ran it: that is the whole round trip in one string.
    if (seen.opencode !== null && /\d+\.\d+\.\d+/.test(seen.opencode) && !/Install\b/.test(seen.opencode)) {
      outcome = { kind: 'connected', seen, seconds }
      break
    }
    if (seconds % 30 === 0) say(`   ${seconds}s: installing=${String(seen.installing)} row="${seen.opencode ?? seen.status}"`)
  }
  say(`   ${JSON.stringify(outcome ?? { kind: 'timed out' })}`)
  timing.install = outcome?.seconds
  // What the host itself believes, past whatever the screen drew: the
  // discovery answer names the runtime, whether it is available, and the
  // version its probe read -- or the reason it did not.
  const believed = await cdp.eval(`(async () => {
    const r = await window.desktop.getLocalRuntimes()
    if (!r.ok) return JSON.stringify(r)
    const oc = r.data.runtimes.find(x => x.id === 'opencode')
    return JSON.stringify({ npmPresent: r.data.npmPresent, npmIsBundled: r.data.npmIsBundled, opencode: oc })
  })()`)
  say(`   host: ${believed}`)
  check('the install finished without a failure card', outcome?.kind !== 'failed', JSON.stringify(outcome?.seen?.failed))
  check('the runtime row reports a version, so Locust found the CLI and ran it', outcome?.kind === 'connected', JSON.stringify(outcome?.seen?.opencode))

  // And once something is connected, the banner it was held for.
  const afterConnect = await cdp.eval(`(() => {
    const clean = (s) => (s || '').replace(new RegExp('[' + String.fromCharCode(32, 9, 13, 10) + ']+', 'g'), ' ').trim()
    return JSON.stringify({ banner: /is running[.] Here is what changed/.test(clean(document.body.innerText)) })
  })()`)
  // The home banner is gone by design (Colin, 2026-09-23: "it adds a needless scrollbar"); the changelog is Settings' What's new.
  check('no changelog banner on Home once a runtime is connected', JSON.parse(afterConnect).banner === false, afterConnect)

  say('3. where it went')
  const shim = join(profile, 'npm', 'opencode.cmd')
  check('the shim is in the folder Locust owns, not beside the binary', existsSync(shim), shim)
  check('and nothing landed beside the binary', !existsSync(join(APP_DIR, 'release', 'win-unpacked', 'opencode.cmd')))

  /*
   * 4. THE FIRST ANSWER (2026-10-06). The plan's phase-1 task is the whole
   * path, timed: from a machine with nothing on it to a reply on the free
   * model. Until now this drive stopped once the CLI was found. Free model
   * only; nothing is spent.
   */
  say('4. the first answer, on the free model just installed')
  const route = await cdp.eval(`[...document.querySelectorAll('form.command-dock button')].map((b) => b.innerText.trim()).find((t) => /OpenCode|Claude|Codex|Cursor/.test(t)) ?? ''`)
  say(`   the box is on: ${String(route)}`)
  check('the box is on the free model once it is installed', /OpenCode/.test(String(route)), String(route))
  const sentAt = Date.now()
  const sent = await cdp.eval(`(async () => {
    const field = document.querySelector('form.command-dock textarea')
    if (!field) return 'no composer'
    Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set.call(field, 'Say hello to me in five words.')
    field.dispatchEvent(new Event('input', { bubbles: true }))
    for (let i = 0; i < 120; i += 1) {
      await new Promise((r) => setTimeout(r, 250))
      const b = document.querySelector('button[aria-label="Send"]')
      if (b && !b.disabled) { b.click(); return 'sent' }
    }
    const b = document.querySelector('button[aria-label="Send"]')
    return 'send never enabled: ' + (b?.title || 'no button')
  })()`)
  check('a first message can be sent', sent === 'sent', String(sent))
  let firstWords
  let answered
  for (let i = 0; sent === 'sent' && i < 300; i += 1) {
    await sleep(1000)
    const now = JSON.parse(String(await cdp.eval(`JSON.stringify({ text: [...document.querySelectorAll('.lc-agentline__body')].map((e) => e.innerText.trim()).join(' ').length, running: Boolean(document.querySelector('button[aria-label^="Stop the running"]')), failed: document.querySelector('.lc-diagnostic.lc-tone-red, .lc-card.is-terminal.is-red')?.innerText?.slice(0, 200) ?? null })`)))
    if (firstWords === undefined && now.text > 0) firstWords = Math.round((Date.now() - sentAt) / 1000)
    if (now.failed !== null) { say(`   failed: ${now.failed}`); break }
    if (now.text > 0 && !now.running) { answered = Math.round((Date.now() - sentAt) / 1000); break }
  }
  timing.firstWords = firstWords
  timing.answered = answered
  timing.total = Math.round((Date.now() - launchedAt) / 1000)
  check('the free model answered', answered !== undefined, JSON.stringify(timing))
  check('from a bare machine to a first answer in under five minutes', answered !== undefined && timing.total < 300, JSON.stringify(timing))
  say(`FIRST HOUR (seconds): ${JSON.stringify(timing)}`)

  const shot = await cdp.send('Page.captureScreenshot', { format: 'png' })
  const out = new URL('../docs/chain-measure/install-without-node-2026-09-18.png', import.meta.url)
  await writeFile(out, Buffer.from(shot.result.data, 'base64'))
  say(`   screenshot ${out.pathname.slice(1)}`)
} finally {
  child.kill()
  await sleep(800)
  await rm(root, { recursive: true, force: true }).catch(() => undefined)
}
console.error(failures === 0 ? 'INSTALL WITHOUT NODE DRIVE PASSED' : `${String(failures)} check(s) failed`)
process.exitCode = failures === 0 ? 0 : 1
