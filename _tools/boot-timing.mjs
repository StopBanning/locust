// Where a launch spends its time, from the process starting to the app on screen.
//
//   node _tools/boot-timing.mjs [label] [--profile <dir>] [--runs N] [--packaged | --exe <Locust.exe>]
//
// Colin, 2026-09-22: "look into the probing process since its part of the
// loading, i want the users to have a seamless, fast experience". This is the
// measurement under that: every stage on ONE clock (the drive's Date.now,
// which the app's discovery log also uses), so the stages add up.
//
//   splash page up      -- the loading window has a renderer
//   app page up         -- the (hidden) app window has a renderer
//   app DOM ready       -- its bundle parsed and ran
//   sweep started       -- discovery began asking the runtimes
//   each probe          -- per runtime, how long its answer took
//   sweep finished      -- the last runtime answered
//   app visible         -- the loading window let go and the app is on screen
//
// `--profile` reuses a profile so the runtime facts cache is warm, which is
// every launch after a person's first. Spends nothing: no mission is sent.

import { execFile, spawn } from 'node:child_process'
import { createServer } from 'node:net'
import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { localPath } from './local-path.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const PORT = 9281
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const arg = (name) => {
  const at = process.argv.indexOf(name)
  return at === -1 ? undefined : process.argv[at + 1]
}
const label = process.argv[2] !== undefined && !process.argv[2].startsWith('--') ? process.argv[2] : ''
const runs = Number(arg('--runs') ?? '1')
const reused = arg('--profile')
const packaged = process.argv.includes('--packaged') || process.argv.includes('--exe')
// `--exe <Locust.exe>` measures another packaged copy (implies --packaged): a
// before/after on one machine re-runs the old build after the new one.
const PACKAGED = arg('--exe') ?? join(APP_DIR, 'release', 'win-unpacked', 'Locust.exe')

async function page(filter) {
  const list = await (await fetch(`http://127.0.0.1:${String(PORT)}/json/list`)).json()
  return list.find((t) => t.type === 'page' && t.webSocketDebuggerUrl && filter(t.url))
}

function connect(url) {
  return new Promise((resolve, reject) => {
    const socket = new WebSocket(url)
    let id = 0
    const pending = new Map()
    socket.addEventListener('message', (event) => {
      const message = JSON.parse(event.data)
      const waiter = pending.get(message.id)
      if (waiter) { pending.delete(message.id); waiter(message) }
    })
    socket.addEventListener('open', () => resolve({
      evaluate: (expression) => new Promise((done) => {
        const next = ++id
        pending.set(next, (message) => done(message?.result?.result?.value))
        socket.send(JSON.stringify({ id: next, method: 'Runtime.evaluate', params: { expression, awaitPromise: true, returnByValue: true } }))
      }),
      close: () => socket.close()
    }), { once: true })
    socket.addEventListener('error', reject, { once: true })
  })
}

const profileOf = (child) => child.spawnargs.find((a) => a.startsWith('--user-data-dir='))?.slice('--user-data-dir='.length) ?? 'no-such-profile'

