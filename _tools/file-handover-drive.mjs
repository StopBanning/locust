// A teammate hands the person a file, watched happening.
//
//   node _tools/file-handover-drive.mjs [--keep]
//
// Colin, 2026-09-19, with a screenshot: he asked Yurt to "send me an md of
// your report" and got back a file path as a sentence. Locust could take
// files IN -- the `+` in the composer -- and nothing went the other way.
//
// So: ask the free model to write a short file and hand it over, then read
// the screen the way a person does. The four things that have to be true,
// and each of them was a shipped defect in one of the four blocks that came
// before this one:
//
//   the file is really on disk        (a card pointing at nothing)
//   the card names it                 (a feature that draws nothing)
//   the raw tags are NOT in the prose (the room card, 2026-09-05)
//   pressing it reveals, never opens  (the whole reason openPath is refused)
//
// Live: one real exchange on the free OpenCode model. No quota. Refuses to
// send on any other route.

import '../_tools/scratch-root.mjs'
import { localPath, FREE_ROW, FREE_SEARCH } from './drive-lib.mjs'

import { spawn } from 'node:child_process'
import { mkdtemp, mkdir, rm, stat, writeFile } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { join } from 'node:path'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const NPM_DIR = join(homedir(), 'AppData', 'Roaming', 'npm')
const PORT = 9504
const KEEP = process.argv.includes('--keep')
const WAIT_MS = 5 * 60 * 1000

/*
 * Written the way Colin wrote it, plus the one thing his message could not
 * say: which file. A drive that let the model choose the name would have to
 * guess what to look for on disk, and a guess is how the ownerless-run drive
 * came to judge an answer on a JSON to-do list.
 *
 * The block is NOT named here. The briefing is what is being measured: if a
 * teammate only hands a file over when told the tag, the feature does not
 * work, it is a party trick. The word "send" is the person's own.
 */
const ASK = [
  'Read README.md, then write a two-sentence summary of it to notes/summary.md.',
  'Send me the file when you are done.'
].join(' ')

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

const workspace = await mkdtemp(join(tmpdir(), 'locust-handover-ws-'))
const profile = await mkdtemp(join(tmpdir(), 'locust-handover-'))
await mkdir(join(workspace, 'notes'), { recursive: true })
await writeFile(
  join(workspace, 'README.md'),
  '# Pelican\n\nPelican is a scratch project for measuring how long a build takes.\nIt has one script and no dependencies.\n',
  'utf8'
)
await writeFile(
  join(profile, 'teammates.json'),
  JSON.stringify({ schemaVersion: 1, teammates: [], missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 6, autoMode: false, memoryMode: 'off' } })
)

