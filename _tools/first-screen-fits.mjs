// Does the first screen FIT, at the window a person actually has?
//
//   node _tools/first-screen-fits.mjs [--width 1201] [--height 720]
//
// Colin, 2026-09-20, with a frame of his own machine: six agents installed,
// five rows drawn, and a scrollbar whose entire job was to reveal Antigravity
// sitting just under the fold.
//
//   "it adds a needless scroll bar that goes to nothing but antigravity
//   hiding below"
//
// The scroll is on `.lc-empty`, the whole first screen, not on the agent
// panel -- so the question is not "is the list too long" but "is this screen
// taller than the window", and the list is only the tallest compressible
// thing on it.
//
// MEASURED IN BOTH DIRECTIONS, because the two audiences are opposite. Six
// installed is Colin. NOTHING installed is the person the first screen exists
// for, and Colin's own instruction about the fix was "if it breaks the system
// for new user just ignore". A change measured only on the machine that
// reported the bug is the 0.198.0 regression again.
//
// Costs nothing: no provider run, nothing pressed, nothing written.

import '../_tools/scratch-root.mjs'

import { spawn } from 'node:child_process'
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { join } from 'node:path'
import { localPath } from './local-path.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const NPM_DIR = join(homedir(), 'AppData', 'Roaming', 'npm')
const OUT = localPath(new URL('../docs/chain-measure/', import.meta.url))

const argumentOf = (name, fallback) => {
  const at = process.argv.indexOf(`--${name}`)
  return at === -1 ? fallback : Number(process.argv[at + 1])
}
const WIDTH = argumentOf('width', 1201)
const HEIGHT = argumentOf('height', 720)

const say = (line) => console.error(line)
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
let failures = 0
const check = (label, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${label}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 200)}`}`)
}

