// How long one discovery sweep takes on THIS machine, per runtime.
//
//   node _tools/discovery-timing-drive.mjs [label]
//
// Launches the dev build on a fresh profile with the real PATH, waits for
// every probe the boot log started to finish, and prints each runtime's
// probe duration and the sweep's total. Nothing is asserted; this is the
// measurement behind Fable's probing review #3 (one runtime's probes at
// once), run against the OLD build and the NEW one on the same box --
// because a before/after is not a control unless the old thing is re-run.

import { spawn } from 'node:child_process'
import { existsSync, readdirSync } from 'node:fs'
import { mkdir, mkdtemp, rm } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { join } from 'node:path'
import { localPath } from './local-path.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const PORT = 9279
// Codex's bin folder is named by a hash that changes with each Codex update:
// find the one that holds codex.exe instead of naming it.
const CODEX_BIN_ROOT = join(homedir(), 'AppData', 'Local', 'OpenAI', 'Codex', 'bin')
const CODEX_BIN_DIR = existsSync(CODEX_BIN_ROOT)
  ? (readdirSync(CODEX_BIN_ROOT).map((name) => join(CODEX_BIN_ROOT, name)).find((dir) => existsSync(join(dir, 'codex.exe'))) ?? '')
  : ''
const NPM_DIR = join(homedir(), 'AppData', 'Roaming', 'npm')
const CURSOR_DIR = join(homedir(), 'AppData', 'Local', 'cursor-agent')
const label = process.argv[2] ?? ''
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))

/*
 * `--profile <dir>` REUSES a profile, and that is the only way to see the
 * binary-facts cache at all: it is read from the profile at launch, so a
 * fresh profile is always a cold sweep. Run the same directory twice and
 * the second launch is the warm one.
 */
const asked = process.argv.indexOf('--profile')
const reused = asked === -1 ? undefined : process.argv[asked + 1]
const profile = reused ?? (await mkdtemp(join(tmpdir(), 'locust-timing-')))
if (reused !== undefined) await mkdir(reused, { recursive: true })
const child = spawn(ELECTRON, [APP_DIR, `--remote-debugging-port=${String(PORT)}`, `--user-data-dir=${profile}`], {
  cwd: APP_DIR,
  env: { ...process.env, PATH: `${CODEX_BIN_DIR};${NPM_DIR};${CURSOR_DIR};${process.env.PATH ?? ''}` },
  stdio: ['ignore', 'ignore', 'ignore']
})
try {
  let page
  for (let attempt = 0; attempt < 80 && page === undefined; attempt += 1) {
    await sleep(500)
    if (child.exitCode !== null) throw new Error(`app exited ${String(child.exitCode)}`)
    try {
      const list = await (await fetch(`http://127.0.0.1:${String(PORT)}/json/list`)).json()
      page = list.find((t) => t.type === 'page' && t.webSocketDebuggerUrl && !t.url.includes('#splash'))
    } catch { /* not up */ }
  }
  if (page === undefined) throw new Error('renderer never came up')
  const socket = new WebSocket(page.webSocketDebuggerUrl)
  await new Promise((res, rej) => { socket.addEventListener('open', res, { once: true }); socket.addEventListener('error', rej, { once: true }) })
  let id = 0
  const pending = new Map()
  socket.addEventListener('message', (event) => {
    const message = JSON.parse(event.data)
    const waiter = pending.get(message.id)
    if (waiter) { pending.delete(message.id); waiter(message) }
  })
  const evaluate = (expression) => new Promise((resolve_) => {
    const next = ++id
    pending.set(next, (message) => resolve_(message?.result?.result?.value))
    socket.send(JSON.stringify({ id: next, method: 'Runtime.evaluate', params: { expression, awaitPromise: true, returnByValue: true } }))
  })
  let log = []
  for (let i = 0; i < 240; i += 1) {
    await sleep(500)
    log = JSON.parse(await evaluate(`window.desktop.discoveryLog().then(l => JSON.stringify(l))`))
    const started = log.filter((e) => e.kind === 'probe.started')
    const finished = log.filter((e) => e.kind === 'probe.finished')
    if (started.length > 0 && finished.length >= started.length && i > 4) break
  }
  const t0 = log.find((e) => e.kind === 'started')?.at ?? log[0]?.at ?? 0
  const rows = []
  for (const s of log.filter((e) => e.kind === 'probe.started')) {
    const f = log.find((e) => e.kind === 'probe.finished' && e.id === s.id)
    rows.push({ id: s.id, ms: f === undefined ? null : f.at - s.at, endAt: f === undefined ? null : f.at - t0, outcome: f?.outcome?.status ?? f?.outcome ?? '?' })
  }
  const total = Math.max(...rows.map((r) => r.endAt ?? 0))
  // The moment the sweep said it was done: what the first screen waits for, and what Antigravity must never extend.
  const released = log.find((e) => e.kind === 'finished')
  console.log(`[${label}] sweep total ${String(total)} ms; sweep finished event at ${released === undefined ? '?' : String(released.at - t0)} ms`)
  for (const r of rows.sort((a, b) => (b.ms ?? 0) - (a.ms ?? 0))) {
    console.log(`  ${r.id.padEnd(12)} ${String(r.ms ?? '?').padStart(6)} ms  ${typeof r.outcome === 'string' ? r.outcome : JSON.stringify(r.outcome)}`)
  }
} finally {
  child.kill()
  await sleep(500)
  // A reused profile is the point: leave it for the next run.
  if (reused === undefined) await rm(profile, { recursive: true, force: true }).catch(() => undefined)
}
