// Is the box you type in above the fold on a first launch?
//
//   node _tools/verify-firstfold.mjs
//
// Colin, 2026-09-05: on a 1280x860 window the welcome screen's runtime list
// pushed the composer below the fold, while the copy said "describe a mission
// in the box below". The sentence was true and the layout made it a lie.
//
// This is geometry, not a screenshot: a composer that is 200px past the
// bottom edge still renders perfectly in a screenshot of the page. It reads
// the composer's own rectangle against the viewport at two sizes -- Colin's
// window, and the smaller content pane the design pass measured -- and it
// carries its own control: with the pre-0.21.2 state restored the same check
// (re-measured 0.21.5 with the roster open by default, plus the folder card)
// must fail. Measured 2026-09-05 at 1280x860 -- fixed: 16px above the edge;
// old: 395px below it. The control had to be corrected once: restoring the
// rule but leaving the roster collapsed kept the screen short and the check
// passed, which claimed it could not fail when it can.

import { spawn } from 'node:child_process'
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { join } from 'node:path'
import { localPath } from './local-path.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const NPM_DIR = join(homedir(), 'AppData', 'Roaming', 'npm')
const PORT = 9302
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

const profile = await mkdtemp(join(tmpdir(), 'locust-fold-'))
const workspace = await mkdtemp(join(tmpdir(), 'locust-fold-ws-'))
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
  env: { ...process.env, PATH: `${NPM_DIR};${process.env.PATH ?? ''}` },
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
  const send = (method, params = {}) => new Promise((res) => {
    const n = ++id
    pending.set(n, res)
    socket.send(JSON.stringify({ id: n, method, params }))
  })
  const evaluate = async (expression) =>
    (await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })).result?.result?.value

  await evaluate(`(async () => {
    for (let i = 0; i < 240; i += 1) {
      const f = document.querySelector('form.command-dock textarea')
      if (f && !/Checking local runtimes/.test(f.placeholder)) return true
      await new Promise(r => setTimeout(r, 250))
    }
    return false
  })()`)

  // A first launch is what this screen is: no teammate, no mission.
  // The welcome renders a tick after discovery settles; give it a moment
  // rather than reading the one frame in between (a flake seen 2026-09-05).
  const ready = await evaluate(`(async () => { for (let i = 0; i < 12; i += 1) { if (document.querySelector('.lc-empty')) return true; await new Promise(r => setTimeout(r, 250)) } return false })()`)
  check('the welcome screen is what is on screen', ready === true)

  // `tall` plants a block taller than the pane inside the welcome, which is
  // what the old six-row runtime list did on a small window. That is the
  // condition the rule exists for, so the control has to create it: since the
  // roster became a two-column panel the real screen is short enough to pass
  // with or without the rule, and a control that cannot fail proves nothing.
  const measure = async (width, height, oldRule, tall) => {
    await send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: false })
    await sleep(400)
    return JSON.parse(await evaluate(`(async () => {
      const empty = document.querySelector('.lc-empty')
      document.getElementById('firstfold-tall-probe')?.remove()
      if (${tall ? 'true' : 'false'}) {
        const filler = document.createElement('div')
        filler.id = 'firstfold-tall-probe'
        filler.style.height = '900px'
        filler.style.flex = '0 0 auto'
        empty.querySelector('.lc-empty__inner').append(filler)
      }
      if (${oldRule ? 'true' : 'false'}) {
        empty.style.minHeight = 'auto'
        empty.style.overflowY = 'visible'
      } else {
        empty.style.minHeight = ''
        empty.style.overflowY = ''
      }
      await new Promise(r => setTimeout(r, 250))
      const dock = document.querySelector('form.command-dock')
      const box = dock.getBoundingClientRect()
      return JSON.stringify({
        viewport: window.innerHeight,
        dockBottom: Math.round(box.bottom),
        below: Math.round(box.bottom - window.innerHeight)
      })
    })()`))
  }

  for (const [width, height, name] of [[1280, 860, "Colin's window"], [1014, 786, 'the design pass pane']]) {
    const now = await measure(width, height, false, false)
    check(
      `the composer is above the fold at ${String(width)}x${String(height)} · ${name}`,
      now.below <= 0,
      JSON.stringify(now)
    )
  }

  // Control: the same check must FAIL on the rule this replaced, or it is
  // not measuring what it claims to.
  // Both halves of the control, because either alone is misleading: a tall
  // welcome WITH the rule must still keep the composer on screen, and the
  // same tall welcome WITHOUT it must push it off.
  const tallFixed = await measure(1280, 860, false, true)
  check(
    'a welcome taller than the pane still keeps the composer on screen',
    tallFixed.below <= 0,
    JSON.stringify(tallFixed)
  )
  const old = await measure(1280, 860, true, true)
  check(
    'CONTROL: without the rule that same welcome pushes it below the fold, so this check can fail',
    old.below > 0,
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
  say(`\n${String(failures)} FIRST-FOLD FAILURE(S)`)
  process.exit(1)
}
say('\nFIRST FOLD OK')
