// The pass nobody had done (design agent's punch-list, 2026-09-19, last line):
//
//   node _tools/crowded-window-drive.mjs --fixture <ledger.jsonl> --workspace <original-folder>
//
// 1120x720 -- the smallest window Locust allows -- with everything open at
// once: a conversation eight turns long, a room of eight teammates in the
// sidebar, and the inspector. Clipping shows here if it is not uniformly
// handled, and no drive had ever put all three on one screen.
//
// Seeds a chain of eight turns from one real ledger (only ids, prompts and
// `continuesFrom` differ), eight teammates and one room holding all of them,
// and a saved window of 1120x720. Opens the conversation, opens Activity,
// captures a frame, then asks the layout engine what a person would see:
// nothing scrolls sideways, no text spills past its own box, the composer is
// whole, and the top band is the header, not the thread. No provider run.

import '../_tools/scratch-root.mjs'

import { spawn } from 'node:child_process'
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { requiredPathArgument } from './required-path-argument.mjs'
import { localPath } from './local-path.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const PORT = 9493
const FIXTURE = requiredPathArgument('--fixture')
const ORIGINAL = 'mission_53ff0015-2289-44c5-9829-9e480926f48c'
// The folder whose id the fixture carries; the sidebar lists only this folder's work.
const WORKSPACE = requiredPathArgument('--workspace')
const TURNS = 8
// `--layout wide` forces the full sidebar at 1120 (a person's setting);
// `compact`, the default, is what `auto` resolves to at this width: the 64px
// avatar rail, where conversations open from a teammate's flyout.
const LAYOUT = process.argv.includes('--layout') ? process.argv[process.argv.indexOf('--layout') + 1] : 'compact'
if (LAYOUT !== 'compact' && LAYOUT !== 'wide') throw new Error('--layout compact|wide')
const FRAME = new URL(`../docs/chain-measure/crowded-window-${LAYOUT}-2026-09-19.png`, import.meta.url)

