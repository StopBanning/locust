// How many characters a reply line actually holds, at each size on offer.
//
//   node _tools/prose-size-measure.mjs [--keep]
//
// The design agent's release check against `f911eb3c` leaves the reply's type
// size open, and it is the most-read text in the app. `--lc-text-prose` is
// 15px; the comment above it quotes the guidance it was chosen under, which
// says body serif at 20px and sans is UI chrome only. Their argument is that a
// serif needs MORE size than a sans, not less -- the strokes that make it a
// serif are the thin ones, and a dark ground eats thin strokes first.
//
// They also name the measurement to take before anybody decides, and they are
// right that it cannot be taken from source: the reply's `max-width: 90ch` is
// ninety ZEROS wide, and a serif zero is comparatively wide, so the rendered
// line is shorter than the number suggests. `shell.css` already records this
// trap biting twice, at 68ch and again at 48ch.
//
// So this counts characters off the rendered line boxes, character by
// character, at each size in the argument -- the same method the 2026-09-10
// measure used, because text has no mean character width and probing with
// zeros understates prose by about a fifth.
//
// The comfortable band is 45-75 characters a line; past about 85 the eye
// loses its place on the return sweep.
//
// Live: ONE real exchange on the free OpenCode model, because the thing being
// measured is PROSE and a fixture typed here would be prose I chose the width
// of. Refuses to send on any other route.

import '../_tools/scratch-root.mjs'
import { localPath, FREE_ROW, FREE_SEARCH } from './drive-lib.mjs'

import { spawn } from 'node:child_process'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { join } from 'node:path'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const NPM_DIR = join(homedir(), 'AppData', 'Roaming', 'npm')
const PORT = 9505
const KEEP = process.argv.includes('--keep')
const WAIT_MS = 5 * 60 * 1000
const SIZES = ['15px', '16px', '17px', '18px', '20px']
/*
 * Which size the CEILING sweep runs at.
 *
 * It ran at `SIZES[0]` and that was wrong the moment the size question got an
 * answer: `ch` scales with the font, so a ceiling counted at 15px says
 * nothing about the same ceiling at 18px. `--at 18px` counts it where it will
 * actually live.
 */
const AT = process.argv.includes('--at') ? process.argv[process.argv.indexOf('--at') + 1] : SIZES[0]
const ASK =
  'Write three paragraphs of ordinary prose about why very long lines of text are tiring to read. No lists, no code, no headings, no file edits. Plain sentences.'

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

const workspace = await mkdtemp(join(tmpdir(), 'locust-prose-ws-'))
const profile = await mkdtemp(join(tmpdir(), 'locust-prose-'))
await writeFile(join(workspace, 'README.md'), '# scratch\n', 'utf8')
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

/*
 * Counted off the rendered line boxes, one character at a time, grouped by
 * the top of each character's own rectangle. Slow and exact. Anything faster
 * -- a mean width, a `ch` conversion -- is the trap this exists to avoid.
 */
