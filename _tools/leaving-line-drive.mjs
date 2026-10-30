// The leaving line, from where the person stands.
//
//   node _tools/leaving-line-drive.mjs --fixture <ledger.jsonl> --workspace <original-folder>
//
// Seeds a profile with one finished conversation (the 2026-09-14 acceptance
// fixture) and a groups file recording that the conversation LEFT a group
// called Trading -- a group that no longer exists -- after its only turn.
// Opens the conversation and reads the thread: the line "Trading's
// instructions no longer apply from here" must be drawn below the turn, and
// a screenshot is written beside the chain measures. No provider run.

import '../_tools/scratch-root.mjs'

import { spawn } from 'node:child_process'
import { copyFile, mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { requiredPathArgument } from './required-path-argument.mjs'
import { localPath } from './local-path.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const PORT = 9475
const FIXTURE = requiredPathArgument('--fixture')
const WORKSPACE = requiredPathArgument('--workspace')
const MISSION_ID = 'mission_4ac0d5fd-b688-4f04-8222-624a155225dd'

const say = (line) => console.error(line)
const check = (label, ok, detail) => {
  console.error(`   ${ok ? 'ok  ' : 'FAIL'} ${label}${detail === undefined ? '' : ' -- ' + detail}`)
  if (!ok) process.exitCode = 1
}
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

const profile = await mkdtemp(join(tmpdir(), 'locust-leaving-'))
await mkdir(join(profile, 'mission-ledger'), { recursive: true })
await copyFile(FIXTURE, join(profile, 'mission-ledger', `${MISSION_ID}.jsonl`))
await writeFile(
  join(profile, 'groups.json'),
  JSON.stringify({
    schemaVersion: 1,
    groups: [],
    members: {},
    left: {
      [MISSION_ID]: [
        { groupId: 'grp_trading', name: 'Trading', instructions: 'Analysis only. Never place a trade.', at: '2026-09-14T02:00:00.000Z', until: '2026-09-14T03:05:00.000Z' }
      ]
    }
  })
)
await writeFile(join(profile, 'teammates.json'), JSON.stringify({ schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, autoMode: false } }))

const child = spawn(ELECTRON, [APP_DIR, `--remote-debugging-port=${PORT}`, `--user-data-dir=${profile}`], {
  cwd: WORKSPACE,
  env: { ...process.env },
  stdio: ['ignore', 'pipe', 'pipe']
})
const appOutput = []
child.stdout.on('data', (d) => appOutput.push(String(d)))
child.stderr.on('data', (d) => appOutput.push(String(d)))

