// Frames of a busy Locust for locust.lol (2026-10-27): the homepage's picture of the app at work, and the cards its
// feature grid shows. The promo frames (drive-promo-shots.mjs) show each part alone; the site wants the app mid-work,
// as OpenBot's does: conversations in the sidebar, one handed on and checked, one waiting on you.
//
//   LOCUST_SPEND=1 node _tools/drive-site-frames.mjs --packaged <exe> --out <folder>
//
// Spends: Claude Haiku through Claude Code (Wren's diagnosis and fix) and GPT-6-Luna, low, through Codex (Atlas's
// check, Sable's one command). Nothing else runs. The window is 1200x780 at 2x, the site's size.
//
// Frames: hero.png, Sable's conversation with its card waiting and the others in the sidebar; card-approval.png and
// card-handoff.png, those two cards alone; board.png, the Board while the card waits.

import { execFileSync } from 'node:child_process'
import { mkdir, rm, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import { openTeammateScript, say, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const SHOTS = arg('--out')
if (SHOTS === undefined) throw new Error('--out <folder for the frames> is required')
const WIDTH = 1200, HEIGHT = 780, SCALE = 2
const HAIKU = { runtime: 'claude', model: 'haiku' }
const LUNA = { runtime: 'codex', model: 'gpt-6-luna', effort: 'low' }
await mkdir(SHOTS, { recursive: true })

// The same demo project as the promo frames, so no user path shows in a card's WHERE line.
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

const at = '2026-10-27T09:00:00.000Z'
const drive = await startDrive({
  name: 'site-frames', port: 9774, workspace, spends: true,
  ...(packaged === undefined ? {} : { packaged }),
  seed: {
    schemaVersion: 1,
    teammates: [
      { teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: at, route: { ...HAIKU, mode: 'accept-edits' } },
      { teammateId: 'tm_atlas', name: 'Atlas', hue: 'blue', role: 'Research & Briefs', createdAt: at, route: { ...LUNA, mode: 'ask' } },
      { teammateId: 'tm_sable', name: 'Sable', hue: 'clay', role: 'Docs & QA', createdAt: at, route: { ...LUNA, mode: 'approve-each' } },
      // Never run: they fill the team, each chip naming a real model on another agent.
      { teammateId: 'tm_quill', name: 'Quill', hue: 'violet', role: 'Ops & Scheduling', createdAt: at, route: { runtime: 'claude', model: 'opus', mode: 'ask' } },
      { teammateId: 'tm_marlow', name: 'Marlow', hue: 'blue', role: 'Code & Migrations', createdAt: at, route: { runtime: 'cursor', model: 'composer-2.5', mode: 'accept-edits' } },
      { teammateId: 'tm_juniper', name: 'Juniper', hue: 'violet', role: 'Data & Reporting', createdAt: at, route: { runtime: 'opencode', model: 'opencode/nemotron-3-ultra-free', mode: 'ask' } }
    ],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false, keepATodoList: true }
  },
  files: {
    'routines.json': {
      schemaVersion: 1,
      routines: [{
        routineId: 'rt_site',
        name: 'Bug to checked diagnosis',
        teammateId: 'tm_wren',
        route: { ...HAIKU, mode: 'ask' },
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
const shoot = async (file, note, clip) => {
  await sleep(900)
  const shot = await drive.send('Page.captureScreenshot', { format: 'png', ...(clip === undefined ? {} : { clip: { ...clip, scale: 1 } }) })
  await writeFile(join(SHOTS, file), Buffer.from(shot.result.data, 'base64'))
  said.push(`${file}: ${note}`)
  say(`  frame ${file} -- ${note}`)
}
// The box an element sits in, a little wider, for a frame of that card alone.
const boxOf = async (selector) => {
  const box = JSON.parse(String(await drive.evaluate(`(() => {
    const el = ${selector}
    if (!el) return 'null'
    el.scrollIntoView({ block: 'center' })
    const r = el.getBoundingClientRect()
    return JSON.stringify({ x: Math.max(0, r.left - 12), y: Math.max(0, r.top - 12), width: r.width + 24, height: r.height + 24 })
  })()`)))
  return box
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

  // 1. The hand-off: Wren (Claude Code) diagnoses, Atlas (Codex) checks.
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
    return (document.querySelector('.lc-thread')?.innerText ?? '').replace(/\\s+/g, ' ').slice(-240)
  })()`))
  say(`  the chain: ${chain}`)
  // The whole window: the hand-off note and the verdict are rows of the thread, not one card, so they are cropped after.
  await shoot('handoff-window.png', `Atlas's check of Wren's step: ${/approved/i.test(chain) ? 'approved' : 'verdict not seen'}`)

  // 2. Wren fixes the bug, in Edit, on Claude Code.
  await drive.evaluate(openTeammateScript('Wren'))
  await sleep(1000)
  await drive.evaluate(type('Fix the bug in issue.md: make total() in src/cart.py return the right total, and add a test for it in tests/. Keep a short todo list as you go.'))
  say(`  Wren's fix: ${String(await drive.evaluate(waitEnd))}`)

  // 3. Sable, in Approve each on Codex, asks before running the tests; the card waits.
  await drive.evaluate(openTeammateScript('Sable'))
  await sleep(1000)
  await drive.evaluate(type('Run python -m pytest -q in this folder and tell me the result in one line.'))
  const card = String(await drive.evaluate(`(async () => {
    for (let i = 0; i < 240; i += 1) {
      await new Promise((r) => setTimeout(r, 500))
      if (document.querySelector('[role=group][aria-label="Approval required"]')) return 'card'
      if (i > 10 && !document.querySelector('button[aria-label^="Stop the running"]')) return 'ended without a card'
    }
    return 'no card'
  })()`))
  await sleep(1500)
  await shoot('hero.png', `Sable's card waits, the others in the sidebar (${card})`)
  const approval = await boxOf(`document.querySelector('[role=group][aria-label="Approval required"]')`)
  if (approval !== null) await shoot('card-approval.png', 'the card alone', approval)
  const board = String(await drive.evaluate(`(async () => {
    ;[...document.querySelectorAll('button')].find((b) => b.innerText.trim() === 'Board')?.click()
    await new Promise((r) => setTimeout(r, 1500))
    return [...document.querySelectorAll('.lc-boardsection')].map((section) => (section.querySelector('.lc-boardsection__title')?.textContent.trim() ?? '') + ' ' + (section.querySelector('.lc-boardsection__count')?.textContent.trim() ?? '')).join(' | ') || 'no board'
  })()`))
  await shoot('board.png', `the Board while Sable's card waits (${board})`)

  // Let Sable's command run, so nothing is left waiting.
  await drive.evaluate(openTeammateScript('Sable'))
  await sleep(1200)
  await drive.evaluate(`(async () => {
    for (let i = 0; i < 20; i += 1) {
      const approval = document.querySelector('[role=group][aria-label="Approval required"]')
      if (!approval) break
      ;[...approval.querySelectorAll('button')].find((b) => /^Approve once|^Allow once/.test(b.innerText.trim()))?.click()
      await new Promise((r) => setTimeout(r, 1500))
    }
  })()`)
  await drive.evaluate(waitEnd)
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Site frames, build ${packaged ?? 'out/'}.`, extra: said.join('\n') })
}
say(said.join('\n'))
process.exit(0)
