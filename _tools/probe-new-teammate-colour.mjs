// Does a new teammate start on a colour the team does not wear yet?
//
//   node _tools/probe-new-teammate-colour.mjs [--packaged <exe>] [--tag <name>]
//
// The design review (#7): "same default accent for every new teammate". With
// Wren (lime) already on the team, opens New teammate from the + menu and
// reads which colour is selected. Creates nobody; sends nothing.

import { mkdir } from 'node:fs/promises'
import { join } from 'node:path'

import { localPath, say, scratchRepository, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const OUT = join(localPath(new URL('../docs/beta-fixes-2026-09-23/', import.meta.url)), `new-teammate-colour-${tag}`)
await mkdir(OUT, { recursive: true })

const drive = await startDrive({
  name: `new-teammate-colour-${tag}`,
  port: 9429,
  workspace: await scratchRepository('locust-colour-ws-'),
  sendsNothing: true,
  outPath: OUT,
  ...(packaged === undefined ? {} : { packaged }),
  seed: {
    schemaVersion: 1,
    teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-09-05T05:00:00.000Z' }],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false }
  }
})

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}

try {
  await drive.ready()
  await drive.resize(1215, 800)
  const picked = JSON.parse(await drive.capture('New teammate, with a lime teammate on the team', () => drive.evaluate(`(async () => {
    const add = [...document.querySelectorAll('button[aria-label="Add"]')].find((b) => b.getBoundingClientRect().width > 0)
    if (!add) return JSON.stringify({ error: 'no + button' })
    add.click()
    await new Promise((r) => setTimeout(r, 300))
    const item = [...document.querySelectorAll('.lc-menu__item')].find((b) => /New teammate/.test(b.textContent))
    if (!item) return JSON.stringify({ error: 'no New teammate item' })
    item.click()
    for (let i = 0; i < 20 && !document.querySelector('.lc-hues'); i += 1) await new Promise((r) => setTimeout(r, 150))
    const checked = document.querySelector('.lc-hues [role="radio"][aria-checked="true"]')
    return JSON.stringify({ colour: checked?.getAttribute('aria-label') ?? '' })
  })()`)))
  say(`selected: ${JSON.stringify(picked)}`)
  check('the dialog starts on a colour Wren does not wear', picked.colour.length > 0 && picked.colour !== 'Lime', picked.colour || picked.error)
  check('the next one along, Blue', picked.colour === 'Blue', picked.colour)
  say(failures === 0 ? '\nNEW TEAMMATE COLOUR PASSED' : `\nNEW TEAMMATE COLOUR: ${String(failures)} FAILED`)
} catch (error) {
  say(`probe failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: 'New teammate opened with a lime teammate already on the team. Nobody was created; nothing was sent.' })
}