const say = (line) => console.error(line)
let failures = 0
const check = (label, ok, detail) => {
  if (!ok) failures += 1
  console.error(`   ${ok ? 'ok  ' : 'FAIL'} ${label}${detail === undefined ? '' : ' -- ' + detail}`)
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

const profile = await mkdtemp(join(tmpdir(), 'locust-crowded-'))
await mkdir(join(profile, 'mission-ledger'), { recursive: true })

// Eight teammates, every role the store knows and one Custom with a title.
const hex24 = (n) => (String(n) + 'a'.repeat(24)).slice(0, 24)
const ROLES = ['Code & Migrations', 'Research & Briefs', 'Ops & Scheduling', 'Docs & QA', 'Data & Reporting', 'Chief of Staff', 'Custom', 'Code & Migrations']
const NAMES = ['Wren', 'Atlas', 'Marlow', 'Quill', 'Sable', 'Bartholomew Longname', 'Pip', 'Juniper']
const HUES = ['lime', 'blue', 'violet', 'clay']
const teammates = NAMES.map((name, i) => ({
  teammateId: 'tm_' + hex24(i + 1),
  name,
  hue: HUES[i % HUES.length],
  role: ROLES[i],
  ...(ROLES[i] === 'Custom' ? { roleTitle: 'Release Captain' } : {}),
  createdAt: '2026-09-14T02:00:00.000Z'
}))
// UUID-shaped, like the ids the app mints: a chain whose ids were not
// ('mission_crowded-1...') read as one conversation and seven unreadable.
const ids = Array.from({ length: TURNS }, (_, i) => `mission_c0000000-0000-4000-8000-00000000000${String(i + 1)}`)
// The chain belongs to the first teammate, so the rail's flyout lists it.
const missionOwners = Object.fromEntries(ids.map((id) => [id, teammates[0].teammateId]))
await writeFile(
  join(profile, 'teammates.json'),
  JSON.stringify({ schemaVersion: 1, teammates, missionOwners, settings: { swarm: false, relay: false, autoMode: false, layout: LAYOUT === 'wide' ? 'wide' : 'auto' } })
)
await writeFile(
  join(profile, 'rooms.json'),
  JSON.stringify({
    schemaVersion: 1,
    rooms: [{ roomId: 'room_' + 'c'.repeat(20), name: 'Release', teammateIds: teammates.map((t) => t.teammateId), createdAt: '2026-09-14T02:30:00.000Z', posts: [], tasks: [] }]
  })
)
// The smallest window the app allows, which is the one the design agent named.
await writeFile(join(profile, 'window.json'), JSON.stringify({ x: 0, y: 0, width: 1120, height: 720, maximized: false }))

// A chain of eight turns built from one real ledger so every record is one
// this build accepts. The prompts get longer down the chain so the thread
// carries both short and wrapped person-bubbles.
const source = await readFile(FIXTURE, 'utf8')
const NEWLINE = String.fromCharCode(10)
const PROMPTS = [
  'Read the README and tell me what this folder is for.',
  'Now write a one-line note file beside it.',
  'What did the last step change?',
  'Give me the same in a table with the file, the size, and when it was written, and say whether anything in the folder looks like it should not be committed.',
  'Rename the note.',
  'Now check the rename took, list the folder again, and confirm the old name is gone from every place it was referenced including any index or manifest you can find here.',
  'And the version?',
  'Last one: summarise everything you did in this conversation as a changelog entry a stranger could read, with one line per turn, and tell me which of those lines you are least sure about.'
]
for (let i = 0; i < TURNS; i += 1) {
  const out = []
  for (const line of source.split(NEWLINE)) {
    if (line.trim().length === 0) continue
    const record = JSON.parse(line.split(ORIGINAL).join(ids[i]))
    if (record.recordType === 'mission.created') {
      record.metadata.prompt = PROMPTS[i]
      if (i > 0) record.metadata.continuesFrom = { missionId: ids[i - 1], checkpointEpoch: 1, reason: 'follow-up' }
    }
    // The timestamps stay the fixture's own: shifting `occurredAt` per turn
    // made every header after the first invalid (the ledger checks the
    // header against its metadata), and the chain orders by `continuesFrom`,
    // not by time.
    out.push(JSON.stringify(record))
  }
  await writeFile(join(profile, 'mission-ledger', `${ids[i]}.jsonl`), out.join(NEWLINE) + NEWLINE)
}

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
  await sleep(1500)

  say('1. the window')
  const size = JSON.parse(await cdp.eval(`JSON.stringify({ w: window.innerWidth, h: window.innerHeight, ratio: window.devicePixelRatio })`))
  say(`   ${JSON.stringify(size)}`)
  check('the window is 1120x720, the smallest allowed', size.w === 1120 && size.h === 720, JSON.stringify(size))

  say('1b. the first screen at this height: the lockup must be reachable')
  const firstScreen = JSON.parse(await cdp.eval(`(async () => {
    await new Promise(r => setTimeout(r, 2500))
    const pane = document.querySelector('.lc-empty')
    const card = document.querySelector('.lc-cover')
    if (!pane || !card) return JSON.stringify({ pane: !!pane, card: !!card })
    const top = (el) => Math.round(el.getBoundingClientRect().top)
    const install = document.querySelector('.lc-runtimecell__install, .lc-runtimecell')
    const before = { overflow: pane.scrollHeight - pane.clientHeight, scrollTop: pane.scrollTop, cardTop: top(card), listTop: install ? top(install) : null }
    pane.scrollTop = 0
    await new Promise(r => setTimeout(r, 200))
    const atTop = { cardTop: top(card) }
    pane.scrollTop = pane.scrollHeight
    await new Promise(r => setTimeout(r, 200))
    return JSON.stringify({ before, atTop, titlebarBottom: 38 })
  })()`))
  say(`   ${JSON.stringify(firstScreen)}`)
  if (firstScreen.before !== undefined) {
    check('the lockup can be brought fully into view (its top clears the title bar when scrolled up)', firstScreen.atTop.cardTop >= 38, JSON.stringify(firstScreen))
    check('and the pane opens at its end, where Install and the composer are', firstScreen.before.overflow === 0 || firstScreen.before.scrollTop >= firstScreen.before.overflow - 2, JSON.stringify(firstScreen))
  }

  say(`2. the sidebar (${LAYOUT}): a room of eight, eight teammates, one conversation`)
  const sidebar = JSON.parse(await cdp.eval(`(async () => {
    await new Promise(r => setTimeout(r, 1500))
    const side = document.querySelector('.lc-sidebar') || document.body
    const text = side.innerText.replace(new RegExp('[' + String.fromCharCode(32, 9, 13, 10) + ']+', 'g'), ' ')
    const names = ${JSON.stringify(NAMES)}
    const compact = !!document.querySelector('.lc-shell.is-compact')
    return JSON.stringify({
      compact,
      width: Math.round(side.getBoundingClientRect().width),
      teammatesNamed: names.filter(n => text.indexOf(n.split(' ')[0]) >= 0).length,
      // The wide sidebar draws a faces row: some faces and a "+N" for the rest.
      faces: document.querySelectorAll('.lc-faces__one').length + (parseInt(((document.querySelector('.lc-faces__more') || {}).innerText || '0').replace(/[^0-9]/g, ''), 10) || 0),
      railSlots: document.querySelectorAll('.lc-railslot').length,
      // On the 720-tall rail, every face has to be on screen (Grok, pass 13,
      // saw seven of eight).
      railVisible: [...document.querySelectorAll('.lc-railslot')].filter(s => { const r = s.getBoundingClientRect(); return r.top >= 38 && r.bottom <= window.innerHeight }).length,
      unreadable: (document.querySelector('.lc-sidebar__unreadable-happened') || {}).innerText || null,
      room: /Release/.test(text) || [...side.querySelectorAll('[aria-label], [title]')].some(el => /Release/.test((el.getAttribute('aria-label') || '') + (el.getAttribute('title') || ''))),
      convrows: document.querySelectorAll('.lc-convrow').length
    })
  })()`))
  say(`   ${JSON.stringify(sidebar)}`)
  check(`the shell is ${LAYOUT} at 1120`, sidebar.compact === (LAYOUT === 'compact'), JSON.stringify(sidebar))
  if (LAYOUT === 'compact') {
    check('the rail carries eight teammate slots', sidebar.railSlots === 8, JSON.stringify(sidebar))
    check('and all eight faces are on the 720-tall rail', sidebar.railVisible === 8, JSON.stringify(sidebar))
  } else {
    check('the faces row accounts for all eight teammates', sidebar.faces === 8, JSON.stringify(sidebar))
    check('the eight-turn chain is one conversation row', sidebar.convrows === 1, JSON.stringify(sidebar))
  }
  check('every seeded turn was readable', sidebar.unreadable === null, JSON.stringify(sidebar.unreadable))

  say('3. open the conversation, then Activity')
  const opened = JSON.parse(await cdp.eval(`(async () => {
    let row = null
    if (${JSON.stringify(LAYOUT)} === 'wide') {
      const title = document.querySelector('.lc-convrow .lc-conv__title')
      row = title ? title.closest('button') : null
    } else {
      // The rail: click the first teammate's slot to pin their flyout, then
      // the one conversation it lists.
      const slot = document.querySelector('.lc-railslot')
      if (!slot) return JSON.stringify({ opened: false, why: 'no rail slot' })
      // The face inside the slot is the button that pins; the slot itself only hovers.
      const face = slot.querySelector('button')
      if (!face) return JSON.stringify({ opened: false, why: 'no face button in the slot' })
      face.click()
      for (let i = 0; i < 40 && !row; i += 1) {
        await new Promise(r => setTimeout(r, 250))
        row = document.querySelector('.lc-railflyout__row')
      }
      if (!row) return JSON.stringify({ opened: false, why: 'no flyout row', flyout: !!document.querySelector('.lc-railflyout') })
    }
    if (!row) return JSON.stringify({ opened: false })
    row.click()
    let bubbles = 0
    for (let i = 0; i < 80; i += 1) {
      await new Promise(r => setTimeout(r, 250))
      bubbles = document.querySelectorAll('.lc-thread .lc-bubble').length
      if (bubbles >= ${TURNS}) break
    }
    const activity = [...document.querySelectorAll('button')].find(b => /Activity/.test(b.innerText || ''))
    if (!activity) return JSON.stringify({ opened: true, bubbles, activity: false })
    activity.click()
    let inspector = null
    for (let i = 0; i < 40; i += 1) {
      await new Promise(r => setTimeout(r, 250))
      inspector = document.querySelector('.lc-inspector')
      if (inspector) break
    }
    // The thread at its end, the way it is when a person has been typing.
    const thread = document.querySelector('.lc-thread')
    if (thread) thread.scrollTop = thread.scrollHeight
    await new Promise(r => setTimeout(r, 600))
    return JSON.stringify({ opened: true, bubbles, activity: true, inspector: !!inspector })
  })()`))
  say(`   ${JSON.stringify(opened)}`)
  check('the conversation opened with all eight turns', opened.opened === true && opened.bubbles >= TURNS, JSON.stringify(opened))
  check('the inspector is open', opened.inspector === true, JSON.stringify(opened))

  say('4. what the layout engine says a person sees')
  const layout = JSON.parse(await cdp.eval(`(() => {
    const ws = new RegExp('[' + String.fromCharCode(32, 9, 13, 10) + ']+', 'g')
    const clean = (s) => (s || '').replace(ws, ' ').trim()
    const box = (sel) => {
      const el = document.querySelector(sel)
      if (!el) return null
      const r = el.getBoundingClientRect()
      return { left: Math.round(r.left), right: Math.round(r.right), top: Math.round(r.top), bottom: Math.round(r.bottom), width: Math.round(r.width), scrollW: el.scrollWidth, clientW: el.clientWidth }
    }
    // Text that spills past its own box with nothing clipping it: the
    // crowding defect a person sees as words on top of other words.
    const spills = []
    for (const el of document.querySelectorAll('body *')) {
      if (el.children.length > 0 && !/^(BUTTON|A|SPAN|LABEL|TD|TH)$/.test(el.tagName)) continue
      const text = clean(el.innerText)
      if (!text) continue
      const r = el.getBoundingClientRect()
      if (r.width === 0 || r.height === 0 || r.bottom < 0 || r.top > window.innerHeight) continue
      const s = getComputedStyle(el)
      if (el.scrollWidth > el.clientWidth + 1 && s.overflowX === 'visible' && s.whiteSpace !== 'normal' && s.whiteSpace !== 'pre-wrap' && s.whiteSpace !== 'pre-line' && s.whiteSpace !== 'break-spaces') {
        spills.push({ cls: (el.className || el.tagName).toString().slice(0, 50), text: text.slice(0, 40), over: el.scrollWidth - el.clientWidth })
      }
      if (r.right > window.innerWidth + 1) spills.push({ cls: (el.className || el.tagName).toString().slice(0, 50), text: text.slice(0, 40), pastWindow: Math.round(r.right - window.innerWidth) })
    }
    const at = (x, y) => {
      const el = document.elementFromPoint(x, y)
      return el ? (el.className || el.tagName).toString().split(' ')[0] : 'nothing'
    }
    const columns = { left: 120, middle: Math.round(window.innerWidth * 0.5), right: window.innerWidth - 120 }
    const probes = {}
    for (const [name, x] of Object.entries(columns)) probes[name] = [8, 40, 70, 110, 200, 360, 520, 640, 700].map(y => y + ':' + at(x, y))
    return JSON.stringify({
      docScrollW: document.documentElement.scrollWidth,
      sidebar: box('.lc-sidebar'),
      thread: box('.lc-thread'),
      column: box('.lc-thread__column'),
      inspector: box('.lc-inspector'),
      composer: box('.lc-composer textarea, .lc-composer'),
      controls: box('.lc-composer__controls'),
      header: box('.lc-workroom__header'),
      activity: box('.lc-workroom__header .lc-button[aria-pressed]'),
      banner: box('.lc-changelog, .lc-banner'),
      // The last thing a person would read: the newest reply's right edge.
      lastBubble: (() => { const all = document.querySelectorAll('.lc-thread .lc-bubble, .lc-thread .lc-agent-message, .lc-thread .lc-para'); const el = all[all.length - 1]; if (!el) return null; const r = el.getBoundingClientRect(); return { right: Math.round(r.right), text: clean(el.innerText).slice(0, 40) } })(),
      spills,
      probes
    }, null, 1)
  })()`))
  say('   ' + JSON.stringify({ docScrollW: layout.docScrollW, sidebar: layout.sidebar, thread: layout.thread, column: layout.column, inspector: layout.inspector, composer: layout.composer }))
  say('   probes ' + JSON.stringify(layout.probes))
  say('   spills ' + JSON.stringify(layout.spills))
  check('nothing scrolls sideways', layout.docScrollW <= 1120, String(layout.docScrollW))
  for (const [name, part] of [['sidebar', layout.sidebar], ['thread', layout.thread], ['inspector', layout.inspector]]) {
    check(`${name} does not scroll sideways`, part !== null && part.scrollW <= part.clientW + 1, JSON.stringify(part))
  }
  check('the composer is whole, inside the window', layout.composer !== null && layout.composer.left >= 0 && layout.composer.right <= 1120 && layout.composer.bottom <= 720, JSON.stringify(layout.composer))
  check('the thread column has room to read (at least 360px)', layout.column !== null && layout.column.width >= 360, JSON.stringify(layout.column))
  check('the inspector is not squeezed below 240px', layout.inspector !== null && layout.inspector.width >= 240, JSON.stringify(layout.inspector))
  check('no text spills past its box or the window', layout.spills.length === 0, JSON.stringify(layout.spills))
  /*
   * THE INSPECTOR MUST NOT COVER WHAT IT INSPECTS. In the compact shell it
   * is a drawer over the content (absolute, z-index 40); on the first run
   * of this drive the thread column stayed centred in the full width and
   * the drawer covered its right 160px: every reply's line end, the
   * receipt, the banner's Dismiss, the header's own Activity button and
   * the composer's route chip (frame, 2026-09-19). Everything a person
   * reads or presses must end left of the drawer's edge.
   */
  const edge = layout.inspector === null ? 1120 : layout.inspector.left
  check('the thread column ends left of the inspector', layout.column !== null && layout.column.right <= edge + 1, JSON.stringify({ column: layout.column, edge }))
  check('the header’s Activity button is not under the inspector', layout.activity !== null && layout.activity.right <= edge + 1, JSON.stringify({ activity: layout.activity, edge }))
  check('the composer’s controls end left of the inspector and inside their row', layout.controls !== null && layout.controls.right <= edge + 1 && layout.controls.scrollW <= layout.controls.clientW + 1, JSON.stringify({ controls: layout.controls, edge }))
  if (layout.banner !== null) check('the changelog banner ends left of the inspector', layout.banner.right <= edge + 1, JSON.stringify({ banner: layout.banner, edge }))
  if (layout.lastBubble !== null) check('the newest reply ends left of the inspector', layout.lastBubble.right <= edge + 1, JSON.stringify({ last: layout.lastBubble, edge }))
  const middleTop = layout.probes.middle.slice(0, 3)
  check('the top band in the middle column is the header, not the thread', middleTop.every((p) => !/lc-bubble|lc-thread/.test(p)), JSON.stringify(middleTop))

  await sleep(400)
  const shot = await cdp.send('Page.captureScreenshot', { format: 'png' })
  await writeFile(FRAME, Buffer.from(shot.result.data, 'base64'))
  say(`   frame ${FRAME.pathname.slice(1)}`)

  /*
   * 5. THE OTHER SCREENS AT THIS SIZE. Nobody had looked at All missions,
   * the room, Routines, Settings or the Team screen at 1120x720 either. Each
   * is opened the way a person opens it (the sidebar's own control, by its
   * title), audited the same way (nothing sideways, nothing spilling), and
   * captured, so the frames can be looked at.
   */
  say('5. the other screens at 1120x720')
  const audit = `(() => {
    const ws = new RegExp('[' + String.fromCharCode(32, 9, 13, 10) + ']+', 'g')
    const clean = (s) => (s || '').replace(ws, ' ').trim()
    const spills = []
    for (const el of document.querySelectorAll('body *')) {
      if (el.children.length > 0 && !/^(BUTTON|A|SPAN|LABEL|TD|TH)$/.test(el.tagName)) continue
      const text = clean(el.innerText)
      if (!text) continue
      const r = el.getBoundingClientRect()
      if (r.width === 0 || r.height === 0 || r.bottom < 0 || r.top > window.innerHeight) continue
      const s = getComputedStyle(el)
      if (el.scrollWidth > el.clientWidth + 1 && s.overflowX === 'visible' && s.whiteSpace !== 'normal' && s.whiteSpace !== 'pre-wrap' && s.whiteSpace !== 'pre-line' && s.whiteSpace !== 'break-spaces') {
        spills.push({ cls: (el.className || el.tagName).toString().slice(0, 50), text: text.slice(0, 40), over: el.scrollWidth - el.clientWidth })
      }
      if (r.right > window.innerWidth + 1) spills.push({ cls: (el.className || el.tagName).toString().slice(0, 50), text: text.slice(0, 40), pastWindow: Math.round(r.right - window.innerWidth) })
    }
    // A scroll container that scrolls sideways is content cut at the edge.
    const sideways = []
    for (const el of document.querySelectorAll('main *')) {
      const s = getComputedStyle(el)
      if ((s.overflowX === 'auto' || s.overflowX === 'scroll') && el.scrollWidth > el.clientWidth + 1 && !/lc-diff|lc-code|pre|lc-mono/.test(el.className || '')) {
        // Name what pokes out, so the fix has an address.
        const edge = el.getBoundingClientRect().right
        const culprits = [...el.querySelectorAll('*')].filter(d => d.getBoundingClientRect().right > edge + 1 && d.getBoundingClientRect().width > 0).slice(0, 4).map(d => (d.className || d.tagName).toString().split(' ')[0] + '+' + Math.round(d.getBoundingClientRect().right - edge))
        sideways.push({ cls: (el.className || el.tagName).toString().slice(0, 50), over: el.scrollWidth - el.clientWidth, culprits })
      }
    }
    return JSON.stringify({ docScrollW: document.documentElement.scrollWidth, spills, sideways, heading: clean((document.querySelector('main h1, main h2, .lc-screen__title') || {}).innerText).slice(0, 40) })
  })()`
  const screens = [
    ['missions', 'All missions'],
    ['rooms', 'Rooms'],
    ['room', null],
    ['routines', 'Routines'],
    ['settings', 'Settings'],
    ['team', 'Team']
  ]
  for (const [name, titleStart] of screens) {
    const went = await cdp.eval(`(async () => {
      let control = null
      if (${JSON.stringify(titleStart)} === null) {
        // The room itself: the row named Release on the Rooms screen.
        control = [...document.querySelectorAll('main button, main [role=button]')].find(b => /^Release/.test((b.innerText || '').trim()))
      } else {
        control = [...document.querySelectorAll('.lc-sidebar button')].find(b => (b.getAttribute('title') || '').indexOf(${JSON.stringify(titleStart)}) === 0)
      }
      if (!control) return 'no control'
      control.click()
      await new Promise(r => setTimeout(r, 900))
      return 'opened'
    })()`)
    if (went !== 'opened') {
      say(`   ${name}: ${went}`)
      if (name !== 'team') check(`the ${name} screen can be opened from the sidebar`, false, went)
      continue
    }
    const seen = JSON.parse(await cdp.eval(audit))
    say(`   ${name}: ${JSON.stringify(seen)}`)
    if (name === 'missions') {
      // The row's shape at this width: its computed columns and whether the
      // title is drawn with any width at all.
      const row = JSON.parse(await cdp.eval(`(() => {
        const rows = document.querySelector('.lc-missionrows')
        const row = document.querySelector('.lc-missionrow')
        const title = document.querySelector('.lc-missionrow__title')
        if (!rows || !row || !title) return JSON.stringify({ rows: !!rows, row: !!row, title: !!title })
        const w = (el) => Math.round(el.getBoundingClientRect().width)
        return JSON.stringify({ listW: w(rows), rowW: w(row), columns: getComputedStyle(row).gridTemplateColumns, titleW: w(title), title: (title.innerText || '').slice(0, 30) })
      })()`))
      say(`   missions row: ${JSON.stringify(row)}`)
      check('the mission title has room to be read (at least 120px)', row.titleW >= 120, JSON.stringify(row))
    }
    check(`${name}: nothing scrolls sideways`, seen.docScrollW <= 1120 && seen.sideways.length === 0, JSON.stringify(seen.sideways))
    check(`${name}: no text spills past its box or the window`, seen.spills.length === 0, JSON.stringify(seen.spills))
    const frame = new URL(`../docs/chain-measure/crowded-window-${LAYOUT}-${name}-2026-09-19.png`, import.meta.url)
    const png = await cdp.send('Page.captureScreenshot', { format: 'png' })
    await writeFile(frame, Buffer.from(png.result.data, 'base64'))
  }
} finally {
  child.kill()
  await sleep(800)
  await rm(profile, { recursive: true, force: true }).catch(() => undefined)
}
console.error(failures === 0 ? 'CROWDED WINDOW DRIVE PASSED' : `${String(failures)} check(s) failed`)
process.exitCode = failures === 0 ? 0 : 1