/** One launch, one reading. `bare` strips the PATH so nothing is installed. */
async function measure({ name, port, bare }) {
  const root = await mkdtemp(join(tmpdir(), `locust-fits-${name}-`))
  const workspace = join(root, 'workspace')
  const profile = join(root, 'profile')
  const appData = join(root, 'AppData', 'Roaming')
  const localAppData = join(root, 'AppData', 'Local')
  for (const dir of [workspace, profile, appData, localAppData]) await mkdir(dir, { recursive: true })
  await writeFile(join(workspace, 'README.md'), 'Scratch workspace.\n', 'utf8')
  await writeFile(
    join(profile, 'teammates.json'),
    JSON.stringify({ schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, autoMode: false } })
  )
  const barePath = ['C:\\Windows\\System32', 'C:\\Windows', 'C:\\Windows\\System32\\Wbem'].join(';')
  const path = bare ? barePath : `${NPM_DIR};${process.env.PATH ?? ''}`
  const child = spawn(ELECTRON, [APP_DIR, `--remote-debugging-port=${String(port)}`, `--user-data-dir=${profile}`], {
    cwd: workspace,
    env: {
      ...process.env,
      PATH: path,
      Path: path,
      ...(bare ? { APPDATA: appData, LOCALAPPDATA: localAppData, NPM_CONFIG_PREFIX: '' } : {})
    },
    stdio: ['ignore', 'pipe', 'pipe']
  })

  let page
  for (let i = 0; i < 80 && page === undefined; i += 1) {
    await sleep(500)
    try {
      const list = await (await fetch(`http://127.0.0.1:${String(port)}/json/list`)).json()
      page = list.find((t) => t.type === 'page' && t.webSocketDebuggerUrl && !t.url.includes('#splash'))
    } catch { /* not up */ }
  }
  if (page === undefined) { try { child.kill() } catch { /* gone */ } throw new Error(`${name}: renderer never came up`) }
  const socket = new WebSocket(page.webSocketDebuggerUrl)
  await new Promise((res, rej) => { socket.addEventListener('open', res, { once: true }); socket.addEventListener('error', rej, { once: true }) })
  let id = 0
  const pending = new Map()
  socket.addEventListener('message', (event) => {
    const message = JSON.parse(event.data)
    const waiter = pending.get(message.id)
    if (waiter) { pending.delete(message.id); waiter(message) }
  })
  const send = (method, params = {}) => new Promise((resolve) => {
    const next = ++id
    pending.set(next, resolve)
    socket.send(JSON.stringify({ id: next, method, params }))
  })
  const evaluate = async (expression) => {
    const message = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })
    return message?.result?.result?.value
  }
  await send('Runtime.enable')
  await send('Emulation.setDeviceMetricsOverride', { width: WIDTH, height: HEIGHT, deviceScaleFactor: 1, mobile: false })

  // Wait for discovery to settle, so the count is the real one.
  await evaluate(`(async () => {
    for (let i = 0; i < 160; i += 1) {
      const field = document.querySelector('form.command-dock textarea')
      if (field && !/Checking local runtimes/.test(field.placeholder)) break
      await new Promise(r => setTimeout(r, 250))
    }
    let seen = ''
    let stable = 0
    for (let i = 0; i < 160; i += 1) {
      const now = document.querySelector('.lc-agenthead')?.innerText ?? ''
      stable = now === seen ? stable + 1 : 0
      seen = now
      if (stable >= 6) return seen
      await new Promise(r => setTimeout(r, 250))
    }
    return seen
  })()`)

  const reading = JSON.parse(await evaluate(`(() => {
    const pane = document.querySelector('.lc-empty')
    const panel = document.querySelector('.lc-runtimepanel')
    const cells = [...document.querySelectorAll('.lc-runtimecell')]
    const rows = new Set(cells.map(c => Math.round(c.getBoundingClientRect().top)))
    /*
     * A cell whose text is CUT, not a cell that is narrow. Two columns halve
     * the width, and the account fact ("installs from opencode.ai") is the
     * longest thing in a row -- so this asks each span whether its content
     * is wider than the box drawing it, which is what clipping actually is.
     */
    const clipped = []
    for (const cell of cells) {
      for (const span of cell.querySelectorAll('span')) {
        if (span.scrollWidth > span.clientWidth + 1 && span.innerText.trim().length > 0) {
          clipped.push(span.innerText.trim().slice(0, 40))
        }
      }
    }
    return JSON.stringify({
      head: document.querySelector('.lc-agenthead')?.innerText.replace(/\\s+/g, ' ') ?? '(no head)',
      agents: cells.length,
      columns: cells.length === 0 ? 0 : cells.length / rows.size,
      rows: rows.size,
      panelHeight: panel ? Math.round(panel.getBoundingClientRect().height) : null,
      scrolls: pane ? pane.scrollHeight > pane.clientHeight + 1 : null,
      hiddenBelow: pane ? Math.max(0, pane.scrollHeight - pane.clientHeight) : null,
      clipped
    })
  })()`))

  await mkdir(OUT, { recursive: true })
  const shot = await send('Page.captureScreenshot', { format: 'png' })
  const file = join(OUT, `first-screen-${name}-${String(WIDTH)}x${String(HEIGHT)}.png`)
  if (shot?.result?.data) {
    const { writeFile: write } = await import('node:fs/promises')
    await write(file, Buffer.from(shot.result.data, 'base64'))
  }
  try { socket.close() } catch { /* gone */ }
  try { child.kill() } catch { /* gone */ }
  await sleep(800)
  await rm(root, { recursive: true, force: true }).catch(() => undefined)
  return { ...reading, frame: file }
}

say(`the first screen at ${String(WIDTH)}x${String(HEIGHT)}\n`)

for (const run of [
  { name: 'installed', port: 9521, bare: false, what: 'this machine, everything installed' },
  { name: 'nothing', port: 9522, bare: true, what: 'a machine with nothing installed' }
]) {
  const reading = await measure(run)
  say(`${run.what}`)
  say(`   head: ${reading.head}`)
  say(`   ${String(reading.agents)} agents in ${String(reading.rows)} rows x ${String(reading.columns)} columns, panel ${String(reading.panelHeight)}px`)
  say(`   scrolls: ${String(reading.scrolls)}${reading.hiddenBelow > 0 ? ` (${String(reading.hiddenBelow)}px below the fold)` : ''}`)
  say(`   frame: ${reading.frame}`)
  check(`${run.name}: the first screen fits without scrolling`, reading.scrolls === false, `${String(reading.hiddenBelow)}px hidden`)
  check(`${run.name}: no row's text is cut`, reading.clipped.length === 0, reading.clipped.join(' | '))
  say('')
}

say(failures === 0 ? 'the first screen fits both ways' : `${String(failures)} check(s) failed`)
process.exitCode = failures === 0 ? 0 : 1
