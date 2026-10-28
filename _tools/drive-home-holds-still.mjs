// Home holds still at every window width (0.516).
//
//   node _tools/drive-home-holds-still.mjs [--packaged <exe>] [--tag <name>] [--team <n>]
//
// --team 6 (0.838): six teammates and the Meet Scout card. On 0.837 Home
// flicked every frame from 882px tall up (766px with three): tight spacing
// shrank its own sections, which argued it loose again (Colin, 2026-10-27:
// "twitchy home screen bug is back").
//
// Colin, 2026-10-01, three teammates on Home: "when the screen resized to fit
// 1 row it started stuttering ... shaking like crazy". The cover grows into
// the height left over; the team cards wrap by width; a scrollbar takes width.
// At the wrong width those three chase each other. Home with three teammates
// is opened at widths around Colin's (1209x770), and at each the cover's
// height and the team's rows are sampled for two seconds: they must settle.
// Sends nothing.

import { join } from 'node:path'
import { recordRoot, say, scratchRepository, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'
const team = Number(arg('--team') ?? '3')
const EXTRA = [
  { name: 'Ada', hue: 'clay' }, { name: 'Max', hue: 'lime' }, { name: 'Ivy', hue: 'violet' },
  { name: 'Kit', hue: 'blue' }, { name: 'Lou', hue: 'clay' }
]
const workspace = await scratchRepository('locust-drive-home-still-ws-')
const route = { runtime: 'opencode', model: 'opencode/mimo-v2.6-flash-free', mode: 'ask' }
const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `home-holds-still-${tag}`,
  port: 9831,
  workspace,
  sendsNothing: true,
  outPath: join(recordRoot('home-holds-still-2026-10-01'), tag),
  seed: {
    schemaVersion: 1,
    teammates: [
      { teammateId: 'tm_robin', name: 'Robin', hue: 'lime', role: 'Custom', roleTitle: 'Finance Bro', createdAt: '2026-09-05T05:00:00.000Z', route },
      { teammateId: 'tm_ghost', name: 'Ghost', hue: 'violet', role: 'Code & Migrations', createdAt: '2026-09-05T05:01:00.000Z', route },
      { teammateId: 'tm_boss', name: 'Boss', hue: 'blue', role: 'Custom', roleTitle: 'Chief of Staff', createdAt: '2026-09-05T05:02:00.000Z', route },
      ...EXTRA.slice(0, Math.max(0, team - 3)).map((mate, i) => ({ teammateId: `tm_${mate.name.toLowerCase()}`, name: mate.name, hue: mate.hue, role: 'Custom', roleTitle: 'Helper', createdAt: `2026-09-05T05:1${String(i)}:00.000Z`, route }))
    ],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' }
  }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 260)}`}`)
}

try {
  await drive.capture('launch', () => drive.ready())
  // What Home loads after it opens (the Meet Scout card, the usage line) has arrived before the first size.
  await drive.evaluate('new Promise((r) => setTimeout(r, 4000))')
  const shaking = []
  for (let width = 1150; width <= 1320; width += 10) {
    for (const height of team > 3 ? [770, 793, 800, 860, 884, 890, 900] : [740, 766, 770, 800]) {
      await drive.resize(width, height)
      const seen = JSON.parse(String(await drive.evaluate(`(async () => {
        await new Promise((r) => setTimeout(r, 700))
        const sample = () => {
          const cover = document.querySelector('.lc-cover')?.getBoundingClientRect()
          const cards = [...document.querySelectorAll('.lc-hometeam__card, .lc-hometeam button, .lc-hometeam li')].map((el) => Math.round(el.getBoundingClientRect().top))
          return (cover ? Math.round(cover.height) : -1) + ':' + new Set(cards).size
        }
        const states = []
        for (let i = 0; i < 40; i += 1) {
          states.push(sample())
          await new Promise((r) => setTimeout(r, 50))
        }
        let changes = 0
        for (let i = 1; i < states.length; i += 1) if (states[i] !== states[i - 1]) changes += 1
        return JSON.stringify({ changes, distinct: [...new Set(states)] })
      })()`)))
      if (seen.changes > 1) shaking.push({ width, height, ...seen })
    }
  }
  if (shaking.length > 0) await drive.resize(shaking[0].width, shaking[0].height)
  await drive.capture(shaking.length > 0 ? `still shaking at ${String(shaking[0].width)}x${String(shaking[0].height)}` : 'held still at every size', () => drive.evaluate('1'))
  check(`Home holds still with ${String(team)} teammates at every width from 1150 to 1320`, shaking.length === 0, JSON.stringify(shaking.slice(0, 4)))
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. Home, ${String(team)} teammates.`, extra: `Checks failed: ${String(failures)}` })
}
if (failures > 0) process.exitCode = 1
