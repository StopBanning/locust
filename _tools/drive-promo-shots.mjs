// Frames of the current build for locust.lol, the README and the social card.
//
//   node _tools/drive-promo-shots.mjs --packaged <exe> --out <folder>
//
// Spends: one short Codex turn (Atlas, gpt-6-luna, low), and since 0.619 one
// short Blind compare (Haiku 4.5 on Claude Code, GPT-6-Luna on Codex: a
// four-line poem); everything else is on the free OpenCode model. Run with
// LOCUST_SPEND=1. Quill is on the roster so the model picker shows Claude.
// Frames since 0.619 also: 07/08 the Blind compare hidden then revealed, 09
// the Board while a card waits. The site's size: --width 1200 --height 780 --scale 2.
//
// A clean demo: the project is C:\acme-storefront (so no user path shows in
// an approval card's WHERE line), the teammates have friendly names, and the
// profile is fresh. Frames at 1920x1080: the home cover, the model picker, a
// run in progress, an approval card, a hand-off chain across two runtimes,
// and a finished result.

import { execFileSync } from 'node:child_process'
import { mkdir, rm, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import { FREE_ROUTE, openTeammateScript, say, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const SHOTS = arg('--out')
if (SHOTS === undefined) throw new Error('--out <folder for the frames> is required')
// drive-lib's free model, never one named here: OpenCode drops free models weekly (Nemotron 3 Ultra was gone, 2026-10-28).
const FREE = FREE_ROUTE.model
// The window's size and pixel density: Colin's own window is about 1200x780,
// which fills without dead space; at 2x the frames stay crisp on the site.
const WIDTH = Number(arg('--width') ?? 1920)
const HEIGHT = Number(arg('--height') ?? 1080)
const SCALE = Number(arg('--scale') ?? 1)
// --only run,approval re-shoots those sections and leaves the other frames as they are.
const ONLY = arg('--only')?.split(',').map((name) => name.trim())
const want = (name) => ONLY === undefined || ONLY.includes(name)
await mkdir(SHOTS, { recursive: true })

const workspace = 'C:/acme-storefront'
await rm(workspace, { recursive: true, force: true })
await mkdir(join(workspace, 'src'), { recursive: true })
await mkdir(join(workspace, 'tests'), { recursive: true })
await writeFile(join(workspace, '.gitignore'), '__pycache__/\n.pytest_cache/\n', 'utf8')
await writeFile(join(workspace, 'README.md'), '# Acme Storefront\n\nA small shop backend: carts, prices and checkout.\n\nRun the tests with `python -m pytest -q`.\n', 'utf8')
await writeFile(join(workspace, 'issue.md'), '# Bug: cart total is doubled\n\nA cart with one $10.00 item shows a total of $20.00 at checkout. Expected $10.00.\n', 'utf8')
await writeFile(join(workspace, 'src', 'cart.py'), 'def total(items):\n    subtotal = sum(item["price"] * item["qty"] for item in items)\n    return subtotal + sum(item["price"] * item["qty"] for item in items)\n', 'utf8')
await writeFile(join(workspace, 'src', 'prices.py'), 'TAX_RATE = 0.07\n\ndef with_tax(amount):\n    return round(amount * (1 + TAX_RATE), 2)\n', 'utf8')
await writeFile(join(workspace, 'tests', 'test_prices.py'), 'from src.prices import with_tax\n\ndef test_with_tax():\n    assert with_tax(10) == 10.7\n', 'utf8')
const git = (...args) => execFileSync('git', args, { cwd: workspace, windowsHide: true })
git('init', '-q', '-b', 'main')
git('config', 'user.email', 'demo@acme.example')
git('config', 'user.name', 'Acme Demo')
git('add', '.')
git('commit', '-q', '-m', 'Acme storefront')

const at = '2026-09-28T09:00:00.000Z'
const drive = await startDrive({
  // Only the hand-off (Atlas on Codex) and the Blind compare spend: --only without them runs free (0.838.x).
  name: 'promo-shots', port: 9773, workspace, spends: ONLY === undefined || ONLY.includes('handoff') || ONLY.includes('blind'),
  ...(packaged === undefined ? {} : { packaged }),
  seed: {
    schemaVersion: 1,
    teammates: [
      { teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: at, route: { runtime: 'opencode', model: FREE, mode: 'accept-edits' } },
      { teammateId: 'tm_atlas', name: 'Atlas', hue: 'blue', role: 'Research & Briefs', createdAt: at, route: { runtime: 'codex', model: 'gpt-6-luna', effort: 'low', mode: 'ask' } },
      { teammateId: 'tm_sable', name: 'Sable', hue: 'clay', role: 'Docs & QA', createdAt: at, route: { runtime: 'opencode', model: FREE, mode: 'approve-each' } },
      { teammateId: 'tm_quill', name: 'Quill', hue: 'violet', role: 'Ops & Scheduling', createdAt: at, route: { runtime: 'claude', model: 'opus', mode: 'ask' } },
      // Never run: they fill the team grid, each chip naming a real model.
      { teammateId: 'tm_marlow', name: 'Marlow', hue: 'blue', role: 'Code & Migrations', createdAt: at, route: { runtime: 'cursor', model: 'composer-2.5', mode: 'accept-edits' } },
      { teammateId: 'tm_juniper', name: 'Juniper', hue: 'violet', role: 'Data & Reporting', createdAt: at, route: { runtime: 'claude', model: 'sonnet', mode: 'ask' } }
    ],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false, keepATodoList: true }
  },
  files: {
    'routines.json': {
      schemaVersion: 1,
      routines: [{
        routineId: 'rt_promo',
        name: 'Bug to reviewed fix plan',
        teammateId: 'tm_wren',
        route: { runtime: 'opencode', model: FREE, mode: 'ask' },
        steps: ['Read issue.md and src/cart.py. In two sentences, say what is wrong and where.', 'Check that diagnosis against the code.'],
        handOffs: [{}, { teammateId: 'tm_atlas', check: true }],
        learnedFrom: [],
        createdAt: at,
        runs: 0
      }]
    }
  }
})
const said = []
const shoot = async (file, note) => {
  await sleep(900)
  const shot = await drive.send('Page.captureScreenshot', { format: 'png' })
  await writeFile(join(SHOTS, file), Buffer.from(shot.result.data, 'base64'))
  said.push(`${file}: ${note}`)
  say(`  frame ${file} -- ${note}`)
}
const type = (text) => `(async () => {
  const field = document.querySelector('form.command-dock textarea')
  const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
  setter.call(field, ${JSON.stringify(text)})
  field.dispatchEvent(new Event('input', { bubbles: true }))
  for (let i = 0; i < 120; i += 1) {
    await new Promise((r) => setTimeout(r, 250))
    const button = document.querySelector('button[aria-label="Send"]')
    if (button && !button.disabled) { button.click(); break }
  }
  return 'sent'
})()`
// The conversation's mode, from its own control: a frame of an applied fix needs Edit, and
// the routine's conversation Wren is opened in is in Ask (0.619's run: "5 refused").
const setMode = (name) => `(async () => {
  const control = [...document.querySelectorAll('.lc-control')].find((b) => /^(Ask|Edit|Accept edits|Plan|Approve|Auto)\\b/.test(b.innerText))
  if (!control) return 'no mode control'
  control.click(); await new Promise((r) => setTimeout(r, 400))
  const item = [...document.querySelectorAll('[role=menuitemradio]')].find((b) => b.innerText.trim().startsWith(${JSON.stringify(name)}))
  if (!item || item.disabled) { control.click(); return 'not offered' }
  item.click(); await new Promise((r) => setTimeout(r, 400))
  return 'mode: ' + control.innerText.split(/\\s+/).join(' ').trim()
})()`
const waitEnd = `(async () => {
  for (let i = 0; i < 900; i += 1) {
    await new Promise((r) => setTimeout(r, 500))
    if (i > 6 && !document.querySelector('button[aria-label^="Stop the running"]')) return 'ended'
  }
  return 'timed out'
})()`

try {
  await drive.ready()
  await drive.send('Emulation.setDeviceMetricsOverride', { width: WIDTH, height: HEIGHT, deviceScaleFactor: SCALE, mobile: false })
  await sleep(3000)

  if (want('home')) {
  // 1. Home: the cover with the team. Quill picked first, so the composer's
  // chip names a model ("Claude / Opus 5.5"), never "Account Default".
  await drive.evaluate(openTeammateScript('Quill'))
  await sleep(1000)
  await drive.evaluate(`document.querySelector('button.lc-brand__lockup')?.click()`)
  await sleep(4000)
  await shoot('01-home.png', 'Home with six teammates on four runtimes')
  }

  if (want('picker')) {
  // 2. The model picker, from Quill's composer (Claude Code), searched to show Claude and Codex.
  await drive.evaluate(openTeammateScript('Quill'))
  await sleep(1200)
  const picker = String(await drive.evaluate(`(async () => {
    const control = [...document.querySelectorAll('.lc-control')].find((b) => b.getAttribute('aria-haspopup') === 'listbox')
    control?.click()
    await new Promise((r) => setTimeout(r, 1200))
    return document.querySelector('.lc-picker') ? [...document.querySelectorAll('.lc-picker__row')].length + ' rows' : 'no picker'
  })()`))
  await shoot('02-model-picker.png', `the model picker open (${picker})`)
  await drive.evaluate(`document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))`)
  await sleep(600)
  }

  if (want('handoff')) {
  // 5. A hand-off chain across two runtimes: Wren (OpenCode) diagnoses, Atlas (Codex) checks.
  await drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('.lc-sidebar__nav button')].find((b) => /Routines/.test(b.innerText))?.click()
    await new Promise((r) => setTimeout(r, 1200))
    const row = document.querySelector('.lc-routinerow:not(.lc-routineadd)')
    ;[...(row?.querySelectorAll('button') ?? [])].find((b) => /run/i.test(b.innerText || b.getAttribute('aria-label') || ''))?.click()
  })()`)
  let quiet = 0
  for (let waited = 0; waited < 600_000 && quiet < 3; waited += 5000) {
    await sleep(5000)
    const running = await drive.evaluate(`document.querySelectorAll('.lc-spark').length + (document.querySelector('button[aria-label^="Stop the running"]') ? 1 : 0)`)
    quiet = Number(running) === 0 ? quiet + 1 : 0
  }
  const chain = String(await drive.evaluate(`(async () => {
    const row = [...document.querySelectorAll('.lc-convrow .lc-conv')].find((r) => /Check that diagnosis/.test(r.innerText))
    row?.click()
    await new Promise((r) => setTimeout(r, 1500))
    return (document.querySelector('.lc-thread')?.innerText ?? '').replace(/\\s+/g, ' ').slice(-200)
  })()`))
  await shoot('05-hand-off.png', `Atlas (Codex) checks Wren's (OpenCode) step: ${/approved by Atlas/.test(chain) ? 'approved' : /asked for changes|gave no verdict/.test(chain) ? 'NOT approved' : 'verdict not seen'}`)
  }

  if (want('run')) {
  // 3. A run in progress, and 6. its finished result: Wren fixes the bug, in Edit.
  await drive.evaluate(openTeammateScript('Wren'))
  await sleep(1000)
  const wrenMode = String(await drive.evaluate(setMode('Edit')))
  say(`  Wren's conversation: ${wrenMode}`)
  await drive.evaluate(type('Fix the bug in issue.md: make total() in src/cart.py return the right total, and add a test for it in tests/. Keep a short todo list as you go.'))
  const midway = String(await drive.evaluate(`(async () => {
    for (let i = 0; i < 120; i += 1) {
      await new Promise((r) => setTimeout(r, 500))
      if (document.querySelector('.lc-plan, .lc-plan__step')) return 'plan card'
      if (!document.querySelector('button[aria-label^="Stop the running"]') && i > 10) return 'ended before a plan showed'
    }
    return 'no plan card within a minute'
  })()`))
  await shoot('03-run-in-progress.png', `Wren working (${midway})`)
  const wren = String(await drive.evaluate(waitEnd))
  await sleep(2500)
  await drive.evaluate(`document.querySelector('.lc-thread')?.scrollTo?.(0, 1e9)`)
  await shoot('06-finished.png', `Wren's run ${wren}: the result`)
  }

  if (want('approval')) {
  // 4. An approval card: Sable in Approve each asks before running a command.
  await drive.evaluate(openTeammateScript('Sable'))
  await sleep(1000)
  const askSable = async () => {
    await drive.evaluate(type('Run python -m pytest -q in this folder and tell me the result in one line.'))
    return String(await drive.evaluate(`(async () => {
      for (let i = 0; i < 240; i += 1) {
        await new Promise((r) => setTimeout(r, 500))
        if (document.querySelector('[role=group][aria-label="Approval required"]')) return 'card'
        if (i > 10 && !document.querySelector('button[aria-label^="Stop the running"]')) return 'ended without a card'
      }
      return 'no card'
    })()`))
  }
  let card = await askSable()
  // OpenCode's serve path failed to come up once (0.619's run: "could not be reached just now"); one more try, said.
  if (card !== 'card' && /could not be reached/.test(String(await drive.evaluate(`document.querySelector('.lc-thread')?.innerText ?? ''`)))) {
    say('  OpenCode could not be reached on the first try; asking once more in 20 s')
    await sleep(20_000)
    card = await askSable()
  }
  await shoot('04-approval.png', `Sable asks first (${card})`)
  // 9. The Board, while Sable's card waits: a conversation under "Needs you", the others by state.
  const board = String(await drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Board')?.click()
    await new Promise((r) => setTimeout(r, 1500))
    return [...document.querySelectorAll('.lc-boardsection')].map((section) => (section.querySelector('.lc-boardsection__title')?.textContent.trim() ?? '') + ' ' + (section.querySelector('.lc-boardsection__count')?.textContent.trim() ?? '')).join(' | ') || (document.querySelector('.lc-screen__title')?.textContent ?? 'no board')
  })()`))
  await shoot('09-board.png', `the Board while Sable's card waits (${board})`)
  await drive.evaluate(openTeammateScript('Sable'))
  await sleep(1200)
  await drive.evaluate(`(async () => {
    for (let i = 0; i < 20; i += 1) {
      const approval = document.querySelector('[role=group][aria-label="Approval required"]')
      if (!approval) break
      ;[...approval.querySelectorAll('button')].find((b) => /^Approve once/.test(b.innerText.trim()))?.click()
      await new Promise((r) => setTimeout(r, 1500))
    }
  })()`)
  await drive.evaluate(waitEnd)
  }

  if (want('blind')) {
  // 7 and 8. A Blind compare: Haiku 4.5 (Claude Code) and GPT-6-Luna (Codex) answer one short
  // ask; the names stay hidden until one answer is kept, then the same view shows them.
  const blind = String(await drive.evaluate(`(async () => {
    document.querySelector('.lc-control--chatmode')?.click()
    await new Promise((r) => setTimeout(r, 500))
    ;[...document.querySelectorAll('.lc-menu[aria-label="Direct or compare"] .lc-menu__item')].find((item) => item.querySelector('.lc-menu__name')?.textContent.trim() === 'Blind')?.click()
    for (let i = 0; i < 20 && !document.querySelector('.lc-slotgroup'); i += 1) await new Promise((r) => setTimeout(r, 250))
    const done = [...document.querySelectorAll('.lc-picker__foot--compare button')].find((b) => b.textContent.trim() === 'Done')
    if (done && !done.disabled) done.click()
    await new Promise((r) => setTimeout(r, 600))
    const pick = async (index, search, label, runtime) => {
      document.querySelectorAll('.lc-slotgroup')[index]?.querySelector('.lc-control--slot')?.click()
      await new Promise((r) => setTimeout(r, 700))
      const box = document.querySelector('.lc-picker__input')
      if (!box) return 'no picker'
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(box, search)
      box.dispatchEvent(new Event('input', { bubbles: true }))
      await new Promise((r) => setTimeout(r, 900))
      let group = ''
      for (const el of document.querySelectorAll('.lc-picker__group, .lc-picker__row')) {
        if (el.classList.contains('lc-picker__group')) { group = el.textContent; continue }
        if (el.classList.contains('is-recent') || el.disabled) continue
        if (runtime.test(group) && label.test(el.querySelector('.lc-picker__label')?.textContent.trim() ?? '')) { el.click(); await new Promise((r) => setTimeout(r, 700)); return 'picked' }
      }
      document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
      return 'not offered'
    }
    const first = await pick(0, 'haiku', /^Haiku 4\\.5$/i, /Claude Code/i)
    const second = await pick(1, 'luna', /^GPT-6-Luna$/i, /Codex/i)
    const again = first === 'picked' ? first : await pick(0, 'haiku', /^Haiku 4\\.5$/i, /Claude Code/i)
    return JSON.stringify({ first: again, second, slots: [...document.querySelectorAll('.lc-slotgroup')].map((g) => g.innerText.replace(/\\s+/g, ' ').trim()) })
  })()`))
  say(`  blind set up: ${blind}`)
  await drive.evaluate(type('Write a four-line poem about a locust swarm at dusk.'))
  const compared = String(await drive.evaluate(`(async () => {
    for (let i = 0; i < 360; i += 1) {
      await new Promise((r) => setTimeout(r, 500))
      const states = [...document.querySelectorAll('.lc-compare__state')].map((el) => el.textContent.trim())
      if (states.length === 2 && i > 6 && states.every((state) => !/working|waiting|starting/i.test(state))) return states.join(' / ')
    }
    return 'still running'
  })()`))
  await shoot('07-blind-compare.png', `a Blind compare, names hidden (${compared})`)
  const revealed = String(await drive.evaluate(`(async () => {
    document.querySelector('.lc-compare__foot .lc-primarybutton')?.click()
    for (let i = 0; i < 60 && document.querySelector('.lc-compare'); i += 1) await new Promise((r) => setTimeout(r, 250))
    await new Promise((r) => setTimeout(r, 1000))
    document.querySelector('.lc-compared__open')?.click()
    await new Promise((r) => setTimeout(r, 1500))
    return [...document.querySelectorAll('.lc-compare__head')].map((el) => el.innerText.replace(/\\s+/g, ' ').trim()).join(' | ')
  })()`))
  await shoot('08-blind-revealed.png', `the same compare after one answer was kept, names shown (${revealed})`)
  }

} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Promo frames, build ${packaged ?? 'out/'}.`, extra: said.join('\n') })
}
say(said.join('\n'))
process.exit(0)
