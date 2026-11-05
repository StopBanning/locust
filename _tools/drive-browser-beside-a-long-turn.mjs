// THE BUILT-IN BROWSER BESIDE A LONG TURN, ON A MAC (2026-11-05). Colin's screenshots of 0.866, the browser open in
// a 1205x803 window beside a Codex teammate's long turn: a sideways scrollbar under the running row, its "step 2 of"
// pill cut at the edge, the conversation's title cut to "han..." and a red "-0" on every step's count. The only
// browser drive (drive-agent-browser.mjs) runs on packaged Windows and measures what the tools do, not how the panel
// sits beside a conversation; the browser had never been driven on a Mac.
//
//   node _tools/drive-browser-beside-a-long-turn.mjs [--replay <mission ledger .jsonl>]... [--tag <name>]
//
// --replay copies a finished conversation's record into the drive's own throwaway profile (its folder rewritten to
// the drive's), so a real long turn is measured as it was drawn; the copy is read here and never leaves the scratch
// profile. With none, the live turn below is the only conversation.
//
// The live turn is a stand-in ACP agent (no model, no account, nothing spent): a plan, steps with commands and an
// edit, then one long command it holds until the drive has measured it running, then the browser tools on a local
// page through the per-run MCP bridge, then its answer.
//
// At each window size (opened at that size from the window file Locust keeps; the browser is a native view placed
// from the real window, so CDP's page resize would measure a layout no person has), with the browser open:
//   - nothing scrolls sideways in the conversation column;
//   - nothing in the column reaches past its right edge;
//   - no text is cut without an ellipsis;
//   - the conversation's title is not cut while the header has room for it;
//   - no count shows a zero side ("-0", "+0").
// Frames: the page (CDP) and the real window (screencapture -l, which shows the native browser view), never brought
// to the front.
import { execFile, execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { createServer } from 'node:http'
import { existsSync, readFileSync } from 'node:fs'
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { basename, join } from 'node:path'
import { promisify } from 'node:util'

import { openTeammateScript, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

if (process.platform !== 'darwin') throw new Error('this drive measures the browser panel on a Mac')
const run = promisify(execFile)
const args = process.argv.slice(2)
const replays = args.flatMap((value, i) => (args[i - 1] === '--replay' ? [value] : []))
const tag = args.includes('--tag') ? args[args.indexOf('--tag') + 1] : 'local'
const SIZES = [
  { width: 1205, height: 803, browser: true },
  { width: 1205, height: 803, browser: false },
  { width: 1120, height: 720, browser: true },
  { width: 1440, height: 900, browser: true },
  { width: 1728, height: 1080, browser: true }
]

const stamp = new Date().toISOString().slice(0, 19).replace(/[:T]/g, '-')
const root = join(homedir(), 'work', 'scratch', 'browser-drive', `${tag}-${stamp}`)
await mkdir(root, { recursive: true })
const tempRoot = await mkdtemp(join(tmpdir(), 'locust-browser-long-'))
const workspace = await scratchRepository('locust-browser-long-ws-')
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const brief = join(workspace, 'handoff-first-visit-audit-and-touch-polish.md')
await writeFile(brief, '# First-visit audit\n\nRun the first-visit audit at phone and desktop sizes, fix what it finds.\n', 'utf8')

// A local page for the browser tools: tall enough to scroll, with a button the agent presses.
const server = createServer((request, response) => {
  response.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' })
  response.end(`<!doctype html><html><head><title>First-visit audit</title><style>
    body { font: 15px system-ui, sans-serif; margin: 0; padding: 24px; background: #1d2bb3; color: #f4f6ff; }
    main { max-width: 720px; margin: 0 auto; } pre { background: #0d1460; padding: 12px; border-radius: 8px; white-space: pre-wrap; }
    button { font: inherit; padding: 8px 14px; border-radius: 8px; border: 0; background: #fff; color: #1d2bb3; }
  </style></head><body><main><h1>toaster v0.5.0</h1><p>turns whatever's playing into live code</p>
  <button id="hear" onclick="document.getElementById('out').textContent = JSON.stringify({ mode: 'idle', touch: false, samples: [1000, 1008] }, null, 2)">hear it on a demo song</button>
  <pre id="out">{ "mode": "refused" }</pre>${'<p>Line of the audit page.</p>'.repeat(40)}</main></body></html>`)
})
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve))
const pageUrl = `http://127.0.0.1:${String(server.address().port)}/`

