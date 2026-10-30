// Frames of Locust's own selects, light and dark, at 1200x720.
//
//   node _tools/drive-select-polish.mjs
//
// Opens every screen that used to hold a native select, opens each menu, and
// keeps a frame. Sends nothing. A missing control is written into the step
// note and the run continues, so a frame of what was there is still kept.

import { createHash } from 'node:crypto'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { columnEvents } from './compare-layout-seed.mjs'
import { localPath, FREE_ROUTE, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const PROMPT = 'Two short answers side by side.'
const COLUMNS = [
  { slot: 'a', runtime: 'opencode', model: FREE_ROUTE.model, label: 'DeepSeek', parts: [{ say: 'A short answer, one paragraph, nothing else.' }] },
  { slot: 'b', runtime: 'opencode', model: 'opencode/mimo-v2.6-flash-free', label: 'Mimo', parts: [{ say: 'Another short answer, also one paragraph.' }] }
]

const root = localPath(new URL('..', import.meta.url))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-select-polish-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-select-polish-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const startedAt = new Date(Date.now() - 2 * 3_600_000).toISOString()
const slots = []
let n = 0
for (const column of COLUMNS) {
  n += 1
  const missionId = `mission_5e210000-0000-4000-8000-0008${String(n).padStart(8, '0')}`
  const runId = `run_5e2108${String(n)}`
  await ledger.createMission({
    missionId, runId, prompt: PROMPT,
    runtime: column.runtime, model: column.model, requestedRouteId: 'opencode', resolvedRouteId: 'opencode-account:default', cliVersion: null,
    workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: startedAt
  })
  await ledger.appendEvents(missionId, columnEvents(column, { missionId, runId, startedAt, minutes: 12 }))
  slots.push({ slot: column.slot, route: { runtime: column.runtime, model: column.model, label: column.label, mode: 'auto' }, missionIds: [missionId] })
}
await ledger.flush?.()

const routine = {
  routineId: 'rt_notes',
  name: 'Notes',
  teammateId: 'tm_wren',
  route: { ...FREE_ROUTE },
  steps: ['Write the note in a {{tone}} voice.', 'Check the note.'],
  learnedFrom: [],
  createdAt: '2026-10-18T12:00:00.000Z',
  runs: 0,
  schedule: { kind: 'every', hours: 4 },
  untilCheck: { tries: 3 },
  handOffs: [{}, { teammateId: 'tm_sable', check: true }],
  inputs: [{ key: 'tone', label: 'Tone', kind: 'choice', required: false, choices: ['Brief', 'Detailed'], default: 'Brief' }]
}
const scout = {
  settings: { on: true, everyMinutes: 60, watches: { github: true, teammates: true }, notify: false },
  cards: [], seen: [], lookIds: [], prs: []
}

const drive = await startDrive({
  name: 'select-polish',
  port: 9461,
  workspace,
  profilePath,
  sendsNothing: true,
  seed: {
    schemaVersion: 1,
    teammates: [
      { teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-10-18T12:00:00.000Z', route: { ...FREE_ROUTE } },
      { teammateId: 'tm_sable', name: 'Sable', hue: 'blue', role: 'Docs & QA', createdAt: '2026-10-18T12:00:00.000Z', route: { ...FREE_ROUTE } }
    ],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' }
  },
  files: {
    'routines.json': { schemaVersion: 1, routines: [routine] },
    'scout.json': scout,
    'compares.json': { schemaVersion: 1, compares: [{ compareId: 'cmp_select', prompt: PROMPT, createdAt: startedAt, slots }] }
  }
})

const seen = []
const buttonOf = (target) => target.startsWith('#')
  ? `document.querySelector(${JSON.stringify(target)})`
  : `[...document.querySelectorAll('[role="combobox"]')].find((el) => el.getAttribute('aria-label') === ${JSON.stringify(target)})`

const openMenu = (target) => `(async () => {
  const button = ${buttonOf(target)}
  if (!button) return 'missing'
  if (button.disabled) return 'disabled'
  button.scrollIntoView({ block: 'center', inline: 'nearest' })
  await new Promise((r) => setTimeout(r, 120))
  if (button.getAttribute('aria-expanded') !== 'true') button.click()
  for (let i = 0; i < 30; i += 1) {
    const list = [...document.querySelectorAll('[role="listbox"]')].find((el) => !el.hidden && el.getAttribute('aria-labelledby') === button.id)
    if (list) {
      const box = list.getBoundingClientRect()
      const names = [...list.querySelectorAll('[role="option"]')].slice(0, 8).map((el) => el.innerText.replace(/\\s+/g, ' ').trim())
      return 'open ' + Math.round(box.width) + 'x' + Math.round(box.height) + ' at ' + Math.round(box.left) + ',' + Math.round(box.top) + ' :: ' + names.join(' | ')
    }
    await new Promise((r) => setTimeout(r, 50))
  }
  return 'did not open'
})()`

const themeOf = (mode) => drive.evaluate(`window.desktop.setTheme(${JSON.stringify(mode)})`)

async function closeMenu() {
  const open = await drive.evaluate(`(() => [...document.querySelectorAll('[role="combobox"]')].some((el) => el.getAttribute('aria-expanded') === 'true'))()`)
  if (open !== true) return
  await drive.send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27 })
  await drive.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27 })
  await sleep(200)
  const still = await drive.evaluate(`(() => {
    const button = [...document.querySelectorAll('[role="combobox"]')].find((el) => el.getAttribute('aria-expanded') === 'true')
    if (!button) return ''
    button.click()
    return 'clicked'
  })()`)
  if (still === 'clicked') await sleep(150)
}

