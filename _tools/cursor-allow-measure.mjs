// Cursor connectors outside Auto: measured, not asserted.
//
//   node _tools/cursor-allow-measure.mjs --label before
//   node _tools/cursor-allow-measure.mjs --label after
//
// One teammate on Cursor / Grok 4.6 Low in ACCEPT EDITS is asked to call a
// read-only Robinhood connector tool (get_indexes: analysis only, never a
// trade). The ledger is then read for `user rejected MCP` -- 17 in one such
// run of Colin's on 0.161 -- and the workspace's `.cursor/cli.json` is read
// back to see what Locust wrote there. Costs one short Cursor run on a cheap
// Cursor model, which Colin cleared for testing ("use luna or a cheap model").
//
// Never enters a credential: the connector is whatever `cursor-agent mcp list`
// already has.

import '../_tools/scratch-root.mjs'

import { spawn } from 'node:child_process'
import { mkdtemp, readdir, readFile, rm, writeFile } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { join } from 'node:path'
import { localPath } from './local-path.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const PORT = 9473
const CURSOR_DIR = join(homedir(), 'AppData', 'Local', 'cursor-agent')
const MODEL = 'cursor-grok-4.6-low'
const LABEL = process.argv[process.argv.indexOf('--label') + 1] || 'run'
const PROMPT = 'Using your robinhood-local connector, call get_indexes and tell me the current S&P 500 level in one line. Read-only: do not place, preview, cancel or modify anything. If the connector call is refused, say exactly what the refusal said.'

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

const workspace = await mkdtemp(join(tmpdir(), 'locust-allow-'))
const profile = await mkdtemp(join(tmpdir(), 'locust-allow-profile-'))
await writeFile(join(workspace, 'README.md'), 'Scratch workspace for a connector call.\n', 'utf8')
const createdAt = new Date().toISOString()
await writeFile(
  join(profile, 'teammates.json'),
  JSON.stringify({
    schemaVersion: 1,
    teammates: [{ teammateId: 'tm_jim', name: 'Jim', hue: 'blue', role: 'Data & Reporting', createdAt, route: { runtime: 'cursor', model: MODEL, mode: 'accept-edits' } }],
    missionOwners: {},
    settings: { swarm: false, relay: false, autoMode: false }
  })
)
const LEDGER_DIR = join(profile, 'mission-ledger')
const child = spawn(ELECTRON, [APP_DIR, `--remote-debugging-port=${PORT}`, `--user-data-dir=${profile}`], {
  cwd: workspace,
  env: { ...process.env, PATH: `${CURSOR_DIR};${process.env.PATH ?? ''}` },
  stdio: ['ignore', 'pipe', 'pipe']
})
const appOutput = []
child.stdout.on('data', (d) => appOutput.push(String(d)))
child.stderr.on('data', (d) => appOutput.push(String(d)))
const ledgers = async () => (await readdir(LEDGER_DIR).catch(() => [])).filter((name) => name.endsWith('.jsonl'))

