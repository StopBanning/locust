// Is the first screen's line centred under the machine? Measured on the built
// app at three window sizes. Sends nothing; no run starts.
//
//   node _tools/probe-first-screen-centre.mjs [--packaged <exe>] [--tag <name>]
//
// Colin, 2026-09-24, with a frame of the first screen (the machine, the three
// teammates on it, and "Your coding agents, on your own accounts. OpenCode
// works without one." under it): "text is way off center". Read off the
// screen: the machine's centre, the line's centre (its text, not its box),
// and the column both sit in.

import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import { localPath, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const OUT = join(localPath(new URL('../docs/beta-fixes-2026-09-24/', import.meta.url)), `first-screen-centre-${tag}`)
await mkdir(OUT, { recursive: true })

const drive = await startDrive({
  name: `first-screen-centre-${tag}`,
  port: 9547,
  workspace: await scratchRepository('locust-first-screen-ws-'),
  outPath: OUT,
  sendsNothing: true,
  ...(packaged === undefined ? {} : { packaged }),
  seed: { schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}

const MEASURE = `(() => {
  const box = (el) => { if (!el) return null; const r = el.getBoundingClientRect(); return { left: Math.round(r.left), right: Math.round(r.right), width: Math.round(r.width), mid: Math.round(r.left + r.width / 2) } }
  const intro = document.querySelector('.lc-intro')
  let text = null
  if (intro) {
    const range = document.createRange()
    range.selectNodeContents(intro)
    const rects = [...range.getClientRects()]
    const left = Math.min(...rects.map((r) => r.left))
    const right = Math.max(...rects.map((r) => r.right))
    text = { left: Math.round(left), right: Math.round(right), mid: Math.round((left + right) / 2), lines: rects.length }
  }
  return JSON.stringify({
    column: box(document.querySelector('.lc-empty__inner')),
    cover: box(document.querySelector('.lc-cover')),
    machine: box(document.querySelector('.lc-cover__machineslot')),
    introBox: box(intro),
    text,
    words: intro?.textContent?.trim() ?? null
  })
})()`

try {
  await drive.ready()
  const results = []
  for (const [width, height] of [[1215, 800], [1600, 900], [1920, 1040]]) {
    await drive.resize(width, height)
    await sleep(900)
    await drive.waitForSelector('.lc-intro', { timeoutMs: 60_000, what: 'the first screen line' })
    await sleep(600)
    const at = JSON.parse(String(await drive.evaluate(MEASURE)))
    const shot = await drive.send('Page.captureScreenshot', { format: 'png' })
    if (shot?.result?.data) await writeFile(join(OUT, `first-screen-${String(width)}.png`), Buffer.from(shot.result.data, 'base64'))
    say(`${String(width)} x ${String(height)}: ${JSON.stringify(at)}`)
    results.push({ width, ...at })
  }
  for (const at of results) {
    const off = at.text.mid - at.machine.mid
    check(`at ${String(at.width)} wide, the line is centred under the machine`, Math.abs(off) <= 2, `the line's centre is ${String(off)} px from the machine's`)
  }
  say(failures === 0 ? '\nFIRST SCREEN CENTRE PASSED' : `\nFIRST SCREEN CENTRE: ${String(failures)} FAILED`)
} catch (error) {
  say(`probe failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'whatever pnpm build last wrote to out/'}. The first screen with no teammates, at 1215, 1600 and 1920 wide.` })
}
