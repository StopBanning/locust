// Chain measure: what two teammates actually WRITE to each other.
//
//   node _tools/chain-measure.mjs --label before
//   node _tools/chain-measure.mjs --label after
//
// Runs one real exchange on the FREE OpenCode route -- Wren asks Booty for a
// fact that lives in a file only Booty will read; Booty answers; Wren's
// follow-up decides whether to reply -- and writes every share block and every
// final message, in order, to docs/chain-measure/<label>-<stamp>.md.
//
// It asserts nothing about register. It records what was said so the same
// chain can be read before and after a change to the briefs
// (`workroom-briefing.ts` share form, `relay.ts` reply brief), which is the
// only way to know whether a wording change changed what the models wrote.
// Adapted from _smoke/relay-smoke.mjs; costs nothing (free model).

import '../_tools/scratch-root.mjs'
import { localPath, FREE_ROUTE, FREE_ROW, FREE_SEARCH } from './drive-lib.mjs'

import { spawn } from 'node:child_process'
import { mkdtemp, mkdir, readdir, readFile, rm, writeFile } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { join } from 'node:path'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const PORT = Number(process.argv.includes('--port') ? process.argv[process.argv.indexOf('--port') + 1] : 9471)
const NPM_DIR = join(homedir(), 'AppData', 'Roaming', 'npm')
const CURSOR_DIR = join(homedir(), 'AppData', 'Local', 'cursor-agent')
// --runtime cursor --model cursor-grok-4.6-low   (Colin's own models; cheap Cursor models only, on his go)
// --runtime claude --model haiku                 (one paid Claude run, on his go)
const RUNTIME = process.argv.includes('--runtime') ? process.argv[process.argv.indexOf('--runtime') + 1] : 'opencode'
const MODEL = process.argv.includes('--model') ? process.argv[process.argv.indexOf('--model') + 1] : FREE_ROUTE.model

/*
 * THE TOOL THAT CAN SPEND WAS THE ONE WITHOUT THE GATE.
 *
 * Every drive that presses Send asserts its route first and throws unless it
 * is the free model -- a rule added on 2026-09-19 after one of them fell
 * through to the composer's default and spent a turn of quota that is not
 * mine. This file takes `--runtime` and `--model` as arguments, which is the
 * whole point of it, and had no such check: `--runtime codex` just ran.
 *
 * So the gate is here now, and it is the SAME one `drive-lib.mjs` uses. A
 * paid route is allowed and deliberate: `LOCUST_SPEND=1` says the person
 * meant it. The free OpenCode model needs nothing, because it costs nothing.
 */
