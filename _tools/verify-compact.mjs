// Does a small window stay clean?
//
//   node _tools/verify-compact.mjs
//
// The 0.21.2 QA pass at 900x650: "the collapsed sidebar shows a horizontal
// scrollbar and clipped labels". Below 1120px the sidebar becomes a 68px
// rail and hides its prose; anything that escapes that rule scrolls or
// clips. This reads the geometry: no horizontal overflow at the document,
// none inside the rail, no prose left visible in it, and the composer still
// above the fold. Its control forces the two things the fix relies on back
// off (the rail's overflow clip and the hidden empty-state sentence) and the
// same check must fail.

import { spawn } from 'node:child_process'
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { localPath } from './local-path.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const PORT = 9305
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

const profile = await mkdtemp(join(tmpdir(), 'locust-compact-'))
const workspace = await mkdtemp(join(tmpdir(), 'locust-compact-ws-'))
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
      new Promise((resolve) => { const t = setTimeout(() => resolve({ error: { message: 'cdp timeout' } }), 60_000); t.unref() })
    ])
  const evaluate = async (expression) => {
    const m = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })
    if (m.error) throw new Error(JSON.stringify(m.error))
    if (m.result?.exceptionDetails) throw new Error(m.result.exceptionDetails.exception?.description ?? 'evaluate threw')
    return m.result?.result?.value
  }

  await evaluate(`(async () => {
    for (let i = 0; i < 360; i += 1) {
      const f = document.querySelector('form.command-dock textarea')
      const painted = document.querySelector('.lc-runtimepanel')
      if (f && painted && !/Checking local runtimes/.test(f.placeholder)) return true
      await new Promise(r => setTimeout(r, 250))
    }
    return false
  })()`)

  const measure = async (width, height, control) => {
    await send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: false })
    await sleep(400)
    return JSON.parse(await evaluate(`(async () => {
      document.getElementById('compact-control-probe')?.remove()
      if (${control ? 'true' : 'false'}) {
        // The pre-fix state: the rail free to overflow and its empty-state
        // sentence visible again, both of which 0.21.5 closed.
        const style = document.createElement('style')
        style.id = 'compact-control-probe'
        style.textContent = '.lc-sidebar { overflow: visible !important; } .lc-sidebar__empty { display: block !important; }'
        document.head.append(style)
      }
      await new Promise(r => setTimeout(r, 250))
      const doc = document.documentElement
      const sidebar = document.querySelector('.lc-sidebar')
      const empty = document.querySelector('.lc-sidebar__empty')
      const dock = document.querySelector('form.command-dock').getBoundingClientRect()
      const emptyBox = empty ? empty.getBoundingClientRect() : null
      const emptyVisible = empty !== null && getComputedStyle(empty).display !== 'none' && emptyBox.width > 0
      // Anything inside the rail wider than the rail.
      const railWidth = sidebar.clientWidth
      const spill = [...sidebar.querySelectorAll('*')]
        .map(el => el.getBoundingClientRect())
        .filter(b => b.width > 0 && b.right > sidebar.getBoundingClientRect().right + 1).length
      return JSON.stringify({
        viewport: [window.innerWidth, window.innerHeight],
        docOverflow: doc.scrollWidth - doc.clientWidth,
        railWidth,
        railOverflow: sidebar.scrollWidth - sidebar.clientWidth,
        spill,
        emptyVisible,
        composerBelow: Math.round(dock.bottom - window.innerHeight)
      })
    })()`))
  }

  const now = await measure(900, 650, false)
  say(`       ${JSON.stringify(now)}`)
  check('the window does not scroll sideways', now.docOverflow <= 0, String(now.docOverflow))
  check('the rail does not scroll sideways', now.railOverflow <= 0, String(now.railOverflow))
  check('nothing in the rail spills past its edge', now.spill === 0, String(now.spill))
  check('the empty-state sentence is not squeezed into the rail', now.emptyVisible === false)
  check('the composer is still above the fold', now.composerBelow <= 0, String(now.composerBelow))

  const old = await measure(900, 650, true)
  say(`       control: ${JSON.stringify(old)}`)
  check(
    'CONTROL: with the rail free to overflow and the sentence back, something spills or shows',
    old.spill > 0 || old.emptyVisible === true || old.railOverflow > 0,
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
  say(`\n${String(failures)} COMPACT FAILURE(S)`)
  process.exit(1)
}
say('\nCOMPACT OK')
