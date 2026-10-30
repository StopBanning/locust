// Does a room nobody has posted to yet have a row in the sidebar?
//
//   node _tools/probe-empty-room-row.mjs [--packaged <exe>] [--tag <name>]
//
// A room made and left had no row -- a row stood for a room's answers, and
// an empty room has none. Colin left the call to me ("I'll let you choose
// design choice"): it is listed, where its making puts it. Seeds one empty
// room and reads the sidebar. SPENDS NOTHING.

import { mkdir, mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { localPath, say, scratchRepository, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const OUT = join(localPath(new URL('../docs/beta-fixes-2026-09-23/', import.meta.url)), `empty-room-row-${tag}`)
await mkdir(OUT, { recursive: true })

const profile = await mkdtemp(join(tmpdir(), 'locust-empty-room-'))
await mkdir(join(profile, 'mission-ledger'), { recursive: true })
const T0 = '2026-09-23T05:00:00.000Z'
await writeFile(
  join(profile, 'teammates.json'),
  JSON.stringify({
    schemaVersion: 1,
    teammates: [
      { teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: T0 },
      { teammateId: 'tm_booty', name: 'Booty', hue: 'blue', role: 'Custom', createdAt: T0 }
    ],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2 }
  }),
  'utf8'
)
await writeFile(
  join(profile, 'rooms.json'),
  JSON.stringify({
    schemaVersion: 1,
    rooms: [{ roomId: 'room_fresh', name: 'Release planning', teammateIds: ['tm_wren', 'tm_booty'], createdAt: T0, posts: [], tasks: [] }]
  }),
  'utf8'
)

const drive = await startDrive({
  name: `empty-room-row-${tag}`,
  port: 9432,
  workspace: await scratchRepository('locust-empty-room-ws-'),
  profilePath: profile,
  sendsNothing: true,
  outPath: OUT,
  ...(packaged === undefined ? {} : { packaged })
})

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}

try {
  await drive.ready()
  await drive.resize(1215, 800)
  const rows = JSON.parse(await drive.capture('the sidebar, with an empty room', () => drive.evaluate(`(async () => {
    for (let i = 0; i < 40 && !document.querySelector('.lc-conv--room'); i += 1) await new Promise((r) => setTimeout(r, 250))
    return JSON.stringify([...document.querySelectorAll('.lc-conv--room')].map((row) => ({
      name: row.querySelector('.lc-conv__title')?.textContent.trim() ?? '',
      age: row.querySelector('.lc-conv__age')?.textContent.trim() ?? ''
    })))
  })()`)))
  say(`room rows: ${JSON.stringify(rows)}`)
  check('the room nobody has posted to is in the sidebar', rows.some((row) => row.name === 'Release planning'), JSON.stringify(rows))
  check('with its age from when it was made', rows.some((row) => row.name === 'Release planning' && row.age.length > 0), JSON.stringify(rows))
  const opened = await drive.evaluate(`(async () => {
    const row = [...document.querySelectorAll('.lc-conv--room')].find((node) => /Release planning/.test(node.textContent))
    row?.click()
    await new Promise((r) => setTimeout(r, 900))
    return document.querySelector('.lc-room__name')?.textContent.trim() ?? ''
  })()`)
  check('and it opens the room', opened === 'Release planning', opened)
  say(failures === 0 ? '\nEMPTY ROOM ROW PASSED' : `\nEMPTY ROOM ROW: ${String(failures)} FAILED`)
} catch (error) {
  say(`probe failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: 'A room nobody has posted to yet, as the sidebar lists it. Nothing was sent.' })
}