// The stand-in agent.
const release = join(tempRoot, 'release')
const agentLog = join(tempRoot, 'agent.log')
const harness = join(workspace, 'scripts', 'first-visit-harness.html')
const agentScript = join(tempRoot, 'long-turn-agent.mjs')
await writeFile(agentScript, `import { spawn } from 'node:child_process'
import { appendFileSync, existsSync } from 'node:fs'
import { createInterface } from 'node:readline'
const log = (line) => { try { appendFileSync(${JSON.stringify(agentLog)}, line + '\\n') } catch {} }
const send = (message) => process.stdout.write(JSON.stringify(message) + '\\n')
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
const update = (body) => send({ jsonrpc: '2.0', method: 'session/update', params: { sessionId: 'sess-long', update: body } })
let servers = []
const PLAN = ['Read the brief and the audit notes', 'Run the first-visit audit at phone and desktop sizes', 'Fix the touch menu and the header', 'Check first paint and fonts', 'Report what changed']
const plan = (done) => update({ sessionUpdate: 'plan', entries: PLAN.map((content, i) => ({ content, priority: 'medium', status: i < done ? 'completed' : i === done ? 'in_progress' : 'pending' })) })
const STEPS = [
  ['Parsing audit JSON', 'node scripts/read-audit.mjs .work/first-visit/audit.json --summary'],
  ['Compacting harness output', 'node scripts/compact-harness.mjs --keep 40'],
  ['Preparing touch CSS transformation', 'rg -n "touch-action|pointer: coarse" src/web/*.css'],
  ['Checking font request sizes', 'curl -sI http://127.0.0.1:5173/fonts/space-mono-regular.woff2'],
  ['Fixing touch menu controls', 'node scripts/check-menu.mjs --viewport 844x390 --touch'],
  ['Inspecting harness constraints', 'node scripts/harness-limits.mjs --print']
]
async function browse(spec, url) {
  const env = { ...process.env }
  for (const item of spec.env ?? []) if (item && typeof item.name === 'string') env[item.name] = item.value
  const child = spawn(spec.command, spec.args ?? [], { env, stdio: ['pipe', 'pipe', 'pipe'] })
  const lines = createInterface({ input: child.stdout })
  const waiting = new Map()
  let next = 1
  lines.on('line', (line) => { try { const message = JSON.parse(line); waiting.get(message.id)?.(message); waiting.delete(message.id) } catch {} })
  const call = (method, params) => new Promise((resolve) => { const id = next++; waiting.set(id, resolve); child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\\n') })
  await call('initialize', { protocolVersion: '2024-11-05', capabilities: {}, clientInfo: { name: 'long-turn', version: '1' } })
  const tool = async (name, input) => { const answer = await call('tools/call', { name, arguments: input }); log(name + ' ' + JSON.stringify(answer).slice(0, 200)); return answer.result }
  try {
    await tool('browser_open', { url })
    const read = await tool('browser_read', {})
    const text = read?.content?.[0]?.text ?? ''
    const ref = /hear it on a demo song.*?\\[ref=(@?e\\d+)\\]/.exec(text)?.[1] ?? /\\[ref=(@?e\\d+)\\][^\\n]*hear it on a demo song/.exec(text)?.[1]
    if (ref !== undefined) await tool('browser_click', { ref })
    return ref !== undefined
  } finally { child.kill() }
}
createInterface({ input: process.stdin }).on('line', async (line) => {
  let message
  try { message = JSON.parse(line) } catch { return }
  if (message.method === 'initialize') send({ jsonrpc: '2.0', id: message.id, result: { protocolVersion: 1, agentInfo: { name: 'LongTurn', version: '1.0.0' } } })
  else if (message.method === 'session/new') {
    servers = Array.isArray(message.params?.mcpServers) ? message.params.mcpServers : []
    send({ jsonrpc: '2.0', id: message.id, result: { sessionId: 'sess-long', configOptions: [{ id: 'model', category: 'model', currentValue: 'opencode/long-turn-free', options: [{ value: 'opencode/long-turn-free', name: 'Long Turn Free' }] }] } })
  } else if (message.method === 'session/set_config_option') send({ jsonrpc: '2.0', id: message.id, result: {} })
  else if (message.method === 'session/prompt') {
    plan(0)
    update({ sessionUpdate: 'agent_message_chunk', content: { type: 'text', text: "I'll start from the brief, then run the audit at phone and desktop sizes. The browser panel can load the built files, so I'll use it for layout and interaction checks, with the measurement limits stated clearly." } })
    await pause(300)
    plan(1)
    for (const [i, [title, command]] of STEPS.entries()) {
      update({ sessionUpdate: 'agent_thought_chunk', content: { type: 'text', text: '**' + title + '**\\n\\nWorking through the audit.' } })
      update({ sessionUpdate: 'tool_call', toolCallId: 'c' + i, title: command, kind: 'execute', status: 'in_progress', rawInput: { command } })
      await pause(120)
      update({ sessionUpdate: 'tool_call_update', toolCallId: 'c' + i, status: i % 3 === 2 ? 'failed' : 'completed' })
      if (i % 2 === 0) {
        const newText = Array.from({ length: 299 }, (_, n) => '<p>harness line ' + n + '</p>').join('\\n') + '\\n'
        update({ sessionUpdate: 'tool_call', toolCallId: 'e' + i, title: 'Write scripts/first-visit-harness.html', kind: 'edit', status: 'completed', locations: [{ path: ${JSON.stringify(harness)} }], content: [{ type: 'diff', path: ${JSON.stringify(harness)}, oldText: null, newText }] })
      }
      await pause(120)
    }
    // The long one, held until the drive has measured it running.
    const long = 'TOASTER_HIDDEN_FIRST_VISIT=1 TOASTER_AUDIT_FRAMES=820 node scripts/first-visit-audit.mjs --viewport 844x390 --touch --frames 820 --out .work/first-visit/next-820'
    update({ sessionUpdate: 'tool_call', toolCallId: 'long', title: long, kind: 'execute', status: 'in_progress', rawInput: { command: long } })
    for (let n = 0; n < 900 && !existsSync(${JSON.stringify(release)}); n += 1) await pause(200)
    update({ sessionUpdate: 'tool_call_update', toolCallId: 'long', status: 'completed' })
    plan(3)
    const spec = servers.find((one) => one.name === 'locust-browser' || one.name === 'locust_browser')
    const clicked = spec === undefined ? false : await browse(spec, ${JSON.stringify(pageUrl)})
    plan(5)
    update({ sessionUpdate: 'agent_message_chunk', content: { type: 'text', text: 'Fixed three first-visit issues locally: landscape menu scrolling, larger touch targets, and clearer permission-free demo wording. The browser panel ' + (clicked ? 'opened the audit page and pressed its demo button.' : 'was not offered to this run.') + ' BROWSER:' + (spec === undefined ? 'none' : clicked ? 'clicked' : 'opened') } })
    send({ jsonrpc: '2.0', id: message.id, result: { stopReason: 'end_turn' } })
  }
})
`, 'utf8')

