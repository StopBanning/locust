// Launch the packaged app on a Mac or on Linux, run one free turn, press Stop (CI, 0.496; Linux since 0.789).
//
//   node _smoke/posix-stop-smoke.mjs <Locust.app/Contents/MacOS/Locust | the installed /opt/Locust/locust>
//   node _smoke/posix-stop-smoke.mjs --launch-only <Locust-...-linux-x86_64.AppImage>
//
// The process-group Stop (0.476) was written for macOS and had never run on a
// real Mac: the Windows machine builds and drives everything, and a Mac build
// was only ever made, never started. This runs on GitHub's macOS and Ubuntu
// runners after the images are built: a teammate on a free OpenCode model is
// asked for a long, slow answer; once it is running, Stop is pressed, and
// afterwards no `opencode` process the run started may be left. Spends nothing
// (free model; LOCUST_FREE_ONLY refuses any other route). `--launch-only` stops
// after the window opens: the AppImage is checked for starting on a stock
// Ubuntu, where its sandbox is the risk, not its Stop. Exit 0 only if every
// check passes.
//
// Self-contained on purpose: the drive library assumes Windows (its paths,
// `electron.exe`, npm's folder), and this has to run where that is untrue.

import { execFileSync, spawn } from 'node:child_process'
import { mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

import { portFor, waitForPortFree } from './ports.mjs'

const launchOnly = process.argv.includes('--launch-only')
const given = process.argv.slice(2).find((arg) => !arg.startsWith('--'))
// Absolute: the app is started from its profile folder, not from here.
const app = given === undefined ? undefined : resolve(given)
if (app === undefined) {
  console.log('usage: node _smoke/posix-stop-smoke.mjs [--launch-only] <Locust.app/Contents/MacOS/Locust | /opt/Locust/locust | Locust.AppImage>')
  process.exit(2)
}
const SYSTEM = process.platform === 'darwin' ? 'macOS' : 'Linux'
/** Thrown once the window has opened under `--launch-only`: everything after it is skipped, nothing failed. */
class LaunchOnly extends Error {}
// Every process with its command line: BSD's spelling on a Mac, POSIX's on Linux, where procps reads `-axo` as a mistake.
const PS_EVERY = process.platform === 'darwin' ? ['-axo', 'pid=,command='] : ['-eo', 'pid=,args=']
/**
 * A free model OpenCode serves TODAY (the first Mac run, 2026-10-30): with a fixed `opencode/nemotron-3-ultra-free`
 * the turn ended "Model not found" before Stop had anything to stop -- and `opencode models` still LISTED it, so a
 * listing alone is not enough. The drives' own free route first (drive-lib's FREE_ROUTE), then the old one, then any
 * free model `opencode models` lists; LOCUST_FREE_MODEL still decides when given.
 */
const freeModel = () => {
  if (process.env.LOCUST_FREE_MODEL !== undefined) return process.env.LOCUST_FREE_MODEL
  const preferred = ['opencode/deepseek-v4.1-flash-free', 'opencode/nemotron-3-ultra-free']
  let listed = []
  try {
    listed = execFileSync('opencode', ['models'], { encoding: 'utf8', timeout: 60_000 }).split('\n').map((line) => line.trim())
  } catch {
    // No list to read: the first preference, and the run says what it was.
  }
  return preferred.find((model) => listed.includes(model)) ?? listed.find((model) => /^opencode\/\S+-free$/.test(model)) ?? preferred[0]
}
const MODEL = launchOnly ? 'opencode/nemotron-3-ultra-free' : freeModel()
// Its own port per check (0.789), from _smoke/ports.mjs unless the workflow names one: a copy from the check before
// can still be closing, and a second copy started then gets no debugging port at all -- on Linux the AppImage, right
// after the .deb. On a Mac someone else's app can hold 9333 (Toaster's, the first Mac run): the port must be free.
const PORT = Number(process.env.LOCUST_SMOKE_PORT ?? portFor(import.meta.url))
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  console.log(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}
const git = (args, cwd) => execFileSync('git', args, { cwd, encoding: 'utf8' })
/**
 * The `opencode` processes on this machine, by pid -- not counting Locust's own first-start setup (0.780,
 * opencode-first-start.ts: `opencode debug config`, started when the window finds OpenCode, before and apart from any
 * run). On a runner with a fresh OpenCode it was still installing when Stop was pressed, and was counted as the run's
 * (0.780's first Mac run: "left": the setup's pid).
 */
const opencodes = () => {
  try {
    return execFileSync('ps', PS_EVERY, { encoding: 'utf8' })
      .split('\n')
      .map((line) => line.trim())
      .filter((line) => line.includes('opencode') && !/\bdebug config\b/.test(line))
      .map((line) => line.split(/\s+/)[0])
      .filter(Boolean)
  } catch {
    return []
  }
}

const workspace = await mkdtemp(join(tmpdir(), 'locust-smoke-ws-'))
git(['init', '-q', '-b', 'main'], workspace)
git(['config', 'user.email', 'smoke@locust.test'], workspace)
git(['config', 'user.name', 'Locust smoke'], workspace)
await writeFile(join(workspace, 'README.md'), '# smoke\n', 'utf8')
git(['add', '.'], workspace)
git(['commit', '-q', '-m', 'first'], workspace)
const profile = await mkdtemp(join(tmpdir(), 'locust-smoke-profile-'))
await writeFile(join(profile, 'teammates.json'), JSON.stringify({
  schemaVersion: 1,
  teammates: [{ teammateId: 'tm_ash', name: 'Ash', hue: 'clay', role: 'Custom', roleTitle: 'Helper', createdAt: '2026-09-30T00:00:00.000Z', route: { runtime: 'opencode', model: MODEL, mode: 'accept-edits' } }],
  missionOwners: {},
  settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' }
}), 'utf8')

if (!(await waitForPortFree(PORT))) {
  console.log(`${SYSTEM.toUpperCase()} STOP SMOKE: something already answers on ${String(PORT)}, so the window driven would not be this one`)
  process.exit(1)
}
console.log(`model: ${MODEL}`)
const before = new Set(opencodes())
const child = spawn(app, [`--remote-debugging-port=${String(PORT)}`, `--user-data-dir=${profile}`, `--workspace=${workspace}`], {
  cwd: profile,
  env: { ...process.env, LOCUST_FREE_ONLY: '1' },
  stdio: ['ignore', 'pipe', 'pipe']
})
const output = []
child.stdout.on('data', (data) => output.push(String(data)))
child.stderr.on('data', (data) => output.push(String(data)))

let socket
let nextId = 1
const waiting = new Map()
const evaluateOnce = (expression) => new Promise((resolve, reject) => {
  const id = nextId++
  waiting.set(id, { resolve, reject })
  socket.send(JSON.stringify({ id, method: 'Runtime.evaluate', params: { expression, awaitPromise: true, returnByValue: true } }))
})
// The window may reload once as it finishes starting (a first launch does); the page is asked again.
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

/** What the window shows, as a picture the workflow keeps. */
const shoot = async (file) => {
  const id = nextId++
  const shot = await new Promise((resolve) => {
    waiting.set(id, { resolve: () => undefined, reject: () => undefined })
    const listener = (event) => {
      const message = JSON.parse(String(event.data))
      if (message.id !== id) return
      socket.removeEventListener('message', listener)
      resolve(message.result?.data)
    }
    socket.addEventListener('message', listener)
    socket.send(JSON.stringify({ id, method: 'Page.captureScreenshot', params: { format: 'png' } }))
  })
  if (typeof shot === 'string') await writeFile(file, Buffer.from(shot, 'base64'))
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
  check(`the packaged app starts on ${SYSTEM} and opens its window`, page !== undefined)
  // What the app said, when it opened nothing: a running process with no window has no exit code to report.
  if (page === undefined) throw new Error(`no window; the app said: ${output.join('').slice(-1500) || 'nothing'}`)
  if (launchOnly) throw new LaunchOnly()
  socket = new WebSocket(page.webSocketDebuggerUrl)
  socket.addEventListener('message', (event) => {
    const message = JSON.parse(String(event.data))
    const held = waiting.get(message.id)
    if (held === undefined) return
    waiting.delete(message.id)
    if (message.error !== undefined) held.reject(new Error(message.error.message))
    else held.resolve(message.result?.result?.value)
  })
  await new Promise((resolve) => socket.addEventListener('open', resolve, { once: true }))

  const opened = await evaluate(`(async () => {
    for (let i = 0; i < 60; i += 1) {
      // The face in the full sidebar, or, in a window under 1200px wide, the same teammate in the narrow layout's rail.
      const face = [...document.querySelectorAll('.lc-faces__one')].find((one) => (one.getAttribute('aria-label') ?? '').startsWith('Ash — '))
        ?? [...document.querySelectorAll('.lc-railslot .lc-row--button')].find((one) => (one.getAttribute('aria-label') ?? '').startsWith('Ash'))
      if (face) { face.click(); await new Promise((r) => setTimeout(r, 800)); return 'opened' }
      await new Promise((r) => setTimeout(r, 500))
    }
    // What the window had instead (0.789): its size, the faces it drew, and the roster the app loaded.
    const roster = await window.desktop?.listTeammates?.().catch(() => undefined)
    const names = roster?.ok === true ? roster.data.teammates.map((t) => t.name).join(', ') : 'unreadable'
    const faces = [...document.querySelectorAll('.lc-faces__one')].map((one) => one.getAttribute('aria-label')).join(' | ')
    return 'no face: window ' + window.innerWidth + 'x' + window.innerHeight + ' | roster: ' + names + ' | faces: ' + (faces || 'none')
      + ' | ' + (document.body.innerText ?? '').replace(/\\s+/g, ' ').slice(0, 400)
  })()`)
  check("Ash's conversation opens", opened === 'opened', String(opened))
  if (opened !== 'opened') await shoot(`${SYSTEM === 'macOS' ? 'mac' : 'linux'}-no-face.png`)

  // Up to three turns (0.720): on 0.720's first Mac run OpenCode appeared and was gone 3 s later, the turn over
  // on its own, and Stop had nothing to stop (run 38045981686). A turn that ends before Stop proves nothing
  // either way, so another is sent once the window says the last one is over; Stop is still pressed on a run
  // whose process group is alive, or the check fails as before.
  let sent
  let during = []
  let aliveAtStop = []
  for (let attempt = 1; attempt <= 3; attempt += 1) {
    sent = await evaluate(`(async () => {
      // The exact command (0.715): asked for 'a command that prints 1 to 300', the steadiest free model answered
      // without running one, its OpenCode exited within seconds, and there was no run left to stop (0.714's Mac run).
      const WORDS = 'Run this exact shell command and wait for it to finish, then tell me the last number it printed: for i in $(seq 1 300); do echo $i; sleep 1; done'
      const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
      // The box is found again every time: the window can draw a new one as the
      // conversation opens, and a held reference then reads the old, detached box
      // (0.504's first run: "never ran: start enabled" after the re-press).
      const box = () => document.querySelector('form.command-dock textarea')
      const fill = () => {
        const field = box()
        if (field === null || field.value.length > 0) return
        setter.call(field, WORDS)
        field.dispatchEvent(new Event('input', { bubbles: true }))
      }
      if (box() === null) return 'no box'
      fill()
      // Pressed again if the words are still in the box: on a runner that has just
      // started, a press can land before the window is listening (0.497, run 36696510212).
      let presses = 0
      let sent = false
      for (let i = 0; i < 80 && presses < 3 && !sent; i += 1) {
        fill()
        const button = document.querySelector('button[aria-label="Send"]')
        if (button && !button.disabled && (box()?.value.length ?? 0) > 0) {
          button.click()
          presses += 1
          for (let wait = 0; wait < 16 && (box()?.value.length ?? 0) > 0; wait += 1) await new Promise((r) => setTimeout(r, 500))
          // Emptied, and a turn on screen: it went. Emptied with nothing on screen: the box was redrawn.
          if ((box()?.value.length ?? 0) === 0 && document.querySelector('.lc-thread .lc-bubble')) sent = true
        }
        await new Promise((r) => setTimeout(r, 250))
      }
      window.__presses = presses
      window.__box = box()?.value ?? '(no box)'
      for (let i = 0; i < 180; i += 1) {
        await new Promise((r) => setTimeout(r, 1000))
        if (document.querySelector('button[aria-label="Stop the running reply"]') && document.querySelector('.lc-livestep')) return window.__presses > 1 ? 'running (Start pressed ' + window.__presses + ' times)' : 'running'
      }
      const start = document.querySelector('button[aria-label="Send"]')
      return 'never ran: pressed ' + window.__presses + ' times, box held "' + String(window.__box).slice(0, 40) + '" | start ' + (start === null ? 'missing' : start.disabled ? 'disabled (' + (start.getAttribute('title') ?? '') + ')' : 'enabled')
        + ' | notice: ' + ([...document.querySelectorAll('.lc-notice')].map((el) => el.innerText).join(' / ') || 'none')
        + ' | runtimes: ' + (document.querySelector('.lc-connected')?.getAttribute('title') ?? '?')
        + ' | thread: ' + (document.querySelector('.lc-thread')?.innerText ?? '').replace(/\\s+/g, ' ').slice(-400)
    })()`)
    const running = String(sent).startsWith('running')
    if (!running) {
      await shoot(`${SYSTEM === 'macOS' ? 'mac' : 'linux'}-smoke.png`)
    }
    // Let the runtime get going: the Stop that matters ends a process tree already doing work.
    // Waited for, not assumed: on a runner just started, OpenCode can take a while to appear.
    if (!running) break
    const seenBefore = during
    const waitedFrom = Date.now()
    let fresh = []
    for (let second = 0; second < 45 && fresh.length === 0; second += 1) {
      await sleep(1_000)
      fresh = opencodes().filter((pid) => !before.has(pid) && !seenBefore.includes(pid))
    }
    // Every process seen is kept (0.715): read again after the wait, a run that had already ended
    // wiped what was seen, and the check below reported nothing ran when something had.
    if (fresh.length > 0) await sleep(3_000)
    during = [...new Set([...seenBefore, ...fresh, ...opencodes().filter((pid) => !before.has(pid))])]
    aliveAtStop = opencodes().filter((pid) => during.includes(pid))
    console.log(`  turn ${String(attempt)}: opencode processes during the run: ${String(during.length)}, still running when Stop is pressed: ${String(aliveAtStop.length)} (${String(Math.round((Date.now() - waitedFrom) / 1000))} s after the turn started)`)
    if (during.length === 0) console.log(`  every process now: ${execFileSync('ps', PS_EVERY, { encoding: 'utf8' }).split('\n').filter((line) => /opencode|Locust/i.test(line)).join(' | ').slice(0, 1200)}`)
    if (aliveAtStop.length > 0 || attempt === 3) break
    // Over on its own: wait for the window to say so, then send the next.
    await evaluate(`(async () => {
      for (let i = 0; i < 120 && document.querySelector('button[aria-label="Stop the running reply"]'); i += 1) await new Promise((r) => setTimeout(r, 500))
    })()`)
    await sleep(1_000)
  }
  check('a turn on the free model starts running', String(sent).startsWith('running'), String(sent))

  const stopped = await evaluate(`(async () => {
    document.querySelector('button[aria-label="Stop the running reply"]')?.click()
    for (let i = 0; i < 60; i += 1) {
      await new Promise((r) => setTimeout(r, 500))
      if (!document.querySelector('button[aria-label="Stop the running reply"]')) return 'stopped'
    }
    return 'still running'
  })()`)
  check('Stop ends the run in the window', stopped === 'stopped', String(stopped))
  await sleep(5_000)
  const left = opencodes().filter((pid) => !before.has(pid) && during.includes(pid))
  // With nothing running there was nothing to stop, and that proves nothing.
  check(
    'Stop ends the whole process group: nothing the run started is left',
    aliveAtStop.length > 0 && left.length === 0,
    aliveAtStop.length === 0 ? `the run had ended on its own before Stop -- nothing to prove: ${JSON.stringify({ during })}` : JSON.stringify({ during, aliveAtStop, left })
  )
} catch (error) {
  if (!(error instanceof LaunchOnly)) {
    failures += 1
    console.log(`smoke failed: ${error instanceof Error ? error.message : String(error)}`)
  }
} finally {
  try { socket?.close() } catch { /* closed */ }
  child.kill()
  // Gone before this exits, so the next check starts on a quiet machine: asked to close, then made to.
  for (let i = 0; i < 30 && child.exitCode === null && child.signalCode === null; i += 1) await sleep(500)
  if (child.exitCode === null && child.signalCode === null) child.kill('SIGKILL')
  await sleep(1_000)
}
const passed = `${SYSTEM === 'macOS' ? 'MAC' : 'LINUX'} ${launchOnly ? 'LAUNCH' : 'STOP'} SMOKE PASSED`
console.log(failures === 0 ? passed : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
