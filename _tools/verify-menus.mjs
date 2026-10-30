// Do the composer's menus open UPWARD, inside the window?
//
//   node _tools/verify-menus.mjs
//
// Colin, 2026-09-05: "permissions box folding under". The permission-mode
// menu opens from a control at the very bottom of the window, so it has to
// grow upward. A design pass had added a SECOND `.lc-menu` rule for a
// workroom overflow menu; the markup for that menu was later reverted, the
// CSS was not, and being later in the file it won the cascade and flipped
// `bottom:` to `top:`. The composer's menu then opened downward, off the
// bottom edge.
//
// This is geometry, not a screenshot: a menu 200px past the bottom edge
// still renders perfectly in a screenshot of the page. It opens each menu
// for real and reads its rectangle against the viewport, and it carries its
// own control -- with the deleted rule restored, the same check must fail.

import { spawn } from 'node:child_process'
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { localPath } from './local-path.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const PORT = 9303
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const say = (line) => console.error(line)
let failures = 0
const check = (label, ok, detail = '') => {
  if (ok) say(`  [PASS] ${label}${detail ? ` -- ${detail}` : ''}`)
  else {
    failures += 1
    say(`  [FAIL] ${label}${detail ? ` -- ${detail}` : ''}`)
  }
}

const profile = await mkdtemp(join(tmpdir(), 'locust-menus-'))
const workspace = await mkdtemp(join(tmpdir(), 'locust-menus-ws-'))
await mkdir(profile, { recursive: true })
await writeFile(join(workspace, 'status.ts'), 'export const status = "draft";\n', 'utf8')

try {
  const already = await fetch(`http://127.0.0.1:${String(PORT)}/json/list`, { signal: AbortSignal.timeout(1500) })
  if (already.ok) {
    say(`  [FAIL] something is already debugging on port ${String(PORT)}`)
    process.exit(1)
  }
} catch {
  // Nothing listening, which is what we want.
}

const child = spawn(ELECTRON, [APP_DIR, `--remote-debugging-port=${String(PORT)}`, `--user-data-dir=${profile}`], {
  cwd: workspace,
  stdio: ['ignore', 'pipe', 'pipe']
})
child.stdout.on('data', () => undefined)
child.stderr.on('data', () => undefined)

