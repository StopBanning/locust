// Does the title screen notice you? Measured on the built app.
//
//   node _tools/probe-cover-notices.mjs [--packaged <exe>] [--tag <name>]
//
// Colin's "have fun" list, 2026-09-23, and his answer, "you can run all
// those": the sleeping Hopper opens its eyes and watches a pointer that comes
// near, then dozes off again once it has gone; a click on the glass powers the
// lockup on again; turning swarm on flies a few small swarm bots up across the
// screen, once.
//
// Everything here is read off the drawing, not the props: the real mouse is
// moved over the window (Input.dispatchMouseEvent) and the Hopper's canvas is
// read -- how much dark ink its face has (shut eyes are a line, open ones are
// eyes) and where that ink sits (it moves toward the pointer); the lockup's
// running animations; each flyer's place on the screen over a second and a
// half, and that its canvas has a bot drawn in it. Sends nothing.

import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import { localPath, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const OUT = join(localPath(new URL('../docs/beta-fixes-2026-09-23/', import.meta.url)), `cover-notices-${tag}`)
await mkdir(OUT, { recursive: true })

const workspace = await scratchRepository('locust-cover-notices-ws-')
const drive = await startDrive({
  name: `cover-notices-${tag}`,
  port: 9445,
  workspace,
  outPath: OUT,
  sendsNothing: true,
  ...(packaged === undefined ? {} : { packaged }),
  seed: {
    schemaVersion: 1,
    teammates: [],
    missionOwners: {},
    settings: { relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false, tube: 'full' }
  }
})

let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}
const shoot = async (file) => {
  const shot = await drive.send('Page.captureScreenshot', { format: 'png' })
  if (shot?.result?.data) await writeFile(join(OUT, file), Buffer.from(shot.result.data, 'base64'))
}
const move = (x, y) => drive.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y, button: 'none', buttons: 0 })
const click = async (x, y) => {
  await drive.send('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', buttons: 1, clickCount: 1 })
  await drive.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', buttons: 0, clickCount: 1 })
}

// The Hopper: its state, where its box is, and its face's ink -- the pixels
// darker than the lime body, which are the eyes: how many, and their middle
// relative to the canvas's middle, in canvas pixels.
const HOPPER = `(() => {
  const bot = document.querySelector('.lc-cover__face .lc-bot[data-bot="hopper"]')
  if (!bot) return JSON.stringify({ missing: true })
  const canvas = bot.querySelector('canvas')
  const box = bot.getBoundingClientRect()
  const data = canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data
  let count = 0, sumX = 0, sumY = 0
  for (let y = 0; y < canvas.height; y += 1) {
    for (let x = 0; x < canvas.width; x += 1) {
      const i = (y * canvas.width + x) * 4
      if (data[i + 3] < 200) continue
      const lum = (0.2126 * data[i] + 0.7152 * data[i + 1] + 0.0722 * data[i + 2]) / 255
      if (lum < 0.16) { count += 1; sumX += x; sumY += y }
    }
  }
  return JSON.stringify({
    state: bot.dataset.state,
    box: { x: box.left + box.width / 2, y: box.top + box.height / 2, size: box.width },
    ink: count,
    inkX: count === 0 ? 0 : sumX / count - canvas.width / 2,
    inkY: count === 0 ? 0 : sumY / count - canvas.height / 2
  })
})()`
const hopper = async () => JSON.parse(String(await drive.evaluate(HOPPER)))
// A face's ink over half a second, averaged: it breathes and blinks.
const settledHopper = async () => {
  const reads = []
  for (let n = 0; n < 6; n += 1) {
    reads.push(await hopper())
    await sleep(90)
  }
  const open = reads.filter((read) => read.ink > 0)
  const mean = (key) => (open.length === 0 ? 0 : open.reduce((sum, read) => sum + read[key], 0) / open.length)
  return { state: reads.at(-1).state, box: reads.at(-1).box, ink: Math.round(Math.max(...reads.map((read) => read.ink))), inkX: Number(mean('inkX').toFixed(2)) }
}

