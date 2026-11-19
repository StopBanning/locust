// The composer and the picker, photographed against the reference drawing.
//
//   node _tools/drive-composer-vs-reference.mjs
//
// Page 1 of "Locust UI Review 2026-09-06.dc.html" draws the composer twice:
//
//   SHIPPED   Ask | ridgeline | + | (ring) | Codex CLI / gpt-5.6 | effort · fixed | swarm
//   PROPOSED  Ask | ridgeline |     (ring) | Codex CLI / gpt-5.6 · high v
//
// and the picker below it with a Swarm pill in its header, effort chips
// under the ACTIVE row only, and "held at maximum by swarm" when swarm is on.
//
// I have said three times that this shipped. Colin has said twice that it
// does not look like it. Neither of us should be arguing from memory, so
// this takes the photographs.
//
// Spends nothing -- it opens the composer and the picker.

// The picker no longer holds effort or swarm -- both moved to the composer
// (`lc-effortpanel*`, `lc-swarm`). This asked for `.lc-picker__effort*`, a
// class nothing renders any more, so "none in the picker" was true because
// the class was dead rather than because the control had moved: an
// unfalsifiable pass. Asking for the LIVE class keeps the same question and
// makes a wrong answer possible again.

import { say, scratchRepository, startDrive, teammateFace } from './drive-lib.mjs'

const workspace = await scratchRepository('locust-drive-composer-ws-')
const drive = await startDrive({
  name: 'composer-vs-reference',
  port: 9331,
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

const OPEN_WORKROOM = `(async () => {
  const open = ${teammateFace('Wren')}
  if (open) open.click()
  await new Promise((r) => setTimeout(r, 1200))
  const dock = document.querySelector('form.command-dock')
  if (!dock) return 'no composer'
  // Every control the composer draws, in order, so the count can be compared
  // with the drawing's "7 objects" and "4 objects, strictly more said".
  const controls = [...dock.querySelectorAll('button')].map((b) => {
    const label = b.innerText.split(/\\s+/).join(' ').trim()
    return (label.length > 0 ? label : (b.getAttribute('aria-label') || '?')).slice(0, 40)
  })
  return controls.length + ' controls >> ' + controls.join('  |  ')
})()`

const OPEN_PICKER = `(async () => {
  const control = [...document.querySelectorAll('.lc-control')].find((b) => b.getAttribute('aria-haspopup') === 'listbox')
  if (!control) return 'no route control'
  control.click()
  await new Promise((r) => setTimeout(r, 1400))
  const picker = document.querySelector('.lc-picker')
  if (!picker) return 'picker did not open'
  const flat = (el) => el.innerText.split(/\\s+/).join(' ').trim()
  const head = picker.querySelector('.lc-picker__head')
  const efforts = picker.querySelectorAll('.lc-effortpanel__notch')
  const held = picker.querySelector('.lc-effortpanel__now')
  return [
    'head: ' + (head ? flat(head).slice(0, 70) : 'none'),
    'effort chips: ' + efforts.length + (efforts.length ? ' >> ' + [...efforts].map(flat).join(' ') : ''),
    'held line: ' + (held ? flat(held) : 'none')
  ].join('  ||  ')
})()`

try {
  await drive.capture('the composer, as built', async () => {
    await drive.ready()
    return drive.evaluate(OPEN_WORKROOM)
  })

  await drive.capture('the picker, as built', () => drive.evaluate(OPEN_PICKER))
  // The "swarm on" frame pressed a pill in the composer; swarm is a Settings switch now, so there is none to press.
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({
    intro: 'The composer and picker as the build actually draws them, for comparison with page 1 of the reference.'
  })
}
