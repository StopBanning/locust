// The decision card, which nobody had ever driven live.
//
//   node _tools/decision-card-drive.mjs [--keep]
//
// A run that reaches a real fork is meant to stop and ask, with a block the
// app turns into a card: the question, two to four options as buttons, and
// the answer starting the next turn. Every part of that has unit tests and
// none of it had been watched happen. Fable's first pass looked for one
// across twenty-four live missions and reported "no decision card appeared
// in any live turn of this pass. Not tested." Grok has never seen one
// either.
//
// So: ask the free model something with a genuine fork in it and tell it to
// use the block. Then read the card the way a person reads it, press an
// option, and check the answer starts a turn that knows which option was
// chosen.
//
// Live: two real exchanges on the free OpenCode model. No quota. Refuses to
// send on any other route.

import '../_tools/scratch-root.mjs'
import { localPath, FREE_ROW, FREE_SEARCH } from './drive-lib.mjs'

import { spawn } from 'node:child_process'
import { mkdtemp, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { join } from 'node:path'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const NPM_DIR = join(homedir(), 'AppData', 'Roaming', 'npm')
const PORT = 9501
const KEEP = process.argv.includes('--keep')
const WAIT_MS = 5 * 60 * 1000
/*
 * A fork the model cannot settle by reading the folder: the two files say
 * opposite things and neither is authoritative, so picking one is a choice
 * about what the person wants. The block is named because a free model does
 * not reliably reach for it unasked -- this drive is about the CARD, not
 * about whether the model volunteers one.
 */
const ASK = [
  'notes/a.md and notes/b.md give two different release dates for the same release and neither says which wins.',
  'Do not edit anything and do not guess. Stop and ask me which one to treat as correct,',
  'using exactly the <locust-ask> block you were shown, with one option per file.'
].join(' ')

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

const workspace = await mkdtemp(join(tmpdir(), 'locust-decide-ws-'))
const profile = await mkdtemp(join(tmpdir(), 'locust-decide-'))
await mkdir(join(workspace, 'notes'), { recursive: true })
await writeFile(join(workspace, 'README.md'), '# scratch\n', 'utf8')
await writeFile(join(workspace, 'notes', 'a.md'), 'Release date: 3 October.\n', 'utf8')
await writeFile(join(workspace, 'notes', 'b.md'), 'Release date: 17 October.\n', 'utf8')
await writeFile(
  join(profile, 'teammates.json'),
  JSON.stringify({ schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 6, autoMode: false, memoryMode: 'off' } })
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

  say('1. the free route, and nothing sent on any other')
  const picked = await cdp.eval(`(async () => {
    const control = [...document.querySelectorAll('.lc-control')].find(b => b.getAttribute('aria-haspopup') === 'listbox')
    if (!control) return JSON.stringify({ control: false })
    if (${FREE_ROW}.test(control.innerText)) return JSON.stringify({ picked: true, text: control.innerText })
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
        if (row && !row.disabled && /opencode/i.test(group) && ${FREE_ROW}.test(label) && /free/.test(label)) { target = row; break }
      }
      if (!target) await new Promise(r => setTimeout(r, 500))
    }
    if (!target) return JSON.stringify({ picked: false })
    target.click()
    await new Promise(r => setTimeout(r, 500))
    return JSON.stringify({ picked: true, text: control.innerText })
  })()`)
  say(`   ${picked}`)
  const onFree = new RegExp(FREE_ROW.slice(1, FREE_ROW.lastIndexOf('/')), 'i').test(JSON.parse(picked).text ?? '')
  check('the free OpenCode model is the route', onFree, picked)
  if (!onFree) throw new Error('the free OpenCode route was not picked; nothing was sent')

  say('2. ask something with a real fork in it')
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
  if (sent !== 'clicked') throw new Error(sent)

  say('3. waiting for the card')
  const startedAt = Date.now()
  let card
  while (Date.now() - startedAt < WAIT_MS) {
    await sleep(2000)
    card = JSON.parse(await cdp.eval(`(() => {
      const el = document.querySelector('.lc-decision')
      if (!el) return JSON.stringify({ card: false, running: !!document.querySelector('button[aria-label^="Stop the running"]') })
      const clean = (s) => (s || '').replace(new RegExp('[' + String.fromCharCode(32, 9, 13, 10) + ']+', 'g'), ' ').trim()
      const options = [...el.querySelectorAll('.lc-decision__option')]
      return JSON.stringify({
        card: true,
        title: clean((el.querySelector('.lc-decision__title') || {}).innerText),
        question: clean((el.querySelector('.lc-decision__question') || {}).innerText),
        options: options.map(o => clean(o.innerText).slice(0, 80)),
        enabled: options.filter(o => !o.disabled).length,
        // The "you are not limited to these" line: once, under the options.
        freeform: [...el.querySelectorAll('.lc-decision__foot, .lc-decision__freeform')].length,
        note: clean((el.querySelector('.lc-decision__note') || {}).innerText)
      })
    })()`))
    if (card.card === true) break
    if (card.running === false && Date.now() - startedAt > 30_000) break
  }
  say(`   ${JSON.stringify(card)}`)
  check('a decision card is on screen', card.card === true, JSON.stringify(card))
  if (card.card === true) {
    check('it asks the question in words', (card.question ?? '').length > 10, card.question)
    check('it offers two to four options', card.options.length >= 2 && card.options.length <= 4, JSON.stringify(card.options))
    check('every option can be pressed', card.enabled === card.options.length, JSON.stringify(card))
    // Said once. The card carried the same offer twice, above and below the
    // options, until the first frame of it was looked at (2026-09-19).
    check('it says once that you are not limited to the options', card.freeform === 1, String(card.freeform))
    const frame = await cdp.send('Page.captureScreenshot', { format: 'png' })
    await writeFile(new URL('../docs/chain-measure/decision-card-2026-09-19.png', import.meta.url), Buffer.from(frame.result.data, 'base64'))
  }

  if (card.card === true) {
    say('4. answer by number, as Claude Code does (0.801), and watch the next turn start')
    const before = (await ledgers()).length
    const key = async (name, code, vk) => {
      await cdp.send('Input.dispatchKeyEvent', { type: 'keyDown', key: name, code, windowsVirtualKeyCode: vk, nativeVirtualKeyCode: vk, text: name })
      await cdp.send('Input.dispatchKeyEvent', { type: 'keyUp', key: name, code, windowsVirtualKeyCode: vk, nativeVirtualKeyCode: vk })
      await sleep(300)
    }
    const state = async () => JSON.parse(await cdp.eval(`JSON.stringify({
      keys: [...document.querySelectorAll('.lc-decision__option .lc-decision__key')].map((k) => k.innerText.trim()),
      focus: document.activeElement?.classList.contains('lc-decision__option') ? 'option' : document.activeElement?.closest('form.command-dock') ? 'chat box' : String(document.activeElement?.tagName),
      typed: document.querySelector('form.command-dock textarea')?.value ?? null,
      card: !!document.querySelector('.lc-decision__option:not([disabled])')
    })`))
    const first = await state()
    check('each option wears the number that chooses it', first.keys.join(',') === card.options.map((_, i) => String(i + 1)).join(','), JSON.stringify(first.keys))
    check('the card has the focus, the chat box being empty', first.focus === 'option', first.focus)
    await key('h', 'KeyH', 72)
    const letter = await state()
    check('a letter typed on the card goes to the chat box', letter.focus === 'chat box' && letter.typed === 'h', JSON.stringify(letter))
    await key('2', 'Digit2', 50)
    const inField = await state()
    check('a number typed in the chat box is a number, not an answer', inField.card && inField.typed === 'h2', JSON.stringify(inField))
    await cdp.eval(`(() => {
      const field = document.querySelector('form.command-dock textarea')
      Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set.call(field, '')
      field.dispatchEvent(new Event('input', { bubbles: true }))
      field.blur()
      return 'cleared'
    })()`)
    await sleep(300)
    const pressed = await cdp.eval(`(document.querySelectorAll('.lc-decision__option .lc-decision__label')[1]?.innerText ?? '').replace(new RegExp('[' + String.fromCharCode(32, 9, 13, 10) + ']+', 'g'), ' ').trim()`)
    await key('2', 'Digit2', 50)
    say(`   pressed 2: ${pressed}`)
    const answeredAt = Date.now()
    let after = before
    while (Date.now() - answeredAt < WAIT_MS) {
      await sleep(2000)
      after = (await ledgers()).length
      const running = await cdp.eval(`!!document.querySelector('button[aria-label^="Stop the running"]')`).catch(() => false)
      if (after > before && !running) break
    }
    check('the answer started another turn', after > before, `${String(before)} → ${String(after)}`)
    // The turn it started must carry WHICH option was chosen, or the answer
    // went nowhere and the card was decoration.
    const names = await ledgers()
    let carried = false
    for (const name of names) {
      const text = await readFile(join(LEDGER_DIR, name), 'utf8')
      const first = JSON.parse(text.split('\n')[0] ?? '{}')
      const prompt = String(first.metadata?.prompt ?? '')
      // Only the turn the answer started: the first one names both files.
      if (prompt.length > 0 && pressed.length > 0 && !prompt.includes('neither says which wins') && prompt.includes(pressed) && !prompt.includes(pressed.includes('b.md') ? 'notes/a.md' : 'notes/b.md')) carried = true
      if (prompt.length > 0 && !prompt.includes('neither says which wins')) say(`   the answered turn's prompt: ${prompt.replace(/\s+/g, ' ').slice(0, 200)}`)
    }
    check('and that turn carries the option the person chose', carried, pressed)
    const gone = await cdp.eval(`(() => JSON.stringify({ card: !!document.querySelector('.lc-decision'), standing: !!document.querySelector('.lc-decision__option:not([disabled])') }))()`)
    say(`   after answering: ${gone}`)
    check('the card stops offering options once answered', JSON.parse(gone).standing === false, gone)
  }

  const shot = await cdp.send('Page.captureScreenshot', { format: 'png' })
  const out = new URL('../docs/chain-measure/decision-card-answered-2026-09-19.png', import.meta.url)
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
console.error(failures === 0 ? 'DECISION CARD DRIVE PASSED' : `${String(failures)} check(s) failed`)
process.exitCode = failures === 0 ? 0 : 1
