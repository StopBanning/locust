// Does an interrupted mission actually come back?
//
//   node _tools/verify-resume.mjs
//
// The offer logic has unit tests and the host has typechecks; what neither
// proves is that the two agree about a real ledger written by a real run. So
// this makes a genuine interruption -- start a mission, kill the app while it
// is working, which is exactly how a person produces one -- then relaunches
// into the same profile and presses the button.

import { spawn } from 'node:child_process'
import { mkdtemp, mkdir, rm, readdir, readFile } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { join } from 'node:path'
import { localPath } from './local-path.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const CODEX_BIN_DIR = join(homedir(), 'AppData', 'Local', 'OpenAI', 'Codex', 'bin', 'b99306303521e97e')
const NPM_DIR = join(homedir(), 'AppData', 'Roaming', 'npm')
const PROMPT =
  'List every TypeScript file under apps/desktop/src/main and describe what each one does, one sentence each.'

const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const say = (line) => console.error(line)
let failures = 0
const check = (label, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${label}${detail === undefined ? '' : ` -- ${detail}`}`)
}

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
        for (let i = 0; i < 300; i += 1) {
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
      throw new Error(message.result.exceptionDetails.exception?.description ?? 'threw')
    }
    return message.result?.result?.value
  }
}

async function launch(profile, port) {
  const child = spawn(ELECTRON, ['.', `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`], {
    cwd: APP_DIR,
    env: { ...process.env, PATH: `${CODEX_BIN_DIR};${NPM_DIR};${process.env.PATH ?? ''}` },
    stdio: ['ignore', 'pipe', 'pipe']
  })
  let page
  for (let i = 0; i < 80 && page === undefined; i += 1) {
    await sleep(500)
    if (child.exitCode !== null) throw new Error(`app exited ${child.exitCode}`)
    try {
      const list = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()
      page = list.find((t) => t.type === 'page' && t.webSocketDebuggerUrl && !t.url.includes('#splash'))
    } catch {
      // not up yet
    }
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
  return { child, cdp }
}

const profile = await mkdtemp(join(tmpdir(), 'locust-resume-'))
await mkdir(profile, { recursive: true })
const ledgerDir = join(profile, 'mission-ledger')
const ledgers = async () => (await readdir(ledgerDir).catch(() => [])).filter((n) => n.endsWith('.jsonl'))

try {
  say('1. start a mission, then kill the app while it is working')
  const first = await launch(profile, 9280)
  const submitted = await first.cdp.eval(`(async () => {
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
  check('a mission was started', submitted === 'clicked', submitted)

  // Let it do real work first, so the checkpoint has something in it.
  const worked = await first.cdp.eval(`(async () => {
    for (let i = 0; i < 60; i += 1) {
      await new Promise(r => setTimeout(r, 1000))
      if (document.querySelectorAll('.lc-activity, .lc-livestep').length > 0) return true
    }
    return false
  })()`)
  check('the run got going before the close', worked === true)
  // Closed the way a PERSON closes it, not killed. A hard kill runs no
  // shutdown handler, so no checkpoint is written and the mission is
  // correctly unresumable -- which is a real finding, and also not the
  // scenario the design names ("interrupted when the app closed").
  await first.cdp.eval(`document.querySelector('.lc-close').click(); true`).catch(() => undefined)
  for (let i = 0; i < 30 && first.child.exitCode === null; i += 1) await sleep(500)
  check('the app shut itself down', first.child.exitCode !== null, `exit ${String(first.child.exitCode)}`)
  await sleep(1500)
  check('a ledger was written', (await ledgers()).length >= 1, `files: ${(await ledgers()).length}`)

  say('2. relaunch and look for the offer')
  const second = await launch(profile, 9281)
  const found = await second.cdp.eval(`(async () => {
    for (let i = 0; i < 60; i += 1) {
      const row = document.querySelector('.lc-teammate__mission, .lc-row--mission')
      if (row) { row.click(); await new Promise(r => setTimeout(r, 800)) }
      const card = document.querySelector('.lc-resume')
      const refused = [...document.querySelectorAll('.lc-card__head')].find(h => /Cannot be resumed/.test(h.innerText))
      if (card || refused) {
        const node = card || refused.parentElement
        return JSON.stringify({
          offered: card !== null,
          refused: refused !== undefined,
          text: node.innerText.replace(/[ \\t]+/g, ' ').slice(0, 400)
        })
      }
      await new Promise(r => setTimeout(r, 1000))
    }
    const thread = document.querySelector('.lc-thread')
    return JSON.stringify({ offered: false, refused: false, text: thread ? thread.innerText.slice(-500) : '(no thread)' })
  })()`)
  const offer = JSON.parse(found)
  say(`       ${offer.text}`)
  check('an interrupted mission offers something', offer.offered || offer.refused)

  if (offer.offered) {
    say('3. press it')
    const before = (await ledgers()).length
    await second.cdp.eval(`document.querySelector('.lc-resume .lc-button').click()`)
    await sleep(8000)
    const after = (await ledgers()).length
    check('a new mission was written for the resumed run', after > before, `${before} -> ${after}`)

    const headers = await Promise.all(
      (await ledgers()).map(async (n) =>
        JSON.parse((await readFile(join(ledgerDir, n), 'utf8')).split('\n')[0]).metadata
      )
    )
    headers.sort((a, b) => Date.parse(a.createdAt) - Date.parse(b.createdAt))
    const resumed = headers.at(-1)
    check(
      'the ledger records it as a resume, not as something a person started',
      resumed?.startedBy?.kind === 'resume',
      JSON.stringify(resumed?.startedBy)
    )
    check(
      'it points at the mission it continues',
      resumed?.continuesFrom?.missionId === headers[0]?.missionId,
      JSON.stringify(resumed?.continuesFrom)
    )
    check(
      'it is a follow-up, so no divider claims a runtime change that did not happen',
      resumed?.continuesFrom?.reason === 'follow-up',
      resumed?.continuesFrom?.reason
    )
    const leaked = await second.cdp.eval(
      `document.body.innerText.includes('You are continuing work that another agent')`
    )
    check('the host briefing is not shown as a mission title', leaked === false)
  }
  second.child.kill()
  await sleep(1000)
} catch (error) {
  say(String(error))
  failures += 1
} finally {
  await rm(profile, { recursive: true, force: true }).catch(() => undefined)
}

say(failures === 0 ? '\nRESUME VERIFIED' : `\n${failures} RESUME FAILURE(S)`)
process.exit(failures === 0 ? 0 : 1)
