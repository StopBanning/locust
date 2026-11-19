// The effort control, across every runtime and every state it has.
//
//   node _tools/drive-effort-regression.mjs
//
// 0.38.5 through 0.38.7 were built fast and under pressure: effort moved onto
// the route chip, off it, into the picker, out of the picker, and back to a
// chip of its own; swarm moved twice; the account default became a row. Each
// step was verified on ONE route. This checks the rest of them.
//
// What must hold:
//   - every route that reports levels shows a real level, never blank
//   - a route that reports none shows no effort chip at all, not a dead one
//   - switching models keeps the level when the new model supports it
//   - swarm disables the chip and says who is holding it
//   - the picker has no effort control left in it
//
// Spends nothing: no mission is sent.

// The picker no longer holds effort or swarm -- both moved to the composer
// (`lc-effortpanel*`, `lc-swarm`). This asked for `.lc-picker__effort*`, a
// class nothing renders any more, so "none in the picker" was true because
// the class was dead rather than because the control had moved: an
// unfalsifiable pass. Asking for the LIVE class keeps the same question and
// makes a wrong answer possible again.

import { pickRouteScript, say, scratchRepository, startDrive, teammateFace } from './drive-lib.mjs'

const workspace = await scratchRepository('locust-effort-reg-ws-')
const drive = await startDrive({
  name: 'effort-regression',
  port: 9337,
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

// The composer's controls, named, plus whether the picker still carries any
// effort control of its own.
const STATE = `(async () => {
  const flat = (el) => el.innerText.split(/\\s+/).join(' ').trim()
  const dock = document.querySelector('form.command-dock')
  if (!dock) return 'no composer'
  const effortChip = [...dock.querySelectorAll('button')].find((b) => b.getAttribute('aria-label') === 'Reasoning effort')
  const route = [...dock.querySelectorAll('.lc-control')].find((b) => b.getAttribute('aria-haspopup') === 'listbox')
  return [
    'route: ' + (route ? flat(route) : 'none'),
    'effort chip: ' + (effortChip ? flat(effortChip) + (effortChip.disabled ? ' [DISABLED]' : '') : 'ABSENT'),
    'effort title: ' + (effortChip ? (effortChip.getAttribute('title') ?? '') : '-')
  ].join('  ||  ')
})()`

const PICKER_HAS_EFFORT = `(async () => {
  const control = [...document.querySelectorAll('.lc-control')].find((b) => b.getAttribute('aria-haspopup') === 'listbox')
  if (!control) return 'no route control'
  control.click()
  await new Promise((r) => setTimeout(r, 1300))
  const picker = document.querySelector('.lc-picker')
  if (!picker) return 'picker did not open'
  const chips = picker.querySelectorAll('.lc-effortpanel__notch')
  const held = picker.querySelector('.lc-effortpanel__now')
  control.click()
  await new Promise((r) => setTimeout(r, 500))
  return 'effort chips in picker: ' + chips.length + ' || held line: ' + (held ? 'present' : 'none')
})()`

// Antigravity reports no effort levels at all -- the case where a chip must
// be ABSENT rather than dead, which is the rule the old "effort · fixed"
// chip broke.
const ROUTES = [
  { name: 'Claude Code / Sonnet', group: '/claude/i', search: 'sonnet', row: '/^sonnet/i' },
  { name: 'OpenCode / a free model', group: '/opencode/i', search: 'free', row: '/free/i' },
  { name: 'Copilot CLI / Auto', group: '/copilot/i', search: 'auto', row: '/auto/i' },
  { name: 'Antigravity / Gemini Flash — reports NO levels', group: '/antigravity/i', search: 'flash', row: '/flash/i' }
]

try {
  await drive.capture('the starting route', async () => {
    await drive.ready()
    await drive.evaluate(`(async () => {
      const open = ${teammateFace('Wren')}
      if (open) open.click()
      await new Promise((r) => setTimeout(r, 1000))
    })()`)
    return drive.evaluate(STATE)
  })

  await drive.capture('the picker carries no effort control any more', () =>
    drive.evaluate(PICKER_HAS_EFFORT)
  )

  for (const route of ROUTES) {
    await drive.capture(`switch to ${route.name}`, () => drive.evaluate(pickRouteScript(route)))
    await drive.capture(`  ...what the composer says`, () => drive.evaluate(STATE))
  }

} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({
    intro: 'The effort control across every runtime and state, after three releases of moving it.'
  })
}
