// Frames of a seeded compare with a long live line in one column (no model).
//
//   node _tools/drive-compare-live-line-frames.mjs [--tag <name>] [--columns 2|3] [--packaged <Locust.exe>]
//
// Seeds 2- and 3-column comparisons the way
// drive-an-old-comparison-shows-every-answer.mjs does, opens each, injects a
// long `.lc-livestep` into column B, and captures at 1200x780 and 1000x680.
// Tag the out folder so before/after can sit side by side. Sends nothing.
//
// THE INJECTED ROW IS THE COMPONENT'S MARKUP, kept by hand: LiveRegisterLine
// in ThreadItems.tsx draws the words, then the clock, then the note in its
// own box (0.631). If that markup changes, change INJECT with it, or this
// measures a row the app no longer draws. It FAILS when the words or the
// clock leave column B.

import { createHash } from 'node:crypto'
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const tag = arg('--tag') ?? 'local'
const only = arg('--columns') ?? 'both'
const packaged = arg('--packaged')
const LONG = 'Final regression playthroughs across classes and layouts'

const root = localPath(new URL('..', import.meta.url))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-compare-live-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-compare-live-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })

const TWO = [
  { slot: 'a', runtime: 'opencode', model: 'opencode/mimo-v2.6-flash-free', label: 'Mimo V2.6 Flash', answer: 'MIMO-ANSWER: two columns.' },
  { slot: 'b', runtime: 'opencode', model: 'opencode/ling-3.0-flash-fin-free', label: 'Ling 3.0 Flash', answer: 'LING-ANSWER: the middle column holds the live line.' }
]
const THREE = [
  ...TWO,
  { slot: 'c', runtime: 'opencode', model: 'opencode/muse-spark-1.3-contributor-free', label: 'Muse Spark 1.3', answer: 'MUSE-ANSWER: three columns.' }
]
const PROMPT_TWO = 'make a small browser RPG with an enrage aura (two models)'
const PROMPT_THREE = 'make a small browser RPG with an enrage aura (three models)'

let n = 0
async function turn(prompt, route, text, createdAt) {
  n += 1
  const missionId = `mission_5e100000-0000-4000-8000-0007${String(n).padStart(8, '0')}`
  const runId = `run_5e1007${String(n)}`
  await ledger.createMission({
    missionId, runId, prompt,
    runtime: route.runtime, model: route.model, requestedRouteId: 'opencode', resolvedRouteId: 'opencode-account:default', cliVersion: null,
    workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt
  })
  const ended = new Date(Date.parse(createdAt) + 20_000).toISOString()
  const event = (sequence, type, occurredAt, payload) => ({ id: `event_${String(n)}_${String(sequence)}`, runId, missionId, sequence, occurredAt, sourceAdapter: 'opencode', type, payload: { ...payload, evidence: { redacted: true } } })
  await ledger.appendEvents(missionId, [
    event(1, 'message.delta', createdAt, { itemId: 'answer', operation: 'append', text, final: true }),
    event(2, 'run.completed', ended, { usage: { inputTokens: 900, outputTokens: 200 }, process: { exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 2, inputDeliveryFailed: false, outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: createdAt, finishedAt: ended } })
  ])
  return missionId
}

const day = Date.now() - 86_400_000
async function seedCompare(compareId, columns, prompt) {
  const slots = []
  for (const column of columns) {
    const missionId = await turn(prompt, column, column.answer, new Date(day).toISOString())
    slots.push({ slot: column.slot, route: { runtime: column.runtime, model: column.model, label: column.label }, missionIds: [missionId] })
  }
  return { compareId, prompt, createdAt: new Date(day).toISOString(), slots }
}

const compares = []
if (only === 'both' || only === '2') compares.push(await seedCompare('cmp_live_two', TWO, PROMPT_TWO))
if (only === 'both' || only === '3') compares.push(await seedCompare('cmp_live_three', THREE, PROMPT_THREE))
await ledger.flush?.()
await writeFile(join(profilePath, 'compares.json'), JSON.stringify({ schemaVersion: 1, compares }), 'utf8')

const outPath = join(recordRoot('compare-live-line-frames-2026-10-05'), tag)
await mkdir(outPath, { recursive: true })