const profilePath = await mkdtemp(join(tmpdir(), 'locust-browser-long-profile-'))
const agentCommandLine = `"${process.execPath}" "${agentScript}"`
await writeFile(join(profilePath, 'acp-agents.json'), JSON.stringify({ agents: [{ id: 'acp_long-turn', name: 'LongTurn', commandLine: agentCommandLine, answer: { agentName: 'LongTurn', version: '1.0.0', modelOption: { id: 'model', current: 'opencode/long-turn-free', models: [{ value: 'opencode/long-turn-free', name: 'Long Turn Free' }] } }, testedAt: '2026-11-05T00:00:00.000Z' }] }, null, 2), 'utf8')

// The replayed conversations: the same records in the drive's profile, their folder made the drive's.
const at = '2026-11-05T07:00:00.000Z'
const CASPER = { teammateId: 'tm_casper_replay', name: 'Casper', hue: 'blue', role: 'Code & Migrations', createdAt: at, route: { runtime: 'codex', model: 'gpt-6.1-sol', mode: 'accept-edits', effort: 'high' } }
const WREN = { teammateId: 'tm_wren_long', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: at, route: { runtime: 'acp', model: 'acp_long-turn::opencode/long-turn-free', mode: 'accept-edits' } }
const missionOwners = {}
await mkdir(join(profilePath, 'mission-ledger'), { recursive: true })
for (const file of replays) {
  const rows = readFileSync(file, 'utf8').split('\n').filter((line) => line.trim() !== '').map((line) => JSON.parse(line))
  const created = rows.find((row) => row.recordType === 'mission.created')
  if (created === undefined) throw new Error(`${file} has no mission.created record`)
  created.metadata.workspaceId = workspaceId
  missionOwners[created.metadata.missionId] = CASPER.teammateId
  await writeFile(join(profilePath, 'mission-ledger', basename(file)), rows.map((row) => JSON.stringify(row)).join('\n') + '\n', 'utf8')
  say(`replaying ${String(rows.length)} records of ${basename(file)}`)
}

