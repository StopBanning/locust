// Does the window come back the size and place you left it?
//
//   node _tools/verify-bounds.mjs
//
// Two halves, checked separately, because they fail differently:
//   SAVE    -- a launch writes `window.json` in the profile at all.
//   RESTORE -- a launch with a known `window.json` opens at those bounds.
// Only the pure placement decision has unit tests; the file write and the
// startup read are host code that no test reaches.

import { spawn } from 'node:child_process'
import { mkdtemp, rm, readFile, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { localPath } from './local-path.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const say = (line) => console.error(line)

const profile = await mkdtemp(join(tmpdir(), 'locust-bounds-'))

async function launch(port, act) {
  const child = spawn(ELECTRON, ['.', `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`], {
    cwd: APP_DIR,
    stdio: ['ignore', 'pipe', 'pipe']
  })
  const out = []
  child.stdout.on('data', (d) => out.push(String(d)))
  child.stderr.on('data', (d) => out.push(String(d)))
  try {
    let page
    for (let i = 0; i < 80 && page === undefined; i += 1) {
      await sleep(500)
      if (child.exitCode !== null) throw new Error(`app exited ${child.exitCode}: ${out.join('').slice(-400)}`)
      try {
        const list = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()
        page = list.find((t) => t.type === 'page' && t.webSocketDebuggerUrl && !t.url.includes('#splash'))
      } catch { /* not listening yet */ }
    }
    if (page === undefined) throw new Error('renderer never came up')
    await sleep(2500)
    return await act(page)
  } finally {
    child.kill()
    await sleep(2000)
  }
}

async function bounds(page) {
  const socket = new WebSocket(page.webSocketDebuggerUrl)
  await new Promise((resolve, reject) => {
    socket.addEventListener('open', resolve, { once: true })
    socket.addEventListener('error', reject, { once: true })
  })
  const result = await new Promise((resolve) => {
    socket.addEventListener('message', (event) => {
      const message = JSON.parse(event.data)
      if (message.id === 1) resolve(message.result?.result?.value)
    })
    socket.send(JSON.stringify({
      id: 1,
      method: 'Runtime.evaluate',
      params: {
        expression: '({width: window.outerWidth, height: window.outerHeight, x: window.screenX, y: window.screenY})',
        returnByValue: true
      }
    }))
  })
  socket.close()
  return result
}

let ok = true
try {
  const opened = await launch(9241, bounds)
  say(`first launch opened at ${JSON.stringify(opened)}`)

  const saved = JSON.parse(await readFile(join(profile, 'window.json'), 'utf8'))
  say(`SAVE: wrote ${JSON.stringify(saved)}`)
  if (!Number.isFinite(saved.width) || !Number.isFinite(saved.x)) {
    say('FAIL: window.json is not a rectangle')
    ok = false
  }

  // Windows puts an invisible resize border around the frame, so what the
  // renderer reports as `outerWidth`/`screenX` is offset by a constant from
  // the bounds Electron was given. Comparing two restores cancels it: the
  // DIFFERENCE between what the renderer reports must equal the difference
  // between what was asked for, whatever the border is worth.
  const first = { x: 137, y: 91, width: 1243, height: 781, maximized: false }
  const second = { x: 205, y: 148, width: 1160, height: 742, maximized: false }

  await writeFile(join(profile, 'window.json'), JSON.stringify(first), 'utf8')
  const gotFirst = await launch(9242, bounds)
  await writeFile(join(profile, 'window.json'), JSON.stringify(second), 'utf8')
  const gotSecond = await launch(9243, bounds)

  say(`RESTORE A: asked ${JSON.stringify(first)}`)
  say(`RESTORE A: got   ${JSON.stringify(gotFirst)}`)
  say(`RESTORE B: asked ${JSON.stringify(second)}`)
  say(`RESTORE B: got   ${JSON.stringify(gotSecond)}`)

  for (const key of ['x', 'y', 'width', 'height']) {
    const asked = second[key] - first[key]
    const actual = gotSecond[key] - gotFirst[key]
    if (Math.abs(asked - actual) > 2) {
      say(`FAIL: ${key} moved by ${actual}, but was asked to move by ${asked}`)
      ok = false
    } else {
      say(`  ${key}: asked to move ${asked}, moved ${actual}`)
    }
  }
  say(ok ? 'PASS: the window reopens at the bounds it is given' : 'FAIL')
} catch (error) {
  say(String(error))
  ok = false
} finally {
  await rm(profile, { recursive: true, force: true }).catch(() => undefined)
}
process.exit(ok ? 0 : 1)
