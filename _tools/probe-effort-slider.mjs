// How the effort slider answers a real mouse: a click on each dot, and a slow
// drag across the track. Measured on the built app. Sends nothing.
//
//   node _tools/probe-effort-slider.mjs [--packaged <exe>] [--tag <name>]
//
// Colin, 2026-09-23, with frames of both: "our effort slide is a little wonky
// and responds a little oddly compared to claudes, lets also make sure this
// gooey effect in the 3rd image on libraries.dev is used, might have to raise
// the intensity its not all that visual to the user rn".
//
// Read off the screen, not the props: which level each dot's click selects;
// then, pressing on the thumb and moving the real pointer across the track a
// few pixels at a time, where the drawn thumb is, where the fill ends, and
// how far the liquid under the thumb is from it -- every frame of the way.

import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import { localPath, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const OUT = join(localPath(new URL('../docs/beta-fixes-2026-09-24/', import.meta.url)), `effort-slider-${tag}`)
await mkdir(OUT, { recursive: true })

const workspace = await scratchRepository('locust-effort-slider-ws-')
const drive = await startDrive({
  name: `effort-slider-${tag}`,
  port: 9453,
  workspace,
  outPath: OUT,
  sendsNothing: true,
  ...(packaged === undefined ? {} : { packaged }),
  seed: { schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } }
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
const mouse = (type, x, y, buttons = 0) => drive.send('Input.dispatchMouseEvent', { type, x, y, button: type === 'mouseMoved' && buttons === 0 ? 'none' : 'left', buttons, clickCount: type === 'mouseMoved' ? 0 : 1 })

// The panel as drawn: the track, each dot, the thumb, the fill's end, the
// liquid blob under the thumb, and the level the head names.
const PANEL = `(() => {
  const scale = document.querySelector('.lc-effortpanel__scale')
  if (!scale) return JSON.stringify({ missing: true })
  const box = (el) => { if (!el) return null; const r = el.getBoundingClientRect(); return { left: r.left, top: r.top, width: r.width, height: r.height, mid: r.left + r.width / 2 } }
  const track = box(scale)
  const blob = [...document.querySelectorAll('.lc-effortpanel__liquid svg rect, .lc-effortpanel__liquid svg path, .lc-effortpanel__liquid svg ellipse, .lc-effortpanel__liquid svg circle')].map((el) => box(el)).filter((b) => b && b.width > 0)[0] ?? null
  return JSON.stringify({
    track,
    dots: [...document.querySelectorAll('.lc-effortpanel__notch')].map((el) => box(el).mid),
    thumb: box(document.querySelector('.lc-effortpanel__thumb'))?.mid ?? null,
    fillEnd: (() => { const f = box(document.querySelector('.lc-effortpanel__fill')); return f ? f.left + f.width : null })(),
    blob: blob?.mid ?? null,
    level: (document.querySelector('.lc-effortpanel__now')?.textContent ?? '').trim(),
    value: document.querySelector('.lc-effortpanel__slider')?.value ?? null
  })
})()`
const panel = async () => JSON.parse(String(await drive.evaluate(PANEL)))

try {
  await drive.ready()
  await drive.resize(1215, 800)
  await drive.waitFor(`!!document.querySelector('button[aria-label="Reasoning effort"]') && !document.querySelector('button[aria-label="Reasoning effort"]').disabled`, { timeoutMs: 60_000, what: 'the effort chip' })
  await sleep(1500)
  const chip = JSON.parse(String(await drive.evaluate(`(() => { const r = document.querySelector('button[aria-label="Reasoning effort"]').getBoundingClientRect(); return JSON.stringify({ x: r.left + r.width / 2, y: r.top + r.height / 2 }) })()`)))
  await mouse('mousePressed', chip.x, chip.y, 1)
  await mouse('mouseReleased', chip.x, chip.y)
  await sleep(700)
  const first = await panel()
  await shoot('01-the-panel.png')
  say(`the panel: ${JSON.stringify(first)}`)
  if (first.missing) throw new Error('the effort panel did not open')

  // A click on each dot.
  const y = first.track.top + first.track.height / 2
  const landed = []
  for (const [index, x] of first.dots.entries()) {
    await mouse('mousePressed', x, y, 1)
    await mouse('mouseReleased', x, y)
    await sleep(600)
    const now = await panel()
    landed.push({ dot: index, value: Number(now.value), level: now.level, thumbOff: Math.round((now.thumb - x) * 10) / 10 })
  }
  say(`a click on each dot: ${JSON.stringify(landed)}`)
  check('a click on a dot picks that dot', landed.every((entry) => entry.value === entry.dot), JSON.stringify(landed.map((entry) => entry.value)))
  check('and the thumb settles on it', landed.every((entry) => Math.abs(entry.thumbOff) <= 1.5), JSON.stringify(landed.map((entry) => entry.thumbOff)))

  // A slow drag from the first dot to the last, the real pointer, frame by frame.
  const start = first.dots[0]
  const end = first.dots[first.dots.length - 1]
  await mouse('mousePressed', start, y, 1)
  await sleep(120)
  await mouse('mousePressed', start, y, 1)
  const trace = []
  for (let x = start; x <= end; x += 4) {
    await mouse('mouseMoved', x, y, 1)
    await sleep(30)
    const now = await panel()
    trace.push({ pointer: Math.round(x), thumb: now.thumb === null ? null : Math.round(now.thumb), fill: now.fillEnd === null ? null : Math.round(now.fillEnd), blob: now.blob === null ? null : Math.round(now.blob), level: now.level })
  }
  await shoot('02-mid-drag-end.png')
  await mouse('mouseReleased', end, y)
  await sleep(700)
  const after = await panel()
  await writeFile(join(OUT, 'drag.json'), JSON.stringify(trace, null, 1))
  const lag = trace.map((row) => (row.thumb === null ? 0 : Math.abs(row.thumb - row.pointer)))
  const apart = trace.map((row) => (row.thumb === null || row.fill === null ? 0 : Math.abs(row.thumb - row.fill)))
  say(`drag: the thumb is on average ${String(Math.round(lag.reduce((s, v) => s + v, 0) / lag.length))} px from the pointer (most ${String(Math.max(...lag))}); the fill's end and the thumb up to ${String(Math.max(...apart))} px apart; after release: ${after.level}, value ${String(after.value)}`)
  check('while dragging, the thumb stays under the pointer', Math.max(...lag) <= 8, `most ${String(Math.max(...lag))} px`)
  check("and the fill's end stays with the thumb", Math.max(...apart) <= 8, `most ${String(Math.max(...apart))} px apart`)
  check('let go at the end, it is the last level', Number(after.value) === first.dots.length - 1, `${String(after.value)} of ${String(first.dots.length - 1)}`)

  // What the goo looks like, close up: a quick drag back to the first dot,
  // frames of the track at four times its size as it goes, and one after.
  const zoom = async (file) => {
    const shot = await drive.send('Page.captureScreenshot', { format: 'png', clip: { x: first.track.left - 16, y: first.track.top - 14, width: first.track.width + 32, height: first.track.height + 28, scale: 4 } })
    if (shot?.result?.data) await writeFile(join(OUT, file), Buffer.from(shot.result.data, 'base64'))
  }
  // Pressed twice, as the slow drag is: the first press alone did not start
  // a hold, so the "quick drag" frames showed a thumb standing still at Max,
  // and the keys below then had nowhere to step (2026-09-24, both builds).
  await mouse('mousePressed', end, y, 1)
  await sleep(120)
  await mouse('mousePressed', end, y, 1)
  // No screenshots during it: a clipped capture mid-drag made the page lose
  // the pointer, so the old frames here showed a drag that had already ended
  // (2026-09-24: "a quick drag lands there" failed on 0.326 as well).
  for (let x = end; x >= start; x -= 24) {
    await mouse('mouseMoved', x, y, 1)
    await sleep(16)
  }
  await mouse('mouseMoved', start, y, 1)
  await mouse('mouseReleased', start, y)
  await sleep(700)
  const back = await panel()
  check('a quick drag back to the first dot lands there', Number(back.value) === 0, `value ${String(back.value)}`)

  // The liquid, close up: a click on the last dot from the first jumps the
  // thumb, and the drop under it pours after on its spring. Frames as it goes.
  await mouse('mousePressed', end, y, 1)
  await mouse('mouseReleased', end, y)
  for (const [n, wait] of [[1, 10], [2, 40], [3, 60]]) {
    await sleep(wait)
    await zoom(`03-liquid-after-a-jump-${String(n)}.png`)
  }
  await sleep(700)
  await zoom('05-settled.png')

  // The thumb at rest, close up, where Colin's frames were taken: Medium, and
  // the last level (Claude Code's frame was at Max).
  await mouse('mousePressed', first.dots[1], y, 1)
  await mouse('mouseReleased', first.dots[1], y)
  await sleep(700)
  await zoom('06-rest-medium.png')
  await mouse('mousePressed', end, y, 1)
  await mouse('mouseReleased', end, y)
  await sleep(700)
  await zoom('07-rest-last.png')
  await mouse('mousePressed', start, y, 1)
  await mouse('mouseReleased', start, y)
  await sleep(700)

  // The keyboard still moves it: the range input is the control for keys.
  const key = async (name, code) => {
    await drive.send('Input.dispatchKeyEvent', { type: 'rawKeyDown', key: name, code: name, windowsVirtualKeyCode: code })
    await drive.send('Input.dispatchKeyEvent', { type: 'keyUp', key: name, code: name, windowsVirtualKeyCode: code })
  }
  await drive.evaluate(`document.querySelector('.lc-effortpanel__slider').focus()`)
  const before = await panel()
  await key('ArrowRight', 39)
  await sleep(500)
  const right = await panel()
  await key('ArrowRight', 39)
  await sleep(500)
  const twice = await panel()
  say(`keys: ${String(before.value)} -> ${String(right.value)} -> ${String(twice.value)}; the thumb ${String(Math.round(before.thumb))} -> ${String(Math.round(twice.thumb))}`)
  check('the arrow keys still step it, and the thumb goes with it', Number(right.value) === Number(before.value) + 1 && Number(twice.value) === Number(before.value) + 2 && twice.thumb > before.thumb + 20, `${String(before.value)} -> ${String(right.value)} -> ${String(twice.value)}`)
  say(failures === 0 ? '\nEFFORT SLIDER PASSED' : `\nEFFORT SLIDER: ${String(failures)} FAILED`)
} catch (error) {
  say(`probe failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: 'The effort panel with the real mouse: a click on every dot, then a slow drag across. Sends nothing.' })
}