/** The window this drive's app opened: the largest ordinary window of its process or its children. */
const windowOf = (pids) => Number(execFileSync('osascript', ['-l', 'JavaScript', '-e', [
  'ObjC.import("CoreGraphics");',
  'var list = ObjC.castRefToObject($.CGWindowListCopyWindowInfo($.kCGWindowListOptionAll, 0));',
  `var pids = ${JSON.stringify(pids)}; var best = 0, area = 0;`,
  'for (var i = 0; i < list.count; i++) { var d = list.objectAtIndex(i);',
  '  if (pids.indexOf(ObjC.unwrap(d.objectForKey("kCGWindowOwnerPID"))) < 0 || ObjC.unwrap(d.objectForKey("kCGWindowLayer")) !== 0) continue;',
  '  var b = d.objectForKey("kCGWindowBounds"); var a = ObjC.unwrap(b.objectForKey("Width")) * ObjC.unwrap(b.objectForKey("Height"));',
  '  if (a > area) { area = a; best = ObjC.unwrap(d.objectForKey("kCGWindowNumber")); } }',
  'best'
].join(' ')]).toString().trim())
const family = (pid) => {
  const all = [pid]
  for (let i = 0; i < all.length; i += 1) {
    try { all.push(...execFileSync('pgrep', ['-P', String(all[i])]).toString().trim().split('\n').filter(Boolean).map(Number)) } catch { /* none */ }
  }
  return all
}

