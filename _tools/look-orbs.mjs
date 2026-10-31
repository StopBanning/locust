// The thinking and working orbs as the screen shows them, magnified so their pixels can be seen (2026-10-30).
//
//   node _tools/look-orbs.mjs --label <name> [--packaged <app's executable>] [--out <folder>]
//
// Colin, on the Mac: "the thinking/working orbs appear to not look right on mac or be as clear as they are as
// windows". One message on the free OpenCode route; while it runs, the live line's orb is watched, and the first
// time each state shows, the window is photographed and the orb cut out at 8x with no smoothing, so every device
// pixel is a block. Run it on the Mac and on Windows and put the sheets side by side. Spends nothing.
//
// Each crop is `<state>.png` beside `orbs.json`: the state, which drawing it was (the painted-down 64 design or the
// library's own 20), its canvas's backing pixels and CSS size, the window's devicePixelRatio and the platform.

import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import { pickRouteScript, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const label = arg('--label') ?? process.platform
const packaged = arg('--packaged')
const OUT = arg('--out') ?? join(process.cwd(), `orb-crops-${label}`)
const ZOOM = 8
await mkdir(OUT, { recursive: true })

const workspace = await scratchRepository('locust-look-orbs-ws-')
const drive = await startDrive({
  name: `look-orbs-${label}`,
  port: 9396,
  workspace,
  ...(packaged === undefined ? {} : { packaged }),
  seed: { schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } }
})

// The orb's box and a margin, cut from a photograph of the whole window and enlarged in the page itself with
// smoothing off -- so what is saved is the screen's own pixels, each drawn as a ZOOM x ZOOM block.
const crop = (png, box) => drive.evaluate(`(async () => {
  const image = new Image()
  image.src = 'data:image/png;base64,${png}'
  await image.decode()
  const scale = image.width / innerWidth
  const pad = 6
  const sx = Math.max(0, Math.round((${box.x} - pad) * scale)), sy = Math.max(0, Math.round((${box.y} - pad) * scale))
  const side = Math.round((${box.css} + pad * 2) * scale)
  const canvas = document.createElement('canvas')
  canvas.width = side * ${ZOOM}
  canvas.height = side * ${ZOOM}
  const context = canvas.getContext('2d')
  context.imageSmoothingEnabled = false
  context.drawImage(image, sx, sy, side, side, 0, 0, side * ${ZOOM}, side * ${ZOOM})
  return canvas.toDataURL('image/png').split(',')[1]
})()`)

const seen = new Map()
try {
  await drive.ready()
  say(await drive.evaluate(pickRouteScript({ group: '/opencode/i', search: 'free', row: '/free/i' })))
  const route = await drive.evaluate(`[...document.querySelectorAll('.lc-control')].find(b => b.getAttribute('aria-haspopup') === 'listbox')?.innerText.replace(/\\s+/g, ' ').trim() ?? ''`)
  if (!/free/i.test(route)) throw new Error(`refusing to send: the composer is on "${route}"`)
  await drive.evaluate(`(async () => {
    const field = document.querySelector('form.command-dock textarea')
    const set = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
    set.call(field, 'Read README.md. Then make notes.md with a three-step plan for this project. Then search every file here for the word project, list the folder, and finally summarize everything in five sentences.')
    field.dispatchEvent(new Event('input', { bubbles: true }))
    await new Promise((r) => setTimeout(r, 300))
    document.querySelector('button[aria-label="Send"]')?.click()
    return 'sent'
  })()`)
  for (let i = 0; i < 900; i += 1) {
    await sleep(120)
    const found = await drive.evaluate(`(() => {
      const holder = document.querySelector('.lc-livestep__orb')
      const canvas = holder?.querySelector('canvas')
      if (!holder || !canvas) return null
      const box = canvas.getBoundingClientRect()
      return JSON.stringify({ state: holder.getAttribute('data-orb'), backing: canvas.width, css: Math.round(box.width), x: box.left, y: box.top, dpr: devicePixelRatio, theme: document.documentElement.dataset.theme ?? null })
    })()`)
    if (found === null || found === undefined) {
      if (seen.size > 0 && i > 50) {
        const done = await drive.evaluate(`!/\\d+ running/.test(document.body.innerText)`)
        if (done) break
      }
      continue
    }
    const info = JSON.parse(found)
    if (info.state === null || seen.has(info.state)) continue
    // A moment for the drawing to settle on its first frames.
    await sleep(400)
    // What the orb's canvas holds, and in the same moment where it is: the line can move between two asks.
    const held = await drive.evaluate(`(() => {
      const canvas = document.querySelector('.lc-livestep__orb canvas')
      if (!canvas) return null
      const box = canvas.getBoundingClientRect()
      const big = document.createElement('canvas')
      big.width = canvas.width * ${ZOOM}
      big.height = canvas.height * ${ZOOM}
      const context = big.getContext('2d')
      context.imageSmoothingEnabled = false
      context.fillStyle = getComputedStyle(document.body).backgroundColor
      context.fillRect(0, 0, big.width, big.height)
      context.drawImage(canvas, 0, 0, big.width, big.height)
      return JSON.stringify({ x: box.left, y: box.top, css: Math.round(box.width), png: big.toDataURL('image/png').split(',')[1] })
    })()`)
    const shot = await drive.send('Page.captureScreenshot', { format: 'png' })
    if (held === null || held === undefined) continue
    const where = JSON.parse(held)
    await writeFile(join(OUT, `${info.state}-canvas.png`), Buffer.from(where.png, 'base64'))
    const png = await crop(shot.result.data, where)
    await writeFile(join(OUT, `${info.state}-screen.png`), Buffer.from(png, 'base64'))
    seen.set(info.state, { ...info, drawing: info.css === 20 ? 'library 20' : 'painted-down 64', platform: process.platform })
    say(`${info.state}: ${String(info.backing)} backing px in ${String(info.css)} css px, dpr ${String(info.dpr)}`)
  }
  await writeFile(join(OUT, 'orbs.json'), `${JSON.stringify([...seen.values()], null, 2)}\n`, 'utf8')
  say(`${String(seen.size)} states photographed into ${OUT}`)
} catch (error) {
  say(`look failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `The orbs, ${label}, magnified ${String(ZOOM)}x. One free OpenCode message.` })
}
