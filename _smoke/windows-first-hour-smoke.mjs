// The first hour on a Windows machine with nothing on it (CI, 0.847).
//
//   node _smoke/windows-first-hour-smoke.mjs <Locust-x.y.z-setup.exe | Locust.exe>
//
// The Mac's first hour is measured on every build (mac-first-hour-smoke.mjs).
// Windows' was measured by hand, on the PC that built the Windows releases
// (_tools/install-without-node-drive.mjs: 22 s on 0.691), and that PC is
// retired. Since then no Windows Locust was opened before it shipped: CI built
// it, ran the suite on it and checked its update, and nobody saw its first
// screen.
//
// Given the installer, this runs it as a person does -- silently, /S, the same
// pages with nobody to press Next -- and starts what it installed; given a
// Locust.exe (an unpacked build on a PC), it starts that. Either way the app
// gets the PATH Explorer gives an account nobody has used, and an empty home
// and AppData, so neither the runner's Node nor anything installed with it can
// be found -- and it refuses to measure anything if one still could be. Then,
// as on the Mac: the first screen, Install on the free runtime, the version it
// reports, a first message on the free model, and its answer, each timed. A
// provider that is down is met the way a person would meet it: the card's
// "Switch to ..." and Send. So is a free model that is busy or says nothing
// (C9, 0.846): its "Stop and switch to ...", and Send. Free models only
// (LOCUST_FREE_ONLY); nothing is spent.
//
// Self-contained, like the Mac's: the drive library assumes the PC's folders.

import { execFileSync, spawn } from 'node:child_process'
import { appendFileSync, existsSync, readdirSync } from 'node:fs'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'

import { portFor, waitForPortFree } from './ports.mjs'

const given = process.argv[2] === undefined ? undefined : resolve(process.argv[2])
if (given === undefined || process.platform !== 'win32') {
  console.log('usage, on Windows: node _smoke/windows-first-hour-smoke.mjs <Locust-x.y.z-setup.exe | Locust.exe>')
  process.exit(2)
}
const installer = /-setup\.exe$/i.test(given)
// The installer replaces the Locust this account has: a runner's to lose, never a person's.
if (installer && process.env.CI !== 'true') {
  console.log('WINDOWS FIRST HOUR SMOKE: the installer runs on a CI runner only, since it replaces the Locust installed for this account. Give it an unpacked Locust.exe here.')
  process.exit(2)
}
const PORT = portFor(import.meta.url)
// Inside the step's 12 minutes, as on the Mac (0.837): what it saw is said, and the app it started is stopped.
const DEADLINE_MS = 10 * 60_000
const LOG = 'windows-first-hour.log'
const say = console.log
console.log = (...parts) => {
  say(...parts)
  try { appendFileSync(LOG, `${parts.join(' ')}\n`) } catch { /* the console has it */ }
}
const WAIT_FOR_SETUP_MS = 3 * 60_000
const WAIT_FOR_INSTALL_MS = 4 * 60_000
// Long enough for one free model to say nothing for its two minutes and the next to answer.
const WAIT_FOR_ANSWER_MS = 5 * 60_000
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  console.log(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}
const startedAt = Date.now()
// Set once the app is started; until then there is nothing of ours to stop.
let stopAll = () => undefined
setTimeout(() => {
  console.log(`  [FAIL] still going after ${String(DEADLINE_MS / 60_000)} minutes`)
  console.log('WINDOWS FIRST HOUR SMOKE: stopped at its deadline')
  stopAll()
  process.exit(1)
}, DEADLINE_MS).unref()

const timing = {}
let app = given
if (installer) {
  // For this account, into %LOCALAPPDATA%\Programs: where a double-click on the download puts it (perMachine: false).
  const programs = join(process.env.LOCALAPPDATA ?? join(homedir(), 'AppData', 'Local'), 'Programs')
  console.log('0. the installer, as a person runs it')
  const setup = spawn(given, ['/S'], { stdio: 'ignore', windowsHide: true })
  const code = await new Promise((resolve) => {
    setup.on('exit', resolve)
    setup.on('error', (error) => resolve(String(error)))
    setTimeout(() => resolve('still running'), WAIT_FOR_SETUP_MS)
  })
  timing.setup = Math.round((Date.now() - startedAt) / 1000)
  const installed = existsSync(programs) ? readdirSync(programs).map((name) => join(programs, name, 'Locust.exe')).find((path) => existsSync(path)) : undefined
  check('the installer ends and leaves Locust.exe under %LOCALAPPDATA%\\Programs', code === 0 && installed !== undefined, `exit ${String(code)}, ${installed ?? 'no Locust.exe'}, ${String(timing.setup)} s`)
  if (installed === undefined) {
    console.log('WINDOWS FIRST HOUR SMOKE: nothing was installed, so nothing was measured')
    process.exit(1)
  }
  app = installed
}

