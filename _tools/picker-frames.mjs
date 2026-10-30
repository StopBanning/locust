// The model picker, photographed: before and after a change to its rows.
//
//   node _tools/picker-frames.mjs --label after --reuse <profile>
//   node _tools/picker-frames.mjs --label before --packaged <Locust.exe> --reuse <profile>
//
// Opens the picker on the home screen three ways -- as it opens, searched to
// one runtime (Claude Code), and searched to the longest list (Cursor) -- at
// the default window size. Sends nothing.

import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import { localPath, say, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const label = arg('--label')
const reuse = arg('--reuse')
const packaged = arg('--packaged')
if (label === undefined || reuse === undefined) throw new Error('--label <name> --reuse <profile> [--packaged <exe>]')

const OUT = join(localPath(new URL('../docs/picker-frames-2026-09-22/', import.meta.url)), label)
await mkdir(OUT, { recursive: true })
await writeFile(join(reuse, 'window.json'), JSON.stringify({ x: 0, y: 0, width: 1477, height: 920, maximized: false }), 'utf8')

const drive = await startDrive({
  name: `picker-frames-${label}`,
  port: 9361,
  workspace: process.cwd(),
  profilePath: reuse,
  keep: true,
  sendsNothing: true,
  ...(packaged === undefined ? {} : { packaged })
})

const shoot = async (file) => {
  await sleep(700)
  const shot = await drive.send('Page.captureScreenshot', { format: 'png' })
  await writeFile(join(OUT, file), Buffer.from(shot.result.data, 'base64'))
  say(`frame ${file}`)
}
const openPicker = (search) => drive.evaluate(`(async () => {
  document.querySelector('button.lc-brand__lockup')?.click()
  await new Promise(r => setTimeout(r, 700))
  const control = [...document.querySelectorAll('.lc-control')].find(b => b.getAttribute('aria-haspopup') === 'listbox')
  if (!document.querySelector('.lc-picker')) control?.click()
  await new Promise(r => setTimeout(r, 700))
  const box = document.querySelector('.lc-picker__input')
  if (!box) return 'no picker'
  const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set
  set.call(box, ${JSON.stringify(search)})
  box.dispatchEvent(new Event('input', { bubbles: true }))
  await new Promise(r => setTimeout(r, 500))
  const rows = document.querySelectorAll('.lc-picker__row').length
  const list = document.querySelector('.lc-picker__list')
  const visible = [...document.querySelectorAll('.lc-picker__row')].filter((row) => {
    const box = row.getBoundingClientRect(), frame = list.getBoundingClientRect()
    return box.top >= frame.top && box.bottom <= frame.bottom
  }).length
  return rows + ' rows, ' + visible + ' fully in view'
})()`)
const closePicker = () => drive.evaluate(`document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); 'closed'`)

try {
  await drive.ready()
  say(`as it opens: ${await openPicker('')}`)
  await shoot('01-picker-open.png')
  await closePicker()
  say(`claude: ${await openPicker('claude')}`)
  await shoot('02-picker-claude.png')
  await closePicker()
  say(`cursor: ${await openPicker('cursor')}`)
  await shoot('03-picker-cursor.png')
} catch (error) {
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Picker frames, ${label}.` })
}
