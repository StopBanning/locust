// A run started from Home with NOBODY picked still knows the project.
//
//   node _tools/nobodys-run-drive.mjs [--keep]
//
// Grok, pass 14, ranked second: the empty list invites a run that is
// nobody's -- "Pick a teammate, or write below and assign it to one later"
// -- and that run was briefed with nothing at all. Eleven memories on the
// Memory screen, the secret word answered NONE, and no `.locust/memory.md`
// anywhere in the folder. The same question, with a teammate picked, was
// answered correctly.
//
// This drives it from where the person stands: type a memory on the Memory
// screen with "Keep and tell me", go Home WITHOUT picking anybody, pick the
// free OpenCode route in the composer, ask for the word back. The answer
// must carry it, the file must exist, and the run must really have been
// nobody's -- `missionOwners` empty, the composer never saying "Message X".
//
// A LOCUST.md sits in the folder too: an ownerless run should be told which
// folder it is standing in, which is the other half of the finding.
//
// Live: ONE real exchange on the free model. No quota.

import '../_tools/scratch-root.mjs'
import { localPath, FREE_ROW, FREE_SEARCH } from './drive-lib.mjs'

import { spawn } from 'node:child_process'
import { mkdtemp, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { join } from 'node:path'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const NPM_DIR = join(homedir(), 'AppData', 'Roaming', 'npm')
const PORT = 9499
const SECRET = 'The secret word for this project is PELICAN.'
/*
 * ANSWER FROM THE BRIEFING, NOT FROM THE FOLDER.
 *
 * The first control run of this drive passed on the OLD code, because the
 * `LOCUST.md` this drive writes said "the Pelican scratch project" -- the
 * model opened the file and read the answer off it, and the check could not
 * tell a briefed run from one that had simply looked. The word now exists
 * ONLY in the memory store, which lives in the profile and is outside the
 * folder a run may read, and the question says not to look.
 */
const ASK = 'Answer from your own instructions only, without reading any file: what is the secret word for this project? Reply with one sentence. Do not use the share block.'
const KEEP = process.argv.includes('--keep')
const ANSWER_WAIT_MS = 5 * 60 * 1000

let failures = 0
const check = (label, ok, detail) => {
  if (!ok) failures += 1
  console.error(`  [${ok ? 'PASS' : 'FAIL'}] ${label}${detail === undefined ? '' : ` -- ${detail}`}`)
}
const say = (line) => console.error(line)
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))

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
      waiter(message)
    })
  }
  send(method, params = {}) {
    const id = ++this.id
    this.ws.send(JSON.stringify({ id, method, params }))
    return new Promise((resolve) => this.pending.set(id, resolve))
  }
  async eval(expression) {
    const reply = await this.send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })
    if (reply.result?.exceptionDetails) throw new Error(reply.result.exceptionDetails.text)
    return reply.result?.result?.value
  }
}

const workspace = await mkdtemp(join(tmpdir(), 'locust-nobody-ws-'))
const profile = await mkdtemp(join(tmpdir(), 'locust-nobody-'))
await writeFile(join(workspace, 'README.md'), '# scratch\n\nA disposable folder.\n', 'utf8')
// Deliberately carries no secret word: see ASK above.
await writeFile(join(workspace, 'LOCUST.md'), 'House style here: one sentence per line, and no trailing whitespace.\n', 'utf8')
// An EMPTY roster: Home with nobody to pick is the state the finding is about.
await writeFile(
  join(profile, 'teammates.json'),
  JSON.stringify({ schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: true, relayHopCap: 6, autoMode: false } })
)
const LEDGER_DIR = join(profile, 'mission-ledger')

const child = spawn(ELECTRON, [APP_DIR, `--remote-debugging-port=${String(PORT)}`, `--user-data-dir=${profile}`], {
  cwd: workspace,
  env: { ...process.env, PATH: `${NPM_DIR};${process.env.PATH ?? ''}` },
  stdio: ['ignore', 'pipe', 'pipe']
})
const appOutput = []
child.stdout.on('data', (d) => appOutput.push(String(d)))
child.stderr.on('data', (d) => appOutput.push(String(d)))
const ledgers = async () => (await readdir(LEDGER_DIR).catch(() => [])).filter((name) => name.endsWith('.jsonl'))