const root = await mkdtemp(join(tmpdir(), 'locust-windows-first-hour-'))
const home = join(root, 'home')
const profile = join(root, 'profile')
const appData = join(home, 'AppData', 'Roaming')
const localAppData = join(home, 'AppData', 'Local')
// The folder a first launch makes for itself (index.ts: Documents/Locust), kept inside this run.
const folder = join(home, 'Documents', 'Locust')
for (const dir of [profile, appData, localAppData]) await mkdir(dir, { recursive: true })
await writeFile(join(profile, 'teammates.json'), JSON.stringify({ schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, autoMode: false } }), 'utf8')

// What Explorer gives an app on an account nobody has used: Windows' own folders, and the account's WindowsApps.
const windows = process.env.SystemRoot ?? 'C:\\Windows'
const BARE_PATH = [
  join(windows, 'System32'),
  windows,
  join(windows, 'System32', 'Wbem'),
  join(windows, 'System32', 'WindowsPowerShell', 'v1.0'),
  join(windows, 'System32', 'OpenSSH'),
  join(localAppData, 'Microsoft', 'WindowsApps')
].join(';')
const env = {}
for (const [key, value] of Object.entries(process.env)) {
  if (/^(npm_|nvm_|node_|volta_|bun_|pnpm_)/i.test(key)) continue
  // Windows reads these names case-blind, and two spellings of one would reach the app in an order nobody chose.
  if (/^(path|appdata|localappdata|userprofile|home)$/i.test(key)) continue
  env[key] = value
}
Object.assign(env, { Path: BARE_PATH, APPDATA: appData, LOCALAPPDATA: localAppData, USERPROFILE: home, HOME: home, LOCUST_DEFAULT_WORKSPACE: folder, LOCUST_FREE_ONLY: '1' })

/*
 * Bare, or nothing is measured. Where Locust looks on Windows (path-locator.ts):
 * the PATH, then npm's default prefix (%APPDATA%\npm) and the installers' own
 * folders under %LOCALAPPDATA% -- all inside this run's empty home now. The
 * runner's Node (Program Files, the tool cache) is on none of them; a PATH that
 * still reached one would make this a measurement of the runner's npm, not of
 * the one Locust carries.
 */
const looked = [...new Set([...BARE_PATH.split(';'), join(appData, 'npm'), join(localAppData, 'Programs'), join(home, '.opencode', 'bin'), join(home, '.local', 'bin')])]
const present = looked.flatMap((dir) => ['node.exe', 'npm', 'npm.cmd', 'npx.cmd', 'opencode', 'opencode.cmd', 'opencode.exe'].map((name) => join(dir, name))).filter((path) => existsSync(path))
check('a bare Windows account: no node, npm or opencode anywhere Locust looks', present.length === 0, present.length === 0 ? `${String(looked.length)} folders looked in` : present.join(' '))
if (present.length > 0) {
  console.log('WINDOWS FIRST HOUR SMOKE: not a bare machine, so nothing was measured')
  process.exit(1)
}

if (!(await waitForPortFree(PORT))) {
  console.log(`WINDOWS FIRST HOUR SMOKE: something already answers on ${String(PORT)}, so the window driven would not be this one`)
  process.exit(1)
}
const launchedAt = Date.now()
// From the folder its Start-menu shortcut starts it in: its own.
const child = spawn(app, [`--remote-debugging-port=${String(PORT)}`, `--user-data-dir=${profile}`], { cwd: dirname(app), env, stdio: ['ignore', 'pipe', 'pipe'] })
const output = []
child.stdout.on('data', (data) => output.push(String(data)))
child.stderr.on('data', (data) => output.push(String(data)))
const lastLines = () => output.join('').split('\n').slice(-25).join('\n')
// No process groups to signal here: taskkill walks the tree down from the app, so it runs while the app is still up.
stopAll = () => {
  try { execFileSync('taskkill', ['/PID', String(child.pid), '/T', '/F'], { stdio: 'ignore', timeout: 15_000 }) } catch { /* gone */ }
  child.stdout.destroy()
  child.stderr.destroy()
}

