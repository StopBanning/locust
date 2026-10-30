// Putting back one turn of a conversation puts the CONVERSATION back.
//
//   node _tools/trash-chain-drive.mjs --fixture <ledger.jsonl> --workspace <original-folder>
//
// Grok, pass 9, 2026-09-18, on 0.177.0: they seeded a two-turn chain, deleted
// only the first turn, and put it back. The sidebar then showed TWO rows for
// one conversation until the app was restarted. Their words: a person who
// deleted the wrong turn of a long thread and pressed Put back "will see two
// conversations and think restore duplicated it" -- and then tidy up the
// duplicate, which is how they lose the other half for good.
//
// Everything else about trash held. This is the one place where undo does not
// finish the job it started.
//
// No provider run; costs nothing.

import '../_tools/scratch-root.mjs'

import { spawn } from 'node:child_process'
import { mkdtemp, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { requiredPathArgument } from './required-path-argument.mjs'
import { localPath } from './local-path.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const FIXTURE = requiredPathArgument('--fixture')
// The folder whose id the fixture already carries. The sidebar lists THIS
// folder's work, so a seeded mission stamped with another workspace is
// filtered out and the drive would pass by drawing nothing.
const WORKSPACE = requiredPathArgument('--workspace')
const PORT = 9487

const PARENT = 'mission_11111111-1111-4111-8111-111111111111'
const CHILD = 'mission_22222222-2222-4222-8222-222222222222'

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

const profile = await mkdtemp(join(tmpdir(), 'locust-chain-'))
await mkdir(join(profile, 'mission-ledger'), { recursive: true })
await writeFile(
  join(profile, 'teammates.json'),
  JSON.stringify({ schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, autoMode: false } })
)

/*
 * A two-turn chain, built from one real ledger rather than hand-written, so
 * every record is one this build actually accepts. Only the ids, the prompt
 * and -- for the second turn -- `continuesFrom` differ.
 */
const source = await readFile(FIXTURE, 'utf8')
const ORIGINAL = 'mission_4ac0d5fd-b688-4f04-8222-624a155225dd'
const turn = (missionId, prompt, continuesFrom) => {
  const out = []
  const NEWLINE = String.fromCharCode(10)
  for (const line of source.split(NEWLINE)) {
    if (line.trim().length === 0) continue
    const record = JSON.parse(line.split(ORIGINAL).join(missionId))
    if (record.recordType === 'mission.created') {
      record.metadata.prompt = prompt
      if (continuesFrom !== undefined) record.metadata.continuesFrom = continuesFrom
    }
    out.push(JSON.stringify(record))
  }
  return out.join(NEWLINE) + NEWLINE
}
await writeFile(join(profile, 'mission-ledger', `${PARENT}.jsonl`), turn(PARENT, 'alpha parent turn'))
await writeFile(
  join(profile, 'mission-ledger', `${CHILD}.jsonl`),
  turn(CHILD, 'alpha follow-up turn', { missionId: PARENT, checkpointEpoch: 1, reason: 'follow-up' })
)

const child = spawn(ELECTRON, [APP_DIR, `--remote-debugging-port=${PORT}`, `--user-data-dir=${profile}`], {
  cwd: WORKSPACE,
  env: { ...process.env },
  stdio: ['ignore', 'pipe', 'pipe']
})
const appOutput = []
child.stdout.on('data', (d) => appOutput.push(String(d)))
child.stderr.on('data', (d) => appOutput.push(String(d)))

const trashNames = async () =>
  (await readdir(join(profile, 'mission-ledger', '.trash')).catch(() => [])).filter((n) => n.endsWith('.jsonl'))
