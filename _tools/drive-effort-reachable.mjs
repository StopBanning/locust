// Can a person choose an effort level at all?
//
//   node _tools/drive-effort-reachable.mjs
//
// The composer's "effort · fixed" chip was removed on purpose -- the
// reference drawing folds effort onto the route chip as `gpt-5.6 · high` and
// puts the chips under the selected model in the picker. Colin's installed
// build now shows no way to pick effort anywhere, and
// drive-composer-vs-reference found 0 effort chips and no swarm pill on the
// route a fresh profile starts on (Codex CLI / account-default).
//
// So: is effort unreachable only on a route that reports no levels, or
// everywhere? This checks three routes with known levels from the route
// inventory (2026-09-07) and reports, for each, what the picker draws.
//
// Spends nothing -- it opens the picker.

// The picker no longer holds effort or swarm -- both moved to the composer
// (`lc-effortpanel*`, `lc-swarm`). This asked for `.lc-picker__effort*`, a
// class nothing renders any more, so "none in the picker" was true because
// the class was dead rather than because the control had moved: an
// unfalsifiable pass. Asking for the LIVE class keeps the same question and
// makes a wrong answer possible again.

import { pickRouteScript, say, scratchRepository, startDrive, teammateFace } from './drive-lib.mjs'

const workspace = await scratchRepository('locust-drive-effort-ws-')
const drive = await startDrive({
  spends: true,
  name: 'effort-reachable',
  port: 9332,
  workspace,
  seed: {
    schemaVersion: 1,
    teammates: [
      {
        teammateId: 'tm_wren',
        name: 'Wren',
        hue: 'lime',
        role: 'Code & Migrations',
        createdAt: '2026-09-05T05:00:00.000Z'
      }
    ],
    missionOwners: {},
    settings: { relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false }
  }
})

// What the picker draws right now, without changing anything.
const REPORT = `(async () => {
  const flat = (el) => el.innerText.split(/\\s+/).join(' ').trim()
  const control = [...document.querySelectorAll('.lc-control')].find((b) => b.getAttribute('aria-haspopup') === 'listbox')
  if (!control) return 'no route control'
  const chip = flat(control)
  if (!document.querySelector('.lc-picker')) {
    control.click()
    await new Promise((r) => setTimeout(r, 1300))
  }
  const picker = document.querySelector('.lc-picker')
  if (!picker) return 'chip: ' + chip + ' || picker did not open'
  const list = picker.querySelector('.lc-picker__list')
  const activeRows = []
  for (const node of (list ? list.children : [])) {
    const row = node.querySelector('.lc-picker__row')
    if (!row) continue
    const tag = row.querySelector('.lc-picker__tag')
    if (tag && /ACTIVE/.test(tag.innerText)) {
      const detail = row.querySelector('.lc-picker__detail')
      activeRows.push(flat(row).slice(0, 40) + (detail ? ' [detail: ' + flat(detail).slice(0, 46) + ']' : ' [no detail]'))
    }
  }
  const chips = picker.querySelectorAll('.lc-effortpanel__notch')
  return [
    'chip: ' + chip.slice(0, 46),
    'ACTIVE rows: ' + activeRows.length + (activeRows.length ? ' >> ' + activeRows.join(' ;; ') : ''),
    'effort chips: ' + chips.length + (chips.length ? ' >> ' + [...chips].map(flat).join(' ') : '')
  ].join('  ||  ')
})()`

const CLOSE = `(async () => {
  const control = [...document.querySelectorAll('.lc-control')].find((b) => b.getAttribute('aria-haspopup') === 'listbox')
  if (control && document.querySelector('.lc-picker')) control.click()
  await new Promise((r) => setTimeout(r, 500))
  return document.querySelector('.lc-picker') ? 'still open' : 'closed'
})()`

try {
  await drive.capture('the route a fresh profile starts on', async () => {
    await drive.ready()
    await drive.evaluate(`(async () => {
      const open = ${teammateFace('Wren')}
      if (open) open.click()
      await new Promise((r) => setTimeout(r, 1200))
    })()`)
    return drive.evaluate(REPORT)
  })

  // Three routes the inventory says report levels: Sonnet (5), GPT-5.6-Sol
  // (6), and cursor-grok-4.6 (8). If chips are missing on all three, they
  // never render; if they appear, the fault is the starting route.
  const routes = [
    { name: 'Claude Code / Sonnet — inventory says 5 levels', group: '/claude/i', search: 'sonnet', row: '/^sonnet/i' },
    { name: 'Codex CLI / GPT-5.6-Sol — inventory says 6 levels', group: '/codex/i', search: 'sol', row: '/sol/i' },
    { name: 'Cursor / cursor-grok-4.6 — inventory says 8 levels', group: '/cursor/i', search: 'grok', row: '/grok/i' }
  ]
  for (const route of routes) {
    await drive.capture(`pick ${route.name}`, async () => {
      await drive.evaluate(CLOSE)
      return drive.evaluate(pickRouteScript(route))
    })
    await drive.capture(`  ...and what the picker then draws`, () => drive.evaluate(REPORT))
  }
  // The last link of the drawing: the chosen level rides on the route chip
  // as `gpt-5.6 · high`, which is why the composer chip that used to say
  // "effort · fixed" could be removed at all.
  await drive.capture('choose high, and read the composer chip', () =>
    drive.evaluate(`(async () => {
      const flat = (el) => el.innerText.split(/\\s+/).join(' ').trim()
      const chips = [...document.querySelectorAll('.lc-effortpanel__notch')]
      const high = chips.find((c) => flat(c) === 'high')
      if (!high) return 'no high chip among ' + chips.map(flat).join(' ')
      high.click()
      await new Promise((r) => setTimeout(r, 900))
      const control = [...document.querySelectorAll('.lc-control')].find((b) => b.getAttribute('aria-haspopup') === 'listbox')
      return 'chip now: ' + (control ? flat(control) : 'gone') + '  ||  pressed: ' + high.getAttribute('aria-pressed')
    })()`)
  )

} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({
    intro: 'Whether effort can be chosen at all, on three routes that report levels.'
  })
}