let socket
let nextId = 1
const waiting = new Map()
// A window that stops answering is a failure that says so, with the app's own last lines (the Mac's 0.836).
const targets = async () => {
  try {
    const response = await fetch(`http://127.0.0.1:${String(PORT)}/json/list`, { signal: AbortSignal.timeout(5_000) })
    console.log(`   DevTools targets: ${JSON.stringify(await response.json())}`)
  } catch (error) { console.log(`   DevTools targets unavailable: ${String(error)}`) }
}
const rejectPending = (error) => {
  for (const held of waiting.values()) held.reject(error)
  waiting.clear()
}
const SILENT_MS = 90_000
const send = (method, params = {}) => new Promise((resolve, reject) => {
  const id = nextId++
  const stalled = setTimeout(() => {
    console.log(`   Still waiting for ${method} after 15 s`)
    void targets()
  }, 15_000)
  const silent = setTimeout(async () => {
    waiting.delete(id)
    await targets()
    reject(new Error(`the window did not answer ${method} in ${String(SILENT_MS / 1000)} s; the app's last lines:\n${lastLines()}`))
  }, SILENT_MS)
  waiting.set(id, { resolve: (value) => { clearTimeout(stalled); clearTimeout(silent); resolve(value) }, reject: (error) => { clearTimeout(stalled); clearTimeout(silent); reject(error) } })
  try { socket.send(JSON.stringify({ id, method, params })) } catch (error) {
    waiting.delete(id)
    clearTimeout(stalled)
    clearTimeout(silent)
    reject(error)
  }
})
const evaluate = async (expression) => {
  for (let attempt = 0; ; attempt += 1) {
    try {
      const result = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })
      return result?.result?.value
    } catch (error) {
      if (attempt >= 5 || !/context was destroyed|Cannot find context/i.test(String(error?.message ?? error))) throw error
      await sleep(1_500)
    }
  }
}
const picture = async (name) => {
  try {
    const shot = await send('Page.captureScreenshot', { format: 'png' })
    if (typeof shot?.data === 'string') await writeFile(`windows-first-hour-${name}.png`, Buffer.from(shot.data, 'base64'))
  } catch { /* a picture is only evidence */ }
}

