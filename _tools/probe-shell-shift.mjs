// Does anything shift the whole window? (0.366)
//
//   node _tools/probe-shell-shift.mjs [--packaged <exe>] [--tag <name>]
//
// A drive's scrollIntoView moved the whole window up about 11px, title bar
// and all, while a Chief of Staff's exchange ran (packaged 0.366). A
// container with `overflow: hidden` is still a scroll container: anything
// that scrolls an element into view scrolls it too, if its content is even
// a pixel taller than it. The app does that itself -- a Settings search
// lands on its section with scrollIntoView. This lists every hidden-overflow
// container whose content is taller or wider than it is, then does what the
// app does and reports anything that moved.

import { mkdir } from 'node:fs/promises'
import { join } from 'node:path'

import { localPath, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag')
const outPath = tag === undefined ? undefined : join(localPath(new URL('../docs/beta-fixes-2026-09-24/', import.meta.url)), `shell-shift-${tag}`)
if (outPath !== undefined) await mkdir(outPath, { recursive: true })

const workspace = await scratchRepository('locust-probe-shell-shift-ws-')
const drive = await startDrive({
  name: 'shell-shift',
  port: 9619,
  workspace,
  ...(packaged === undefined ? {} : { packaged }),
  ...(outPath === undefined ? {} : { outPath }),
  seed: { schemaVersion: 1, teammates: [], missionOwners: {} }
})

/** Hidden-overflow containers holding more than they show, and any that have moved. */
const HIDDEN = `JSON.stringify([...document.querySelectorAll('*')].filter((el) => {
  const style = getComputedStyle(el)
  const hidden = /hidden|clip/.test(style.overflowY) || /hidden|clip/.test(style.overflowX)
  return hidden && (el.scrollHeight > el.clientHeight + 1 || el.scrollWidth > el.clientWidth + 1 || el.scrollTop > 0 || el.scrollLeft > 0)
}).map((el) => ({
  el: (el.className && typeof el.className === 'string' ? '.' + el.className.split(' ').slice(0, 2).join('.') : el.tagName).slice(0, 48),
  over: [el.scrollWidth - el.clientWidth, el.scrollHeight - el.clientHeight],
  moved: [el.scrollLeft, el.scrollTop]
})).filter((entry) => entry.moved[0] > 0 || entry.moved[1] > 0 || /shell|main|workroom|sidebar|settings|screen|app|root/i.test(entry.el)).slice(0, 14))`

try {
  await drive.capture('launch', () => drive.ready())
  await drive.resize(1440, 900)
  await sleep(1200)
  const home = await drive.capture('Home: what overflows while hidden', () => drive.evaluate(HIDDEN))
  say(`home: ${home}`)
  // Which of the shell's own children reach past its bottom edge, and by how much.
  const children = await drive.evaluate(`(() => {
    const shell = document.querySelector('.lc-shell')
    if (!shell) return 'no shell'
    const box = shell.getBoundingClientRect()
    const style = getComputedStyle(shell)
    return JSON.stringify({
      shell: { height: Math.round(box.height), scrollHeight: shell.scrollHeight, display: style.display, rows: style.gridTemplateRows, padding: style.padding },
      window: innerHeight,
      children: [...shell.children].map((child) => {
        const r = child.getBoundingClientRect()
        return { el: (typeof child.className === 'string' ? child.className.split(' ').slice(0, 2).join('.') : child.tagName).slice(0, 40), top: Math.round(r.top - box.top), bottom: Math.round(r.bottom - box.top), marginBottom: getComputedStyle(child).marginBottom }
      })
    })
  })()`)
  say(`children: ${children}`)
  // The deepest elements whose bottom edge is past the window's.
  const below = await drive.evaluate(`JSON.stringify([...document.querySelectorAll('.lc-shell *')].map((el) => ({ el, r: el.getBoundingClientRect() }))
    .filter(({ r }) => r.height > 0 && r.bottom > innerHeight + 0.5)
    .map(({ el, r }) => ({ el: (typeof el.className === 'string' && el.className.length > 0 ? '.' + el.className.split(' ').slice(0, 2).join('.') : el.tagName).slice(0, 44), bottom: Math.round(r.bottom), position: getComputedStyle(el).position, visible: getComputedStyle(el).visibility, opacity: getComputedStyle(el).opacity }))
    .slice(-12))`)
  say(`below: ${below}`)
  // And whether a plain focus on the chat box moves the window, as a person's click would.
  const focused = await drive.evaluate(`(async () => {
    const field = document.querySelector('form.command-dock textarea')
    field?.focus()
    await new Promise((r) => setTimeout(r, 300))
    const shell = document.querySelector('.lc-shell')
    return 'after focus, shell scrollTop ' + String(shell?.scrollTop) + ', root scrollTop ' + String(document.scrollingElement.scrollTop)
  })()`)
  say(`focus: ${focused}`)
  // The shift itself, reproduced: scroll the chat box into view, as a drive
  // (or any future code) would, and see whether the window moved (0.366).
  const scrolled = await drive.evaluate(`(async () => {
    document.querySelector('form.command-dock')?.scrollIntoView({ block: 'start' })
    await new Promise((r) => setTimeout(r, 300))
    const shell = document.querySelector('.lc-shell')
    const title = document.querySelector('.lc-titlebar')?.getBoundingClientRect().top ?? 0
    return JSON.stringify({ shellScrollTop: shell?.scrollTop ?? -1, titleTop: Math.round(title) })
  })()`)
  say(`scrolled: ${scrolled}`)
  const settings = await drive.capture('a Settings search lands on its section', () => drive.evaluate(`(async () => {
    window.dispatchEvent(new KeyboardEvent('keydown', { key: '3', ctrlKey: true, bubbles: true }))
    await new Promise((r) => setTimeout(r, 900))
    const search = document.querySelector('input[aria-label="Search settings"]')
    if (!search) return 'no settings search'
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set
    setter.call(search, 'own models')
    search.dispatchEvent(new Event('input', { bubbles: true }))
    await new Promise((r) => setTimeout(r, 1200))
    return ${HIDDEN}
  })()`))
  say(`settings: ${settings}`)
} catch (error) {
  say(`probe failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'whatever pnpm build last wrote to out/'}. Hidden-overflow containers that hold more than they show, and whether a Settings search moves the window.` })
}
