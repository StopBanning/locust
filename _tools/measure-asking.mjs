// How often does a runtime ask when it should NOT?
//
//   node _tools/measure-asking.mjs [--runs 2]
//
// Shipping the ask block created a risk nothing measures: an agent that asks
// about everything is worse than one that asks about nothing, because then the
// person makes every decision AND reads the questions. The briefing tells the
// runtime when not to ask. Nothing enforces it, and "seems fine" is not a
// number.
//
// So this runs a fixed set of prompts of three kinds and counts cards:
//
//   fork      a genuine two-way decision the prompt does not settle.
//             SHOULD ask. A miss here means the feature does not work.
//   settled   the prompt already says which way to go.
//             MUST NOT ask -- asking is ignoring an explicit instruction.
//   plain     ordinary read-only work with no decision in it.
//             MUST NOT ask -- asking here is the over-asking failure.
//
// Every run is read-only, so nothing it decides can touch the workspace.
// Model output varies, so this is a measurement and not a pass/fail gate: it
// prints a table and a rate. Run it again after changing the briefing to see
// whether the wording moved the number.

import { spawn } from 'node:child_process'
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { join } from 'node:path'
import { localPath } from './local-path.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const CODEX_BIN_DIR = join(homedir(), 'AppData', 'Local', 'OpenAI', 'Codex', 'bin', 'b99306303521e97e')
const NPM_DIR = join(homedir(), 'AppData', 'Roaming', 'npm')

const runsPerPrompt = Number(process.argv[process.argv.indexOf('--runs') + 1]) || 1

/**
 * Deliberately about THIS repository, so the runtime can actually read enough
 * to have an opinion. The settled ones name the choice explicitly, because the
 * rule being measured is "do not ask about what the prompt already decided".
 */
const PROMPTS = [
  // Phrased as work, not as a quiz. The first version said "do not write
  // code", which turns the task into a question the model simply ANSWERS --
  // and then a missing card measures the prompt, not the feature. Read-only
  // mode is what keeps these safe, not the wording.
  {
    kind: 'fork',
    text: 'Add a retry to the update check in apps/desktop/src/main/updates.ts. Tell me exactly what you would change.'
  },
  {
    kind: 'fork',
    text: 'The history screen loads every mission at once. Put a cap on it and tell me exactly what you would change.'
  },
  // The control that separates "the feature is broken" from "the model was
  // never going to ask here": same fork, but told to ask.
  {
    kind: 'fork-told',
    text: 'Add a retry to the update check in apps/desktop/src/main/updates.ts. There is more than one reasonable design and I have not decided; ask me which one you should use rather than picking.'
  },
  {
    kind: 'settled',
    text: 'Add a retry to the update check. Use a fixed three attempts, not a time-based backoff. Do not write code, just tell me which file it goes in.'
  },
  {
    kind: 'settled',
    text: 'Cap the history screen at 50 missions. Do not write code; tell me where that number would live.'
  },
  {
    kind: 'plain',
    text: 'Read the README at the top of this workspace and tell me in three sentences what this project is.'
  },
  {
    kind: 'plain',
    text: 'How many test files are in apps/desktop? Just count them and say the number.'
  }
]

const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const say = (line) => console.error(line)

