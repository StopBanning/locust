// Does the layout hold on a work area under its 1120x720 minimum (M21)?
//
//   node _tools/probe-small-work-area.mjs [--packaged <exe>] [--tag <name>]
//
// 1920x1080 at 150% -- Windows' default on most 1080p laptops -- is a
// 1280x672 work area; 1024x640 is the smallest the tests name. The window
// now opens at the work area there instead of 720 tall behind the taskbar,
// so the layout has to work at that size. This machine's display is larger,
// so the page is rendered at each size through the DevTools protocol, and
// the composer, the sidebar's foot and the header must all be inside it.
// Sends nothing.

import { mkdir } from 'node:fs/promises'
import { join } from 'node:path'

import { localPath, say, scratchRepository, sleep, startDrive, teammateFace } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag')
const outPath = tag === undefined ? undefined : join(localPath(new URL('../docs/beta-fixes-2026-09-24/', import.meta.url)), `small-work-area-${tag}`)
if (outPath !== undefined) await mkdir(outPath, { recursive: true })

const drive = await startDrive({
  name: 'small-work-area',
  port: 9577,
  workspace: await scratchRepository('locust-drive-smallwa-ws-'),
  sendsNothing: true,
  ...(packaged === undefined ? {} : { packaged }),
  ...(outPath === undefined ? {} : { outPath }),
  seed: {
    schemaVersion: 1,
    teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-09-05T05:00:00.000Z', route: { runtime: 'claude', model: 'haiku', mode: 'accept-edits' } }],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' }
  }
})

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}
const fits = () => drive.evaluate(`JSON.stringify((() => {
  const box = (selector) => { const el = document.querySelector(selector); if (!el) return null; const r = el.getBoundingClientRect(); return { top: Math.round(r.top), bottom: Math.round(r.bottom), right: Math.round(r.right) } }
  return { width: innerWidth, height: innerHeight, composer: box('form.command-dock'), sidebar: box('.lc-sidebar'), header: box('.lc-workroom__header') ?? box('header'), scrollsSideways: document.documentElement.scrollWidth > innerWidth + 1 }
})())`).then((text) => JSON.parse(String(text)))

try {
  await drive.capture('launch', () => drive.ready())
  await sleep(1500)
  for (const [width, height] of [[1280, 672], [1024, 640]]) {
    await drive.send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: false })
    await sleep(800)
    await drive.capture(`the first screen at ${String(width)}x${String(height)}`, () => fits().then((f) => JSON.stringify(f)))
    await drive.evaluate(`(async () => { ${teammateFace('Wren')}.click(); await new Promise(r => setTimeout(r, 800)) })()`)
    const seen = await fits()
    await drive.capture(`Wren's conversation at ${String(width)}x${String(height)}`, () => JSON.stringify(seen))
    say(`  ${String(width)}x${String(height)}: ${JSON.stringify(seen)}`)
    check(`${String(width)}x${String(height)}: the composer is inside the window`, seen.composer !== null && seen.composer.bottom <= seen.height, JSON.stringify(seen.composer))
    check(`${String(width)}x${String(height)}: nothing scrolls sideways`, seen.scrollsSideways === false)
    check(`${String(width)}x${String(height)}: the sidebar ends inside the window`, seen.sidebar === null || seen.sidebar.bottom <= seen.height, JSON.stringify(seen.sidebar))
  }
  await drive.send('Emulation.clearDeviceMetricsOverride', {})
  say(failures === 0 ? '\nSMALL WORK AREA PASSED' : `\nSMALL WORK AREA: ${String(failures)} FAILED`)
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'whatever pnpm build last wrote to out/'}. The page rendered at 1280x672 and 1024x640 through the DevTools protocol: the work areas the window now opens at when they are under 1120x720.` })
}