/** What the drive checks, read in the page: one JSON string. */
const MEASURE = `(() => {
  const thread = document.querySelector('.lc-thread')
  if (thread === null) return JSON.stringify({ found: false })
  // The conversation column: the thread's nearest ancestor that also holds the header and the composer.
  let column = thread.parentElement
  while (column !== null && !(column.querySelector('form.command-dock') && column.contains(thread) && column.getBoundingClientRect().height > innerHeight * 0.6)) column = column.parentElement
  column = column ?? thread
  const edge = column.getBoundingClientRect()
  const said = (el) => (el.getAttribute('aria-label') || el.innerText || el.textContent || '').replace(/\\s+/g, ' ').trim().slice(0, 70)
  const name = (el) => (el.className && typeof el.className === 'string' ? '.' + el.className.trim().split(/\\s+/).slice(0, 2).join('.') : el.tagName.toLowerCase())
  const shown = (el) => { const r = el.getBoundingClientRect(); const s = getComputedStyle(el); return r.width > 0 && r.height > 0 && s.visibility !== 'hidden' && s.display !== 'none' }
  // Canvases are drawing, not layout: the send button's metal glow (MetalSend.tsx) bleeds past its box by design.
  const inside = [...column.querySelectorAll('*')].filter((el) => shown(el) && el.tagName !== 'CANVAS')
  const sideways = [column, ...inside].filter((el) => {
    const s = getComputedStyle(el)
    return (s.overflowX === 'auto' || s.overflowX === 'scroll') && el.scrollWidth > el.clientWidth + 1 && !el.closest('pre, code, .lc-diff, .lc-terminal, table')
  }).map((el) => ({ el: name(el), by: el.scrollWidth - el.clientWidth }))
  const past = inside.filter((el) => {
    const r = el.getBoundingClientRect()
    if (r.right <= edge.right + 1) return false
    // Only the outermost of a run of elements past the edge, and only what a clipping ancestor inside the column does not hide.
    if (el.parentElement && el.parentElement.getBoundingClientRect().right > edge.right + 1 && column.contains(el.parentElement) && el.parentElement !== column) return false
    for (let up = el.parentElement; up !== null && up !== column; up = up.parentElement) {
      const s = getComputedStyle(up)
      if ((s.overflowX === 'hidden' || s.overflowX === 'clip') && up.getBoundingClientRect().right <= edge.right + 1) return false
    }
    return true
  }).map((el) => ({ el: name(el), by: Math.round(el.getBoundingClientRect().right - edge.right), text: said(el) }))
  const cut = inside.filter((el) => {
    if (el.children.length > 0 && el.innerText === undefined) return false
    const s = getComputedStyle(el)
    if (!(s.overflowX === 'hidden' || s.overflowX === 'clip' || s.overflow === 'hidden')) return false
    if (el.scrollWidth <= el.clientWidth + 1) return false
    if (s.textOverflow === 'ellipsis') return false
    if (el.closest('pre, code, .lc-diff, .lc-terminal, canvas, svg')) return false
    return (el.innerText || '').trim().length > 0
  }).map((el) => ({ el: name(el), by: el.scrollWidth - el.clientWidth, text: said(el) }))
  const zero = [...document.querySelectorAll('.lc-diff__delmark, .lc-diff__addmark')].filter(shown).map((el) => el.innerText.trim()).filter((text) => /^[+−-]0$/.test(text))
  // The title: the conversation header's text that is cut with an ellipsis, beside the room its header has.
  const header = column.querySelector('header') ?? column.firstElementChild
  const titles = header === null ? [] : [...header.querySelectorAll('*')].filter((el) => shown(el) && el.children.length === 0 && el.scrollWidth > el.clientWidth + 1 && getComputedStyle(el).textOverflow === 'ellipsis')
  const room = (() => {
    if (header === null) return 0
    const kids = [...header.querySelectorAll(':scope > *')].filter(shown).map((el) => el.getBoundingClientRect()).sort((a, b) => a.left - b.left)
    let gap = 0
    for (let i = 1; i < kids.length; i += 1) gap = Math.max(gap, kids[i].left - kids[i - 1].right)
    return Math.round(gap)
  })()
  const titleCut = titles.map((el) => ({ el: name(el), shows: el.clientWidth, needs: el.scrollWidth, text: said(el) }))
  return JSON.stringify({ found: true, column: Math.round(edge.width), window: innerWidth + 'x' + innerHeight, sideways, past, cut, zero: zero.length, zeroSample: zero.slice(0, 4), titleCut, headerRoom: room })
})()`

