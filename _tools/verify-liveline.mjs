// Measure the gap between pressing send and the thread showing anything.
//
//   node _tools/verify-liveline.mjs
//
// Colin reported twice that the working bounce lags the send. The claim this
// checks is the fix for it: a live run draws a line IMMEDIATELY, before the
// runtime has reported a single event. It samples the DOM every 50ms from the
// moment the send button is clicked and reports when a live line first
// appeared, what it said, and whether the dots were on it.
//
// It seeds a roster, runs ONE real read-only Codex mission through the UI, and
// captures the window once the mission has finished and the thread has
// something in it. The picture is therefore a real run against the real CLI,
// which is what the README's caption says it is -- a mocked screenshot would
// make that caption a lie.

import { spawn } from 'node:child_process'
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { localPath } from './local-path.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const PORT = 9229
const CODEX_BIN_DIR = join(homedir(), 'AppData', 'Local', 'OpenAI', 'Codex', 'bin', 'b99306303521e97e')
const NPM_DIR = join(homedir(), 'AppData', 'Roaming', 'npm')
// Flags and the output path may come in any order; `--idle` is not a path.
const positional = process.argv.slice(2).filter((argument) => !argument.startsWith('--'))
const OUT = resolve(positional[0] ?? join(APP_DIR, '..', '..', 'docs', 'assets', 'shell.png'))

const PROMPT =
  'Read the README at the top of this workspace and tell me, in three short sentences, what this project is and what it can do today.'

const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const say = (line) => console.error(line)

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
      waiter.resolve(message)
    })
  }
  send(method, params = {}) {
    const id = ++this.id
    this.ws.send(JSON.stringify({ id, method, params }))
    const settleTimers = new Set()
    const clearTimers = () => { for (const t of settleTimers) clearInterval(t) }
    return Promise.race([
      new Promise((resolve) => this.pending.set(id, { resolve: (m) => { clearTimers(); resolve(m) } })),
      new Promise((resolve) => {
        // One timer, cleared when the answer wins and unref()'d: the sleep loop
        // this replaced kept the process alive for up to 400s after the
        // last line (six smokes 'not exiting cleanly', QA on 0.21.2).
        let ticks = 0
        const tick = setInterval(() => {
          ticks += 1
          if (child.exitCode !== null) { clearInterval(tick); resolve({ error: { message: `app exited ${child.exitCode}` } }) }
          else if (ticks >= 400) { clearInterval(tick); resolve({ error: { message: 'cdp timeout' } }) }
        }, 1000)
        tick.unref()
        settleTimers.add(tick)
        })
    ])
  }
  async eval(expression) {
    const message = await this.send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })
    if (message.error) throw new Error(JSON.stringify(message.error))
    if (message.result?.exceptionDetails) {
      throw new Error(message.result.exceptionDetails.exception?.description ?? 'evaluate threw')
    }
    return message.result?.result?.value
  }
}

const profile = await mkdtemp(join(tmpdir(), 'locust-shot-'))
await mkdir(profile, { recursive: true })
const createdAt = new Date().toISOString()
await writeFile(
  join(profile, 'teammates.json'),
  JSON.stringify({
    schemaVersion: 1,
    teammates: [
      { teammateId: 'tm_wren_0001', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt },
      { teammateId: 'tm_atlas_002', name: 'Atlas', hue: 'blue', role: 'Research & Briefs', createdAt },
      { teammateId: 'tm_juno_0003', name: 'Juno', hue: 'violet', role: 'Docs & QA', createdAt },
      { teammateId: 'tm_sable_004', name: 'Sable', hue: 'clay', role: 'Data & Reporting', createdAt }
    ],
    missionOwners: {},
    settings: { swarm: false }
  }, null, 2)
)

const child = spawn(ELECTRON, ['.', `--remote-debugging-port=${PORT}`, `--user-data-dir=${profile}`], {
  cwd: APP_DIR,
  env: { ...process.env, PATH: `${CODEX_BIN_DIR};${NPM_DIR};${process.env.PATH ?? ''}` },
  stdio: ['ignore', 'pipe', 'pipe']
})
const appOutput = []
child.stdout.on('data', (d) => appOutput.push(String(d)))
child.stderr.on('data', (d) => appOutput.push(String(d)))