const child = spawn(ELECTRON, [APP_DIR, `--remote-debugging-port=${String(PORT)}`, `--user-data-dir=${profile}`], {
  cwd: workspace,
  env: { ...process.env, PATH: `${NPM_DIR};${process.env.PATH ?? ''}` },
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
  const ready = await cdp.eval(`(async () => {
    for (let i = 0; i < 240; i += 1) {
      const r = await window.desktop.getLocalRuntimes().catch(() => undefined)
      const oc = r && r.ok ? r.data.runtimes.find(x => x.id === 'opencode') : undefined
      if (oc && oc.installed) return true
      await new Promise(r => setTimeout(r, 500))
    }
    return false
  })()`)
  check('OpenCode is on this machine', ready === true)

  /*
   * THE ROUTE, ASSERTED BEFORE ANYTHING IS SENT.
   *
   * A drive that pressed Send without this once fell through to the
   * composer's default and spent a turn of quota that is not mine. The
   * picker spells the model with hyphens when no teammate is picked and with
   * spaces otherwise, which is why the matcher accepts both.
   */
  say('1. the free route, and nothing sent on any other')
  const picked = await cdp.eval(`(async () => {
    const control = [...document.querySelectorAll('.lc-control')].find(b => b.getAttribute('aria-haspopup') === 'listbox')
    if (!control) return JSON.stringify({ control: false })
    if (${FREE_ROW}.test(control.innerText)) return JSON.stringify({ picked: true, text: control.innerText })
    control.click()
    await new Promise(r => setTimeout(r, 500))
    let target
    for (let attempt = 0; attempt < 90 && !target; attempt += 1) {
      const picker = document.querySelector('.lc-picker')
      if (!picker) { control.click(); await new Promise(r => setTimeout(r, 500)); continue }
      const input = picker.querySelector('.lc-picker__input')
      const setInput = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set
      setInput.call(input, ${JSON.stringify(FREE_SEARCH)})
      input.dispatchEvent(new Event('input', { bubbles: true }))
      await new Promise(r => setTimeout(r, 500))
      let group = ''
      for (const node of picker.querySelector('.lc-picker__list').children) {
        const header = node.querySelector('.lc-picker__group')
        if (header) group = header.innerText
        const row = node.querySelector('.lc-picker__row')
        const label = row ? row.innerText.trim().toLowerCase() : ''
        if (row && !row.disabled && /opencode/i.test(group) && ${FREE_ROW}.test(label) && /free/.test(label)) { target = row; break }
      }
      if (!target) await new Promise(r => setTimeout(r, 500))
    }
    if (!target) return JSON.stringify({ picked: false })
    target.click()
    await new Promise(r => setTimeout(r, 500))
    return JSON.stringify({ picked: true, text: control.innerText })
  })()`)
  say(`   ${picked}`)
  /*
   * THE FREE ROUTE UNLESS SOMEBODY DELIBERATELY SAID OTHERWISE.
   *
   * `--spend` plus `LOCUST_SPEND=1` runs this on whatever route the composer
   * is already on, which is how the protocol blocks get measured on a CAPABLE
   * model rather than only on the free one. That distinction matters here
   * more than anywhere else: this drive measures whether a model reaches for
   * `<locust-file>` from the words "send me the file", and "a weak model does"
   * is weaker evidence than it sounds -- a stronger model has more room to
   * decide it knows better and write a path in prose instead.
   *
   * Both are required. One flag is a typo; two is a decision.
   */
  const paid = process.argv.includes('--spend') && process.env.LOCUST_SPEND === '1'
  const onFree = new RegExp(FREE_ROW.slice(1, FREE_ROW.lastIndexOf('/')), 'i').test(JSON.parse(picked).text ?? '')
  if (paid) {
    say(`   SPENDING DELIBERATELY: ${JSON.parse(picked).text ?? '(unknown route)'}`)
  } else {
    check('the free OpenCode model is the route', onFree, picked)
    if (!onFree) throw new Error('the free OpenCode route was not picked; nothing was sent')
  }

  say('2. ask for a file, in the words a person would use')
  const sent = await cdp.eval(`(async () => {
    const field = document.querySelector('form.command-dock textarea')
    const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
    setter.call(field, ${JSON.stringify(ASK)})
    field.dispatchEvent(new Event('input', { bubbles: true }))
    for (let i = 0; i < 120; i += 1) {
      await new Promise(r => setTimeout(r, 250))
      const send = document.querySelector('form.command-dock .send-button')
      if (send && !send.disabled && send.getAttribute('aria-label') === 'Send') { send.click(); return 'clicked' }
    }
    return 'send stayed disabled: ' + (field.placeholder || '')
  })()`)
  if (sent !== 'clicked') throw new Error(sent)

  say('3. waiting for the handover')
  const startedAt = Date.now()
  let seen = { card: false }
  while (Date.now() - startedAt < WAIT_MS) {
    await sleep(2000)
    seen = JSON.parse(await cdp.eval(`(() => {
      const clean = (s) => (s || '').replace(new RegExp('[' + String.fromCharCode(32, 9, 13, 10) + ']+', 'g'), ' ').trim()
      // The PILL is the container now and the two controls live inside it:
      // the name reveals, the icon saves a copy. Read the one that reveals.
      const buttons = [...document.querySelectorAll('.lc-handedfile__open')]
      return JSON.stringify({
        card: buttons.length > 0,
        saves: document.querySelectorAll('.lc-handedfile__save').length,
        running: !!document.querySelector('button[aria-label^="Stop the running"]'),
        files: buttons.map(b => clean(b.innerText).slice(0, 90)),
        titles: buttons.map(b => b.getAttribute('title') || ''),
        // What the PERSON reads. If a raw tag survives here, the app is
        // showing its own protocol instead of the answer.
        said: [...document.querySelectorAll('.lc-agentline__body')].map(el => clean(el.innerText)).join(' ')
      })
    })()`))
    if (seen.card === true) break
    if (seen.running === false && Date.now() - startedAt > 30_000) break
  }
  say(`   ${JSON.stringify(seen).slice(0, 500)}`)

  say('4. what is true on disk, and what is true on screen')
  const wrote = await stat(join(workspace, 'notes', 'summary.md')).then((s) => s.size).catch(() => -1)
  say(`   notes/summary.md: ${wrote < 0 ? 'absent' : `${String(wrote)} bytes`}`)
  check('the teammate really wrote the file', wrote > 0, String(wrote))
  check('the thread draws it as a control', seen.card === true, JSON.stringify(seen.files ?? []))
  if (seen.card === true) {
    check('the control names the file', seen.files.some((text) => /summary\.md/.test(text)), JSON.stringify(seen.files))
    // Reveal, never open. `shell.openPath` would RUN a `.bat` a model wrote.
    // The title carries the teammate's note first, then the reveal sentence,
    // because the note is what the pill truncates and it has to be readable
    // somewhere (Colin, 2026-09-20).
    /*
     * The NAME now opens the file in the panel beside the conversation;
     * reveal and save are that panel's own controls. "Open" here means
     * Locust draws it, never that the operating system is handed the file --
     * `reveal-file.ts` still refuses `shell.openPath`, and the viewer has no
     * control that would.
     */
    check(
      'the name offers to open it in the panel',
      seen.titles.every((title) => /Open /.test(title)),
      JSON.stringify(seen.titles)
    )
    // And the other thing a person wants from a file: it somewhere else.
    check('the pill also offers to save a copy', seen.saves === seen.files.length, String(seen.saves))
  }
  check('the raw block is not in what the person reads', !/locust-file/.test(seen.said ?? ''), (seen.said ?? '').slice(0, 200))

  if (seen.card === true) {
    say('5. press it -- the panel should open with the file in it')
    // The host answers `{ok}`; a refusal here would mean the card is
    // decoration, which is the failure mode this whole drive exists for.
    const revealed = await cdp.eval(`(async () => {
      const before = document.querySelectorAll('.lc-handedfile__open').length
      document.querySelector('.lc-handedfile__open').click()
      await new Promise(r => setTimeout(r, 2000))
      const viewer = document.querySelector('.lc-viewer')
      const shown = viewer ? (viewer.innerText || '') : ''
      return JSON.stringify({ before, after: document.querySelectorAll('.lc-handedfile__open').length, error: !!document.querySelector('.lc-card.is-red'), viewer: !!viewer, viewerText: shown.slice(0, 180), inset: document.querySelector('.lc-shell.has-viewer') !== null })
    })()`)
    say(`   ${revealed}`)
    const pressed = JSON.parse(revealed)
    check('the panel opened', pressed.viewer === true, revealed)
    // The file's own words, not a spinner and not an empty pane.
    check('and it is showing the file', /Pelican|scratch project|summary/i.test(pressed.viewerText ?? ''), (pressed.viewerText ?? '').slice(0, 120))
    // The workroom insets under it. Its OWN class, not the inspector's: the
    // viewer is wider, and a mismatched inset puts the composer under the
    // panel -- the exact defect 0.187.0 was spent fixing.
    check('the workroom insets under it', pressed.inset === true, String(pressed.inset))
    check('and nothing raised an error', pressed.error === false, revealed)
  }

  const shot = await cdp.send('Page.captureScreenshot', { format: 'png' })
  const out = new URL('../docs/chain-measure/file-handover-2026-09-19.png', import.meta.url)
  await writeFile(out, Buffer.from(shot.result.data, 'base64'))
  say(`   frame ${out.pathname.slice(1)}`)
} finally {
  child.kill()
  await sleep(1000)
  /*
   * CLOSE THE FILE MANAGER THIS DRIVE OPENED.
   *
   * Step 5 presses the reveal, which is the point -- and a reveal opens a real
   * Explorer window on the person's own desktop, pointed into this drive's
   * temp workspace. Then the teardown below deletes that workspace, and
   * Explorer is left staring at a folder that no longer exists, with a
   * "Location is not available" dialog on top of it.
   *
   * Colin found four of them on his machine (2026-09-20: "this you?"). It was
   * mine, four runs of it, and it is exactly the kind of mess a test fixture
   * has no business leaving on a working machine.
   *
   * Targeted by path rather than "close Explorer": the only windows shut are
   * the ones showing this run's own temp folder.
   */
  const scratch = workspace.replace(/\\/g, '\\\\')
  spawn('powershell', [
    '-NoProfile',
    '-Command',
    `$shell = New-Object -ComObject Shell.Application; @($shell.Windows()) | Where-Object { $_.LocationURL -and $_.LocationURL -match [regex]::Escape('${scratch.split('\\\\').pop() ?? ''}') } | ForEach-Object { $_.Quit() }`
  ], { stdio: 'ignore', windowsHide: true }).unref()
  await sleep(1200)
  if (KEEP) say(`profile kept at ${profile}, workspace at ${workspace}`)
  else {
    await rm(profile, { recursive: true, force: true }).catch(() => undefined)
    await rm(workspace, { recursive: true, force: true }).catch(() => undefined)
  }
}
console.error(failures === 0 ? 'FILE HANDOVER DRIVE PASSED' : `${String(failures)} check(s) failed`)
process.exitCode = failures === 0 ? 0 : 1