try {
  let page
  for (let i = 0; i < 80 && page === undefined; i += 1) {
    await sleep(500)
    if (child.exitCode !== null) throw new Error(`app exited ${String(child.exitCode)}`)
    try {
      const list = await (await fetch(`http://127.0.0.1:${String(PORT)}/json/list`)).json()
      page = list.find((t) => t.type === 'page' && t.webSocketDebuggerUrl && !t.url.includes('#splash'))
    } catch { /* not up */ }
  }
  if (page === undefined) throw new Error('renderer never came up')
  const socket = new WebSocket(page.webSocketDebuggerUrl)
  await new Promise((res) => socket.addEventListener('open', res, { once: true }))
  let id = 0
  const pending = new Map()
  socket.addEventListener('message', (e) => {
    const m = JSON.parse(e.data)
    const w = pending.get(m.id)
    if (w) { pending.delete(m.id); w(m) }
  })
  const send = (method, params = {}) =>
    Promise.race([
      new Promise((res) => {
        const n = ++id
        pending.set(n, res)
        socket.send(JSON.stringify({ id: n, method, params }))
      }),
      // An unref()'d timer, so a finished smoke is not held open for a minute
      // by a timeout that already lost its race.
      new Promise((resolve) => { const t = setTimeout(() => resolve({ error: { message: 'cdp timeout' } }), 60_000); t.unref() })
    ])
  const evaluate = async (expression) => {
    const m = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })
    if (m.error) throw new Error(JSON.stringify(m.error))
    if (m.result?.exceptionDetails) throw new Error(m.result.exceptionDetails.exception?.description ?? 'evaluate threw')
    return m.result?.result?.value
  }

  await evaluate(`(async () => {
    for (let i = 0; i < 240; i += 1) {
      const f = document.querySelector('form.command-dock textarea')
      if (f && !/Checking local runtimes/.test(f.placeholder)) return true
      await new Promise(r => setTimeout(r, 250))
    }
    return false
  })()`)
  await send('Emulation.setDeviceMetricsOverride', { width: 1280, height: 860, deviceScaleFactor: 1, mobile: false })
  await sleep(400)

  // `which` picks the menu by POSITION on the bar -- 0 is the permission
  // control, the last is the route -- because matching on aria-label silently
  // fell back to the first menu and reported the route's geometry as the
  // mode's. Two identical numbers were the tell.
  const measure = async (which, oldRule) => {
    return JSON.parse(await evaluate(`(async () => {
      // Close by pressing the control that opened it -- these menus toggle,
      // and nothing here listens for a click on the document, so clicking
      // the page background left the menu open forever.
      const close = async () => {
        for (const trigger of document.querySelectorAll('form.command-dock button[aria-haspopup]')) {
          if (!document.querySelector('.lc-menu, .lc-picker')) return true
          if (trigger.getAttribute('aria-expanded') === 'true') {
            trigger.click()
            await new Promise(r => setTimeout(r, 250))
          }
        }
        return !document.querySelector('.lc-menu, .lc-picker')
      }
      if (!(await close())) return JSON.stringify({ found: false, why: 'a menu would not close' })

      document.getElementById('menu-control-probe')?.remove()
      if (${oldRule ? 'true' : 'false'}) {
        const style = document.createElement('style')
        style.id = 'menu-control-probe'
        // The rule exactly as the reverted design pass left it.
        style.textContent = '.lc-menu { position: absolute; top: calc(100% + var(--lc-space-3)); right: 0; min-width: 190px; }'
        document.head.append(style)
      }

      // The route control is a LISTBOX, not a menu: selecting on
      // aria-haspopup="menu" found two buttons, neither of them the route,
      // and the probe reported the route picker as never opening.
      const buttons = [...document.querySelectorAll('form.command-dock button[aria-haspopup]')]
      const target = WHICH === 'mode'
        ? buttons.find(b => b.getAttribute('aria-haspopup') === 'menu')
        : buttons.find(b => b.getAttribute('aria-haspopup') === 'listbox')
      if (!target) return JSON.stringify({ found: false, why: 'no menu button', buttons: buttons.length })
      target.click()
      await new Promise(r => setTimeout(r, 400))
      // The route control opens a picker DIALOG, not a menu -- looking only
      // for the menu class reported 'the menu did not open' for a surface
      // that had opened perfectly well. (No backticks in here: this whole
      // block is inside a template literal, and one closes it.)
      const menu = WHICH === 'mode' ? document.querySelector('.lc-menu') : document.querySelector('.lc-picker')
      if (!menu) return JSON.stringify({ found: false, why: 'the menu did not open' })
      const box = menu.getBoundingClientRect()
      const anchor = target.getBoundingClientRect()
      // How much of it a person can actually SEE. A menu pushed past the
      // bottom edge can still report a sane rectangle; what it cannot do is
      // show its own content.
      const visible = Math.max(0, Math.min(box.bottom, window.innerHeight) - Math.max(box.top, 0))
      return JSON.stringify({
        found: true,
        label: menu.getAttribute('aria-label') ?? '',
        buttons: buttons.length,
        viewport: window.innerHeight,
        top: Math.round(box.top),
        bottom: Math.round(box.bottom),
        height: Math.round(box.height),
        visible: Math.round(visible),
        below: Math.round(box.bottom - window.innerHeight),
        opensUpward: box.bottom <= Math.round(anchor.bottom) + 1
      })
    })()`.replace(/WHICH/g, JSON.stringify(which))))
  }

  const seen = []
  for (const [which, name] of [['mode', 'Permission mode'], ['route', 'Route']]) {
    const now = await measure(which, false)
    if (now.found !== true) {
      check(`the ${name} menu opens`, false, JSON.stringify(now))
      continue
    }
    say(`       ${name}: ${JSON.stringify(now)}`)
    seen.push(now)
    check(`the ${name} menu is fully visible`, now.visible === now.height && now.top >= 0, JSON.stringify(now))
    check(`and it opens upward, away from the bottom edge`, now.opensUpward === true, JSON.stringify(now))
  }
  // The two menus are different sizes at different places on the bar, so two
  // identical rectangles mean the probe measured one of them twice -- which
  // it silently did on the first run.
  check(
    'the two measurements are of two different menus',
    seen.length === 2 && (seen[0].top !== seen[1].top || seen[0].label !== seen[1].label),
    seen.map((entry) => `${entry.label}@${String(entry.top)}`).join(' vs ')
  )

  const old = await measure('mode', true)
  say(`       control: ${JSON.stringify(old)}`)
  // The deleted rule opens the menu DOWNWARD from a control at the bottom of
   // the window. What a person sees is a sliver -- "folding under" -- so the
   // control asserts on visibility, not on a raw coordinate.
  check(
    'CONTROL: the deleted rule leaves it all but invisible, so this check can fail',
    old.found === true && old.opensUpward === false && old.visible < 20,
    JSON.stringify(old)
  )
} catch (error) {
  failures += 1
  say(`  [FAIL] ${error instanceof Error ? error.message : String(error)}`)
} finally {
  try { child.kill() } catch { /* gone */ }
  await sleep(1200)
  await rm(profile, { recursive: true, force: true }).catch(() => undefined)
  await rm(workspace, { recursive: true, force: true }).catch(() => undefined)
}

if (failures > 0) {
  say(`\n${String(failures)} MENU FAILURE(S)`)
  process.exit(1)
}
say('\nMENUS OK')
