// What a person sees the first time they open Locust from the Start menu.
//
//   node _tools/first-launch-folder-drive.mjs [--keep]
//
// Nobody had looked at this. Every drive and every smoke starts the app with
// its working directory already pointing at a project, which is not how an
// installed Locust is opened: the Start menu launches it from its own
// install folder, that folder is refused on purpose, and the app then makes
// `Documents\Locust` and works there. So the first run of a freshly
// installed Locust happens in a folder the person did not choose and which
// has nothing in it -- and the whole promise of the app is that a teammate
// works in YOUR project.
//
// This launches with the same test seams the host already has for that path
// (`LOCUST_INSTALL_DIR`, `LOCUST_DEFAULT_WORKSPACE`), then reads what the
// screen actually says about where the work will happen, and whether a
// person can get from there to their own folder without hunting.
//
// No provider run; costs nothing.

import '../_tools/scratch-root.mjs'

import { spawn } from 'node:child_process'
import { mkdtemp, mkdir, readdir, rm, stat, writeFile } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { join } from 'node:path'
import { localPath } from './local-path.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const NPM_DIR = join(homedir(), 'AppData', 'Roaming', 'npm')
const PORT = 9503
const KEEP = process.argv.includes('--keep')

let failures = 0
const check = (label, ok, detail) => {
  if (!ok) failures += 1
  console.error(`  [${ok ? 'PASS' : 'FAIL'}] ${label}${detail === undefined ? '' : ` -- ${detail}`}`)
}
const say = (line) => console.error(line)
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))

class Cdp {
  constructor(ws) {
    this.ws = ws
    this.id = 0
    this.pending = new Map()
    ws.addEventListener('message', (event) => {
      const message = JSON.parse(event.data)
      const waiter = this.pending.get(message.id)
      if (waiter === undefined) return
      this.pending.delete(message.id)
      waiter(message)
    })
  }
  send(method, params = {}) {
    const id = ++this.id
    this.ws.send(JSON.stringify({ id, method, params }))
    return new Promise((resolve) => this.pending.set(id, resolve))
  }
  async eval(expression) {
    const reply = await this.send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })
    if (reply.result?.exceptionDetails) throw new Error(reply.result.exceptionDetails.text)
    return reply.result?.result?.value
  }
}

const root = await mkdtemp(join(tmpdir(), 'locust-firstlaunch-'))
// The install folder, which the Start menu launches from and which the host
// refuses as a place to work.
const installDirectory = join(root, 'Programs', 'Locust')
// Where the app will put its own folder. Deliberately does NOT exist yet.
const defaultWorkspace = join(root, 'Documents', 'Locust')
const profile = join(root, 'profile')
for (const dir of [installDirectory, profile]) await mkdir(dir, { recursive: true })

const child = spawn(ELECTRON, [APP_DIR, `--remote-debugging-port=${String(PORT)}`, `--user-data-dir=${profile}`], {
  // Launched FROM the install folder, the way the Start menu does it.
  cwd: installDirectory,
  env: {
    ...process.env,
    PATH: `${NPM_DIR};${process.env.PATH ?? ''}`,
    LOCUST_INSTALL_DIR: installDirectory,
    LOCUST_DEFAULT_WORKSPACE: defaultWorkspace
  },
  stdio: ['ignore', 'pipe', 'pipe']
})
const appOutput = []
child.stdout.on('data', (d) => appOutput.push(String(d)))
child.stderr.on('data', (d) => appOutput.push(String(d)))

