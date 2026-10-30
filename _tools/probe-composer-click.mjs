// How much of the chat box starts a message when clicked?
//
//   node _tools/probe-composer-click.mjs [--packaged <exe>] [--tag <name>]
//
// Colin, 2026-09-23: "only a very small portion of the chat window is
// actually clickable to begin chatting". The box is a one-line field above
// a row of chips, inside padding; a click anywhere but the field's own line
// did nothing. This presses the mouse -- real input events, not element
// clicks -- on a grid of points across the box, skipping the box's own
// controls (the +, the chips, send), and counts how many leave the caret in
// the message field. SPENDS NOTHING.

import { mkdir } from 'node:fs/promises'
import { join } from 'node:path'

import { localPath, say, scratchRepository, startDrive, teammateFace } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const OUT = join(localPath(new URL('../docs/beta-fixes-2026-09-23/', import.meta.url)), `composer-click-${tag}`)
await mkdir(OUT, { recursive: true })

const drive = await startDrive({
  name: `composer-click-${tag}`,
  port: 9440,
  workspace: await scratchRepository('locust-composer-click-ws-'),
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

const CONTROL = 'button, a, input, select, [role="button"], [role="menu"], [role="menuitem"], [role="listbox"], [role="option"]'

try {
  await drive.ready()
  await drive.resize(1215, 800)
  await drive.evaluate(`(async () => { ${teammateFace('Wren')}?.click(); await new Promise((r) => setTimeout(r, 900)) })()`)
  const layout = JSON.parse(await drive.capture('the chat box', () => drive.evaluate(`(() => {
    const box = document.querySelector('form.command-dock .lc-composer__box')
    const field = document.querySelector('textarea[aria-label="Message"]')
    if (!box || !field) return JSON.stringify({ error: 'no chat box' })
    const b = box.getBoundingClientRect()
    const f = field.getBoundingClientRect()
    // Every point on a 12px grid inside the box that is NOT one of its controls.
    const points = []
    for (let y = Math.ceil(b.top) + 4; y < b.bottom - 3; y += 12) {
      for (let x = Math.ceil(b.left) + 4; x < b.right - 3; x += 12) {
        const hit = document.elementFromPoint(x, y)
        if (!hit || !box.contains(hit) || hit.closest(${JSON.stringify(CONTROL)})) continue
        points.push({ x, y, onField: hit === field })
      }
    }
    return JSON.stringify({
      box: [Math.round(b.width), Math.round(b.height)],
      field: [Math.round(f.width), Math.round(f.height)],
      points
    })
  })()`)))
  if (layout.error !== undefined) throw new Error(layout.error)
  say(`box ${layout.box.join('x')}, field ${layout.field.join('x')}; ${String(layout.points.length)} points in the box off its controls, ${String(layout.points.filter((p) => p.onField).length)} of them on the field itself`)

  let focused = 0
  const missed = []
  for (const point of layout.points) {
    await drive.evaluate(`(() => { document.activeElement?.blur(); return true })()`)
    await drive.send('Input.dispatchMouseEvent', { type: 'mousePressed', x: point.x, y: point.y, button: 'left', buttons: 1, clickCount: 1 })
    await drive.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: point.x, y: point.y, button: 'left', buttons: 0, clickCount: 1 })
    const inField = await drive.evaluate(`document.activeElement?.getAttribute('aria-label') === 'Mission instruction'`)
    if (inField === true) focused += 1
    else missed.push(`${String(point.x)},${String(point.y)}`)
  }
  const share = layout.points.length === 0 ? 0 : focused / layout.points.length
  say(`clicks that left the caret in the field: ${String(focused)} of ${String(layout.points.length)} (${String(Math.round(share * 100))}%)`)
  if (missed.length > 0) say(`missed at: ${missed.slice(0, 12).join(' ')}${missed.length > 12 ? ' ...' : ''}`)
  check('a click anywhere in the chat box that is not one of its controls starts a message', share >= 0.97, `${String(Math.round(share * 100))}%`)

  // And the controls still do their own thing: the + opens its menu, not the field.
  const plus = JSON.parse(await drive.evaluate(`(() => {
    const plus = [...document.querySelectorAll('form.command-dock button')].find((b) => /attach|plus|\\+/i.test(b.getAttribute('aria-label') ?? b.textContent ?? ''))
    if (!plus) return JSON.stringify(null)
    const r = plus.getBoundingClientRect()
    return JSON.stringify({ x: Math.round(r.left + r.width / 2), y: Math.round(r.top + r.height / 2), label: plus.getAttribute('aria-label') })
  })()`))
  if (plus !== null) {
    await drive.evaluate(`(() => { document.activeElement?.blur(); return true })()`)
    await drive.send('Input.dispatchMouseEvent', { type: 'mousePressed', x: plus.x, y: plus.y, button: 'left', buttons: 1, clickCount: 1 })
    await drive.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: plus.x, y: plus.y, button: 'left', buttons: 0, clickCount: 1 })
    await new Promise((r) => setTimeout(r, 400))
    const where = await drive.evaluate(`document.activeElement?.getAttribute('aria-label') ?? document.activeElement?.tagName ?? ''`)
    check('the + is still the +: pressing it does not hand its click to the field', where !== 'Mission instruction', `${String(plus.label)} -> focus on ${String(where)}`)
  }
  say(failures === 0 ? '\nCOMPOSER CLICK PASSED' : `\nCOMPOSER CLICK: ${String(failures)} FAILED`)
} catch (error) {
  say(`probe failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: 'Mouse presses on a grid across the chat box, off its controls: how many start a message. Nothing was sent.' })
}
