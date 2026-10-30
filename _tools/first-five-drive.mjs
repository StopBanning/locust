// The first five minutes, and the frame that started it.
//
//   node _tools/first-five-drive.mjs --fixture <ledger.jsonl> --workspace <original-folder>
//
// Five things a person meets before they have done anything, each measured
// from the screen rather than from the source:
//
//   1. The conversation title is ONE line and stays inside the header.
//      (Found from the design agent's frame 03, 2026-09-17; the cause was
//      the header title, not the thread failing to clip.)
//   2. The plus menu closes on Escape.
//   3. It closes when you click somewhere else.
//   4. It closes when you change screen.
//      (2-4: outside tester on 0.164.0 -- none of the three worked.)
//   5. A boundary note has the air the design agent asked for: 24 above.
//
// Seeded with the acceptance fixture, whose first line is 480 words, because
// a title only breaks a header when it is long. No provider run; costs
// nothing.

import '../_tools/scratch-root.mjs'

import { spawn } from 'node:child_process'
import { copyFile, mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { requiredPathArgument } from './required-path-argument.mjs'
import { localPath } from './local-path.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const PORT = 9481
const FIXTURE = requiredPathArgument('--fixture')
const WORKSPACE = requiredPathArgument('--workspace')
const MISSION_ID = 'mission_4ac0d5fd-b688-4f04-8222-624a155225dd'

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

const profile = await mkdtemp(join(tmpdir(), 'locust-firstfive-'))
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

  say('1. the conversation with a 480-word first line')
  const header = await cdp.eval(`(async () => {
    const all = [...document.querySelectorAll('button')].find(b => /^All missions/.test(b.getAttribute('aria-label') || b.getAttribute('title') || ''))
    if (all) { all.click(); await new Promise(r => setTimeout(r, 600)) }
    let row
    for (let attempt = 0; attempt < 40 && !row; attempt += 1) {
      row = [...document.querySelectorAll('button')].find(b => /Two independent tasks/.test(b.innerText || ''))
      if (!row) await new Promise(r => setTimeout(r, 250))
    }
    if (!row) return JSON.stringify({ opened: false })
    row.click()
    await new Promise(r => setTimeout(r, 1500))
    const name = document.querySelector('.lc-workroom__name')
    const head = document.querySelector('.lc-workroom__header')
    const nameBox = name ? name.getBoundingClientRect() : null
    const headBox = head ? head.getBoundingClientRect() : null
    // What is painted at the very top of the window, over the chrome.
    const atTop = document.elementFromPoint(640, 10)
    return JSON.stringify({
      opened: true,
      nameHeight: nameBox ? Math.round(nameBox.height) : null,
      nameTop: nameBox ? Math.round(nameBox.top) : null,
      headerTop: headBox ? Math.round(headBox.top) : null,
      headerHeight: headBox ? Math.round(headBox.height) : null,
      atTop: atTop ? (atTop.className || atTop.tagName).toString().slice(0, 40) : 'nothing',
      noteMarginTop: (() => {
        const note = document.querySelector('.lc-thread__note')
        return note ? getComputedStyle(note).marginTop : 'no note'
      })(),
      bubbleMarginTop: (() => {
        const b = document.querySelector('.lc-thread__column > .lc-bubble')
        return b ? getComputedStyle(b).marginTop : 'no bubble'
      })()
    })
  })()`)
  const h = JSON.parse(header)
  say(`   ${header}`)
  check('the conversation opened', h.opened === true)
  // One line of the header's own type, not a wall of text.
  check('the title is one line', h.nameHeight !== null && h.nameHeight <= 32, `height ${String(h.nameHeight)}px`)
  check('the title stays inside the header', h.nameTop >= h.headerTop, `title top ${String(h.nameTop)}, header top ${String(h.headerTop)}`)
  check('nothing paints over the window chrome', h.atTop !== 'lc-workroom__name', `at (640,10): ${h.atTop}`)
  say('5. the boundary note has the air the design agent asked for')
  check('a note sits 24px below what precedes it', h.noteMarginTop === '24px', h.noteMarginTop)
  check('a turn still opens 22px down', h.bubbleMarginTop === '10px', `${h.bubbleMarginTop} + 12px column gap`)

  const menuState = async (step) => cdp.eval(`(async () => {
    ${step}
    await new Promise(r => setTimeout(r, 400))
    return document.querySelector('.lc-context[aria-label="Add"]') === null ? 'closed' : 'open'
  })()`)
  const openMenu = `[...document.querySelectorAll('button')].find(b => (b.getAttribute('aria-label') || '') === 'Add').click(); await new Promise(r => setTimeout(r, 300));`

  say('2. plus, then Escape')
  check('the menu opens', (await menuState(openMenu)) === 'open')
  check(
    'Escape closes it',
    (await menuState(`window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));`)) === 'closed'
  )

  say('3. plus, then click somewhere else')
  check('the menu opens again', (await menuState(openMenu)) === 'open')
  check(
    'clicking the thread closes it',
    (await menuState(`(document.querySelector('.lc-thread') || document.body).dispatchEvent(new PointerEvent('pointerdown', { bubbles: true }));`)) === 'closed'
  )

  say('4. plus, then change screen')
  check('the menu opens a third time', (await menuState(openMenu)) === 'open')
  check(
    'opening Settings closes it',
    (await menuState(`const s = [...document.querySelectorAll('button')].find(b => /^Settings/.test(b.getAttribute('aria-label') || b.getAttribute('title') || '')); if (s) { s.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true })); s.click(); }`)) === 'closed'
  )

  const shot = await cdp.send('Page.captureScreenshot', { format: 'png' })
  const out = new URL('../docs/chain-measure/first-five-2026-09-17.png', import.meta.url)
  await writeFile(out, Buffer.from(shot.result.data, 'base64'))
  say(`   screenshot ${out.pathname.slice(1)}`)
} finally {
  child.kill()
  await sleep(500)
  await rm(profile, { recursive: true, force: true }).catch(() => undefined)
}
console.error(failures === 0 ? 'FIRST-FIVE DRIVE PASSED' : `${String(failures)} check(s) failed`)
process.exitCode = failures === 0 ? 0 : 1