class Cdp {
  constructor(ws, child) {
    this.ws = ws
    this.child = child
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
    return Promise.race([
      new Promise((resolve) => this.pending.set(id, { resolve })),
      (async () => {
        for (let i = 0; i < 400; i += 1) {
          await sleep(1000)
          if (this.child.exitCode !== null) return { error: { message: `app exited ${this.child.exitCode}` } }
        }
        return { error: { message: 'cdp timeout' } }
      })()
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

async function runOnce(prompt, port) {
  const profile = await mkdtemp(join(tmpdir(), 'locust-ask-'))
  await mkdir(profile, { recursive: true })
  const child = spawn(ELECTRON, ['.', `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`], {
    cwd: APP_DIR,
    env: { ...process.env, PATH: `${CODEX_BIN_DIR};${NPM_DIR};${process.env.PATH ?? ''}` },
    stdio: ['ignore', 'pipe', 'pipe']
  })
  try {
    let page
    for (let i = 0; i < 80 && page === undefined; i += 1) {
      await sleep(500)
      if (child.exitCode !== null) throw new Error(`app exited ${child.exitCode}`)
      try {
        const list = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()
        page = list.find((t) => t.type === 'page' && t.webSocketDebuggerUrl && !t.url.includes('#splash'))
      } catch { /* not listening yet */ }
    }
    if (page === undefined) throw new Error('renderer never came up')
    const socket = new WebSocket(page.webSocketDebuggerUrl)
    await new Promise((resolve, reject) => {
      socket.addEventListener('open', resolve, { once: true })
      socket.addEventListener('error', reject, { once: true })
    })
    const cdp = new Cdp(socket, child)
    await cdp.send('Runtime.enable')

    await cdp.eval(`(async () => {
      for (let i = 0; i < 240; i += 1) {
        const field = document.querySelector('form.command-dock textarea')
        if (field && !/Checking local runtimes/.test(field.placeholder)) return true
        await new Promise(r => setTimeout(r, 250))
      }
      return false
    })()`)

    // Read-only, always: a measurement must not be able to change the tree.
    await cdp.eval(`(async () => {
      const control = [...document.querySelectorAll('.lc-control')].find(c => /Accept edits|Ask|Approve/i.test(c.innerText))
      if (!control) return 'no mode control'
      control.click()
      await new Promise(r => setTimeout(r, 400))
      const ask = [...document.querySelectorAll('.lc-menu[role="menu"] .lc-menu__item')]
        .find(b => ((b.querySelector('.lc-menu__name') || {}).innerText || '').trim() === 'Ask')
      if (!ask) return 'no Ask item'
      ask.click()
      await new Promise(r => setTimeout(r, 300))
      return 'ask'
    })()`)

    const submitted = await cdp.eval(`(async () => {
      const field = document.querySelector('form.command-dock textarea')
      const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
      setter.call(field, ${JSON.stringify(prompt)})
      field.dispatchEvent(new Event('input', { bubbles: true }))
      for (let i = 0; i < 120; i += 1) {
        await new Promise(r => setTimeout(r, 250))
        const send = document.querySelector('form.command-dock .send-button')
        if (send && !send.disabled && send.getAttribute('aria-label') === 'Send') { send.click(); return 'clicked' }
      }
      return 'send stayed disabled'
    })()`)
    if (submitted !== 'clicked') throw new Error(`could not start: ${submitted}`)

    return JSON.parse(await cdp.eval(`(async () => {
      for (let i = 0; i < 300; i += 1) {
        await new Promise(r => setTimeout(r, 1000))
        const stop = document.querySelector('button[aria-label^="Stop the running"]')
        const done = document.querySelector('.lc-agentline') || document.querySelector('.lc-decision')
        if (!stop && done) {
          const card = document.querySelector('.lc-decision')
          return JSON.stringify({
            asked: card !== null,
            question: card ? (card.querySelector('.lc-decision__question') || {}).innerText : null,
            leakedTag: document.body.innerText.includes('locust-ask')
          })
        }
      }
      return JSON.stringify({ asked: null, question: null, leakedTag: false })
    })()`))
  } finally {
    child.kill()
    await sleep(800)
    await rm(profile, { recursive: true, force: true }).catch(() => undefined)
  }
}

const results = []
let port = 9260
for (const prompt of PROMPTS) {
  for (let run = 0; run < runsPerPrompt; run += 1) {
    port += 1
    let outcome
    try {
      outcome = await runOnce(prompt.text, port)
    } catch (error) {
      outcome = { asked: null, question: null, leakedTag: false, error: String(error) }
    }
    results.push({ kind: prompt.kind, ...outcome })
    const mark = outcome.asked === null ? '?' : outcome.asked ? 'ASKED' : 'no'
    say(`  ${prompt.kind.padEnd(8)} ${mark}${outcome.error ? ` (${outcome.error})` : ''}`)
    if (outcome.question) say(`           "${String(outcome.question).slice(0, 90)}"`)
  }
}

const count = (kind, asked) => results.filter((r) => r.kind === kind && r.asked === asked).length
const total = (kind) => results.filter((r) => r.kind === kind && r.asked !== null).length

say('')
say('kind      asked  did not  unusable')
for (const kind of ['fork', 'fork-told', 'settled', 'plain']) {
  const unusable = results.filter((r) => r.kind === kind && r.asked === null).length
  say(`${kind.padEnd(9)} ${String(count(kind, true)).padStart(5)}  ${String(count(kind, false)).padStart(7)}  ${String(unusable).padStart(8)}`)
}
const overAsked = count('settled', true) + count('plain', true)
const shouldNot = total('settled') + total('plain')
const missed = count('fork', false)
const missedTold = count('fork-told', false)
say('')
say(`over-asking: ${overAsked} of ${shouldNot} runs that should NOT have asked, did`)
say(`missed forks: ${missed} of ${total('fork')} runs that SHOULD have asked, did not`)
say(`missed when TOLD to ask: ${missedTold} of ${total('fork-told')} -- if this is 0 while the line above is not, the block works and the runtime just does not reach for it on its own`)
if (results.some((r) => r.leakedTag)) say('WARNING: a raw ask tag was visible on the page')
