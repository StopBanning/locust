// Does the first Home screen fit a laptop? (0.360)
//
//   node _tools/probe-home-fits.mjs [--packaged <exe>] [--tag <name>]
//
// The first-session drive on 0.358 showed the cover's middle bot frame
// touching the top edge at 1440x900 with nobody on the team, and the page
// scrolling. This measures, at the sizes laptops come in, with nobody on the
// team: how far the page overflows, whether anything sits ABOVE the top of
// its scroller at scrollTop 0 (cut off where no scroll reaches), and where
// the chat box starts. Numbers, not judgement; a screenshot per size.

import { mkdir } from 'node:fs/promises'
import { join } from 'node:path'

import { localPath, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag')
const outPath = tag === undefined ? undefined : join(localPath(new URL('../docs/beta-fixes-2026-09-24/', import.meta.url)), `home-fits-${tag}`)
if (outPath !== undefined) await mkdir(outPath, { recursive: true })

const workspace = await scratchRepository('locust-probe-home-fits-ws-')
const drive = await startDrive({
  name: 'home-fits',
  port: 9618,
  workspace,
  ...(packaged === undefined ? {} : { packaged }),
  ...(outPath === undefined ? {} : { outPath }),
  seed: { schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: true, relayHopCap: 2, memoryMode: 'on' } }
})

const MEASURE = `(() => {
  const cover = document.querySelector('.lc-cover')
  // The pane FirstLaunch measures and scrolls: .lc-empty around .lc-empty__inner.
  const scroller = cover?.closest('.lc-empty') ?? document.querySelector('main')
  scroller.scrollTop = 0
  const box = scroller.getBoundingClientRect()
  const tops = [...scroller.querySelectorAll('*')].map((el) => el.getBoundingClientRect()).filter((r) => r.width > 0 && r.height > 0)
  const highest = Math.min(...tops.map((r) => r.top))
  const dock = document.querySelector('form.command-dock')?.getBoundingClientRect()
  const agents = document.querySelector('.lc-agenthead')
  return JSON.stringify({
    window: innerWidth + 'x' + innerHeight,
    scroller: scroller.className.slice(0, 40),
    overflowPx: Math.round(scroller.scrollHeight - scroller.clientHeight),
    cutAboveTopPx: Math.max(0, Math.round(box.top - highest)),
    coverTop: cover ? Math.round(cover.getBoundingClientRect().top - box.top) : null,
    dockTop: dock ? Math.round(dock.top) : null,
    agentsFolded: agents ? agents.classList.contains('is-folded') : null,
    // The window's own root must never scroll: everything scrolls inside it.
    rootOverflowPx: Math.round(document.scrollingElement.scrollHeight - document.scrollingElement.clientHeight),
    rootOverflowXPx: Math.round(document.scrollingElement.scrollWidth - document.scrollingElement.clientWidth),
    // What sits above the top, by class, and whether a person could see it.
    above: [...scroller.querySelectorAll('*')]
      .map((el) => ({ el, r: el.getBoundingClientRect() }))
      .filter(({ r }) => r.width > 0 && r.height > 0 && r.top < box.top - 1)
      .map(({ el, r }) => (typeof el.className === 'string' ? el.className : el.tagName).slice(0, 36) + '@' + Math.round(r.top - box.top) + (getComputedStyle(el).opacity === '0' || getComputedStyle(el).visibility === 'hidden' ? '(unseen)' : ''))
      .slice(0, 12)
  })
})()`

try {
  await drive.capture('launch: nobody on the team', () => drive.ready())
  for (const [width, height] of [[1440, 900], [1366, 768], [1280, 800], [1920, 1080]]) {
    await drive.resize(width, height)
    await sleep(1500)
    const measured = await drive.capture(`Home at ${width}x${height}`, () => drive.evaluate(MEASURE))
    say(measured)
  }
} catch (error) {
  say(`probe failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'whatever pnpm build last wrote to out/'}. Home with nobody on the team, at four laptop sizes: overflow, anything cut above the top, where the chat box starts.` })
}
