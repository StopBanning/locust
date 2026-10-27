// A Mac Locust updates itself: download, Restart and install, back on the new copy (CI, 0.516).
//
//   node _smoke/mac-update-smoke.mjs <path to the built Locust.app> <path to a disk image of it>
//
// Colin, 2026-10-01, for a tester on a Mac: "try and make it work for him and
// just like how ours is". mac-self-update.ts replaces the app with Locust's
// own download, because macOS's updater refuses the unsigned build -- and the
// Windows machine that builds everything cannot run any of it. This runs on
// GitHub's macOS runner:
//
//   1. The built app is copied into /Applications, as a drag-install leaves
//      it, and a marker file is put inside it: the OLD copy.
//   2. It is started with its own disk image offered as version 9.9.9
//      (LOCUST_MAC_UPDATE_TEST_DMG/_VERSION), so there is something to
//      install without a real newer release.
//   3. The ordinary update service must say it is ready; Restart and install
//      is pressed through the same bridge the banner uses.
//   4. Locust must quit, the app in /Applications must be the NEW copy (no
//      marker), nothing staged or set aside may be left, and Locust must be
//      running again from /Applications.
//
// Exit 0 only if every check passes. Self-contained, like posix-stop-smoke.mjs.

import { execFileSync, spawn } from 'node:child_process'
import { appendFileSync, existsSync } from 'node:fs'
import { mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

import { portFor, waitForPortFree } from './ports.mjs'

const built = process.argv[2] === undefined ? undefined : resolve(process.argv[2])
const image = process.argv[3] === undefined ? undefined : resolve(process.argv[3])
if (built === undefined || image === undefined) {
  console.log('usage: node _smoke/mac-update-smoke.mjs <Locust.app> <Locust.dmg>')
  process.exit(2)
}
const APP = '/Applications/Locust.app'
const BINARY = `${APP}/Contents/MacOS/Locust`
const MARKER = `${APP}/Contents/Resources/locust-update-smoke-old.txt`
const PORT = portFor(import.meta.url)
// 0.837's ran 30 minutes into the job's limit, driving the window the first-hour check had left on the port both
// used, and the dmgs went out with no Mac build. Now: a port of its own that must be free, a window that stops
// answering is a failure that says so, the whole check ends inside its step, and every line also goes to a file
// the workflow keeps, since GitHub keeps no log of a job that runs out its time.
const SILENT_MS = 90_000
const DEADLINE_MS = 6 * 60_000
const LOG = 'mac-update.log'
const say = console.log
console.log = (...parts) => {
  say(...parts)
  try { appendFileSync(LOG, `${parts.join(' ')}\n`) } catch { /* the console has it */ }
}
const sleep = (ms) => new Promise((done) => setTimeout(done, ms))
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  console.log(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}
const running = () => {
  try {
    return execFileSync('pgrep', ['-f', BINARY], { encoding: 'utf8' }).split('\n').map((line) => line.trim()).filter(Boolean)
  } catch {
    return []
  }
}

// 1. Installed, as a person's drag leaves it, and marked as the old copy.
execFileSync('rm', ['-rf', APP])
execFileSync('ditto', [built, APP])
await writeFile(MARKER, 'the copy that was installed first\n', 'utf8')
check('the built app is installed in /Applications, marked as the old copy', existsSync(MARKER))

if (!(await waitForPortFree(PORT))) {
  console.log(`MAC UPDATE SMOKE: something already answers on ${String(PORT)}, so the window driven would not be this one`)
  process.exit(1)
}

// 2. Started with its own disk image offered as a newer version.
const profile = await mkdtemp(join(tmpdir(), 'locust-mac-update-profile-'))
const child = spawn(BINARY, [`--remote-debugging-port=${String(PORT)}`, `--user-data-dir=${profile}`], {
  cwd: profile,
  env: { ...process.env, LOCUST_FREE_ONLY: '1', LOCUST_MAC_UPDATE_TEST_DMG: image, LOCUST_MAC_UPDATE_TEST_VERSION: '9.9.9' },
  stdio: ['ignore', 'pipe', 'pipe'],
  // Its own process group, so the app and anything it started can be stopped together.
  detached: true
})
const output = []
child.stdout.on('data', (data) => output.push(String(data)))
child.stderr.on('data', (data) => output.push(String(data)))
const lastLines = () => output.join('').split('\n').slice(-25).join('\n')
const stopAll = () => {
  try { process.kill(-child.pid, 'SIGKILL') } catch { /* gone */ }
  child.stdout.destroy()
  child.stderr.destroy()
}
setTimeout(() => {
  console.log(`  [FAIL] still going after ${String(DEADLINE_MS / 60_000)} minutes; the app's last lines:\n${lastLines()}`)
  console.log('MAC UPDATE SMOKE: stopped at its deadline')
  stopAll()
  process.exit(1)
}, DEADLINE_MS).unref()

let socket
let nextId = 1
const waiting = new Map()
const evaluateOnce = (expression) => new Promise((done, fail) => {
  const id = nextId++
  const silent = setTimeout(() => {
    waiting.delete(id)
    fail(new Error(`the window did not answer in ${String(SILENT_MS / 1000)} s; the app's last lines:\n${lastLines()}`))
  }, SILENT_MS)
  waiting.set(id, { resolve: (value) => { clearTimeout(silent); done(value) }, reject: (error) => { clearTimeout(silent); fail(error) } })
  socket.send(JSON.stringify({ id, method: 'Runtime.evaluate', params: { expression, awaitPromise: true, returnByValue: true } }))
})
const evaluate = async (expression) => {
  for (let attempt = 0; ; attempt += 1) {
    try {
      return await evaluateOnce(expression)
    } catch (error) {
      if (attempt >= 5 || !/context was destroyed|Cannot find context/i.test(String(error?.message ?? error))) throw error
      await sleep(1_500)
    }
  }
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
  check('the installed app starts', page !== undefined)
  if (page === undefined) throw new Error('no window')
  socket = new WebSocket(page.webSocketDebuggerUrl)
  socket.addEventListener('message', (event) => {
    const message = JSON.parse(String(event.data))
    const held = waiting.get(message.id)
    if (held === undefined) return
    waiting.delete(message.id)
    if (message.error !== undefined) held.reject(new Error(message.error.message))
    // A page that threw comes back as its exception: said, not parsed as an answer.
    else if (message.result?.exceptionDetails !== undefined) held.reject(new Error(`the page threw: ${String(message.result.exceptionDetails.exception?.description ?? message.result.exceptionDetails.text)}`))
    else held.resolve(message.result?.result?.value)
  })
  await new Promise((done) => socket.addEventListener('open', done, { once: true }))
  // The window fully up: the bridge there and the composer drawn.
  const up = await evaluate(`(async () => {
    for (let i = 0; i < 120; i += 1) {
      if (window.desktop?.checkForUpdate !== undefined && document.querySelector('form.command-dock textarea')) return 'up'
      await new Promise((r) => setTimeout(r, 500))
    }
    return 'not up: ' + typeof window.desktop + ' ' + (document.body?.innerText ?? '').slice(0, 200)
  })()`)
  check('the window is up, with the bridge', up === 'up', String(up))

  // 3. The update service: checked, downloaded (here: the test image staged), ready.
  const ready = await evaluate(`(async () => {
    let state = null
    for (let i = 0; i < 90; i += 1) {
      let answer
      try {
        answer = await window.desktop.checkForUpdate()
      } catch (error) {
        state = { threw: String(error?.message ?? error) }
        await new Promise((r) => setTimeout(r, 2000))
        continue
      }
      state = answer.ok ? answer.data : answer.error
      if (answer.ok && answer.data.phase === 'ready') break
      await new Promise((r) => setTimeout(r, 2000))
    }
    return JSON.stringify(state)
  })()`)
  const state = JSON.parse(String(ready))
  check('the update service says 9.9.9 is downloaded and ready', state?.phase === 'ready' && state?.availableVersion === '9.9.9', String(ready))
  check('the new copy is staged beside the old one', existsSync('/Applications/.Locust-update.app'))
  const firstPid = child.pid

  const pressed = await evaluate(`window.desktop.installUpdate().then((answer) => JSON.stringify(answer.ok ? { ok: true } : answer.error))`).catch((error) => JSON.stringify({ gone: String(error?.message ?? error) }))
  console.log(`  install: ${String(pressed)}`)

  // 4. Gone, swapped, and back.
  for (let i = 0; i < 60 && child.exitCode === null; i += 1) await sleep(500)
  check('Locust quits for the install', child.exitCode !== null, `exit ${String(child.exitCode)}`)
  let again = []
  for (let i = 0; i < 60 && again.length === 0; i += 1) {
    await sleep(1_000)
    again = running().filter((pid) => pid !== String(firstPid))
  }
  check('the app in /Applications is the new copy: the old one\'s marker is gone', !existsSync(MARKER))
  check('nothing staged or set aside is left', !existsSync('/Applications/.Locust-update.app') && !existsSync(`${APP}.replaced`))
  check('and Locust is running again from /Applications', again.length > 0, again.join(','))
  for (const pid of again) {
    try { process.kill(Number(pid)) } catch { /* gone */ }
  }
} catch (error) {
  failures += 1
  console.log(`smoke failed: ${error instanceof Error ? error.message : String(error)}`)
  console.log(output.join('').slice(-1500))
} finally {
  try { child.kill() } catch { /* gone */ }
  try { socket?.close() } catch { /* closed */ }
  stopAll()
}
console.log(failures === 0 ? 'MAC UPDATE SMOKE PASSED' : `${String(failures)} MAC UPDATE SMOKE FAILURE(S)`)
process.exit(failures === 0 ? 0 : 1)