async function shot(title, expression) {
  const note = await drive.capture(title, () => (expression === undefined ? Promise.resolve(title) : drive.evaluate(expression)))
  const line = String(note ?? '').replace(/\s+/g, ' ').slice(0, 220)
  say(`  ${title}: ${line}`)
  seen.push(`${title} :: ${line}`)
  return line
}

async function both(title, expression) {
  for (const mode of ['light', 'dark']) {
    await themeOf(mode)
    await sleep(250)
    await shot(`${mode} ${title}`, expression)
  }
}

async function menus(targets) {
  for (const target of targets) {
    const name = target.startsWith('#') ? target.slice(1) : target
    for (const mode of ['light', 'dark']) {
      await themeOf(mode)
      await sleep(200)
      await shot(`${mode} menu ${name}`, openMenu(target))
      await closeMenu()
    }
  }
}

async function closeDialog() {
  await closeMenu()
  await drive.evaluate(`(() => { document.querySelector('[role="dialog"] button[aria-label="Close"]')?.click(); return 'closed' })()`)
  await sleep(250)
}

const clickText = (text) => `(() => {
  const node = [...document.querySelectorAll('button')].find((b) => b.innerText.replace(/\\s+/g, ' ').trim().includes(${JSON.stringify(text)}))
  if (!node) return 'missing'
  node.click()
  return 'clicked'
})()`