try {
  let page
  for (let attempt = 0; attempt < 80 && page === undefined; attempt += 1) {
    await sleep(500)
    if (child.exitCode !== null) throw new Error(`app exited ${String(child.exitCode)}`)
    try {
      const list = await (await fetch(`http://127.0.0.1:${String(PORT)}/json/list`)).json()
      page = list.find((t) => t.type === 'page' && t.webSocketDebuggerUrl && !t.url.includes('#splash'))
    } catch { /* not up yet */ }
  }
  if (page === undefined) { say(appOutput.join('')); throw new Error('no renderer target') }
  const socket = new WebSocket(page.webSocketDebuggerUrl)
  await new Promise((resolve, reject) => {
    socket.addEventListener('open', resolve, { once: true })
    socket.addEventListener('error', reject, { once: true })
  })
  const cdp = new Cdp(socket)
  await cdp.send('Runtime.enable')
  const ready = await cdp.eval(`(async () => {
    for (let i = 0; i < 240; i += 1) {
      const r = await window.desktop.getLocalRuntimes().catch(() => undefined)
      const oc = r && r.ok ? r.data.runtimes.find(x => x.id === 'opencode') : undefined
      if (oc && oc.installed) return true
      await new Promise(r => setTimeout(r, 500))
    }
    return false
  })()`)
  check('OpenCode is on this machine', ready === true)

  say('1. remember a line on the Memory screen, mode Keep and tell me')
  const remembered = await cdp.eval(`(async () => {
    window.dispatchEvent(new KeyboardEvent('keydown', { key: '5', code: 'Digit5', ctrlKey: true, bubbles: true }))
    await new Promise(r => setTimeout(r, 900))
    const radio = [...document.querySelectorAll('[role="radio"]')].find(b => /Keep and tell me/.test(b.innerText))
    if (!radio) return JSON.stringify({ screen: false, body: document.body.innerText.slice(0, 200) })
    radio.click()
    await new Promise(r => setTimeout(r, 400))
    const input = [...document.querySelectorAll('textarea.lc-input')].find(i => /specific enough to act on/.test(i.getAttribute('placeholder') || ''))
    if (!input) return JSON.stringify({ screen: true, input: false })
    const setInput = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
    setInput.call(input, ${JSON.stringify(SECRET)})
    input.dispatchEvent(new Event('input', { bubbles: true }))
    await new Promise(r => setTimeout(r, 300))
    const button = [...document.querySelectorAll('button.lc-primarybutton')].find(b => /^Remember$/.test(b.innerText.trim()))
    if (!button || button.disabled) return JSON.stringify({ screen: true, input: true, button: !!button })
    button.click()
    await new Promise(r => setTimeout(r, 1200))
    const kept = [...document.querySelectorAll('.lc-memory__text')].map(p => p.innerText.trim())
    window.dispatchEvent(new KeyboardEvent('keydown', { key: '1', code: 'Digit1', ctrlKey: true, bubbles: true }))
    await new Promise(r => setTimeout(r, 800))
    return JSON.stringify({ screen: true, input: true, kept })
  })()`)
  say(`   ${remembered}`)
  check('the line is kept on the Memory screen', (JSON.parse(remembered).kept ?? []).some((line) => line.includes('PELICAN')), remembered)

  say('2. Home, with nobody picked: the composer says so')
  const home = await cdp.eval(`(async () => {
    const back = [...document.querySelectorAll('button')].find(b => /^Home$/.test(b.getAttribute('aria-label') || ''))
    if (back) { back.click(); await new Promise(r => setTimeout(r, 800)) }
    const field = document.querySelector('form.command-dock textarea')
    return JSON.stringify({ placeholder: field ? field.getAttribute('placeholder') : null })
  })()`)
  say(`   ${home}`)
  // "Message Wren…" is the picked state. Nobody picked is the state this
  // drive is about, so the placeholder must not name anybody.
  check('nobody is picked: the box does not say "Message <name>"', !/^Message /.test(JSON.parse(home).placeholder ?? ''), home)

  say('3. pick the free OpenCode route in the composer, with no teammate')
  const picked = await cdp.eval(`(async () => {
    const control = [...document.querySelectorAll('.lc-control')].find(b => b.getAttribute('aria-haspopup') === 'listbox')
    if (!control) return JSON.stringify({ control: false })
    if (${FREE_ROW}.test(control.innerText)) return JSON.stringify({ control: true, already: true, text: control.innerText })
    control.click()
    await new Promise(r => setTimeout(r, 500))
    let target
    for (let attempt = 0; attempt < 90 && !target; attempt += 1) {
      const picker = document.querySelector('.lc-picker')
      if (!picker) { control.click(); await new Promise(r => setTimeout(r, 500)); continue }
      const input = picker.querySelector('.lc-picker__input')
      const setInput = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set
      setInput.call(input, ${JSON.stringify(FREE_SEARCH)})
      input.dispatchEvent(new Event('input', { bubbles: true }))
      await new Promise(r => setTimeout(r, 500))
      let group = ''
      for (const node of picker.querySelector('.lc-picker__list').children) {
        const header = node.querySelector('.lc-picker__group')
        if (header) group = header.innerText
        const row = node.querySelector('.lc-picker__row')
        const label = row ? row.innerText.trim().toLowerCase() : ''
        // Spelled either way: with a teammate picked the row reads the
        // display name; with nobody picked it reads the raw model id.
        if (row && !row.disabled && /opencode/i.test(group) && ${FREE_ROW}.test(label) && /free/.test(label)) { target = row; break }
      }
      if (!target) await new Promise(r => setTimeout(r, 500))
    }
    if (!target) {
      const picker = document.querySelector('.lc-picker')
      const rows = picker ? [...picker.querySelectorAll('.lc-picker__row')].map(r => r.innerText.replace(new RegExp('[' + String.fromCharCode(32, 9, 13, 10) + ']+', 'g'), ' ').trim().slice(0, 60)) : []
      const groups = picker ? [...picker.querySelectorAll('.lc-picker__group')].map(g => g.innerText.trim()) : []
      return JSON.stringify({ control: true, picked: false, picker: !!picker, groups, rows: rows.slice(0, 12) })
    }
    target.click()
    await new Promise(r => setTimeout(r, 500))
    return JSON.stringify({ control: true, picked: true, text: control.innerText })
  })()`)
  say(`   ${picked}`)
  const onFreeModel = new RegExp(FREE_ROW.slice(1, FREE_ROW.lastIndexOf('/')), 'i').test(JSON.parse(picked).text ?? '')
  check('the free OpenCode model is the route', onFreeModel, picked)
  /*
   * NOTHING IS SENT ON ANOTHER ROUTE.
   *
   * The first run of this drive could not find the free model, fell through
   * to the composer's default -- Codex -- and spent a turn of quota that is
   * not mine to spend (2026-09-19). A drive that cannot get the free route
   * stops before the send rather than paying for the measurement.
   */
  if (!onFreeModel) throw new Error('the free OpenCode route was not picked; nothing was sent')

  say('4. ask, with nobody picked')
  const sent = await cdp.eval(`(async () => {
    const field = document.querySelector('form.command-dock textarea')
    const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
    setter.call(field, ${JSON.stringify(ASK)})
    field.dispatchEvent(new Event('input', { bubbles: true }))
    for (let i = 0; i < 120; i += 1) {
      await new Promise(r => setTimeout(r, 250))
      const send = document.querySelector('form.command-dock .send-button')
      if (send && !send.disabled && send.getAttribute('aria-label') === 'Send') { send.click(); return 'clicked' }
    }
    return 'send stayed disabled: ' + (field.placeholder || '')
  })()`)
  say(`   ${sent}`)
  if (sent !== 'clicked') throw new Error(sent)

  say('5. waiting for the answer')
  const startedAt = Date.now()
  let quiet = 0
  while (Date.now() - startedAt < ANSWER_WAIT_MS && quiet < 12) {
    await sleep(2000)
    const running = await cdp.eval(`!!document.querySelector('button[aria-label^="Stop the running"]')`).catch(() => true)
    quiet = running ? 0 : quiet + 1
  }
  const names = await ledgers()
  say(`   ledgers: ${String(names.length)}`)

  say('6. what it answered, and what it was briefed with')
  let answer = ''
  let briefed = ''
  let header = {}
  for (const name of names) {
    const lines = (await readFile(join(LEDGER_DIR, name), 'utf8')).split('\n').filter(Boolean).map((l) => { try { return JSON.parse(l) } catch { return undefined } }).filter(Boolean)
    header = lines[0]?.metadata ?? {}
    briefed = header.prompt ?? ''
    /*
     * The LAST final message, which is what the thread shows -- not the
     * longest string in the file. The longest string was a todo list in
     * JSON, and before that a Codex skills warning, so the first two runs
     * of this drive judged the answer on something the person never sees.
     */
    answer =
      lines
        .map((line) => line.event)
        .filter((event) => event?.type === 'message.delta' && event.payload?.final === true && typeof event.payload.text === 'string')
        .map((event) => event.payload.text)
        .pop() ?? ''
  }
  say(`   answer: ${answer.replace(new RegExp('[' + String.fromCharCode(32, 9, 13, 10) + ']+', 'g'), ' ').slice(0, 300)}`)
  const owners = JSON.parse(await readFile(join(profile, 'teammates.json'), 'utf8')).missionOwners ?? {}
  say(`   missionOwners: ${JSON.stringify(owners)}`)
  const file = await readFile(join(workspace, '.locust', 'memory.md'), 'utf8').catch(() => undefined)
  say(`   memory file: ${file === undefined ? 'NOT WRITTEN' : `${String(file.split('\n').filter((l) => l.startsWith('- ')).length)} line(s)`}`)

  check('the run really belonged to nobody', Object.keys(owners).length === 0, JSON.stringify(owners))
  check('it answered with the remembered word', /PELICAN/i.test(answer), answer.slice(0, 160))
  check('the memory file was written for the folder', file !== undefined && file.includes('PELICAN'), file === undefined ? 'no file written' : 'written')
  /*
   * The ledger keeps the PERSON's words as the prompt on purpose, so the
   * runtime prompt is not on disk to read -- an earlier version of this
   * drive asserted "not briefed as a teammate" against `metadata.prompt`
   * and passed on a run that had been briefed with nothing at all. The
   * answer is the only evidence of what reached the model, and the two
   * checks above are that evidence. What the header CAN say is which route
   * really ran, and that it was nobody's.
   */
  say(`   header: runtime=${String(header.runtime)} model=${String(header.model)}`)
  check('it ran on the free OpenCode model', header.runtime === 'opencode' && /free/i.test(String(header.model)), JSON.stringify({ runtime: header.runtime, model: header.model }))
  void briefed

  const shot = await cdp.send('Page.captureScreenshot', { format: 'png' })
  const out = new URL('../docs/chain-measure/nobodys-run-2026-09-19.png', import.meta.url)
  await writeFile(out, Buffer.from(shot.result.data, 'base64'))
  say(`   frame ${out.pathname.slice(1)}`)
} finally {
  child.kill()
  await sleep(1000)
  if (KEEP) say(`profile kept at ${profile}`)
  else {
    await rm(profile, { recursive: true, force: true }).catch(() => undefined)
    await rm(workspace, { recursive: true, force: true }).catch(() => undefined)
  }
}
console.error(failures === 0 ? 'NOBODYS RUN DRIVE PASSED' : `${String(failures)} check(s) failed`)
process.exitCode = failures === 0 ? 0 : 1
