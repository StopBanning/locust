// A Linux AppImage updates itself: download from a generic feed, Restart and install, back on the new copy (CI, 0.789).
//
//   node _smoke/linux-update-smoke.mjs <path to the built Locust-...-linux-x86_64.AppImage>
//
// An AppImage is one file the person keeps where they like; the AppImage
// runtime names it in $APPIMAGE, and electron-updater replaces that file with
// the new one and starts it again (linux-install.ts). The Windows machine that
// builds everything cannot run a Linux AppImage. This runs on GitHub's Ubuntu
// runner after the AppImage is built:
//
//   1. The built AppImage is copied into a folder of its own, as a person
//      keeps it, and hard-linked beside itself so replacing the file unlinks
//      the old inode.
//   2. A local HTTP server offers `latest-linux.yml` naming the AppImage under
//      test as version 9.9.9 (with its real sha512 and size), and the AppImage
//      is started with LOCUST_LINUX_UPDATE_TEST_FEED and _VERSION.
//   3. The ordinary update service must download the update and say 9.9.9 is
//      ready; Restart and install is pressed through the same bridge the
//      banner uses.
//   4. Locust must quit, the AppImage file on disk must be replaced (new
//      inode, old hard-link unlinked), and Locust must be running again from
//      the replaced AppImage reporting version 9.9.9.
//
// Exit 0 only if every check passes. Self-contained, like mac-update-smoke.mjs.

import { spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import { chmodSync, copyFileSync, createReadStream, existsSync, linkSync, readdirSync, readFileSync, statSync } from 'node:fs'
import { mkdir, mkdtemp, rm } from 'node:fs/promises'
import { createServer } from 'node:http'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

const built = process.argv[2] === undefined ? undefined : resolve(process.argv[2])
if (built === undefined) {
  console.log('usage: node _smoke/linux-update-smoke.mjs <Locust.AppImage>')
  process.exit(2)
}
const PORT = Number(process.env.LOCUST_SMOKE_PORT ?? 9335)
const FEED_FILE = 'Locust-9.9.9-linux-x86_64.AppImage'
const sleep = (ms) => new Promise((done) => setTimeout(done, ms))
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  console.log(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${detail}`}`)
}

const dir = await mkdtemp(join(tmpdir(), 'locust-linux-update-'))
const APP = join(dir, 'Locust.AppImage')
const OLD = join(dir, 'Locust.AppImage.old')
const profile = join(dir, 'profile')
const xdgConfig = join(dir, 'xdg-config')
const xdgCache = join(dir, 'xdg-cache')
await mkdir(profile, { recursive: true })
await mkdir(xdgConfig, { recursive: true })
await mkdir(xdgCache, { recursive: true })

// 1. Placed where a person keeps it, and hard-linked as the old copy so a replacement unlinks the old inode.
copyFileSync(built, APP)
chmodSync(APP, 0o755)
linkSync(APP, OLD)
const firstIno = statSync(APP).ino
const size = statSync(built).size
check('the built AppImage is placed on disk, linked as the old copy', existsSync(APP) && statSync(OLD).nlink === 2)

// 2. Local generic feed offering the AppImage under test as 9.9.9, with its real sha512 and size.
const sha512 = await new Promise((done, fail) => {
  const hash = createHash('sha512')
  createReadStream(built).on('data', (chunk) => hash.update(chunk)).on('end', () => done(hash.digest('base64'))).on('error', fail)
})
const yml = [
  'version: 9.9.9',
  'files:',
  `  - url: ${FEED_FILE}`,
  `    sha512: ${sha512}`,
  `    size: ${String(size)}`,
  `path: ${FEED_FILE}`,
  `sha512: ${sha512}`,
  "releaseDate: '2026-10-18T00:00:00.000Z'",
  ''
].join('\n')