try {
  say(`[${LABEL}] 1. app starts; workspace ${workspace}`)
  let page
  for (let attempt = 0; attempt < 60 && page === undefined; attempt += 1) {
    await sleep(500)
    if (child.exitCode !== null) break
    try {
      const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json()
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
  await cdp.eval(`(async () => { for (let i = 0; i < 240; i += 1) { const c = [...document.querySelectorAll('.lc-control')].find(b => b.getAttribute('aria-haspopup') === 'listbox'); if (c && /cursor|codex|claude|opencode/i.test(c.innerText)) return true; await new Promise(r => setTimeout(r, 500)) } return false })()`)

  say(`[${LABEL}] 2. select Jim; the composer follows the seeded route (Cursor / Grok 4.6 Low, Accept edits)`)
  const setup = await cdp.eval(`(async () => {
    const teamView = [...document.querySelectorAll('button')].find(b => /^Team/.test(b.getAttribute('aria-label') || b.getAttribute('title') || ''))
    if (teamView) { teamView.click(); await new Promise(r => setTimeout(r, 600)) }
    let jim
    for (let attempt = 0; attempt < 40 && !jim; attempt += 1) {
      jim = [...document.querySelectorAll('button')].find(b => (b.querySelector('.lc-row__name') || { innerText: '' }).innerText.trim().startsWith('Jim'))
        || [...document.querySelectorAll('button')].find(b => /^(Message )?Jim\\b/.test(b.getAttribute('title') || b.getAttribute('aria-label') || ''))
      if (!jim) await new Promise(r => setTimeout(r, 500))
    }
    if (!jim) return JSON.stringify({ jim: false, buttons: [...document.querySelectorAll('button')].map(x => (x.getAttribute('aria-label') || x.getAttribute('title') || x.innerText || '').replace(/[ \\t\\r\\n]+/g, ' ').trim().slice(0, 50)).filter(Boolean).slice(0, 40) })
    jim.click()
    await new Promise(r => setTimeout(r, 800))
    return JSON.stringify({ jim: true, controls: [...document.querySelectorAll('.lc-control')].map(c => c.innerText.replace(/[^a-z0-9 ./-]+/gi, ' ').trim()) })
  })()`)
  say(`   ${setup}`)
  const chosen = JSON.parse(setup)
  if (!chosen.jim || !/grok 4\.6 low/i.test(chosen.controls.join(' ')) || !/accept edits/i.test(chosen.controls.join(' '))) throw new Error('route or mode not as seeded')

  say(`[${LABEL}] 3. ask for the connector call`)
  const submitted = await cdp.eval(`(async () => {
    const field = document.querySelector('form.command-dock textarea')
    const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
    setter.call(field, ${JSON.stringify(PROMPT)})
    field.dispatchEvent(new Event('input', { bubbles: true }))
    for (let i = 0; i < 120; i += 1) {
      await new Promise(r => setTimeout(r, 250))
      const send = document.querySelector('form.command-dock .send-button')
      if (send && !send.disabled && send.getAttribute('aria-label') === 'Send') { send.click(); return 'clicked' }
    }
    return 'send stayed disabled'
  })()`)
  say(`   ${submitted}`)
  if (submitted !== 'clicked') throw new Error(submitted)

  say(`[${LABEL}] 4. waiting for the run to end`)
  let sawRunning = false
  for (let i = 0; i < 600; i += 1) {
    await sleep(1000)
    const running = await cdp.eval(`!!document.querySelector('button[aria-label^="Stop the running"]')`).catch(() => false)
    if (running) sawRunning = true
    if (sawRunning && !running) break
  }

  say(`[${LABEL}] 5. reading the ledger and the workspace's Cursor settings`)
  const names = await ledgers()
  let rejected = 0
  let calls = 0
  let final = ''
  for (const name of names) {
    const text = await readFile(join(LEDGER_DIR, name), 'utf8')
    rejected += (text.match(/user rejected MCP/gi) ?? []).length
    calls += (text.match(/robinhood-[a-z]+-get_indexes/g) ?? []).length
    const lines = text.split('\n').filter(Boolean)
    final = lines.slice(-6).join('\n').slice(-1500)
  }
  const config = await readFile(join(workspace, '.cursor', 'cli.json'), 'utf8').catch(() => '(no .cursor/cli.json in the workspace)')
  const thread = await cdp.eval(`(document.querySelector('.lc-thread') || { innerText: '' }).innerText.slice(-800)`).catch(() => '')
  const out = [
    `# Cursor connectors outside Auto: ${LABEL}`,
    '',
    `Model ${MODEL}, Accept edits. Prompt: ${PROMPT}`,
    '',
    `- ledgers: ${names.length}`,
    `- mentions of a get_indexes call: ${calls}`,
    `- \`user rejected MCP\`: **${rejected}**`,
    '',
    '## workspace/.cursor/cli.json after the run',
    '',
    '```json',
    config,
    '```',
    '',
    '## thread tail',
    '',
    '```',
    thread,
    '```',
    '',
    '## ledger tail',
    '',
    '```',
    final,
    '```'
  ].join('\n')
  const stamp = new Date().toISOString().slice(0, 19).replace(/[:T]/g, '-')
  const file = new URL(`../docs/chain-measure/cursor-allow-${LABEL}-${stamp}.md`, import.meta.url)
  await writeFile(file, out, 'utf8')
  say(`   rejected: ${rejected}; calls: ${calls}; wrote ${file.pathname.slice(1)}`)
} finally {
  child.kill()
  await sleep(500)
  if (process.argv.includes('--keep')) say(`profile kept at ${profile}`)
  else {
    await rm(profile, { recursive: true, force: true }).catch(() => undefined)
    await rm(workspace, { recursive: true, force: true }).catch(() => undefined)
  }
}
