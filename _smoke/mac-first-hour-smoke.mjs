// The first hour on a Mac with nothing on it (CI, 0.712).
//
//   node _smoke/mac-first-hour-smoke.mjs <path to Locust.app/Contents/MacOS/Locust>
//
// The to-market plan's phase-1 measure -- a stranger installs Locust and gets
// an answer within five minutes -- was timed on a bare Windows machine
// (_tools/install-without-node-drive.mjs: 22 s on 0.691) and never on a Mac,
// though most of the people it launches to are on one. On a Mac the path is
// its own: the app's PATH comes from the login shell (mac-path.ts), npm is the
// one Locust carries, run by the app's binary as Node, and OpenCode's install
// script needs a `node` that only Locust's shim provides. The Mac launch check
// (posix-stop-smoke.mjs) never walks it: it installs OpenCode with the runner's
// own npm first.
//
// This starts the packaged app the way Finder does (from "/", with the bare
// system PATH) under a home with nothing in it, after the workflow has moved
// the runner's node, npm and opencode out of the folders Locust searches --
// and refuses to measure anything if one is still there. Then: the first
// screen, Install on the free runtime, the version it reports, a first message
// on the free model, and its answer, each timed. A provider that is down is
// met the way a person would meet it: the card's "Switch to ..." and Send.
// Free models only (LOCUST_FREE_ONLY); nothing is spent.
//
// Self-contained, like posix-stop-smoke.mjs: the drive library assumes Windows.

import { execFileSync, spawn } from 'node:child_process'
import { existsSync } from 'node:fs'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

const app = process.argv[2] === undefined ? undefined : resolve(process.argv[2])
if (app === undefined) {
  console.log('usage: node _smoke/mac-first-hour-smoke.mjs <Locust.app/Contents/MacOS/Locust>')
  process.exit(2)
}
const PORT = 9334
const WAIT_FOR_INSTALL_MS = 4 * 60_000
const WAIT_FOR_ANSWER_MS = 4 * 60_000
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  console.log(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}

const root = await mkdtemp(join(tmpdir(), 'locust-mac-first-hour-'))
const home = join(root, 'home')
const profile = join(root, 'profile')
// The folder a first launch makes for itself (index.ts: Documents/Locust), kept inside this run.
const folder = join(root, 'Documents', 'Locust')
for (const dir of [home, profile]) await mkdir(dir, { recursive: true })
await writeFile(join(profile, 'teammates.json'), JSON.stringify({ schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, autoMode: false } }), 'utf8')

// What launchd gives an app opened from Finder, under a home nobody has used.
const BARE_PATH = '/usr/bin:/bin:/usr/sbin:/sbin'
const env = {}
for (const [key, value] of Object.entries(process.env)) {
  if (/^(npm_|nvm_|node_|volta_|bun_|pnpm_)/i.test(key)) continue
  env[key] = value
}
Object.assign(env, { PATH: BARE_PATH, HOME: home, LOCUST_DEFAULT_WORKSPACE: folder, LOCUST_FREE_ONLY: '1' })

/*
 * Bare, or nothing is measured. The folders mac-path.ts searches: what the
 * login shell says under this home, the bare PATH, and the install folders it
 * adds after them (macInstallFolders). A runner's node in /opt/homebrew/bin
 * would make this a measurement of the runner's npm, not of Locust's.
 */
let shellPath = ''
try {
  const said = execFileSync(process.env.SHELL?.startsWith('/') ? process.env.SHELL : '/bin/zsh', ['-ilc', 'printf "__LOCUST_PATH__%s__LOCUST_PATH__" "$PATH"'], { env, encoding: 'utf8', timeout: 10_000, stdio: ['ignore', 'pipe', 'ignore'] })
  shellPath = /__LOCUST_PATH__(.*?)__LOCUST_PATH__/s.exec(said)?.[1] ?? ''
} catch { /* a shell that cannot be asked adds nothing */ }
const installFolders = ['/opt/homebrew/bin', '/usr/local/bin', ...['.local/bin', '.npm-global/bin', '.opencode/bin', '.claude/local', '.bun/bin', '.volta/bin'].map((tail) => join(home, tail))]
const looked = [...new Set([...shellPath.split(':'), ...BARE_PATH.split(':'), ...installFolders])].filter((dir) => dir.length > 0)
const present = looked.flatMap((dir) => ['node', 'npm', 'npx', 'opencode'].map((name) => join(dir, name))).filter((path) => existsSync(path))
check('a bare Mac: no node, npm or opencode anywhere Locust looks', present.length === 0, present.length === 0 ? `${String(looked.length)} folders looked in` : present.join(' '))
if (present.length > 0) {
  console.log('MAC FIRST HOUR SMOKE: not a bare machine, so nothing was measured')
  process.exit(1)
}

