// The message box on one row, as Claude Code's (0.837; Colin, 2026-10-27: "we can condense this like claude code now
// that we have this new setup"). The + left of the field, voice typing, Talk and send right of it.
//
//   node _tools/drive-one-row-box.mjs [--packaged <exe>] [--tag <name>]
//
// Sends nothing. With Wren picked, at three window sizes: empty, the box is one row no taller than 60px, the + and the
// send centred on the line of text, every button inside the box; with six lines typed, the box grows, and the + and
// the send stay on its last line.

import { mkdir } from 'node:fs/promises'
import { join } from 'node:path'

import { FREE_ROUTE, recordRoot, say, scratchRepository, sleep, startDrive, teammateFace } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? 'local'
const OUT = join(recordRoot('one-row-box-2026-10-27'), `one-row-box-${tag}`)
await mkdir(OUT, { recursive: true })
const drive = await startDrive({
  name: `one-row-box-${tag}`, port: 9741, workspace: await scratchRepository('locust-one-row-ws-'), outPath: OUT, sendsNothing: true,
  ...(packaged === undefined ? {} : { packaged }),
  seed: { schemaVersion: 1, teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-10-27T05:00:00.000Z', route: FREE_ROUTE }], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}
const type = (text) => `(() => {
  const field = document.querySelector('form.command-dock textarea')
  field.focus()
  const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
  setter.call(field, ${JSON.stringify(text)})
  field.dispatchEvent(new Event('input', { bubbles: true }))
})()`
const measure = `JSON.stringify((() => {
  const box = document.querySelector('.lc-composer__box')
  const at = (el) => { const b = el.getBoundingClientRect(); return { l: Math.round(b.left), t: Math.round(b.top), r: Math.round(b.right), b: Math.round(b.bottom), cy: Math.round((b.top + b.bottom) / 2) } }
  const buttons = [...box.querySelectorAll('button')].filter((b) => b.offsetParent !== null)
  return {
    box: at(box),
    field: at(box.querySelector('textarea')),
    plus: at(box.querySelector('.lc-plusmenu__main') ?? box.querySelector('.lc-composer__group:first-child')),
    send: at(box.querySelector('.lc-send')),
    buttons: buttons.map((b) => ({ name: b.getAttribute('aria-label') ?? b.innerText.trim(), ...at(b) }))
  }
})())`
const SIX = 'First line of a longer message\nsecond\nthird\nfourth\nfifth\nsixth'

try {
  await drive.ready()
  await drive.resize(1440, 900)
  await drive.evaluate(`${teammateFace('Wren')}?.click()`)
  await sleep(800)
  // The model chip under the box (0.836 holds model and effort in one chip); the box itself does not wait on it.
  await drive.waitFor(`document.querySelector('.lc-composer__under [aria-haspopup="listbox"]') !== null`, { timeoutMs: 60_000, what: "the model chip under the box" })
  await sleep(2000)
  for (const [width, height] of [[1440, 900], [1120, 720], [860, 700]]) {
    await drive.resize(width, height)
    await drive.evaluate(type(''))
    await sleep(900)
    const size = `${String(width)}x${String(height)}`
    const empty = JSON.parse(String(await drive.capture(`${size} empty`, () => drive.evaluate(measure))))
    const height1 = empty.box.b - empty.box.t
    check(`${size} empty: one row, no taller than 60px`, height1 <= 60, `${String(height1)}px`)
    check(`${size} empty: the +, the line of text and send share a centre`, Math.abs(empty.plus.cy - empty.send.cy) <= 2 && Math.abs(empty.field.cy - empty.send.cy) <= 2, `+ ${String(empty.plus.cy)}, text ${String(empty.field.cy)}, send ${String(empty.send.cy)}`)
    check(`${size} empty: + left of the text, send right of it`, empty.plus.r <= empty.field.l && empty.field.r <= empty.send.l, JSON.stringify({ plus: empty.plus.r, field: [empty.field.l, empty.field.r], send: empty.send.l }))
    const outside = empty.buttons.filter((b) => b.l < empty.box.l || b.r > empty.box.r || b.t < empty.box.t || b.b > empty.box.b)
    check(`${size} empty: every button inside the box`, outside.length === 0, outside.map((b) => b.name).join(', ') || `${String(empty.buttons.length)} buttons`)
    await drive.evaluate(type(SIX))
    await sleep(700)
    const tall = JSON.parse(String(await drive.capture(`${size} six lines`, () => drive.evaluate(measure))))
    check(`${size} six lines: the box grows`, tall.box.b - tall.box.t >= 100, `${String(tall.box.b - tall.box.t)}px`)
    check(`${size} six lines: the + and send stay on the last line`, Math.abs(tall.plus.b - tall.send.b) <= 2 && tall.box.b - tall.send.b <= 12, `+ bottom ${String(tall.plus.b)}, send bottom ${String(tall.send.b)}, box bottom ${String(tall.box.b)}`)
  }
  await drive.evaluate(type(''))
  // The + opens its menu from the row's left end now, not from under the field.
  await drive.resize(1120, 720)
  await sleep(700)
  await drive.evaluate(`document.querySelector('.lc-plusmenu__main')?.click()`)
  await sleep(900)
  const menu = JSON.parse(String(await drive.capture('the + menu open', () => drive.evaluate(`JSON.stringify((() => {
    const shown = [...document.querySelectorAll('.lc-plusmenu.is-open .lc-plusmenu__satellite')].filter((el) => el.offsetParent !== null)
    return { open: document.querySelector('.lc-plusmenu.is-open') !== null, inside: shown.every((el) => { const b = el.getBoundingClientRect(); return b.left >= 0 && b.top >= 0 && b.right <= innerWidth && b.bottom <= innerHeight }), items: shown.map((el) => el.getAttribute('aria-label') ?? el.innerText.trim()) }
  })())`))))
  check('the + menu opens whole inside the window', menu.open && menu.inside && menu.items.length >= 2, JSON.stringify(menu))
  await drive.evaluate(`document.querySelector('.lc-plusmenu__main')?.click()`)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged === undefined ? 'out/' : 'the packaged build'}. Wren picked; the message box measured empty and with six lines, at three sizes. Sends nothing.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
