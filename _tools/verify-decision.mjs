// Does a real model actually produce the ask block, and does the card appear?
//
//   node _tools/verify-decision.mjs
//
// The parser has unit tests; what they cannot tell you is whether a runtime,
// given the one paragraph the briefing spends on this, emits the form at all.
// That is the whole risk of the feature. So this asks a question with a
// genuine fork in it and reads the DOM for the card.
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
  'I want to add a retry to the update check. There are two defensible ways to do it and I have not decided: retry a fixed three times, or back off and give up after a minute. Do not write any code and do not pick for me. Ask me which one you should do.'

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

  // Put the run in a read-only mode FIRST. The card's "nothing was changed" is
  // a guarantee that comes from the sandbox, and it is the one claim on it
  // that must not be loose -- so it has to be exercised, not assumed. The
  // mode control is the lc-control that is not the route picker.
  const mode = await cdp.eval(`(async () => {
    const controls = [...document.querySelectorAll('.lc-control')]
    const modeButton = controls.find(c => /Accept edits|Ask|Approve/i.test(c.innerText))
    if (!modeButton) return 'no mode control'
    modeButton.click()
    await new Promise(r => setTimeout(r, 400))
    const ask = [...document.querySelectorAll('.lc-menu[role="menu"] .lc-menu__item')]
      .find(b => ((b.querySelector('.lc-menu__name') || {}).innerText || '').trim() === 'Ask')
    if (!ask) return 'no Ask item; menus: ' + document.querySelectorAll('.lc-menu').length
    ask.click()
    await new Promise(r => setTimeout(r, 400))
    return [...document.querySelectorAll('.lc-control')].map(c => c.innerText).join(' | ')
  })()`)
  say(`mode control: ${JSON.stringify(mode)}`)

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

  const timingUnused = await cdp.eval(`(async () => {
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
  say(JSON.stringify({ liveLine: timingUnused?.firstLine ?? null }))

  const finished = await cdp.eval(`(async () => {
    let sawRunning = false
    for (let i = 0; i < 300; i += 1) {
      await new Promise(r => setTimeout(r, 1000))
      const stop = document.querySelector('button[aria-label^="Stop the running"]')
      if (stop) sawRunning = true
      // A run whose entire reply IS the question leaves no agent line -- the
      // block is stripped from the bubble and the card carries it. So the
      // finish signal is either one.
      // Not gated on having SEEN it running: a fast run can finish between
      // polls, and then this waits forever for a state that already passed.
      // What matters is that nothing is running NOW and there is output.
      if (!stop && (document.querySelector('.lc-agentline') || document.querySelector('.lc-decision'))) return true
    }
    // Say WHY rather than just false: "never finished" could be a slow run, a
    // run that failed, or a wait condition that can never be met.
    return JSON.stringify({
      done: false,
      sawRunning,
      stopStillThere: document.querySelector('button[aria-label^="Stop the running"]') !== null,
      agentLines: document.querySelectorAll('.lc-agentline').length,
      decisionCards: document.querySelectorAll('.lc-decision').length,
      thread: (document.querySelector('.lc-thread') || { innerText: '' }).innerText.slice(-900)
    })
  })()`)
  if (finished !== true) throw new Error(`the mission never finished: ${finished}`)

  // The composer must be empty after a start that went through.
  // The card, read off the page rather than off the transcript: the point is
  // that a person can SEE and answer it, not merely that a tag was emitted.
  const card = await cdp.eval(`(async () => {
    for (let i = 0; i < 40; i += 1) {
      const node = document.querySelector('.lc-decision')
      if (node) {
        return JSON.stringify({
          found: true,
          title: (node.querySelector('.lc-decision__title') || {}).innerText || '',
          standing: (node.querySelector('.lc-rail__meta') || {}).innerText || '',
          question: (node.querySelector('.lc-decision__question') || {}).innerText || '',
          options: [...node.querySelectorAll('.lc-decision__option')].map((b) => ({
            label: (b.querySelector('.lc-decision__label') || {}).innerText || '',
            note: (b.querySelector('.lc-decision__note') || {}).innerText || null
          }))
        })
      }
      await new Promise(r => setTimeout(r, 500))
    }
    const reply = document.querySelector('.lc-agentline')
    return JSON.stringify({ found: false, reply: reply ? reply.innerText.slice(0, 500) : '(no reply on screen)' })
  })()`)
  const seen = JSON.parse(card)
  say(JSON.stringify(seen, null, 2))
  if (!seen.found) throw new Error('no decision card appeared')
  if (seen.options.length < 2) throw new Error('the card offered fewer than two options')
  // A raw tag on screen would mean the strip and the parse disagree.
  const leaked = await cdp.eval(`document.body.innerText.includes('locust-ask')`)
  if (leaked) throw new Error('the raw ask tag is visible on the page')
  say('DECISION CARD VERIFIED')

  // Scroll the thread to the top of the answer so the picture shows the work
  // rather than whitespace, and let the last paint settle.
  await cdp.eval(`(() => {
    const thread = document.querySelector('.lc-thread')
    if (thread) thread.scrollTop = thread.scrollHeight
    return true
  })()`)
  await sleep(1200)

  // Deliberately no screenshot: this tool is inherited from the README capture
  // and writing there would overwrite the project's shipped picture with a
  // shot of a test question.
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