let failed = false
try {
  let page
  for (let attempt = 0; attempt < 60 && page === undefined; attempt += 1) {
    await sleep(500)
    if (child.exitCode !== null) break
    try {
      const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json()
      page = list.find((t) => t.type === 'page' && t.webSocketDebuggerUrl && !t.url.includes('#splash'))
    } catch {
      // not listening yet
    }
  }
  if (page === undefined) throw new Error('the renderer never came up')

  const socket = new WebSocket(page.webSocketDebuggerUrl)
  await new Promise((resolve, reject) => {
    socket.addEventListener('open', resolve, { once: true })
    socket.addEventListener('error', reject, { once: true })
  })
  const cdp = new Cdp(socket)
  await cdp.send('Runtime.enable')
  await cdp.send('Page.enable')

  say('waiting for discovery')
  await cdp.eval(`(async () => {
    for (let i = 0; i < 240; i += 1) {
      const field = document.querySelector('form.command-dock textarea')
      if (field && document.querySelectorAll('.lc-row--button').length >= 4 && !/Checking local runtimes/.test(field.placeholder)) return true
      await new Promise(r => setTimeout(r, 250))
    }
    return false
  })()`)

  say('running one real read-only mission')
  const submitted = await cdp.eval(`(async () => {
    const field = document.querySelector('form.command-dock textarea')
    const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
    setter.call(field, ${JSON.stringify(PROMPT)})
    field.dispatchEvent(new Event('input', { bubbles: true }))
    for (let i = 0; i < 120; i += 1) {
      await new Promise(r => setTimeout(r, 250))
      const send = document.querySelector('form.command-dock .send-button')
      if (send && !send.disabled && send.getAttribute('aria-label') === 'Send') { send.click(); return 'clicked' }
    }
    return 'send stayed disabled'
  })()`)
  if (submitted !== 'clicked') throw new Error(`could not start a mission: ${submitted}`)

  const timing = await cdp.eval(`(async () => {
    const started = performance.now()
    let firstLine = null
    for (let i = 0; i < 400; i += 1) {
      const line = document.querySelector('.lc-livestep')
      const label = line ? (line.querySelector('.lc-livestep__label')?.textContent ?? '').trim() : ''
      if (line && firstLine === null) {
        firstLine = {
          ms: Math.round(performance.now() - started),
          label,
          dots: line.querySelector('.lc-dots') !== null,
          stepKind: line.getAttribute('data-step-kind')
        }
      }
      // What the OLD build would have waited for: a line the runtime named.
      // Until a real step arrived there was no live line at all, so this is
      // the blank-page gap the fix removes, measured on the same run.
      if (line && !/^(Starting|Working)/.test(label)) {
        return { firstLine, firstNamedStep: { ms: Math.round(performance.now() - started), label } }
      }
      await new Promise(r => setTimeout(r, 50))
    }
    return { firstLine, firstNamedStep: null }
  })()`)
  if (timing.firstLine === null) throw new Error('no live line ever appeared')
  say(JSON.stringify(timing, null, 2))

  const finished = await cdp.eval(`(async () => {
    let sawRunning = false
    for (let i = 0; i < 300; i += 1) {
      await new Promise(r => setTimeout(r, 1000))
      const stop = document.querySelector('button[aria-label^="Stop the running"]')
      if (stop) sawRunning = true
      if (sawRunning && !stop && document.querySelector('.lc-agentline')) return true
    }
    return false
  })()`)
  if (!finished) throw new Error('the mission never finished')

  // The composer must be empty after a start that went through.
  const composer = await cdp.eval(`document.querySelector('form.command-dock textarea').value`)
  say(JSON.stringify({ composerAfterSend: composer }))

  // Scroll the thread to the top of the answer so the picture shows the work
  // rather than whitespace, and let the last paint settle.
  await cdp.eval(`(() => {
    const thread = document.querySelector('.lc-thread')
    if (thread) thread.scrollTop = thread.scrollHeight
    return true
  })()`)
  await sleep(1200)

  const shot = await cdp.send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false })
  const data = shot.result?.data
  if (typeof data !== 'string') throw new Error('no screenshot data came back')
  await writeFile(OUT, Buffer.from(data, 'base64'))
  say(`wrote ${OUT}`)
} catch (error) {
  failed = true
  say(String(error))
  say(appOutput.join('').slice(-2000))
} finally {
  child.kill()
  await sleep(500)
  await rm(profile, { recursive: true, force: true }).catch(() => undefined)
}

process.exit(failed ? 1 : 0)