let feedHits = 0
let downloadHits = 0
const server = createServer((req, res) => {
  const url = req.url ?? '/'
  if (url.startsWith('/latest-linux.yml') || url.startsWith('/beta-linux.yml')) {
    feedHits += 1
    res.writeHead(200, { 'Content-Type': 'text/yaml; charset=utf-8', 'Content-Length': String(Buffer.byteLength(yml)) })
    res.end(yml)
    return
  }
  if (url.startsWith(`/${FEED_FILE}`)) {
    downloadHits += 1
    res.writeHead(200, { 'Content-Type': 'application/octet-stream', 'Content-Length': String(size) })
    if (req.method === 'HEAD') {
      res.end()
      return
    }
    createReadStream(built).pipe(res)
    return
  }
  res.writeHead(404)
  res.end()
})
await new Promise((done) => server.listen(0, '127.0.0.1', done))
const feedPort = server.address().port
const feedUrl = `http://127.0.0.1:${String(feedPort)}`

/** Processes on this machine whose environment names our AppImage (`APPIMAGE=<APP>`). */
const appImagePids = (silentOnly = false) => {
  const found = []
  try {
    for (const entry of readdirSync('/proc')) {
      if (!/^\d+$/.test(entry)) continue
      try {
        const env = readFileSync(`/proc/${entry}/environ`, 'utf8')
        if (!env.includes(`APPIMAGE=${APP}\0`)) continue
        if (silentOnly && !env.includes('APPIMAGE_SILENT_INSTALL=true\0')) continue
        found.push(entry)
      } catch { /* another user's or already gone */ }
    }
  } catch { /* non-procfs */ }
  return found
}

/** Read any `locust-errors.log` written under `root` (the relaunched copy uses `$XDG_CONFIG_HOME`). */
const readStartupLogs = (root) => {
  const texts = []
  const visit = (folder, depth) => {
    if (depth > 3) return
    let entries = []
    try { entries = readdirSync(folder, { withFileTypes: true }) } catch { return }
    for (const entry of entries) {
      const full = join(folder, entry.name)
      if (entry.isDirectory()) visit(full, depth + 1)
      else if (entry.name === 'locust-errors.log') {
        try { texts.push(readFileSync(full, 'utf8')) } catch { /* unreadable */ }
      }
    }
  }
  visit(root, 0)
  return texts.join('\n')
}

const child = spawn(APP, [`--remote-debugging-port=${String(PORT)}`, `--user-data-dir=${profile}`], {
  cwd: dir,
  env: {
    ...process.env,
    XDG_CONFIG_HOME: xdgConfig,
    XDG_CACHE_HOME: xdgCache,
    LOCUST_FREE_ONLY: '1',
    LOCUST_LINUX_UPDATE_TEST_FEED: feedUrl,
    LOCUST_LINUX_UPDATE_TEST_VERSION: '9.9.9'
  },
  stdio: ['ignore', 'pipe', 'pipe']
})
const output = []
child.stdout.on('data', (data) => output.push(String(data)))
child.stderr.on('data', (data) => output.push(String(data)))

let socket
let nextId = 1
const waiting = new Map()
const evaluateOnce = (expression) => new Promise((done, fail) => {
  const id = nextId++
  waiting.set(id, { resolve: done, reject: fail })
  socket.send(JSON.stringify({ id, method: 'Runtime.evaluate', params: { expression, awaitPromise: true, returnByValue: true } }))
})
const evaluate = async (expression) => {
  for (let attempt = 0; ; attempt += 1) {
    try {
      return await evaluateOnce(expression)
    } catch (error) {
      if (attempt >= 5 || !/context was destroyed|Cannot find context/i.test(String(error?.message ?? error))) throw error
      await sleep(1_500)
    }
  }
}

