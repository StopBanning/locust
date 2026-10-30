// A screenshot of the app, for looking at.
//
//   node _tools/shot.mjs <out.png> [width] [height]
//
// Geometry checks answer "is it on screen"; this answers "does it look like
// the reference". Both, always: a screenshot cannot see a composer 200px
// below the fold, and a rectangle cannot see a broken divider.

import { spawn } from 'node:child_process'
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { localPath } from './local-path.mjs'

const OUT = process.argv[2] ?? 'shot.png'
const WIDTH = Number(process.argv[3] ?? 1014)
const HEIGHT = Number(process.argv[4] ?? 786)
const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const PORT = 9304
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))

const profile = await mkdtemp(join(tmpdir(), 'locust-shot-'))
const workspace = await mkdtemp(join(tmpdir(), 'locust-shot-ws-'))
await mkdir(profile, { recursive: true })
await writeFile(join(workspace, 'status.ts'), 'export const status = "draft";\n', 'utf8')

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
  const evaluate = async (expression) =>
    (await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })).result?.result?.value

  // Wait for discovery to have ANSWERED, not merely for the app to paint:
  // a shot taken while it is still checking shows the loading copy, which is
  // a real state but never the one you asked to look at.
  const settled = await evaluate(`(async () => {
    for (let i = 0; i < 360; i += 1) {
      const dock = document.querySelector('form.command-dock textarea')
      const painted = document.querySelector('.lc-runtimepanel, .lc-thread, .lc-screen')
      if (dock && painted && !/Checking local runtimes/.test(dock.placeholder)) return true
      await new Promise(r => setTimeout(r, 250))
    }
    return false
  })()`)
  if (settled !== true) console.error('  (warning: discovery had not finished; the shot shows the loading state)')
  await send('Emulation.setDeviceMetricsOverride', { width: WIDTH, height: HEIGHT, deviceScaleFactor: 2, mobile: false })
  await sleep(900)
  const shot = await send('Page.captureScreenshot', { format: 'png' })
  const data = shot.result?.data
  if (typeof data !== 'string') throw new Error('no screenshot came back')
  await writeFile(OUT, Buffer.from(data, 'base64'))
  console.error(`wrote ${OUT} at ${String(WIDTH)}x${String(HEIGHT)}`)
} finally {
  try { child.kill() } catch { /* gone */ }
  await sleep(1200)
  await rm(profile, { recursive: true, force: true }).catch(() => undefined)
  await rm(workspace, { recursive: true, force: true }).catch(() => undefined)
}