const FREE_MODEL = new RegExp(FREE_ROW.slice(1, FREE_ROW.lastIndexOf('/')), 'i')
if (!(RUNTIME === 'opencode' && FREE_MODEL.test(MODEL)) && process.env.LOCUST_SPEND !== '1') {
  console.error(`refusing to run on ${RUNTIME} / ${MODEL}: that is a paid account.`)
  console.error('Re-run with LOCUST_SPEND=1 if you mean to spend it. The free route is the default.')
  process.exit(1)
}
// -1 + 1 is 0, and argv[0] is the path to node -- so without --label this
// named every output file after C:\Program Files\nodejs\node.exe and the write
// failed on the colon (2026-09-19, measuring the tightened share rule).
const LABEL = process.argv.includes('--label') ? process.argv[process.argv.indexOf('--label') + 1] : 'run'
const TASK = process.argv.includes('--task') ? process.argv[process.argv.indexOf('--task') + 1] : 'fact'
// `fact`: one answer, one evidence line. `judge`: a verdict that needs a why,
// evidence, and something for the person to decide -- the shape Colin's real
// Jimothy/Wembley exchanges have.
const PROMPTS = {
  fact: 'Your teammate Booty has this workspace open. Using the share block form, ask Booty to find where this workspace records the share text limit, and to reply with the number, the file path and the line, quoting the sentence it is in. Do not read any files yourself, and do nothing else.',
  judge: 'Your teammate Booty has this workspace open. I believe the relay hop cap in this app defaults to 6, and I think the notes in this workspace say otherwise. Using the share block form, ask Booty to check the notes and tell you whether they agree with me, with the evidence quoted, so I can decide whether the notes need fixing. Do not read any files yourself, and do nothing else.'
}
// 'cos': Wren is a Chief of Staff and is NOT told to delegate; the role brief alone decides.
PROMPTS.cos = 'What is the share text limit recorded in this workspace, and where? I need the number and the file.'
// 'route': an ask that fits Booty's role (Research & Briefs) and is bigger than one file; Wren is a Chief of Staff.
PROMPTS.route = 'I need a short written brief, for someone joining tomorrow, on what the limits recorded in this workspace mean for an exchange between two teammates and whether the three of them are consistent with each other. Name the sources.'
// 'custom': Wren has a Custom role titled 'release manager'; the reply says what it thinks its role is.
PROMPTS.custom = 'In one line, what is your role on this team, in your own words? Do not read any files and do not use the share block.'
// 'digit': Grok's pass-10 first-hour check. On 0.179.0 in Accept edits this drew
// 25 seconds, three tool calls and a decision card offering "single digit"
// versus "complete sentences", because the last-message brief read as a rule.
// The answer is 2, and the measure is whether the final message IS 2.
PROMPTS.digit = 'What is 1 plus 1? Reply with a single digit.'
// 'quote': Grok's pass-11 memory check, from where the person stands. Their
// seeded memories.json produced NONE and no .locust/memory.md; the suspect
// was the seed's workspace id. This types the line through the Memory screen
// instead (--remember "<line>") and asks the model to quote it back.
PROMPTS.quote = 'Quote, word for word, the remembered line that mentions a secret word. If there is none, reply NONE. Do not use the share block.'
const REMEMBER = process.argv.includes('--remember') ? process.argv[process.argv.indexOf('--remember') + 1] : undefined
// 'memory': 30 team memories, the answer only in the OLDEST -- outside what the brief pastes; the file must be read.
PROMPTS.memory = 'According to the team memory, when does the API key rotate? Answer from memory and say exactly where you found it. Do not use the share block.'
// 'secret': a fake key in a file; the ledger must hold [redacted], never the key.
PROMPTS.secret = 'Read notes/keys.md. First think through what the value in it is and what kind of key it looks like, then reply with the key repeated exactly, character for character, on its own line. Do not use the share block.'
// The word the composer's route control must show for the seeded route to count as followed.
const MODEL_WORD = RUNTIME === 'opencode' ? FREE_MODEL.source : MODEL.replace(/^cursor-/, '').split('-')[0]
const PROMPT = PROMPTS[TASK] ?? PROMPTS.fact
const WREN_ROLE = TASK === 'cos' || TASK === 'route' ? 'Chief of Staff' : TASK === 'custom' ? 'Custom' : 'Code & Migrations'
const WREN_TITLE = TASK === 'custom' ? { roleTitle: 'release manager' } : {}

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
    if (reply.result?.exceptionDetails) throw new Error(reply.result.exceptionDetails.text + ' ' + JSON.stringify(reply.result.exceptionDetails.exception?.description ?? ''))
    return reply.result?.result?.value
  }
}

const workspace = await mkdtemp(join(tmpdir(), 'locust-chain-'))
const profile = await mkdtemp(join(tmpdir(), 'locust-chain-profile-'))
await mkdir(join(workspace, 'notes'), { recursive: true })
await writeFile(join(workspace, 'README.md'), 'Scratch workspace for a teammate exchange. The project notes are in notes/.\n', 'utf8')
await writeFile(
  join(workspace, 'notes', 'limits.md'),
  [
    '# Limits',
    '',
    'Numbers the app holds and where each one is set.',
    '',
    'The share text limit is 1200 characters, set in packages/runtime-adapters/src/peer-share.ts on line 23.',
    'The runtime prompt limit is 12000 characters, set in apps/desktop/src/main/workroom-briefing.ts on line 27.',
    'The relay hop cap defaults to 12 automatic replies per exchange.',
    ''
  ].join('\n'),
  'utf8'
)
const createdAt = new Date().toISOString()
// --mode accept-edits: Cursor cannot be held read-only on Windows, so a Cursor teammate's real route is never 'ask'.
const MODE = process.argv.includes('--mode') ? process.argv[process.argv.indexOf('--mode') + 1] : 'ask'
const route = { runtime: RUNTIME, model: MODEL, mode: MODE }
if (TASK === 'secret') {
  await writeFile(join(workspace, 'notes', 'keys.md'), 'Deploy key for the staging box (rotate monthly):\n\nGITHUB_TOKEN=ghp_Ab12Cd34Ef56Gh78Ij90Kl12Mn34Op56Qr78St90\n', 'utf8')
}
await writeFile(
  join(profile, 'teammates.json'),
  JSON.stringify({
    schemaVersion: 1,
    teammates: [
      { teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: WREN_ROLE, ...WREN_TITLE, createdAt, route },
      { teammateId: 'tm_booty', name: 'Booty', hue: 'violet', role: 'Research & Briefs', createdAt, route }
    ],
    missionOwners: {},
    settings: { swarm: false, relay: true, relayHopCap: 6, autoMode: false }
  })
)
if (TASK === 'memory') {
  const memories = Array.from({ length: 30 }, (_, index) => ({
    memoryId: `mem_seed${String(index).padStart(2, '0')}`,
    text: index === 0
      ? 'The API key rotates on the first Monday of each month; ops posts the new one in #keys.'
      : `Convention ${String(index)}: keep note ${String(index)} short and name the file it is about.`,
    scope: 'global',
    workspaceId: 'ws_elsewhere',
    workspaceName: 'elsewhere',
    by: { name: 'Wembley' },
    createdAt: new Date(Date.parse('2026-08-01T00:00:00.000Z') + index * 86_400_000).toISOString(),
    status: 'kept',
    enabled: true
  }))
  await writeFile(join(profile, 'memories.json'), JSON.stringify({ schemaVersion: 1, memories }))
}
const LEDGER_DIR = join(profile, 'mission-ledger')

