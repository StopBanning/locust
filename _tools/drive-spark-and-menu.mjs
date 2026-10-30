// The working sign in monochrome, and a / menu that shows the runtime's own
// section without scrolling (0.429). The sidebar's spark this drive was named
// for is gone; the sign is the live step's orb and the face's dot (2026-10-30).
//
//   node _tools/drive-spark-and-menu.mjs [--packaged <exe>] [--tag <name>]
//
// Free model (FREE_ROUTE: LOCUST_FREE_MODEL, DeepSeek V4.1 Flash by default): spends nothing.
//
// Colin, 2026-09-28: "since we don't really use any lime accents just make
// the working/thinking sidebar notifier monochrome to match the rest of the
// ui." Ada (OpenCode) is asked something slow enough to watch; while it runs
// the spark's colour is read off the page, and the sidebar is pictured. Then,
// after the run, a bare slash: is OpenCode's section heading inside the
// menu's visible height, at 1440x900 and at 1280x720?
//
// PRIVACY. OpenCode's menu section names every skill on the machine, so the
// menu is measured, never pictured.

import { mkdir } from 'node:fs/promises'
import { join } from 'node:path'

import { FREE_ROUTE, openTeammateScript, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'
// The drives' free route: nemotron-3-ultra-free, named here before, left OpenCode's free list by 2026-10-30.
const MODEL = FREE_ROUTE.model
const OUT = join(recordRoot('spark-and-menu-2026-09-28'), `spark-and-menu-${tag}`)
await mkdir(OUT, { recursive: true })

const drive = await startDrive({
  name: `spark-and-menu-${tag}`, port: 9766, workspace: await scratchRepository('locust-drive-spark-ws-'), outPath: OUT,
  ...(packaged === undefined ? {} : { packaged }),
  seed: {
    schemaVersion: 1,
    teammates: [{ teammateId: 'tm_ada', name: 'Ada', hue: 'violet', role: 'Code & Migrations', createdAt: '2026-09-28T01:00:00.000Z', route: { runtime: 'opencode', model: MODEL, mode: 'ask' } }],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false }
  }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}
const type = (text) => `(async () => {
  const field = document.querySelector('form.command-dock textarea')
  field.focus()
  const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
  setter.call(field, ${JSON.stringify(text)})
  field.dispatchEvent(new Event('input', { bubbles: true }))
  await new Promise((r) => setTimeout(r, 400))
  return 'typed'
})()`
/** Where OpenCode's heading sits in the menu, once it is there. */
const headingInView = `(async () => {
  for (let i = 0; i < 60 && !document.querySelector('.lc-slash .lc-slash__group'); i += 1) await new Promise((r) => setTimeout(r, 500))
  const menu = document.querySelector('.lc-slash')
  const heading = menu?.querySelector('.lc-slash__group')
  if (!menu || !heading) return JSON.stringify({ menu: menu !== null, heading: false })
  const box = menu.getBoundingClientRect()
  const at = heading.getBoundingClientRect()
  return JSON.stringify({ menuHeight: Math.round(box.height), headingTop: Math.round(at.top - box.top), headingBottom: Math.round(at.bottom - box.top), inView: at.bottom <= box.bottom && at.top >= box.top, menuTop: Math.round(box.top) })
})()`

try {
  await drive.ready()
  await drive.resize(1440, 900)
  await sleep(2000)
  await drive.evaluate(openTeammateScript('Ada'))
  await sleep(800)
  await drive.evaluate(type('Read every file in this folder and describe each in one sentence.'))
  await drive.evaluate(`document.querySelector('button[aria-label="Send"]')?.click()`)
  // 0.392's sidebar spark is gone from the page, and lime is retired (--lc-lime is the primary text colour). The working
  // sign is the live step's orb beside "Working…": every pixel and stroke it draws is read while Ada runs.
  const orb = JSON.parse(String(await drive.evaluate(`(async () => {
    for (let i = 0; i < 80 && !document.querySelector('.lc-livestep__orb'); i += 1) await new Promise((r) => setTimeout(r, 250))
    const el = document.querySelector('.lc-livestep__orb')
    if (!el) return JSON.stringify({ found: false })
    await new Promise((r) => setTimeout(r, 600))
    const colours = new Set()
    for (const canvas of el.querySelectorAll('canvas')) {
      const data = canvas.getContext('2d')?.getImageData(0, 0, canvas.width, canvas.height).data ?? []
      for (let i = 0; i < data.length; i += 4) if (data[i + 3] > 40) colours.add(data[i] + ',' + data[i + 1] + ',' + data[i + 2])
    }
    for (const shape of el.querySelectorAll('svg *')) {
      const style = getComputedStyle(shape)
      for (const paint of [style.fill, style.stroke]) {
        const rgb = paint.match(/^rgba?\\((\\d+), (\\d+), (\\d+)/)
        if (rgb) colours.add(rgb.slice(1, 4).join(','))
      }
    }
    const spread = Math.max(0, ...[...colours].map((one) => { const [r, g, b] = one.split(',').map(Number); return Math.max(r, g, b) - Math.min(r, g, b) }))
    return JSON.stringify({ found: true, drawn: colours.size, spread, sample: [...colours].slice(0, 6) })
  })()`)))
  await drive.capture('The sidebar while Ada works', () => drive.evaluate(`document.querySelector('.lc-sidebar, nav')?.innerText.replace(/\\s+/g, ' ').slice(0, 200) ?? ''`))
  say(`  orb: ${JSON.stringify(orb)}`)
  check('the working orb beside "Working…" is drawn in greys alone, no colour', orb.found && orb.drawn > 0 && orb.spread <= 16, JSON.stringify(orb))
  // The primary text colour as the page computes it, from a probe that wears it.
  const primaryRgb = String(await drive.evaluate(`(() => { const probe = document.createElement('span'); probe.style.color = 'var(--lc-text-primary)'; document.body.append(probe); const colour = getComputedStyle(probe).color; probe.remove(); return colour })()`))
  // 0.430: the face's working dot and the title bar's running dot too.
  const dots = JSON.parse(String(await drive.evaluate(`(async () => {
    for (let i = 0; i < 40 && !document.querySelector('.lc-presence'); i += 1) await new Promise((r) => setTimeout(r, 250))
    const face = document.querySelector('.lc-bot .lc-presence, .lc-face .lc-presence')
    const running = document.querySelector('.lc-runstate .lc-dot')
    return JSON.stringify({
      face: face ? getComputedStyle(face).backgroundColor : null,
      running: running ? getComputedStyle(running).backgroundColor : null
    })
  })()`)))
  say(`  dots: ${JSON.stringify(dots)}`)
  check("the working dot on Ada's face is the primary text colour", dots.face === primaryRgb, JSON.stringify({ ...dots, primaryRgb }))
  check('the dot beside "running" in the title bar is the primary text colour', dots.running === primaryRgb, JSON.stringify({ ...dots, primaryRgb }))

  await drive.evaluate(`(async () => {
    for (let i = 0; i < 600; i += 1) {
      await new Promise((r) => setTimeout(r, 500))
      if (i > 6 && !document.querySelector('button[aria-label^="Stop the running"]')) return 'ended'
    }
    return 'timed out'
  })()`)
  await sleep(1500)

  for (const [width, height] of [[1440, 900], [1280, 720]]) {
    await drive.resize(width, height)
    await sleep(800)
    await drive.evaluate(type('/'))
    const where = JSON.parse(String(await drive.evaluate(headingInView)))
    say(`  ${width}x${height}: ${JSON.stringify(where)}`)
    check(`at ${width}x${height}, a bare slash shows OpenCode's heading without scrolling, and the menu stays on screen`, where.inView === true && where.menuTop >= 0, JSON.stringify(where))
    await drive.evaluate(type(''))
  }
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged === undefined ? 'out/' : 'the packaged build'}. Ada on OpenCode / ${MODEL}, Ask. The spark's colour is read while she runs; the menu is measured, never pictured.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