/** Open a conversation by its title: the sidebar's row, or in the collapsed rail (a narrow window) the face's flyout. */
const openConversationScript = (teammate, title) => `(async () => {
  const wait = (ms) => new Promise((r) => setTimeout(r, ms))
  const find = () => [...document.querySelectorAll('.lc-conv, .lc-railflyout button, .lc-railflyout [role="button"]')].find((el) => el.innerText.includes(${JSON.stringify(title)}))
  let row = find()
  if (!row) {
    const face = [...document.querySelectorAll('.lc-faces__one, .lc-rail button, nav button')].find((el) => (el.getAttribute('aria-label') ?? '').includes(${JSON.stringify(teammate)}))
    face?.click()
    for (let i = 0; i < 20 && !row; i += 1) { await wait(150); row = find() }
  }
  if (!row) return 'no row for ' + ${JSON.stringify(title)}
  row.click()
  await wait(400)
  return 'opened'
})()`

const results = []
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 700)}`}`)
  results.push({ what, ok, ...(detail === undefined ? {} : { detail: String(detail) }) })
}

for (const [index, size] of SIZES.entries()) {
  const label = `${String(size.width)}x${String(size.height)}${size.browser ? '-browser' : ''}`
  // Owners the app recorded on an earlier launch (the live turn's) survive the seed being written again.
  try { Object.assign(missionOwners, JSON.parse(readFileSync(join(profilePath, 'teammates.json'), 'utf8')).missionOwners ?? {}) } catch { /* first launch */ }
  // The window opens at this size: the file Locust keeps the window's place in (main/window-bounds.ts).
  await writeFile(join(profilePath, 'window.json'), JSON.stringify({ x: 60, y: 60, width: size.width, height: size.height, maximized: false }), 'utf8')
  const drive = await startDrive({
    name: `browser-long-${tag}-${label}`, port: 9707 + index, workspace, profilePath, keep: true,
    // Nothing here reaches a model: the one turn sent goes to the stand-in agent (drive-agent-browser.mjs does the same).
    outPath: join(root, label), sendsNothing: true, holdCursor: false,
    seed: { schemaVersion: 1, teammates: [CASPER, WREN], missionOwners, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } }
  })
  try {
    await drive.ready()
    await sleep(1500)
    if (index === 0) await drive.evaluate(`window.desktop.acpAgents.save('LongTurn', ${JSON.stringify(agentCommandLine)}).catch(() => undefined)`)
    const shots = []
    const shoot = async (what) => {
      const id = windowOf(family(drive.pid))
      if (!id) return say('  no window to capture')
      const file = join(drive.out, `window-${String(shots.length + 1).padStart(2, '0')}-${what}.png`)
      await run('screencapture', ['-x', '-o', `-l${String(id)}`, file])
      shots.push(file)
    }
    const browser = async (open) => {
      await drive.evaluate(`(() => {
        const button = [...document.querySelectorAll('button[aria-label="Browser"]')].at(-1)
        if (button && (button.getAttribute('aria-pressed') === 'true') !== ${String(open)}) button.click()
      })()`)
      await sleep(900)
    }
    const measure = async (what) => {
      const measured = JSON.parse(String(await drive.capture(`${label} ${what}`, () => drive.evaluate(MEASURE))))
      await shoot(what.replace(/[^a-z0-9]+/gi, '-'))
      say(`  ${label} ${what}: ${JSON.stringify(measured).slice(0, 900)}`)
      if (!measured.found) { check(`${label} ${what}: a conversation is open`, false); return measured }
      check(`${label} ${what}: nothing scrolls sideways`, measured.sideways.length === 0, JSON.stringify(measured.sideways))
      check(`${label} ${what}: nothing reaches past the column`, measured.past.length === 0, JSON.stringify(measured.past))
      check(`${label} ${what}: no text cut without an ellipsis`, measured.cut.length === 0, JSON.stringify(measured.cut))
      check(`${label} ${what}: the title is whole, or the header has no room for more`, measured.titleCut.length === 0 || measured.headerRoom < 24, `${JSON.stringify(measured.titleCut)} room ${String(measured.headerRoom)}px`)
      check(`${label} ${what}: no count shows a zero side`, measured.zero === 0, `${String(measured.zero)} ${JSON.stringify(measured.zeroSample)}`)
      results.push({ label, what, measured })
      return measured
    }
    say(`window at ${String(await drive.evaluate('outerWidth + "x" + outerHeight'))} (asked ${label})`)

    if (index === 0) {
      // The live turn: sent once, on the first size, measured while its long command runs.
      await drive.evaluate(openTeammateScript('Wren'))
      await sleep(900)
      await browser(true)
      const sent = await drive.evaluate(`(async () => {
        const field = document.querySelector('form.command-dock textarea')
        const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
        setter.call(field, ${JSON.stringify(brief)})
        field.dispatchEvent(new Event('input', { bubbles: true }))
        for (let i = 0; i < 60; i += 1) {
          await new Promise((r) => setTimeout(r, 150))
          const button = document.querySelector('button[aria-label="Send"]')
          if (button && !button.disabled) { button.click(); return 'sent' }
        }
        return 'not sent'
      })()`)
      check('the brief is sent to the stand-in agent', sent === 'sent', sent)
      await drive.waitFor(`/TOASTER_HIDDEN_FIRST_VISIT/.test(document.querySelector('.lc-thread')?.innerText ?? '')`, { timeoutMs: 30_000, what: 'the long command running' }).catch((error) => say(`  ${String(error.message)}`))
      await sleep(6000)
      await measure('live turn, long command running')
      await writeFile(release, 'go', 'utf8')
      await drive.waitFor(`/BROWSER:/.test(document.querySelector('.lc-thread')?.innerText ?? '')`, { timeoutMs: 60_000, what: 'the answer' }).catch((error) => say(`  ${String(error.message)}`))
      await sleep(1500)
      const answer = String(await drive.evaluate(`/BROWSER:(\\w+)/.exec(document.querySelector('.lc-thread')?.innerText ?? '')?.[1] ?? 'no answer'`))
      check('the agent used the browser on the local page and pressed its button', answer === 'clicked', answer)
      await measure('live turn, finished')
    } else {
      const opened = String(await drive.evaluate(openConversationScript('Wren', basename(brief).slice(0, 24))))
      if (opened !== 'opened') check(`${label}: the live turn's conversation opens`, false, opened)
      await sleep(900)
      await browser(size.browser)
      await measure('live turn, finished')
    }
    // Each replayed conversation, beside the browser (or not), scrolled to its end.
    for (const file of replays) {
      const prompt = JSON.parse(readFileSync(file, 'utf8').split('\n')[0]).metadata?.prompt ?? ''
      const title = basename(String(prompt)).slice(0, 18)
      const opened = String(await drive.evaluate(openConversationScript('Casper', title)))
      if (opened !== 'opened') { check(`${label}: ${title} opens`, false, opened); continue }
      await sleep(1200)
      await browser(size.browser)
      await drive.evaluate(`(() => { const t = document.querySelector('.lc-thread'); let s = t; while (s && s.scrollHeight <= s.clientHeight + 1) s = s.parentElement; if (s) s.scrollTop = s.scrollHeight })()`)
      await sleep(600)
      await measure(`${title}, end`)
    }
  } catch (error) {
    failures += 1
    say(`drive failed at ${label}: ${error instanceof Error ? error.message : String(error)}`)
  } finally {
    await drive.finish({ intro: `The browser beside a long turn at ${label}. Build: whatever the last build wrote to out/. Replays: ${String(replays.length)}. Nothing spent.` })
  }
}
server.close()
await writeFile(join(root, 'results.json'), JSON.stringify(results, null, 2), 'utf8')
say(`\nframes and results: ${root}`)
if (existsSync(agentLog)) say(`agent log:\n${readFileSync(agentLog, 'utf8').slice(0, 1500)}`)
say(failures === 0 ? 'BROWSER BESIDE A LONG TURN PASSED' : `BROWSER BESIDE A LONG TURN: ${String(failures)} FAILED`)
process.exit(failures === 0 ? 0 : 1)