try {
  let page
  for (let i = 0; i < 120 && page === undefined; i += 1) {
    await sleep(500)
    if (child.exitCode !== null) throw new Error(`the app exited ${String(child.exitCode)}: ${output.join('').slice(-600)}`)
    try {
      const list = await (await fetch(`http://127.0.0.1:${String(PORT)}/json/list`)).json()
      page = list.find((target) => target.type === 'page' && target.webSocketDebuggerUrl && !target.url.includes('#splash'))
    } catch { /* not listening yet */ }
  }
  check('the AppImage starts', page !== undefined)
  if (page === undefined) throw new Error('no window')
  socket = new WebSocket(page.webSocketDebuggerUrl)
  socket.addEventListener('message', (event) => {
    const message = JSON.parse(String(event.data))
    const held = waiting.get(message.id)
    if (held === undefined) return
    waiting.delete(message.id)
    if (message.error !== undefined) held.reject(new Error(message.error.message))
    else if (message.result?.exceptionDetails !== undefined) held.reject(new Error(`the page threw: ${String(message.result.exceptionDetails.exception?.description ?? message.result.exceptionDetails.text)}`))
    else held.resolve(message.result?.result?.value)
  })
  await new Promise((done) => socket.addEventListener('open', done, { once: true }))
  const up = await evaluate(`(async () => {
    for (let i = 0; i < 120; i += 1) {
      if (window.desktop?.checkForUpdate !== undefined && document.querySelector('form.command-dock textarea')) return 'up'
      await new Promise((r) => setTimeout(r, 500))
    }
    return 'not up: ' + typeof window.desktop + ' ' + (document.body?.innerText ?? '').slice(0, 200)
  })()`)
  check('the window is up, with the bridge', up === 'up', String(up))

  // 3. The update service: checked against the local feed, downloaded, ready.
  const ready = await evaluate(`(async () => {
    let state = null
    for (let i = 0; i < 90; i += 1) {
      let answer
      try {
        answer = await window.desktop.checkForUpdate()
      } catch (error) {
        state = { threw: String(error?.message ?? error) }
        await new Promise((r) => setTimeout(r, 2000))
        continue
      }
      state = answer.ok ? answer.data : answer.error
      if (answer.ok && answer.data.phase === 'ready') break
      await new Promise((r) => setTimeout(r, 2000))
    }
    return JSON.stringify(state)
  })()`)
  const state = JSON.parse(String(ready))
  check('the update service says 9.9.9 is downloaded and ready', state?.phase === 'ready' && state?.availableVersion === '9.9.9' && feedHits > 0 && downloadHits > 0, String(ready))

  const pressed = await evaluate(`window.desktop.installUpdate().then((answer) => JSON.stringify(answer.ok ? { ok: true } : answer.error))`).catch((error) => JSON.stringify({ gone: String(error?.message ?? error) }))
  console.log(`  install: ${String(pressed)}`)

  // 4. Gone, swapped on disk, and back as 9.9.9.
  for (let i = 0; i < 60 && child.exitCode === null; i += 1) await sleep(500)
  check('Locust quits for the install', child.exitCode !== null, `exit ${String(child.exitCode)}`)

  let again = []
  let startupLogs = ''
  for (let i = 0; i < 60 && (again.length === 0 || !/\tstart\t9\.9\.9\b/.test(startupLogs)); i += 1) {
    await sleep(1_000)
    again = appImagePids(true)
    startupLogs = readStartupLogs(xdgConfig)
  }
  const replaced = existsSync(APP) && statSync(APP).ino !== firstIno && statSync(OLD).nlink === 1 && statSync(APP).size === size
  check('the AppImage on disk was replaced with the new file', replaced, `ino ${String(firstIno)} -> ${String(existsSync(APP) ? statSync(APP).ino : 'missing')}, old nlink=${String(statSync(OLD).nlink)}`)
  check('and Locust is running again from the replaced AppImage as 9.9.9', again.length > 0 && /\tstart\t9\.9\.9\b/.test(startupLogs), `pids=${again.join(',')}`)
} catch (error) {
  failures += 1
  console.log(`smoke failed: ${error instanceof Error ? error.message : String(error)}`)
  console.log(output.join('').slice(-1500))
} finally {
  try { socket?.close() } catch { /* closed */ }
  try { server.close() } catch { /* closed */ }
  try { child.kill() } catch { /* gone */ }
  for (const pid of appImagePids(false)) {
    try { process.kill(Number(pid)) } catch { /* gone */ }
  }
  for (let i = 0; i < 30 && appImagePids(false).length > 0; i += 1) await sleep(500)
  for (const pid of appImagePids(false)) {
    try { process.kill(Number(pid), 'SIGKILL') } catch { /* gone */ }
  }
  await sleep(1_000)
  await rm(dir, { recursive: true, force: true }).catch(() => undefined)
}
console.log(failures === 0 ? 'LINUX UPDATE SMOKE PASSED' : `${String(failures)} LINUX UPDATE SMOKE FAILURE(S)`)
process.exit(failures === 0 ? 0 : 1)