const child = spawn(ELECTRON, [APP_DIR, `--remote-debugging-port=${PORT}`, `--user-data-dir=${profile}`], {
  cwd: workspace,
  env: { ...process.env, PATH: `${NPM_DIR};${CURSOR_DIR};${process.env.PATH ?? ''}` },
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
    } catch {
      // not up yet
    }
  }
  if (page === undefined) {
    say(appOutput.join(''))
    throw new Error('no renderer target')
  }
  const socket = new WebSocket(page.webSocketDebuggerUrl)
  await new Promise((resolve, reject) => {
    socket.addEventListener('open', resolve, { once: true })
    socket.addEventListener('error', reject, { once: true })
  })
  const cdp = new Cdp(socket)
  await cdp.send('Runtime.enable')
  const discovered = await cdp.eval(`(async () => {
    for (let i = 0; i < 240; i += 1) {
      const control = [...document.querySelectorAll('.lc-control')].find(b => b.getAttribute('aria-haspopup') === 'listbox')
      if (control && /opencode|cursor|codex|claude/i.test(control.innerText)) return control.innerText
      await new Promise(r => setTimeout(r, 500))
    }
    return ''
  })()`)
  say(`   discovery: ${JSON.stringify(discovered)}`)

  if (REMEMBER !== undefined) {
    say(`[${LABEL}] 1b. remember a line through the Memory screen, mode Keep and tell me`)
    const remembered = await cdp.eval(`(async () => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: '5', code: 'Digit5', ctrlKey: true, bubbles: true }))
      await new Promise(r => setTimeout(r, 800))
      const radio = [...document.querySelectorAll('[role="radio"]')].find(b => /Keep and tell me/.test(b.innerText))
      if (!radio) return JSON.stringify({ screen: false, body: document.body.innerText.slice(0, 200) })
      radio.click()
      await new Promise(r => setTimeout(r, 400))
      // A textarea, so the sentence can wrap; the setter has to be the
      // textarea's own or React never hears the change.
      const input = [...document.querySelectorAll('textarea.lc-input')].find(i => /specific enough to act on/.test(i.getAttribute('placeholder') || ''))
      if (!input) return JSON.stringify({ screen: true, input: false })
      const setInput = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
      setInput.call(input, ${JSON.stringify(REMEMBER)})
      input.dispatchEvent(new Event('input', { bubbles: true }))
      await new Promise(r => setTimeout(r, 300))
      const button = [...document.querySelectorAll('button.lc-primarybutton')].find(b => /^Remember$/.test(b.innerText.trim()))
      if (!button || button.disabled) return JSON.stringify({ screen: true, input: true, button: !!button, disabled: button ? button.disabled : null })
      button.click()
      await new Promise(r => setTimeout(r, 1200))
      const kept = [...document.querySelectorAll('.lc-memory__text')].map(p => p.innerText.trim())
      const mode = [...document.querySelectorAll('[role="radio"][aria-checked="true"]')].map(b => b.innerText.trim())
      window.dispatchEvent(new KeyboardEvent('keydown', { key: '1', code: 'Digit1', ctrlKey: true, bubbles: true }))
      await new Promise(r => setTimeout(r, 600))
      return JSON.stringify({ screen: true, input: true, kept, mode })
    })()`)
    say(`   ${remembered}`)
    const seen = JSON.parse(remembered)
    if (!(seen.kept ?? []).some((line) => line.includes(REMEMBER))) { say(appOutput.join('').slice(-2000)); throw new Error('the line was not kept by the Memory screen') }
  }

  say(`[${LABEL}] 2. select Wren; the composer should follow Wren's remembered route`)
  const setup = await cdp.eval(`(async () => {
    // The roster lives under the Team view (Ctrl 2); the home view shows faces only.
    const teamView = [...document.querySelectorAll('button')].find(b => /^Team/.test(b.getAttribute('aria-label') || b.getAttribute('title') || ''))
    if (teamView) { teamView.click(); await new Promise(r => setTimeout(r, 600)) }
    let wren
    for (let attempt = 0; attempt < 40 && !wren; attempt += 1) {
      wren = [...document.querySelectorAll('button')].find(b => (b.querySelector('.lc-row__name') || { innerText: '' }).innerText.trim().startsWith('Wren'))
        || [...document.querySelectorAll('button')].find(b => /^(Message )?Wren\\b/.test(b.getAttribute('title') || b.getAttribute('aria-label') || ''))
      if (!wren) await new Promise(r => setTimeout(r, 250))
    }
    if (!wren) return JSON.stringify({
      wren: false,
      body: document.body.innerText.replace(/[ \\t\\r\\n]+/g, ' ').slice(0, 600),
      buttons: [...document.querySelectorAll('button')].map(x => (x.getAttribute('aria-label') || x.getAttribute('title') || x.innerText || '').replace(/[ \\t\\r\\n]+/g, ' ').trim().slice(0, 50)).filter(Boolean).slice(0, 60),
      team: [...document.querySelectorAll('[class*="lc-team"], [class*="lc-roster"], [class*="lc-row"]')].map(e => e.className).slice(0, 30)
    })
    wren.click()
    await new Promise(r => setTimeout(r, 800))
    const control = [...document.querySelectorAll('.lc-control')].find(b => b.getAttribute('aria-haspopup') === 'listbox')
    let text = control ? control.innerText : ''
    if (!new RegExp(${JSON.stringify(MODEL_WORD)}, "i").test(text)) {
      control.click()
      await new Promise(r => setTimeout(r, 400))
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
      if (!target) return JSON.stringify({ wren: true, picked: false, text })
      target.click()
      await new Promise(r => setTimeout(r, 400))
      text = control.innerText
    }
    return JSON.stringify({ wren: true, picked: true, text, controls: [...document.querySelectorAll('.lc-control')].map(c => c.innerText.replace(/[^a-z0-9 ./-]+/gi, ' ').trim()) })
  })()`)
  say(`   ${setup}`)
  if (!JSON.parse(setup).picked) { say(appOutput.join('').slice(-3000)); throw new Error('route not picked') }

  say(`[${LABEL}] 3. Wren is asked to send Booty the question`)
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
    return 'send stayed disabled: ' + (field.placeholder || '')
  })()`)
  say(`   ${submitted}`)
  if (submitted !== 'clicked') throw new Error(submitted)

  say(`[${LABEL}] 4. waiting for the exchange to end (no new ledger for 90s and nothing running)`)
  let names = await ledgers()
  let quiet = 0
  for (let i = 0; i < 900 && quiet < 90; i += 1) {
    await sleep(1000)
    const now = await ledgers()
    const running = await cdp.eval(`!!document.querySelector('button[aria-label^="Stop the running"]')`).catch(() => true)
    if (now.length !== names.length || running) { names = now; quiet = 0 } else quiet += 1
  }
  say(`   ledgers: ${names.length}`)
  // What the person would see: the thread's tail, notices included, since a relay refusal is said only there.
  const threadTail = await cdp.eval("(document.querySelector('.lc-thread') || document.body).innerText.slice(-2500)").catch(() => '')
  say('   thread tail: ' + threadTail.replace(new RegExp('[' + String.fromCharCode(32, 9, 13, 10) + ']+', 'g'), ' ').slice(-700))

  say(`[${LABEL}] 5. reading what was written`)
  const missions = []
  for (const name of names) {
    const lines = (await readFile(join(LEDGER_DIR, name), 'utf8')).split('\n').filter(Boolean).map((l) => { try { return JSON.parse(l) } catch { return undefined } }).filter(Boolean)
    const header = lines[0]?.metadata ?? {}
    const strings = []
    const walk = (v) => { if (typeof v === 'string') strings.push(v); else if (v && typeof v === 'object') for (const k of Object.keys(v)) walk(v[k]) }
    for (const line of lines.slice(1)) walk(line)
    const shares = [...new Set(strings.flatMap((s) => [...s.matchAll(/<locust-share[^>]*>[\s\S]*?<\/locust-share>/g)].map((m) => m[0])))]
    const longestShare = shares.sort((a, b) => b.length - a.length)[0]
    // The final answer: the longest string that contains the share (or, with none, the longest message-like string).
    const finals = strings.filter((s) => s.length > 40 && !s.startsWith('{') && !/^[A-Za-z_]+$/.test(s))
    const final = (longestShare ? finals.filter((s) => s.includes(longestShare)) : finals).sort((a, b) => b.length - a.length)[0]
    const terminal = lines.filter((l) => l.kind === 'terminal' || l.type === 'terminal' || l.status).map((l) => l.status ?? l.kind).pop()
    missions.push({ name, createdAt: header.createdAt, startedBy: header.startedBy, runtime: header.runtime, model: header.model, prompt: header.prompt, shares, final, terminal })
  }
  missions.sort((a, b) => Date.parse(a.createdAt) - Date.parse(b.createdAt))
  const owners = JSON.parse(await readFile(join(profile, 'teammates.json'), 'utf8')).missionOwners ?? {}
  const stamp = new Date().toISOString().slice(0, 19).replace(/[:T]/g, '-')
  const out = [`# Chain measure: ${LABEL} (${stamp})`, '', `Model: ${MODEL}. Person's prompt to Wren:`, '', `> ${PROMPT}`, '']
  out.push('## Thread tail (what the person saw)', '', '```', threadTail, '```', '')
  missions.forEach((m, i) => {
    const owner = Object.entries(owners).find(([id]) => m.name.includes(id))?.[1] ?? '?'
    out.push(`## ${i + 1}. ${owner} — ${m.startedBy ? `relay hop ${m.startedBy.hop}` : 'person-started'} — ${m.runtime}/${m.model}`, '')
    out.push('**Brief (prompt recorded):**', '', '```', String(m.prompt ?? '').slice(0, 3000), '```', '')
    out.push(`**Share blocks (${m.shares.length}):**`, '')
    for (const s of m.shares) out.push('```', s, '```', '')
    out.push('**Final message:**', '', '```', String(m.final ?? '(none)').slice(-2500), '```', '')
  })
  await mkdir(new URL('../docs/chain-measure/', import.meta.url), { recursive: true })
  const file = new URL(`../docs/chain-measure/${LABEL}-${TASK}-${stamp}.md`, import.meta.url)
  await writeFile(file, out.join('\n'), 'utf8')
  say(`   wrote ${file.pathname.slice(1)}`)
  say(`   missions: ${missions.length}; shares per mission: ${missions.map((m) => m.shares.length).join(',')}`)
  if (TASK === 'secret') {
    const raw = await Promise.all(names.map((name) => readFile(join(LEDGER_DIR, name), 'utf8')))
    const text = raw.join('\n')
    say(`   ledger holds the raw key: ${String(/ghp_Ab12Cd34/.test(text))}; holds [redacted]: ${String((text.match(/\[redacted\]/g) ?? []).length)} times; reasoning records: ${String((text.match(/"stepKind":"reasoning"/g) ?? []).length)}`)
  }
  if (TASK === 'quote') {
    // From where the person stands: the line was typed on the Memory screen,
    // so the brief has to paste it and the model has to say it back.
    const file = await readFile(join(workspace, '.locust', 'memory.md'), 'utf8').catch(() => undefined)
    const word = REMEMBER === undefined ? undefined : REMEMBER.split(' ').find((w) => /^[A-Z]{4,}\.?$/.test(w))?.replace(/\.$/, '')
    say(`   memory file: ${file === undefined ? 'NOT WRITTEN' : 'written, holds the line: ' + String(REMEMBER !== undefined && file.includes(REMEMBER))}`)
    say(`   answer quotes the secret word (${word ?? '?'}): ${String(word !== undefined && new RegExp(word).test(threadTail))}`)
    if (file === undefined || (word !== undefined && !new RegExp(word).test(threadTail))) process.exitCode = 1
  }
  if (TASK === 'memory') {
    const file = await readFile(join(workspace, '.locust', 'memory.md'), 'utf8').catch(() => undefined)
    say(`   memory file: ${file === undefined ? 'NOT WRITTEN' : String(file.split('\n').filter((l) => l.startsWith('- ')).length) + ' lines'}`)
    say(`   answer mentions first Monday: ${String(/first Monday/i.test(missions[0]?.final ?? ''))}; mentions the file: ${String(/memory\.md/i.test(missions[0]?.final ?? ''))}`)
  }
} finally {
  child.kill()
  await sleep(500)
  if (process.argv.includes('--keep')) say(`profile kept at ${profile}`)
  else {
    await rm(profile, { recursive: true, force: true }).catch(() => undefined)
    await rm(workspace, { recursive: true, force: true }).catch(() => undefined)
  }
}