try {
  let page
  for (let attempt = 0; attempt < 80 && page === undefined; attempt += 1) {
    await sleep(500)
    if (child.exitCode !== null) throw new Error(`app exited ${String(child.exitCode)}`)
    try {
      const list = await (await fetch(`http://127.0.0.1:${String(PORT)}/json/list`)).json()
      page = list.find((t) => t.type === 'page' && t.webSocketDebuggerUrl && !t.url.includes('#splash'))
    } catch { /* not up yet */ }
  }
  if (page === undefined) { say(appOutput.join('')); throw new Error('no renderer target') }
  const socket = new WebSocket(page.webSocketDebuggerUrl)
  await new Promise((resolve, reject) => {
    socket.addEventListener('open', resolve, { once: true })
    socket.addEventListener('error', reject, { once: true })
  })
  const cdp = new Cdp(socket)
  await cdp.send('Runtime.enable')
  await cdp.eval(`(async () => {
    for (let i = 0; i < 240; i += 1) {
      const field = document.querySelector('form.command-dock textarea')
      if (field && !/Checking local runtimes/.test(field.placeholder)) return true
      await new Promise(r => setTimeout(r, 400))
    }
    return false
  })()`)
  await sleep(1500)

  say('1. where the app decided the work will happen')
  const made = await stat(defaultWorkspace).then(() => true).catch(() => false)
  const inside = made ? (await readdir(defaultWorkspace).catch(() => [])).length : -1
  say(`   made ${defaultWorkspace}: ${String(made)}, entries inside: ${String(inside)}`)
  check('the app made itself a folder rather than refusing the first message', made === true, defaultWorkspace)
  check('and it is not the install folder', made === true && defaultWorkspace !== installDirectory)

  const said = JSON.parse(await cdp.eval(`(() => {
    const clean = (s) => (s || '').replace(new RegExp('[' + String.fromCharCode(32, 9, 13, 10) + ']+', 'g'), ' ').trim()
    const box = document.querySelector('form.command-dock textarea')
    const folder = [...document.querySelectorAll('.lc-control, .lc-composer__controls button')].map(b => clean(b.innerText)).filter(Boolean)
    return JSON.stringify({
      // Everything the first screen says, so the question "where will this
      // run" can be answered from the words on it or shown not to be.
      body: clean(document.body.innerText).slice(0, 900),
      placeholder: box ? box.getAttribute('placeholder') : null,
      controls: folder,
      // The host's own answer, for comparison with what is drawn.
      hasChooseControl: [...document.querySelectorAll('button')].some(b => /choose|folder|open folder/i.test(clean(b.innerText) + ' ' + (b.getAttribute('title') || '')))
    })
  })()`))
  say(`   controls: ${JSON.stringify(said.controls)}`)
  say(`   placeholder: ${JSON.stringify(said.placeholder)}`)
  say(`   body: ${said.body.slice(0, 600)}`)

  const info = await cdp.eval(`window.desktop.getAppInfo().then(i => JSON.stringify({ workspaceName: i.workspaceName, workspacePath: i.workspacePath, workspaceMade: i.workspaceMade }))`)
  say(`   host: ${info}`)
  const host = JSON.parse(info)
  check('the host reports it made the folder itself', host.workspaceMade === true, info)

  // The two questions a person has in their first ten seconds.
  check('the screen names the folder the work will happen in', said.controls.some((c) => /Locust/i.test(c)) || /Locust\b/.test(said.body), JSON.stringify(said.controls))
  check('there is a way to point it at your own folder without hunting', said.hasChooseControl === true, String(said.hasChooseControl))
  /*
   * OPEN, AND A DESIGN CALL RATHER THAN A DEFECT TO PATCH HERE.
   *
   * The app invents a folder on an installed launch, works in it, and says
   * so ONLY in the folder chip's hover tooltip: "Locust made this folder;
   * pick any other to work there instead." Nobody hovers a chip in their
   * first ten seconds, and the chip reads "Locust" -- which is also the
   * app's name and the window title, so it does not read as a folder
   * anybody would think to change. A person whose project is elsewhere can
   * type their first message into an empty folder and never know.
   *
   * Printed every run rather than asserted: where it should be said belongs
   * to the design agent, who redrew this screen on 2026-09-19 and
   * deliberately took sentences OFF it.
   */
  const tooltip = await cdp.eval(`(() => {
    const chip = document.querySelector('.lc-control--folder')
    return chip ? (chip.getAttribute('title') || '') : '(no folder chip)'
  })()`)
  const saidOnScreen = /made (you )?(a|this) folder|chose this folder|new folder|Locust made/i.test(said.body)
  /*
   * ANSWERED 2026-09-19, so it is an assertion now rather than a note.
   *
   * Colin took the recommendation: the first screen carries one card when
   * the app invented its own folder -- 'Locust made a folder to work in',
   * the path, and the same Choose folder control. The tooltip stays; what
   * changed is that the tooltip is no longer the only place it is said.
   */
  check('the invented folder is announced on screen, not only on hover', saidOnScreen === true, said.body.slice(0, 200))
  say(`   OPEN: the chip's tooltip says: ${JSON.stringify(tooltip)}`)

  // The what-changed banner left Home on 2026-09-23 (Colin: "it adds a needless scrollbar"); the changelog is
  // Settings > What's new now, and drive-whats-new reads its bullets. Its checks here had not run since.
  say('2. no what-changed banner on a first launch')
  const banner = await cdp.eval(`[...document.querySelectorAll('button')].some((b) => /^Read it$/.test((b.innerText || '').trim()))`)
  check('no what-changed banner on Home', banner === false, String(banner))

  const shot = await cdp.send('Page.captureScreenshot', { format: 'png' })
  const out = new URL('../docs/chain-measure/first-launch-folder-2026-09-19.png', import.meta.url)
  await writeFile(out, Buffer.from(shot.result.data, 'base64'))
  say(`   frame ${out.pathname.slice(1)}`)
} finally {
  child.kill()
  await sleep(1000)
  if (KEEP) say(`kept at ${root}`)
  else await rm(root, { recursive: true, force: true }).catch(() => undefined)
}
console.error(failures === 0 ? 'FIRST LAUNCH FOLDER DRIVE PASSED' : `${String(failures)} check(s) failed`)
process.exitCode = failures === 0 ? 0 : 1
