// Can a person SEE the three features, in the shipped build?
//
//   node _tools/probe-ways-in.mjs
//
// Colin has used this app daily for a week and has never made a room or a
// routine: "i still dont see anything for implementation of rooms in the main
// ui" and "i still have not seen that ability pop up for me". Both features
// are built, tested and shipped, so the question is not whether they work --
// it is whether the screen offers them at all.
//
// The PACKAGED binary, because that is the app he has. Nothing is run and
// nothing is sent: this opens the window and reads what is on it.

import { join } from 'node:path'

import { localPath, say, scratchRepository, startDrive, teammateFace } from './drive-lib.mjs'

const workspace = await scratchRepository('locust-waysin-ws-')
const packaged = join(
  localPath(new URL('../apps/desktop/release/win-unpacked/', import.meta.url)),
  'Locust.exe'
)

const drive = await startDrive({
  name: 'ways-in',
  port: 9459,
  workspace,
  packaged,
  seed: {
    schemaVersion: 1,
    teammates: [
      {
        teammateId: 'tm_wren',
        name: 'Wren',
        hue: 'lime',
        role: 'Code & Migrations',
        createdAt: '2026-09-05T05:00:00.000Z',
        route: { runtime: 'claude', model: 'sonnet', mode: 'accept-edits', effort: 'low' }
      }
    ],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false }
  }
})

try {
  await drive.capture('the sidebar, on a profile that has made nothing', async () => {
    await drive.ready()
    return drive.evaluate(`(async () => {
      const rail = document.querySelector('.lc-sidebar') ?? document.body
      const text = rail.innerText.replace(/\\s+/g, ' ').trim()
      return JSON.stringify({
        rooms: /new room/i.test(text),
        automations: /automations/i.test(text),
        text: text.slice(0, 320)
      }, null, 1)
    })()`)
  })

  // The premise, OUTSIDE capture(): this has to be the shipped build, or it
  // is reading a screen nobody has.
  const version = String(await drive.evaluate(`document.title + ' | ' + (document.body.innerText.match(/0[.][0-9]+[.][0-9]+/)?.[0] ?? 'no version on screen')`))
  say(`  ${version}`)

  await drive.capture('what the empty Automations shelf says a routine is', () => drive.evaluate(`(async () => {
    const link = [...document.querySelectorAll('button, a')].find(n => /automations/i.test(n.innerText ?? ''))
    if (link) link.click()
    await new Promise(r => setTimeout(r, 700))
    const screen = document.querySelector('.lc-automations') ?? document.querySelector('.lc-screen')
    if (!screen) return 'no screen'
    const head = screen.querySelector('.lc-screen__head')
    return JSON.stringify({
      headerHasChrome: head === null ? false : getComputedStyle(head).borderBottomWidth !== '0px',
      says: screen.innerText.replace(/\\s+/g, ' ').trim().slice(0, 400)
    }, null, 1)
  })()`))

  await drive.capture('and whether the mode chip says the connectors are off', () => drive.evaluate(`(async () => {
    const back = [...document.querySelectorAll('button, a')].find(n => /^missions$/i.test((n.innerText ?? '').trim()))
    if (back) back.click()
    await new Promise(r => setTimeout(r, 500))
    const rail = ${teammateFace('Wren')}
    if (rail) rail.click()
    await new Promise(r => setTimeout(r, 800))
    const chip = [...document.querySelectorAll('button')].find(b => b.getAttribute('aria-label') === 'Permission mode')
    if (!chip) return 'no mode chip'
    return JSON.stringify({
      mode: chip.innerText.replace(/\\s+/g, ' ').trim(),
      title: chip.getAttribute('title')
    }, null, 1)
  })()`))
} catch (error) {
  say(`probe failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: 'The PACKAGED 0.60.0 binary on a fresh profile with one teammate. Nothing was run and nothing was sent.' })
}