const drive = await startDrive({
  name: `compare-live-${tag}-${only}`,
  port: only === '3' ? 9892 : 9891,
  workspace,
  profilePath,
  sendsNothing: true,
  outPath,
  ...(packaged === undefined ? {} : { packaged }),
  seed: { schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})

const INJECT = `(() => {
  const words = ${JSON.stringify(LONG)}
  const cells = [...document.querySelectorAll('.lc-compare__cell')]
  const cell = cells[1] ?? cells[0]
  if (!cell) return 'no cell'
  cell.querySelectorAll('.lc-livestep[data-look="overflow"]').forEach((el) => el.remove())
  const row = document.createElement('div')
  row.className = 'lc-livestep'
  row.dataset.look = 'overflow'
  row.dataset.stepKind = 'item'
  row.dataset.register = 'tool'
  const gutter = document.createElement('span')
  gutter.className = 'lc-livestep__gutter'
  const label = document.createElement('span')
  label.className = 'lc-livestep__label'
  label.title = words
  const register = document.createElement('span')
  register.className = 'lc-livestep__register'
  const sweep = document.createElement('span')
  sweep.className = 'lc-sweep'
  sweep.dataset.text = words
  sweep.textContent = words
  register.appendChild(sweep)
  label.appendChild(register)
  const clock = document.createElement('span')
  clock.className = 'lc-rail__meta lc-livestep__clock'
  clock.textContent = '4s'
  const note = document.createElement('span')
  note.className = 'lc-rail__meta lc-livestep__note'
  const meta = document.createElement('span')
  meta.className = 'lc-livestep__meta'
  meta.textContent = 'npm test'
  note.appendChild(meta)
  row.append(gutter, label, clock, note)
  cell.prepend(row)
  return 'injected into cell ' + String(cells.indexOf(cell)) + '; labelWidth=' + String(Math.round(label.getBoundingClientRect().width)) + '; scrollWidth=' + String(label.scrollWidth)
})()`

async function openByNeedle(needle) {
  return JSON.parse(String(await drive.evaluate(`(async () => {
    const rows = [...document.querySelectorAll('.lc-convrow button.lc-conv')]
    const row = rows.find((r) => /${needle}/i.test(r.innerText)) ?? rows.find((r) => /enrage aura/i.test(r.innerText) && /${needle.split(' ')[0]}/i.test(r.innerText))
    const titles = rows.map((r) => r.innerText.replace(/\\s+/g, ' ').trim().slice(0, 80))
    row?.click()
    await new Promise((r) => setTimeout(r, 1200))
    ;[...document.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Open the comparison')?.click()
    for (let i = 0; i < 24 && !document.querySelector('.lc-compare'); i += 1) await new Promise((r) => setTimeout(r, 250))
    await new Promise((r) => setTimeout(r, 1500))
    return JSON.stringify({
      compare: !!document.querySelector('.lc-compare'),
      cells: document.querySelectorAll('.lc-compare__cell').length,
      row: row?.innerText?.replace(/\\s+/g, ' ').trim().slice(0, 100),
      titles
    })
  })()`)))
}

let failures = 0
try {
  await drive.capture('launch', () => drive.ready())
  const jobs = []
  if (only === 'both' || only === '2') jobs.push([2, 'two models', 'two'])
  if (only === 'both' || only === '3') jobs.push([3, 'three models', 'three'])
  for (const [wantCells, needle, label] of jobs) {
    // CHANGELOG 0.414.0: "In a narrow window, where the sidebar becomes a strip of
    // faces and its search box is hidden". Restore the wide conversation
    // list after the previous 1000px frame before opening the next fixture.
    await drive.resize(1200, 780)
    await sleep(400)
    const opened = await drive.capture(`open ${label}-column compare`, () => openByNeedle(needle))
    say(`  opened ${label}: ${JSON.stringify(opened)}`)
    if (!opened.compare || opened.cells !== wantCells) failures += 1
    const injected = await drive.evaluate(INJECT)
    say(`  inject: ${String(injected)}`)
    for (const [w, h] of [[1200, 780], [1000, 680]]) {
      await drive.resize(w, h)
      await sleep(400)
      const fit = await drive.evaluate(`(() => {
        const cell = document.querySelectorAll('.lc-compare__cell')[1]
        const labelEl = cell?.querySelector('.lc-livestep__label')
        if (!cell || !labelEl) return 'no-label'
        const c = cell.getBoundingClientRect()
        const l = labelEl.getBoundingClientRect()
        const clock = cell.querySelector('.lc-livestep__clock')?.getBoundingClientRect()
        const note = cell.querySelector('.lc-livestep__note')?.getBoundingClientRect()
        const meta = cell.querySelector('.lc-livestep__meta')?.getBoundingClientRect()
        const row = cell.querySelector('.lc-livestep')?.getBoundingClientRect()
        return JSON.stringify({
          cell: Math.round(c.width),
          row: row ? Math.round(row.width) : null,
          label: Math.round(l.width),
          rightOver: Math.round(l.right - c.right),
          clockRightOver: clock ? Math.round(clock.right - c.right) : null,
          noteShown: note && meta ? meta.top - note.top < 1 : null
        })
      })()`)
      say(`  fit ${label} ${w}x${h}: ${String(fit)}`)
      try {
        const got = JSON.parse(String(fit))
        if (got.rightOver > 0 || got.clockRightOver === null || got.clockRightOver > 0) failures += 1
      } catch {
        failures += 1
      }
      await drive.capture(`${label}-col-${w}x${h}`, async () => {})
    }
  }
  await sleep(200)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
}
await drive.finish({ intro: `Seeded 2- and 3-column compares; long live line injected. Tag=${tag}.`, extra: `Checks failed: ${String(failures)}` })
process.exitCode = failures > 0 ? 1 : 0