const liveNames = async () =>
  (await readdir(join(profile, 'mission-ledger')).catch(() => [])).filter((n) => n.endsWith('.jsonl'))

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
  await sleep(2500)

  const rows = async () => cdp.eval(`(async () => {
    const items = [...document.querySelectorAll('.lc-convrow')]
    return JSON.stringify(items.map(el => ({
      title: (el.querySelector('.lc-conv__title') || {}).innerText || null,
      text: (el.innerText || '').replace(new RegExp('[' + String.fromCharCode(32, 9, 13, 10) + ']+', 'g'), ' ').trim().slice(0, 50)
    })))
  })()`)

  const seeded = await rows()
  say(`   sidebar ${seeded}`)
  const seededRows = JSON.parse(seeded)
  check('the chain is one row before anything is deleted', seededRows.length === 1, seeded)

  /*
   * THROUGH THE CONTROLS A PERSON USES, not the bridge underneath them.
   *
   * The first version of this drive called `window.desktop.deleteMission`
   * and `restoreMission` directly. The files moved, every disk check passed,
   * and the sidebar never redrew once -- so "one row before, one row after"
   * was true of a screen that had not been asked a question. Calling the
   * bridge skips every piece of state the app keeps, which is exactly where
   * the reported defect lives.
   */
  say('1. delete only the first turn, from All missions')
  const deleted = await cdp.eval(`(async () => {
    const open = [...document.querySelectorAll('button')].find(b => /All missions|Missions/.test(b.innerText || b.getAttribute('aria-label') || ''))
    if (!open) return JSON.stringify({ step: 'no way to open All missions' })
    open.click()
    await new Promise(r => setTimeout(r, 900))
    const rows = [...document.querySelectorAll('.lc-missionrow, [class*=missionrow]')]
    const boxes = [...document.querySelectorAll('.lc-missionrow__pick')]
    const labels = boxes.map(b => b.getAttribute('aria-label') || '')
    const wanted = boxes.find(b => /alpha parent turn/.test(b.getAttribute('aria-label') || ''))
    if (!wanted) return JSON.stringify({ step: 'no checkbox for the parent', rows: rows.length, labels })
    wanted.click()
    await new Promise(r => setTimeout(r, 500))
    const press = () => {
      const button = document.querySelector('.lc-pickbar__delete')
      if (!button) return null
      const words = button.innerText
      button.click()
      return words
    }
    const once = press()
    await new Promise(r => setTimeout(r, 400))
    const twice = press()
    await new Promise(r => setTimeout(r, 1200))
    return JSON.stringify({ step: 'pressed', labels, once, twice })
  })()`)
  say(`   ${deleted}`)
  const how = JSON.parse(deleted)
  check('All missions offered the parent to delete', how.step === 'pressed', deleted)
  check('the confirm asked before deleting', /for good/.test(how.twice ?? ''), deleted)
  await sleep(1200)
  check('the parent is in the trash on disk', (await trashNames()).some((n) => n.includes('11111111')), (await trashNames()).join(','))
  check('and gone from the live folder', !(await liveNames()).some((n) => n.includes('11111111')), (await liveNames()).join(','))

  /*
   * THE CONTROL, and it is the whole test.
   *
   * The chain has to be seen BROKEN -- one row, named by the surviving turn,
   * because its first turn is gone -- for the row after the restore to mean
   * that the collapse happened again rather than that nothing ever moved.
   */
  const backToSidebar = async () => cdp.eval(`(async () => {
    const home = [...document.querySelectorAll('button')].find(b => /^(Home|Workroom)/.test(b.innerText || b.getAttribute('aria-label') || ''))
    if (home) home.click()
    await new Promise(r => setTimeout(r, 800))
    return 'ok'
  })()`)
  await backToSidebar()
  const between = await rows()
  say(`   sidebar ${between}`)
  const betweenRows = JSON.parse(between)
  check('the screen redrew: one row for the turn that is left', betweenRows.length === 1, between)
  check(
    'and it is named by that turn, not by the deleted one',
    betweenRows[0] !== undefined && /alpha follow-up turn/.test(betweenRows[0].title ?? ''),
    between
  )

  /*
   * THE STEP THE FIRST VERSION SKIPPED, and the one that reproduces it.
   *
   * Grok, pass 10: open the surviving turn before putting the first one
   * back. A person does exactly this -- they check what is left before they
   * undo. Opening puts a live run in the sidebar's state beside the record,
   * and that run remembers having no parent; after the restore, the record
   * has two turns and the run still says one, so the sidebar drew two rows.
   * The controls-only drive on Windows passed because it never opened.
   */
  say('1b. open the surviving turn, the way a person checks what is left')
  const opened = await cdp.eval(`(async () => {
    // The button that carries the title is the one that opens; the other
    // button on the row is its menu.
    const title = document.querySelector('.lc-convrow .lc-conv__title')
    const row = title ? title.closest('button') : null
    if (!row) return JSON.stringify({ opened: false, why: 'no row button' })
    row.click()
    // The thread itself, not the row: a click that highlighted a row and
    // opened nothing would leave the suspect -- a live run in the sidebar's
    // state -- out of the drive, and the drive would pass on the old code.
    let thread = null
    for (let i = 0; i < 16 && !thread; i += 1) {
      await new Promise(r => setTimeout(r, 500))
      thread = document.querySelector('.lc-thread, .lc-thread__column, .lc-bubble')
    }
    const clean = (s) => (s || '').replace(new RegExp('[' + String.fromCharCode(32, 9, 13, 10) + ']+', 'g'), ' ').trim()
    const main = document.querySelector('.lc-workroom, main')
    return JSON.stringify({ opened: !!thread, thread: clean(thread && thread.innerText).slice(0, 80), main: clean(main && main.innerText).slice(0, 120) })
  })()`)
  say(`   ${opened}`)
  const openedThread = JSON.parse(opened)
  check('the surviving turn is open in the workroom', openedThread.opened === true && /alpha follow-up turn/.test(openedThread.thread), opened)

  say('2. put it back from Settings, WITHOUT restarting')
  const restored = await cdp.eval(`(async () => {
    const settings = [...document.querySelectorAll('button')].find(b => /^Settings/.test(b.getAttribute('aria-label') || b.getAttribute('title') || ''))
    if (settings) settings.click()
    await new Promise(r => setTimeout(r, 900))
    const item = [...document.querySelectorAll('.lc-settings__navitem')].find(b => /^General$/.test(b.innerText.trim()))
    if (item) item.click()
    await new Promise(r => setTimeout(r, 700))
    const back = [...document.querySelectorAll('button')].find(b => /Put back/.test(b.innerText))
    if (!back) return JSON.stringify({ step: 'no Put back control' })
    back.click()
    await new Promise(r => setTimeout(r, 1500))
    return JSON.stringify({ step: 'pressed' })
  })()`)
  say(`   ${restored}`)
  check('Trash offered Put back', JSON.parse(restored).step === 'pressed', restored)
  await sleep(1500)
  check('the file is back in the live folder', (await liveNames()).some((n) => n.includes('11111111')), (await liveNames()).join(','))

  await backToSidebar()
  const after = await rows()
  say(`   sidebar ${after}`)
  const afterRows = JSON.parse(after)
  // THE FINDING. Two rows here is Grok's report; one row is restore having
  // finished the job it started.
  check('the conversation is ONE row again, with no restart', afterRows.length === 1, after)
  check(
    'and it is named by the turn that was put back',
    afterRows[0] !== undefined && /alpha parent turn/.test(afterRows[0].title ?? ''),
    after
  )

  const shot = await cdp.send('Page.captureScreenshot', { format: 'png' })
  const out = new URL('../docs/chain-measure/trash-chain-2026-09-18.png', import.meta.url)
  await writeFile(out, Buffer.from(shot.result.data, 'base64'))
  say(`   screenshot ${out.pathname.slice(1)}`)
} finally {
  child.kill()
  await sleep(500)
  await rm(profile, { recursive: true, force: true }).catch(() => undefined)
}
console.error(failures === 0 ? 'TRASH CHAIN DRIVE PASSED' : `${String(failures)} check(s) failed`)
process.exitCode = failures === 0 ? 0 : 1