const timing = {}
const launchedAt = Date.now()
const child = spawn(app, [`--remote-debugging-port=${String(PORT)}`, `--user-data-dir=${profile}`], { cwd: '/', env, stdio: ['ignore', 'pipe', 'pipe'] })
const output = []
child.stdout.on('data', (data) => output.push(String(data)))
child.stderr.on('data', (data) => output.push(String(data)))

let socket
let nextId = 1
const waiting = new Map()
// 0.836's Mac run pressed Install and heard nothing more for 43 minutes, until the job's limit cancelled it and the
// launch and update checks after it never ran. A window that stops answering is now a failure that says so, with the
// app's own last lines, and the step ends.
const SILENT_MS = 90_000
const send = (method, params = {}) => new Promise((resolve, reject) => {
  const id = nextId++
  const silent = setTimeout(() => {
    waiting.delete(id)
    const last = output.join('').split('\n').slice(-25).join('\n')
    reject(new Error(`the window did not answer ${method} in ${String(SILENT_MS / 1000)} s; the app's last lines:\n${last}`))
  }, SILENT_MS)
  waiting.set(id, { resolve: (value) => { clearTimeout(silent); resolve(value) }, reject: (error) => { clearTimeout(silent); reject(error) } })
  socket.send(JSON.stringify({ id, method, params }))
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
    if (typeof shot?.data === 'string') await writeFile(`mac-first-hour-${name}.png`, Buffer.from(shot.data, 'base64'))
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
  check('the packaged app starts from Finder\'s folder with the bare PATH', page !== undefined)
  if (page === undefined) throw new Error('no window')
  socket = new WebSocket(page.webSocketDebuggerUrl)
  socket.addEventListener('message', (event) => {
    const message = JSON.parse(String(event.data))
    const held = waiting.get(message.id)
    if (held === undefined) return
    waiting.delete(message.id)
    if (message.error !== undefined) held.reject(new Error(message.error.message))
    else held.resolve(message.result)
  })
  await new Promise((resolve) => socket.addEventListener('open', resolve, { once: true }))

  console.log('1. the first screen, on a Mac with nothing on it')
  const first = JSON.parse(String(await evaluate(`(async () => {
    for (let i = 0; i < 120 && !document.querySelector('.lc-runtimecell__install.is-primary'); i += 1) await new Promise((r) => setTimeout(r, 500))
    const button = document.querySelector('.lc-runtimecell__install.is-primary')
    return JSON.stringify({ button: Boolean(button), disabled: button ? button.disabled : null, row: button?.closest('.lc-runtimecell')?.innerText.replace(/\\s+/g, ' ').trim().slice(0, 120) ?? null })
  })()`)))
  timing.firstScreen = Math.round((Date.now() - launchedAt) / 1000)
  check('the free runtime has an Install button', first.button === true, JSON.stringify(first))
  check('and it is enabled with no Node on the Mac', first.disabled === false, JSON.stringify(first))
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
    if (seconds % 30 < 3) console.log(`   ${String(seconds)} s: ${seen.opencode ?? '(no row)'}`)
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
  const launcher = join(profile, 'npm', 'bin', 'opencode')
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
    offer: [...document.querySelectorAll('button')].map((b) => b.innerText.trim()).find((t) => /^Switch to /.test(t)) ?? null
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
    if (now.failures > before.failures && !now.running) {
      // A free provider down is not Locust's failure; the card says what a person does next, and this does it.
      if (now.offer === null || switches.length >= 2) { console.log(`   failed: ${now.failed}`); break }
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
  console.log(`   answered on ${model}${switches.length === 0 ? '' : ` after ${switches.join(', ')}`}`)
  check('the free model answered', answered !== undefined, JSON.stringify(timing))
  check('from a bare Mac to a first answer in under five minutes', answered !== undefined && timing.total < 300, JSON.stringify(timing))
  await picture('3-answered')
  console.log(`MAC FIRST HOUR (seconds): ${JSON.stringify(timing)}`)
} catch (error) {
  failures += 1
  console.log(`  [FAIL] ${error instanceof Error ? error.message : String(error)}`)
} finally {
  socket?.close()
  child.kill()
  for (let i = 0; i < 20 && child.exitCode === null; i += 1) await sleep(250)
  await rm(root, { recursive: true, force: true }).catch(() => undefined)
}
console.log(failures === 0 ? 'MAC FIRST HOUR SMOKE PASSED' : `MAC FIRST HOUR SMOKE: ${String(failures)} check(s) failed`)
process.exitCode = failures === 0 ? 0 : 1