const MEASURE = `((size, ceiling) => {
  const root = document.documentElement
  if (size) root.style.setProperty('--lc-text-prose', size)
  // The ceiling lives on the PARAGRAPH, in ch, so it is swept the same way.
  if (ceiling) for (const el of document.querySelectorAll('.lc-agentline p')) el.style.maxWidth = ceiling
  const paragraphs = [...document.querySelectorAll('.lc-agentline p')].filter(p => (p.textContent || '').length > 200)
  if (paragraphs.length === 0) return JSON.stringify({ error: 'no reply paragraph on screen' })
  const first = paragraphs[0]
  const style = getComputedStyle(first)
  const walker = document.createTreeWalker(first, NodeFilter.SHOW_TEXT)
  const range = document.createRange()
  const counts = new Map()
  let node
  while ((node = walker.nextNode())) {
    const text = node.textContent || ''
    for (let i = 0; i < text.length; i += 1) {
      range.setStart(node, i); range.setEnd(node, i + 1)
      const top = Math.round(range.getBoundingClientRect().top)
      counts.set(top, (counts.get(top) || 0) + 1)
    }
  }
  const lines = [...counts.entries()].sort((a, b) => a[0] - b[0]).map(([, n]) => n)
  // The LAST line of a paragraph is however much was left over, so it says
  // nothing about the measure and would drag every average down.
  const full = lines.slice(0, -1)
  const first_line = (() => {
    const n = first.firstChild
    if (!n || n.nodeType !== 3) return ''
    const r = document.createRange()
    let top = null, out = ''
    for (let i = 0; i < n.textContent.length; i += 1) {
      r.setStart(n, i); r.setEnd(n, i + 1)
      const t = Math.round(r.getBoundingClientRect().top)
      if (top === null) top = t
      if (t !== top) break
      out += n.textContent[i]
    }
    return out
  })()
  return JSON.stringify({
    size: style.fontSize,
    face: style.fontFamily.split(',')[0].replace(new RegExp(String.fromCharCode(34), 'g'), ''),
    lineHeight: style.lineHeight,
    columnPx: Math.round(first.getBoundingClientRect().width),
    lines: full.length,
    shortest: full.length ? Math.min(...full) : 0,
    longest: full.length ? Math.max(...full) : 0,
    mean: full.length ? Math.round(full.reduce((a, b) => a + b, 0) / full.length) : 0,
    firstLine: first_line
  })
})`

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
      const r = await window.desktop.getLocalRuntimes().catch(() => undefined)
      const oc = r && r.ok ? r.data.runtimes.find(x => x.id === 'opencode') : undefined
      if (oc && oc.installed) return true
      await new Promise(r => setTimeout(r, 500))
    }
    return false
  })()`)

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
  if (!new RegExp(FREE_ROW.slice(1, FREE_ROW.lastIndexOf('/')), 'i').test(JSON.parse(picked).text ?? '')) {
    throw new Error('the free OpenCode route was not picked; nothing was sent')
  }

  say('2. ask for three paragraphs of ordinary prose')
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

  const startedAt = Date.now()
  while (Date.now() - startedAt < WAIT_MS) {
    await sleep(2000)
    const running = await cdp.eval(`!!document.querySelector('button[aria-label^="Stop the running"]')`).catch(() => true)
    const have = await cdp.eval(`[...document.querySelectorAll('.lc-agentline p')].some(p => (p.textContent || '').length > 200)`)
    if (have === true && running === false) break
  }
  await sleep(1500)

  say('3. characters a line, counted off the rendered boxes')
  say('')
  say('   size   face                line  shortest  mean  longest   verdict')
  const rows = []
  for (const size of SIZES) {
    const got = JSON.parse(await cdp.eval(`(${MEASURE})(${JSON.stringify(size)}, null)`))
    if (got.error !== undefined) throw new Error(got.error)
    // 45-75 is the comfortable band; past about 85 the return sweep goes.
    const verdict = got.mean > 85 ? 'too wide' : got.mean > 75 ? 'over' : got.mean >= 45 ? 'in band' : 'too narrow'
    rows.push({ ...got, verdict })
    say(
      `   ${got.size.padEnd(6)} ${got.face.padEnd(19)} ${String(got.columnPx).padStart(4)}  ${String(got.shortest).padStart(8)}  ${String(got.mean).padStart(4)}  ${String(got.longest).padStart(7)}   ${verdict}`
    )
    const shot = await cdp.send('Page.captureScreenshot', { format: 'png' })
    await writeFile(
      new URL(`../docs/chain-measure/prose-${size.replace('px', '')}-2026-09-19.png`, import.meta.url),
      Buffer.from(shot.result.data, 'base64')
    )
  }
  /*
   * AND THE CEILING, AT THE SIZE THAT SHIPS.
   *
   * `max-width: 90ch` is the number in `shell.css`, and its own comment says
   * the return sweep breaks past about 85 characters -- so if 90ch renders
   * more than that, the rule and the number disagree and the number is the
   * one that is wrong. The `ch` unit has already understated this twice, at
   * 68 and at 48; measuring it is the only way that has ever worked.
   */
  say('')
  say(`4. and the ceiling, counted at ${AT}`)
  say('')
  say('   ceiling  paragraph  shortest  mean  longest   verdict')
  const ceilings = []
  for (const ceiling of ['90ch', '84ch', '78ch', '72ch', '66ch', '60ch', '54ch']) {
    const got = JSON.parse(await cdp.eval(`(${MEASURE})(${JSON.stringify(AT)}, ${JSON.stringify(ceiling)})`))
    const verdict = got.mean > 85 ? 'too wide' : got.mean > 75 ? 'over' : got.mean >= 45 ? 'in band' : 'too narrow'
    ceilings.push({ ceiling, ...got, verdict })
    say(
      `   ${ceiling.padEnd(8)} ${String(got.columnPx).padStart(9)}  ${String(got.shortest).padStart(8)}  ${String(got.mean).padStart(4)}  ${String(got.longest).padStart(7)}   ${verdict}`
    )
  }
  await writeFile(
    new URL('../docs/chain-measure/prose-ceiling-2026-09-19.json', import.meta.url),
    `${JSON.stringify(ceilings, null, 2)}\n`,
    'utf8'
  )

  say('')
  say(`   the line as rendered at ${String(SIZES[0])}: ${JSON.stringify(rows[0]?.firstLine ?? '')}`)
  say(`   the line as rendered at ${String(SIZES.at(-1))}: ${JSON.stringify(rows.at(-1)?.firstLine ?? '')}`)
  await writeFile(
    new URL('../docs/chain-measure/prose-size-2026-09-19.json', import.meta.url),
    `${JSON.stringify(rows, null, 2)}\n`,
    'utf8'
  )
  say('   frames docs/chain-measure/prose-<size>-2026-09-19.png')
} finally {
  child.kill()
  await sleep(1000)
  if (KEEP) say(`profile kept at ${profile}`)
  else {
    await rm(profile, { recursive: true, force: true }).catch(() => undefined)
    await rm(workspace, { recursive: true, force: true }).catch(() => undefined)
  }
}