try {
  let page
  for (let i = 0; i < 120 && page === undefined; i += 1) {
    await sleep(500)
    if (child.exitCode !== null) throw new Error(`the app exited ${String(child.exitCode)}: ${output.join('').slice(-600)}`)
    try {
      const list = await (await fetch(`http://127.0.0.1:${String(PORT)}/json/list`)).json()
      page = list.find((target) => target.type === 'page' && target.webSocketDebuggerUrl && !target.url.includes('#splash'))
    } catch { /* not listening yet */ }
  }
  check('the installed app starts with the PATH a new account has', page !== undefined, app)
  if (page === undefined) throw new Error('no window')
  socket = new WebSocket(page.webSocketDebuggerUrl)
  socket.addEventListener('message', (event) => {
    const message = JSON.parse(String(event.data))
    if (message.method === 'Inspector.targetCrashed' || message.method === 'Inspector.detached') {
      console.log(`   ${JSON.stringify(message)}`)
      rejectPending(new Error(message.method))
    }
    if (['Runtime.exceptionThrown', 'Page.javascriptDialogOpening'].includes(message.method)) console.log(`   ${JSON.stringify(message)}`)
    const held = waiting.get(message.id)
    if (held === undefined) return
    waiting.delete(message.id)
    if (message.error !== undefined) held.reject(new Error(message.error.message))
    else held.resolve(message.result)
  })
  socket.addEventListener('close', (event) => {
    const error = new Error(`DevTools socket closed (${String(event.code)}): ${event.reason}`)
    console.log(`   ${error.message}`)
    rejectPending(error)
    void targets()
  })
  socket.addEventListener('error', () => rejectPending(new Error('DevTools socket failed')))
  await new Promise((resolve, reject) => {
    socket.addEventListener('open', resolve, { once: true })
    socket.addEventListener('error', () => reject(new Error('DevTools could not connect')), { once: true })
  })
  await send('Inspector.enable')
  await send('Runtime.enable')
  await send('Page.enable')

  console.log('1. the first screen, on a Windows machine with nothing on it')
  const first = JSON.parse(String(await evaluate(`(async () => {
    for (let i = 0; i < 120 && !document.querySelector('.lc-runtimecell__install.is-primary'); i += 1) await new Promise((r) => setTimeout(r, 500))
    const button = document.querySelector('.lc-runtimecell__install.is-primary')
    return JSON.stringify({ button: Boolean(button), disabled: button ? button.disabled : null, row: button?.closest('.lc-runtimecell')?.innerText.replace(/\\s+/g, ' ').trim().slice(0, 120) ?? null })
  })()`)))
  timing.firstScreen = Math.round((Date.now() - launchedAt) / 1000)
  check('the free runtime has an Install button', first.button === true, JSON.stringify(first))
  check('and it is enabled with no Node on the machine', first.disabled === false, JSON.stringify(first))
  await picture('1-first-screen')
  if (first.button !== true || first.disabled !== false) throw new Error('nothing to install with')

  console.log('2. Install, and wait for the version')
  await evaluate(`(() => { document.querySelector('.lc-runtimecell__install.is-primary').click(); return 'pressed' })()`)
  const installFrom = Date.now()
  let installed
  while (Date.now() - installFrom < WAIT_FOR_INSTALL_MS) {
    await sleep(3_000)
    const seen = JSON.parse(String(await evaluate(`JSON.stringify({
      failed: document.querySelector('.lc-installnote--failed')?.innerText.slice(0, 400) ?? null,
      opencode: [...document.querySelectorAll('.lc-runtimecell')].map((c) => c.innerText.replace(/\\s+/g, ' ').trim()).find((c) => /OpenCode/i.test(c)) ?? null
    })`)))
    const seconds = Math.round((Date.now() - installFrom) / 1000)
    if (seen.failed !== null) { installed = { kind: 'failed', seen, seconds }; break }
    // A version on the row: the locator found what was installed AND ran it.
    if (seen.opencode !== null && /\d+\.\d+\.\d+/.test(seen.opencode) && !/Install\b/.test(seen.opencode)) { installed = { kind: 'connected', seen, seconds }; break }
    console.log(`   ${String(seconds)} s: ${seen.opencode ?? '(no row)'}`)
  }
  timing.install = installed?.seconds
  console.log(`   ${JSON.stringify(installed ?? { kind: 'timed out' })}`)
  const host = JSON.parse(String(await evaluate(`(async () => {
    const r = await window.desktop.getLocalRuntimes()
    if (!r.ok) return JSON.stringify({ error: r.error })
    const oc = r.data.runtimes.find((x) => x.id === 'opencode')
    return JSON.stringify({ npmPresent: r.data.npmPresent, npmIsBundled: r.data.npmIsBundled, opencode: oc === undefined ? null : { installed: oc.installed, version: oc.version, ready: oc.ready, status: oc.status } })
  })()`)))
  console.log(`   host: ${JSON.stringify(host)}`)
  check('the install ends without a failure card', installed?.kind !== 'failed', JSON.stringify(installed?.seen?.failed ?? null))
  check('OpenCode reports a version, so Locust found it and ran it', installed?.kind === 'connected', JSON.stringify(installed?.seen?.opencode ?? null))
  check('the npm that ran is the one Locust carries', host.npmIsBundled === true, JSON.stringify(host))
  const launcher = join(profile, 'npm', 'opencode.cmd')
  check('OpenCode is in the folder Locust owns', existsSync(launcher), launcher)
  await picture('2-installed')
  if (installed?.kind !== 'connected') throw new Error('not installed')

  console.log('3. the first answer, on the free model')
  const sentAt = Date.now()
  const sendIt = (words) => evaluate(`(async () => {
    const box = () => document.querySelector('form.command-dock textarea')
    for (let i = 0; i < 60 && box() === null; i += 1) await new Promise((r) => setTimeout(r, 500))
    if (box() === null) return 'no box'
    if (${JSON.stringify(words)}.length > 0 && box().value.length === 0) {
      Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set.call(box(), ${JSON.stringify(words)})
      box().dispatchEvent(new Event('input', { bubbles: true }))
    }
    for (let i = 0; i < 120; i += 1) {
      await new Promise((r) => setTimeout(r, 250))
      const button = document.querySelector('button[aria-label="Send"]')
      if (button && !button.disabled && box().value.length > 0) { button.click(); return 'sent' }
    }
    return 'Send never enabled: ' + (document.querySelector('button[aria-label="Send"]')?.title ?? 'no button')
  })()`)
  const onTheBox = `[...document.querySelectorAll('form.command-dock button')].map((b) => b.innerText.trim()).find((t) => /OpenCode|free/i.test(t)) ?? ''`
  const route = String(await evaluate(onTheBox))
  check('the chat box is on a free model once OpenCode is in', route.length > 0, route)
  // What the thread holds now, so a failure card or words already there are not read as the next run's.
  const reading = `JSON.stringify({
    text: [...document.querySelectorAll('.lc-agentline__body')].map((e) => e.innerText.trim()).join(' ').length,
    running: Boolean(document.querySelector('button[aria-label^="Stop the running"]')),
    failures: document.querySelectorAll('.lc-diagnostic.lc-tone-red, .lc-card.is-terminal.is-red').length,
    failed: [...document.querySelectorAll('.lc-diagnostic.lc-tone-red, .lc-card.is-terminal.is-red')].pop()?.innerText.replace(/\\s+/g, ' ').slice(0, 240) ?? null,
    offer: [...document.querySelectorAll('button')].map((b) => b.innerText.trim()).find((t) => /^Switch to /.test(t)) ?? null,
    stop: [...document.querySelectorAll('button')].map((b) => b.innerText.trim()).find((t) => /^Stop and switch to /.test(t)) ?? null,
    quiet: Boolean(document.querySelector('.lc-rerun[data-quiet-free]'))
  })`
  let before = JSON.parse(String(await evaluate(reading)))
  let sent = String(await sendIt('Say hello to me in five words.'))
  check('a first message can be sent', sent === 'sent', sent)
  const switches = []
  let firstWords
  let answered
  while (sent === 'sent' && Date.now() - sentAt < WAIT_FOR_ANSWER_MS) {
    await sleep(1_000)
    const now = JSON.parse(String(await evaluate(reading)))
    const words = now.text - before.text
    if (firstWords === undefined && words > 0) firstWords = Math.round((Date.now() - sentAt) / 1000)
    if (words > 0 && !now.running) { answered = Math.round((Date.now() - sentAt) / 1000); break }
    // A free model whose provider is busy (C9), or that has said nothing for two minutes (0.846): the note's own
    // press stops it, moves the chat box on and puts the message back, and a person presses it and Send.
    if (now.stop !== null && now.running && switches.length < 3) {
      console.log(`   the model ${now.quiet ? 'said nothing for two minutes' : 'is busy'}; pressing "${now.stop}"`)
      switches.push(now.stop)
      await evaluate(`(() => { [...document.querySelectorAll('button')].find((b) => b.innerText.trim() === ${JSON.stringify(now.stop)})?.click(); return 'pressed' })()`)
      await sleep(1_500)
      before = JSON.parse(String(await evaluate(reading)))
      sent = String(await sendIt(''))
      continue
    }
    if (now.failures > before.failures && !now.running) {
      // A free provider down is not Locust's failure; the card says what a person does next, and this does it.
      if (now.offer === null || switches.length >= 3) { console.log(`   failed: ${now.failed}`); break }
      console.log(`   the model failed (${now.failed}); pressing "${now.offer}"`)
      switches.push(now.offer)
      await evaluate(`(() => { [...document.querySelectorAll('button')].find((b) => b.innerText.trim() === ${JSON.stringify(now.offer)})?.click(); return 'pressed' })()`)
      await sleep(1_500)
      before = JSON.parse(String(await evaluate(reading)))
      sent = String(await sendIt(''))
    }
  }
  timing.firstWords = firstWords
  timing.answered = answered
  timing.total = Math.round((Date.now() - launchedAt) / 1000)
  const model = String(await evaluate(onTheBox))
  // Said either way, on one line (0.848): it read "answered on GLM 5.3 Flash" over a run still waiting on a busy provider.
  console.log(`   ${answered === undefined ? 'no answer yet' : 'answered'} on ${model.replace(/\s+/g, ' ').trim()}${switches.length === 0 ? '' : ` after ${switches.join(', ')}`}`)
  check('the free model answered', answered !== undefined, JSON.stringify(timing))
  check('from a bare Windows machine to a first answer in under five minutes', answered !== undefined && timing.total < 300, JSON.stringify(timing))
  await picture('3-answered')
  console.log(`WINDOWS FIRST HOUR (seconds): ${JSON.stringify(timing)}`)
} catch (error) {
  failures += 1
  console.log(`  [FAIL] ${error instanceof Error ? error.message : String(error)}`)
} finally {
  socket?.close()
  stopAll()
  for (let i = 0; i < 20 && child.exitCode === null; i += 1) await sleep(250)
  // A file the app had open can stay locked for a moment after it is gone.
  await rm(root, { recursive: true, force: true, maxRetries: 5, retryDelay: 500 }).catch(() => undefined)
}
console.log(failures === 0 ? 'WINDOWS FIRST HOUR SMOKE PASSED' : `WINDOWS FIRST HOUR SMOKE: ${String(failures)} check(s) failed`)
process.exit(failures === 0 ? 0 : 1)