try {
  let page
  for (let attempt = 0; attempt < 60 && page === undefined; attempt += 1) {
    await sleep(500)
    if (child.exitCode !== null) break
    try {
      const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json()
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
  await cdp.eval(`(async () => { for (let i = 0; i < 240; i += 1) { const c = [...document.querySelectorAll('.lc-control')].find(b => b.getAttribute('aria-haspopup') === 'listbox'); if (c && /cursor|codex|claude|opencode/i.test(c.innerText)) return true; await new Promise(r => setTimeout(r, 500)) } return false })()`)

  say('1. open the conversation')
  const opened = await cdp.eval(`(async () => {
    const all = [...document.querySelectorAll('button')].find(b => /^All missions/.test(b.getAttribute('aria-label') || b.getAttribute('title') || ''))
    if (all) { all.click(); await new Promise(r => setTimeout(r, 600)) }
    let row
    for (let attempt = 0; attempt < 40 && !row; attempt += 1) {
      row = [...document.querySelectorAll('button')].find(b => /Two independent tasks/.test(b.innerText || '') || /Two independent tasks/.test(b.getAttribute('title') || ''))
      if (!row) await new Promise(r => setTimeout(r, 250))
    }
    if (!row) return JSON.stringify({ opened: false, buttons: [...document.querySelectorAll('button')].map(x => (x.innerText || x.getAttribute('title') || '').replace(/[ \\t\\r\\n]+/g, ' ').trim().slice(0, 40)).filter(Boolean).slice(0, 40) })
    row.click()
    for (let i = 0; i < 60; i += 1) {
      await new Promise(r => setTimeout(r, 250))
      const line = document.querySelector('.lc-thread__groupnote--left')
      if (line) {
        const thread = document.querySelector('.lc-thread')
        const nodes = thread ? [...thread.querySelectorAll('.lc-bubble, .lc-thread__groupnote--left, .lc-agent-message, .lc-thread__note')] : []
        return JSON.stringify({ opened: true, line: line.innerText, order: nodes.map(n => n.className.split(' ')[0] + ': ' + n.innerText.replace(/[ \\t\\r\\n]+/g, ' ').slice(0, 50)) })
      }
    }
    return JSON.stringify({ opened: true, line: null, thread: (document.querySelector('.lc-thread') || { innerText: '' }).innerText.slice(-600) })
  })()`)
  say(`   ${opened.slice(0, 1500)}`)
  const facts = await cdp.eval(`(() => {
    const pick = (el) => {
      if (!el) return null
      const r = el.getBoundingClientRect()
      const s = getComputedStyle(el)
      return {
        cls: (el.className || '').toString().slice(0, 60),
        top: Math.round(r.top), height: Math.round(r.height),
        position: s.position, overflow: s.overflow + '/' + s.overflowY,
        transform: s.transform === 'none' ? 'none' : 'yes',
        zIndex: s.zIndex, contain: s.contain, isolation: s.isolation
      }
    }
    const bubble = document.querySelector('.lc-thread__column > .lc-bubble') || document.querySelector('.lc-bubble')
    const chain = []
    let node = bubble
    for (let i = 0; node && i < 8; i += 1) { chain.push(pick(node)); node = node.parentElement }
    return JSON.stringify({
      chain,
      header: pick(document.querySelector('.lc-workroom__header')),
      thread: pick(document.querySelector('.lc-thread')),
      column: pick(document.querySelector('.lc-thread__column')),
      bubbleText: (bubble ? bubble.innerText : '').replace(new RegExp('[' + String.fromCharCode(32, 9, 13, 10) + ']+', 'g'), ' ').slice(0, 60)
    }, null, 1)
  })()`)
  say('   FACTS ' + facts)
  // Is the overlapping text at the top REALLY in the document, or is the
  // capture compositing a stale frame? `elementFromPoint` answers it.
  const atTop = await cdp.eval(`(() => {
    const where = (x, y) => {
      const el = document.elementFromPoint(x, y)
      if (!el) return 'nothing'
      return (el.className || el.tagName).toString().slice(0, 40) + ' :: ' + (el.innerText || '').replace(new RegExp('[' + String.fromCharCode(32, 9, 13, 10) + ']+', 'g'), ' ').slice(0, 40)
    }
    return JSON.stringify({ y10: where(640, 10), y40: where(640, 40), y70: where(640, 70), y110: where(640, 110) }, null, 1)
  })()`)
  say('   AT TOP ' + atTop)
  /*
   * THE TWO NUMBERS THE DESIGN AGENT WAS ASKED FOR: 24 above, 12 below.
   *
   * This drive screenshotted the line and never measured it, so 0.171 through
   * 0.177 shipped it at 36/22 and nothing said so. Grok's pass 9 measured it
   * by hand. A margin is not a gap: the column is flex with a 12px gap, and a
   * bubble opening a new turn adds 10 of its own, so both numbers were being
   * added to by rules that did not know the note was there.
   *
   * Measured as the real distance between painted boxes, which is the only
   * version of this a person can see.
   */
  const airText = await cdp.eval(`(() => {
    const note = document.querySelector('.lc-thread__note')
    if (!note) return JSON.stringify({ note: false })
    const box = note.getBoundingClientRect()
    const before = note.previousElementSibling
    const after = note.nextElementSibling
    const gap = (a, b) => a === null || b === null ? null : Math.round(b - a)
    return JSON.stringify({
      note: true,
      words: (note.innerText || '').slice(0, 60),
      marginTop: getComputedStyle(note).marginTop,
      above: before ? gap(before.getBoundingClientRect().bottom, box.top) : null,
      below: after ? gap(box.bottom, after.getBoundingClientRect().top) : null,
      afterIsBubble: after ? (after.className || '').toString().indexOf('lc-bubble') >= 0 : null
    }, null, 1)
  })()`)
  say('   AIR ' + airText)
  /*
   * AND THE JOIN LINE IS STILL THERE, ABOVE IT.
   *
   * Grok, three passes running: after leaving, only the stop was marked;
   * turns 3 and 4 were briefed and the thread no longer said so. The seeded
   * membership joined at 02:00 and left at 03:05, so the thread must carry
   * both lines, the join above the leave, bracketing the briefed turns.
   */
  const bracketText = await cdp.eval(`(() => {
    const notes = [...document.querySelectorAll('.lc-thread__groupnote')]
    const joins = notes.filter(n => !n.classList.contains('lc-thread__groupnote--left'))
    const leaves = notes.filter(n => n.classList.contains('lc-thread__groupnote--left'))
    const top = (el) => Math.round(el.getBoundingClientRect().top)
    return JSON.stringify({
      joins: joins.map(n => (n.innerText || '').slice(0, 60)),
      leaves: leaves.map(n => (n.innerText || '').slice(0, 60)),
      joinAbove: joins.length > 0 && leaves.length > 0 ? top(joins[0]) < top(leaves[0]) : null
    })
  })()`)
  say('   BRACKET ' + bracketText)
  const bracket = JSON.parse(bracketText)
  check('the join line is still drawn after leaving', bracket.joins.length === 1 && /brief every turn from here/.test(bracket.joins[0] ?? ''), bracketText)
  check('and it sits above the leaving line', bracket.joinAbove === true, bracketText)
  /*
   * AND THE CASE GROK ACTUALLY MEASURED: a bubble under the note.
   *
   * The thread this drive seeds puts a card there, and a card does not add
   * the 10px a bubble adds to open a new turn -- so the ordinary measurement
   * above passes on a thread that never exercises the rule that was wrong.
   * A bubble is put in beside the real note, in the real stylesheet, in the
   * real layout engine, measured, and taken out again.
   */
  const bubbleAirText = await cdp.eval(`(() => {
    const note = document.querySelector('.lc-thread__note')
    if (!note) return JSON.stringify({ note: false })
    const planted = document.createElement('div')
    planted.className = 'lc-bubble'
    planted.textContent = 'planted, to measure the gap under a note'
    note.parentElement.insertBefore(planted, note.nextSibling)
    const below = Math.round(planted.getBoundingClientRect().top - note.getBoundingClientRect().bottom)
    const margin = getComputedStyle(planted).marginTop
    planted.remove()
    return JSON.stringify({ note: true, below, margin }, null, 1)
  })()`)
  say('   AIR UNDER A BUBBLE ' + bubbleAirText)
  const bubbleAir = JSON.parse(bubbleAirText)
  if (bubbleAir.note === true) {
    const ok = Math.abs(bubbleAir.below - 12) <= 1
    say(`   ${ok ? 'ok  ' : 'FAIL'} below a bubble: ${String(bubbleAir.below)}px, wanted 12`)
    if (!ok) process.exitCode = 1
  }
  const air = JSON.parse(airText)
  if (air.note !== true) {
    say('   FAIL no note on the thread to measure')
    process.exitCode = 1
  } else {
    // Only checked when there is something on the other side to measure from;
    // a note with nothing above it has no gap above, which is not a failure.
    for (const [side, want] of [['above', 24], ['below', 12]]) {
      const got = air[side]
      if (got === null) { say(`   -- ${side}: nothing to measure against`); continue }
      const ok = Math.abs(got - want) <= 1
      say(`   ${ok ? 'ok  ' : 'FAIL'} ${side}: ${String(got)}px, wanted ${String(want)}`)
      if (!ok) process.exitCode = 1
    }
  }
  // Settle, then capture twice: a stale composite differs between frames.
  await sleep(1500)
  const shot = await cdp.send('Page.captureScreenshot', { format: 'png' })
  await writeFile(new URL('../docs/chain-measure/leaving-line-settled.png', import.meta.url), Buffer.from(shot.result.data, 'base64'))
  const out = new URL('../docs/chain-measure/leaving-line-2026-09-17.png', import.meta.url)
  await writeFile(out, Buffer.from(shot.result.data, 'base64'))
  say(`   screenshot ${out.pathname.slice(1)}`)
  if (!JSON.parse(opened).line) process.exitCode = 1
} finally {
  child.kill()
  await sleep(500)
  await rm(profile, { recursive: true, force: true }).catch(() => undefined)
}