try {
  await drive.capture('launch', () => drive.ready())
  await drive.resize(1200, 720)
  await drive.send('Page.bringToFront')
  await drive.evaluate(`document.querySelector('button[aria-label="Home"]')?.click()`)
  await sleep(400)

  await shot('open new routine', `(async () => {
    document.querySelector('button[title="Routines — work that repeats"]')?.click()
    await new Promise((r) => setTimeout(r, 500))
    ;[...document.querySelectorAll('button')].find((b) => b.innerText.trim() === 'New routine')?.click()
    await new Promise((r) => setTimeout(r, 400))
    return document.querySelector('#routine-runner') ? 'who runs it' : (document.querySelector('[role="dialog"]')?.getAttribute('aria-label') ?? 'no dialog')
  })()`)
  await menus(['#routine-runner'])
  await both('new routine', `document.querySelector('[role="dialog"]')?.getAttribute('aria-label') ?? 'no dialog'`)
  await closeDialog()

  await shot('open settings', `(async () => {
    document.querySelector('button[title="Settings (Ctrl 3)"]')?.click()
    await new Promise((r) => setTimeout(r, 500))
    ;[...document.querySelectorAll('.lc-settings__navitem')].find((b) => b.innerText.trim().startsWith('General'))?.click()
    await new Promise((r) => setTimeout(r, 300))
    const voice = document.querySelector('[aria-label="Voice typing model"]')
    const talk = document.querySelector('[aria-label="Talk’s voice"]')
    const scout = document.querySelector('[aria-label="How often Scout looks"]')
    return [voice ? (voice.disabled ? 'voice disabled' : 'voice') : 'no voice', talk ? (talk.disabled ? 'talk disabled' : 'talk') : 'no talk', scout ? 'scout' : 'no scout'].join(', ')
  })()`)
  await drive.waitFor(`(() => {
    const voice = document.querySelector('[aria-label="Voice typing model"]')
    return voice && !voice.disabled ? 'voice ready' : false
  })()`, { what: 'voice typing model enabled', timeoutMs: 15000 }).catch((error) => say(`  ${error instanceof Error ? error.message : String(error)}`))
  await both('settings voice', `(() => {
    document.querySelector('[aria-label="Voice typing model"]')?.scrollIntoView({ block: 'center' })
    return document.querySelector('[aria-label="Voice typing model"] .lc-select__value')?.textContent ?? 'missing'
  })()`)
  await menus(['Voice typing model', 'Talk\u2019s voice', 'How often Scout looks'])

  await shot('open edit routine', `(async () => {
    document.querySelector('button[title="Routines — work that repeats"]')?.click()
    await new Promise((r) => setTimeout(r, 500))
    document.querySelector('button[aria-label="Edit Notes"]')?.click()
    await new Promise((r) => setTimeout(r, 400))
    const labels = [...document.querySelectorAll('[role="combobox"]')].map((el) => el.getAttribute('aria-label') || el.id)
    return labels.join(', ') || (document.querySelector('[role="dialog"]')?.getAttribute('aria-label') ?? 'no dialog')
  })()`)
  await menus(['Fixes at most', 'Who takes step 1', 'Who takes step 2', 'Input 1 kind', 'Input 1 default', 'Hours between runs'])
  await closeDialog()

  await shot('open run', `(async () => {
    const row = [...document.querySelectorAll('.lc-routinerow')].find((el) => el.innerText.includes('Notes'))
    ;[...((row ?? document).querySelectorAll('button'))].find((b) => b.innerText.trim() === 'Run')?.click()
    await new Promise((r) => setTimeout(r, 400))
    return document.querySelector('[aria-label="Value for Tone"]') ? 'value for tone' : (document.querySelector('[role="dialog"]')?.innerText?.slice(0, 160) ?? 'no dialog')
  })()`)
  await menus(['Value for Tone'])
  await closeDialog()

  await shot('open template', `(async () => {
    ;[...document.querySelectorAll('button')].find((b) => b.innerText.replace(/\\s+/g, ' ').includes('Start from a template'))?.click()
    await new Promise((r) => setTimeout(r, 400))
    document.querySelector('.lc-templates__row')?.click()
    await new Promise((r) => setTimeout(r, 500))
    return document.querySelector('[aria-label="Give routine to"]') ? 'give routine to' : (document.querySelector('[role="dialog"]')?.getAttribute('aria-label') ?? 'no dialog')
  })()`)
  await menus(['Give routine to'])
  await closeDialog()

  await shot('open new teammate', `(async () => {
    document.querySelector('button[aria-label="Add"]')?.click()
    await new Promise((r) => setTimeout(r, 300))
    const item = [...document.querySelectorAll('[role="menuitem"]')].find((el) => el.querySelector('.lc-context__label')?.textContent?.trim() === 'New teammate')
    if (!item) return 'no add item: ' + [...document.querySelectorAll('[role="menuitem"]')].map((el) => el.innerText.replace(/\\s+/g, ' ').trim()).join(' | ')
    item.click()
    await new Promise((r) => setTimeout(r, 500))
    const voice = document.querySelector('[aria-label="Voice for this teammate"]')
    voice?.scrollIntoView({ block: 'center' })
    return voice ? 'voice for this teammate' : (document.querySelector('[role="dialog"]')?.getAttribute('aria-label') ?? 'no dialog')
  })()`)
  await menus(['Voice for this teammate'])
  await closeDialog()

  await shot('open comparison', `(async () => {
    document.querySelector('button[aria-label="Home"]')?.click()
    await new Promise((r) => setTimeout(r, 400))
    ;[...document.querySelectorAll('.lc-convrow button.lc-conv')].find((row) => /Two short answers/i.test(row.innerText))?.click()
    await new Promise((r) => setTimeout(r, 800))
    ;[...document.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Open the comparison')?.click()
    for (let i = 0; i < 40 && !document.querySelector('.lc-compare'); i += 1) await new Promise((r) => setTimeout(r, 250))
    const scroll = document.querySelector('.lc-compare__scroll')
    if (scroll) scroll.scrollTop = scroll.scrollHeight
    await new Promise((r) => setTimeout(r, 300))
    const judge = !!document.querySelector('[aria-label="The model that judges"]')
    const add = !!document.querySelector('[aria-label="The model to ask too"]')
    const three = [...document.querySelectorAll('button')].some((b) => b.innerText.trim() === 'Ask three judges to vote')
    return 'compare ' + String(!!document.querySelector('.lc-compare')) + ' judge ' + String(judge) + ' add ' + String(add) + ' three ' + String(three)
  })()`)
  await menus(['The model that judges', 'The model to ask too'])
  await shot('ask three judges', clickText('Ask three judges to vote'))
  await menus(['The first judge', 'The second judge', 'The third judge'])
  await both('comparison menus closed', `(() => {
    const scroll = document.querySelector('.lc-compare__scroll')
    if (scroll) scroll.scrollTop = scroll.scrollHeight
    return [...document.querySelectorAll('[role="combobox"]')].map((el) => el.getAttribute('aria-label')).filter(Boolean).join(', ') || 'no combobox'
  })()`)
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.stack ?? error.message : String(error)}`)
  await drive.capture('drive failed', () => String(error instanceof Error ? error.message : error)).catch(() => undefined)
} finally {
  const errors = drive.record.filter((entry) => entry.errors.length > 0).length
  say('frames:')
  for (const line of seen) say(`  ${line}`)
  await drive.finish({
    intro: 'Build: out/. Locust’s own selects, each menu opened, light and dark, 1200x720. Nothing sent.',
    extra: `Steps with renderer errors: ${String(errors)}.`
  })
}