async function once(profile) {
  const t0 = Date.now()
  const at = {}
  const child = spawn(packaged ? PACKAGED : ELECTRON, [...(packaged ? [] : [APP_DIR]), `--remote-debugging-port=${String(PORT)}`, `--user-data-dir=${profile}`], {
    cwd: APP_DIR,
    env: process.env,
    stdio: ['ignore', 'ignore', 'ignore']
  })
  try {
    let app
    for (let i = 0; i < 600 && app === undefined; i += 1) {
      await sleep(25)
      try {
        if (at.splash === undefined && (await page((url) => url.includes('#splash'))) !== undefined) at.splash = Date.now() - t0
        const found = await page((url) => !url.includes('#splash'))
        if (found !== undefined) { at.appPage = Date.now() - t0; app = found }
      } catch { /* not listening yet */ }
    }
    if (app === undefined) throw new Error('the app window never came up')
    const cdp = await connect(app.webSocketDebuggerUrl)
    // DOM ready and first paint, from the page's own clock, moved onto ours.
    const nav = await cdp.evaluate(`(() => { const n = performance.getEntriesByType('navigation')[0]; return JSON.stringify({ origin: performance.timeOrigin, dcl: n ? n.domContentLoadedEventEnd : null }) })()`)
    const { origin, dcl } = JSON.parse(nav)
    if (dcl !== null) at.appDomReady = Math.round(origin + dcl - t0)
    // The app window reports itself "visible" while it is still held hidden
    // behind the splash (first run of this script), so the moment the app
    // is on screen is read from the other side: the host shows the app and
    // THEN closes the splash, so the splash page leaving is the moment.
    for (let i = 0; i < 1200; i += 1) {
      if ((await page((url) => url.includes('#splash'))) === undefined) { at.appVisible = Date.now() - t0; break }
      await sleep(25)
    }
    // And the sweep is read once it has FINISHED: the log is empty until the
    // app window is up, which is what the first run of this script read.
    // Past the sweep's release too: an agent the sweep did not wait for
    // (0.629) answers after it, and its check time is still a number worth
    // having -- a probe with no answer after 30 s prints "?".
    let log = []
    for (let i = 0; i < 600; i += 1) {
      log = JSON.parse(await cdp.evaluate(`window.desktop.discoveryLog().then(l => JSON.stringify(l))`))
      const begun = log.filter((e) => e.kind === 'probe.started')
      if (log.some((e) => e.kind === 'finished') && begun.every((b) => log.some((e) => e.kind === 'probe.finished' && e.id === b.id))) break
      await sleep(50)
    }
    const started = log.find((e) => e.kind === 'started')
    const finished = log.find((e) => e.kind === 'finished')
    if (started) at.sweepStarted = started.at - t0
    if (finished) at.sweepFinished = finished.at - t0
    const probes = log
      .filter((e) => e.kind === 'probe.started')
      .map((s) => {
        const f = log.find((e) => e.kind === 'probe.finished' && e.id === s.id)
        return { id: s.id, from: s.at - t0, ms: f === undefined ? null : f.at - s.at }
      })
      .sort((a, b) => (b.ms ?? 0) - (a.ms ?? 0))
    cdp.close()
    return { at, probes }
  } finally {
    child.kill()
    await sleep(800)
    // Killing the main process leaves its helpers holding the debugging port
    // for a few seconds, and the next launch then cannot open it ("the app
    // window never came up", seen from the third launch of a run of three).
    // Wait until it can be bound again, and until none of this profile's
    // helper processes is left (a launch that finds them still holding the
    // profile's lock quits at once).
    for (let i = 0; i < 300; i += 1) {
      const free = await new Promise((resolve) => {
        const probe = createServer()
        probe.once('error', () => resolve(false))
        probe.listen(PORT, '127.0.0.1', () => probe.close(() => resolve(true)))
      })
      const lingering = await new Promise((resolve) => {
        execFile('wmic', ['process', 'where', "name='electron.exe' or name='Locust.exe'", 'get', 'CommandLine'], { windowsHide: true }, (_error, stdout) => resolve(String(stdout).includes(profileOf(child))))
      })
      if (free && !lingering) break
      await sleep(100)
    }
  }
}

const profile = reused ?? (await mkdtemp(join(tmpdir(), 'locust-boot-')))
try {
  for (let run = 1; run <= runs; run += 1) {
    const { at, probes } = await once(profile)
    console.log(`[${label}${runs > 1 ? ` run ${String(run)}` : ''}] ${packaged ? 'packaged' : 'out/'} · ${reused === undefined && run === 1 ? 'cold profile' : 'warm profile'}`)
    for (const [stage, ms] of Object.entries(at).sort((a, b) => a[1] - b[1])) console.log(`  ${stage.padEnd(14)} ${String(ms).padStart(6)} ms`)
    for (const p of probes) console.log(`    probe ${p.id.padEnd(12)} ${String(p.ms ?? '?').padStart(6)} ms  (from ${String(p.from)} ms)`)
  }
} finally {
  if (reused === undefined) await rm(profile, { recursive: true, force: true }).catch(() => undefined)
}