try {
  await drive.ready()
  await drive.resize(1215, 800)
  await drive.waitFor(`document.querySelector('.lc-cover__face .lc-bot[data-bot="hopper"]')?.dataset.state !== 'still'`, { timeoutMs: 60_000, what: 'the cover on' })
  // The cover's bots rest while the window is not in front; a drive's window
  // may not be. Held in front, and told so.
  await drive.send('Emulation.setFocusEmulationEnabled', { enabled: true })
  await drive.evaluate(`window.dispatchEvent(new Event('focus'))`)
  await sleep(1500)

  // THE HOPPER.
  await move(20, 780)
  await sleep(1200)
  const asleep = await settledHopper()
  say(`far away: ${JSON.stringify(asleep)}`)
  check('the Hopper is asleep with nobody near', asleep.state === 'sleeping', asleep.state)
  const { x, y, size } = asleep.box
  await move(x - 0.8 * size, y)
  await sleep(1400)
  const left = await settledHopper()
  await shoot('01-the-hopper-awake.png')
  say(`pointer to its left: ${JSON.stringify(left)}`)
  check('a pointer coming near wakes it', left.state === 'default', left.state)
  check('its eyes open: more ink in its face than asleep', left.ink >= asleep.ink * 1.4 && left.ink - asleep.ink >= 20, `${String(asleep.ink)} -> ${String(left.ink)} dark pixels`)
  await move(x + 0.8 * size, y)
  await sleep(1400)
  const right = await settledHopper()
  say(`pointer to its right: ${JSON.stringify(right)}`)
  check('and it watches the pointer: its eyes follow it across', right.inkX - left.inkX >= 2, `eyes' middle ${String(left.inkX)} -> ${String(right.inkX)} canvas px`)
  await move(20, 780)
  await sleep(1500)
  const stillAwake = await hopper()
  check('it stays awake a moment after the pointer goes', stillAwake.state === 'default', stillAwake.state)
  await sleep(4500)
  const dozed = await settledHopper()
  say(`gone a while: ${JSON.stringify(dozed)}`)
  check('then dozes off again', dozed.state === 'sleeping', dozed.state)

  // THE GLASS.
  const glass = JSON.parse(String(await drive.evaluate(`(() => {
    const r = document.querySelector('.lc-cover__glass').getBoundingClientRect()
    return JSON.stringify({ x: r.left + r.width / 2, y: r.top + r.height * 0.8 })
  })()`)))
  await drive.waitFor(`(() => { const l = document.querySelector('.lc-lockup'); return l && !l.classList.contains('is-powering') && !l.classList.contains('is-relighting') })()`, { timeoutMs: 15_000, what: 'the lockup between lightings' })
  await click(glass.x, glass.y)
  await sleep(150)
  const lit = JSON.parse(String(await drive.evaluate(`(() => {
    const lockup = document.querySelector('.lc-lockup')
    return JSON.stringify({ powering: lockup.classList.contains('is-powering'), running: lockup.getAnimations({ subtree: true }).filter((a) => a.playState === 'running').map((a) => a.animationName ?? '').filter(Boolean) })
  })()`)))
  await shoot('02-the-glass-clicked.png')
  say(`after a click on the glass: ${JSON.stringify(lit)}`)
  check('a click on the glass powers the lockup on again, and it is animating', lit.powering === true && lit.running.includes('lcLockupPower'), JSON.stringify(lit))

  // (The swarm flight went with swarm, 0.880.)
  say(failures === 0 ? '\nCOVER NOTICES PASSED' : `\nCOVER NOTICES: ${String(failures)} FAILED`)
} catch (error) {
  say(`probe failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: 'The title screen with a real pointer moved over it: the Hopper, a click on the glass, and swarm turned on, off and on. Sends nothing.' })
}
