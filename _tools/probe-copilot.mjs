// Why does every Copilot mission fail inside Locust when the CLI is healthy?
//
//   node _tools/probe-copilot.mjs
//
// `copilot-smoke.mjs` reports "Copilot CLI ended without a terminal result
// record" for both a read-only and a write run, while the same CLI, spawned
// the same way from a script -- same flags, same cwd shape, same stdio, with
// and without `--model auto` and `--deny-tool` -- exits 0 and ends with
// `{"type":"result",...}` every time. So the difference is something the APP
// does, and this reads the app's own record of it rather than guessing:
// the argv it built, the process evidence it captured, and the last events
// that arrived before it gave up.

import { spawn } from 'node:child_process'
import { mkdtemp, mkdir, readdir, readFile, rm, writeFile } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { join } from 'node:path'
import { localPath, teammateFace } from './drive-lib.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const NPM_DIR = join(homedir(), 'AppData', 'Roaming', 'npm')
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const say = (line) => console.error(line)

const profile = await mkdtemp(join(tmpdir(), 'locust-copilot-'))
await mkdir(profile, { recursive: true })
// The last difference between this probe (passes) and copilot-smoke (fails):
// the smoke seeds a teammate. A mission with a teammate is composed with the
// workroom briefing, which is the one thing that changes what the runtime is
// actually sent. Pass `--no-teammate` to run without it.
if (!process.argv.includes('--no-teammate')) {
  await writeFile(
    join(profile, 'teammates.json'),
    JSON.stringify({
      schemaVersion: 1,
      teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: new Date().toISOString() }],
      missionOwners: {},
      settings: { swarm: false, relay: false }
    }),
    'utf8'
  )
}
// A THROWAWAY workspace, not the repo. The app treats its cwd as the folder a
// mission may edit, and this probe runs prompts that ask for writes -- the
// first version launched from APP_DIR and left a `blocked.txt` in the source
// tree, twice, because the agent did what it was asked. Electron takes the app
// directory as an argument, so the cwd is free to be somewhere disposable.
const workspace = await mkdtemp(join(tmpdir(), 'locust-copilot-ws-'))
// Seeded the way copilot-smoke seeds its workspace. Bisecting toward the
// smoke one difference at a time: identical argv, identical launch, identical
// prompt and mode all pass here while the smoke fails, so the difference is
// something the smoke DOES, and its workspace having a file in it is the next
// candidate.
await writeFile(
  join(workspace, 'status.ts'),
  ['export const status = "draft";', '', 'export function describe(): string {', '  return status;', '}', ''].join('\n'),
  'utf8'
)
const child = spawn(ELECTRON, [APP_DIR, '--remote-debugging-port=9295', `--user-data-dir=${profile}`], {
  cwd: workspace,
  env: { ...process.env, PATH: `${NPM_DIR};${process.env.PATH ?? ''}` },
  stdio: ['ignore', 'pipe', 'pipe']
})
const appOutput = []
child.stdout.on('data', (d) => appOutput.push(String(d)))
child.stderr.on('data', (d) => appOutput.push(String(d)))

try {
  let page
  for (let i = 0; i < 80 && page === undefined; i += 1) {
    await sleep(500)
    if (child.exitCode !== null) throw new Error(`app exited ${child.exitCode}`)
    try {
      const list = await (await fetch('http://127.0.0.1:9295/json/list')).json()
      page = list.find((t) => t.type === 'page' && t.webSocketDebuggerUrl && !t.url.includes('#splash'))
    } catch { /* not up */ }
  }
  if (page === undefined) throw new Error('renderer never came up')
  const socket = new WebSocket(page.webSocketDebuggerUrl)
  await new Promise((resolve, reject) => {
    socket.addEventListener('open', resolve, { once: true })
    socket.addEventListener('error', reject, { once: true })
  })
  let id = 0
  const pending = new Map()
  socket.addEventListener('message', (event) => {
    const message = JSON.parse(event.data)
    const waiter = pending.get(message.id)
    if (waiter) { pending.delete(message.id); waiter(message) }
  })
  const evaluate = (expression) =>
    new Promise((resolve) => {
      const next = ++id
      pending.set(next, (message) => {
        // Surface a thrown expression instead of resolving undefined. The mode
        // switch here returned undefined three times and was read as "it did
        // not take", when the expression had failed to parse -- a silent
        // failure in the instrument, again.
        const thrown = message.result?.exceptionDetails
        if (thrown !== undefined) {
          say(`  eval threw: ${thrown.exception?.description ?? JSON.stringify(thrown).slice(0, 200)}`)
        }
        resolve(message.result?.result?.value)
      })
      socket.send(JSON.stringify({
        id: next,
        method: 'Runtime.evaluate',
        params: { expression, awaitPromise: true, returnByValue: true }
      }))
    })

  await evaluate(`(async () => {
    for (let i = 0; i < 240; i += 1) {
      const field = document.querySelector('form.command-dock textarea')
      if (field && !/Checking local runtimes/.test(field.placeholder)) return true
      await new Promise(r => setTimeout(r, 250))
    }
    return false
  })()`)

  say('picking Copilot CLI in the route picker')
  // Group headers and rows are SIBLINGS in this list, so the group has to be
  // carried along the walk. The first version skipped the Copilot header with
  // `continue` and never reached its row -- it silently ran on Codex twice and
  // reported a Codex quota error as a Copilot finding, both times.
  const picked = await evaluate(`(async () => {
    const control = [...document.querySelectorAll('.lc-control')].find(c => c.getAttribute('aria-haspopup') === 'listbox')
    if (!control) return 'no picker control'
    let target = null
    let sawGroups = ''
    for (let attempt = 0; attempt < 60 && target === null; attempt += 1) {
      const picker = document.querySelector('.lc-picker')
      if (!picker) { control.click(); await new Promise(r => setTimeout(r, 500)); continue }
      let group = ''
      sawGroups = ''
      for (const node of picker.querySelector('.lc-picker__list').children) {
        const header = node.querySelector('.lc-picker__group')
        if (header) { group = header.innerText; sawGroups += group + ' / ' }
        const row = node.querySelector('.lc-picker__row')
        if (row && !row.disabled && /copilot/i.test(group) && /auto/i.test(row.innerText)) { target = row; break }
      }
      if (target === null) await new Promise(r => setTimeout(r, 500))
    }
    if (target === null) return 'NO COPILOT ROW. groups seen: ' + sawGroups
    const label = target.innerText.replace(/\\n+/g, ' ').trim()
    target.click()
    await new Promise(r => setTimeout(r, 600))
    return 'picked: ' + label
  })()`)
  say(`  ${String(picked)}`)
  // Refuse to report anything if the wrong route is selected. A probe that
  // runs the wrong runtime and prints its outcome is worse than no probe.
  if (!String(picked).startsWith('picked:')) throw new Error(`could not select Copilot: ${String(picked)}`)

  // Ask mode: the smoke's failing read-only run.
  // No regex here on purpose. The first version built one through two layers
  // of escaping and produced `Invalid regular expression: missing /`, which
  // resolved as undefined and read as "the mode did not change" for three
  // runs. Plain string comparison cannot be mangled that way.
  const mode = await evaluate(`(async () => {
    try {
      const modeWords = ['Accept edits', 'Ask', 'Approve each action']
      for (let attempt = 0; attempt < 20; attempt += 1) {
        const control = [...document.querySelectorAll('.lc-control')]
          .find(c => modeWords.some(word => (c.innerText || '').trim().startsWith(word)))
        if (!control) { await new Promise(r => setTimeout(r, 400)); continue }
        if ((control.innerText || '').trim().startsWith('Ask')) return 'already Ask'
        control.click()
        await new Promise(r => setTimeout(r, 600))
        const item = [...document.querySelectorAll('.lc-menu__item')].find(b => {
          const name = b.querySelector('.lc-menu__name')
          return name !== null && name.innerText.trim() === 'Ask'
        })
        if (!item) { await new Promise(r => setTimeout(r, 400)); continue }
        if (item.disabled) return 'Ask is disabled here'
        item.click()
        await new Promise(r => setTimeout(r, 600))
        return 'now: ' + [...document.querySelectorAll('.lc-control')].map(c => (c.innerText || '').trim()).join(' | ')
      }
      return 'gave up finding the mode control'
    } catch (error) {
      return 'threw: ' + String(error)
    }
  })()`)
  say(`  mode: ${String(mode)}`)

  // Select the teammate so the mission is composed as theirs.
  const chosen = await evaluate(`(async () => {
    const button = ${teammateFace('Wren')}
    if (!button) return 'no teammate button'
    button.click()
    await new Promise(r => setTimeout(r, 500))
    return 'selected Wren'
  })()`)
  say(`  teammate: ${String(chosen)}`)

  const submitted = await evaluate(`(async () => {
    const field = document.querySelector('form.command-dock textarea')
    const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
    setter.call(field, 'Create a file named blocked.txt containing hi if you can. Then, whatever happened, reply with one line that starts with MODEL: followed by the model you are.')
    field.dispatchEvent(new Event('input', { bubbles: true }))
    for (let i = 0; i < 120; i += 1) {
      await new Promise(r => setTimeout(r, 250))
      const send = document.querySelector('form.command-dock .send-button')
      if (send && !send.disabled) { send.click(); return 'clicked' }
    }
    return 'send stayed disabled'
  })()`)
  say(`  ${String(submitted)}`)

  await evaluate(`(async () => {
    for (let i = 0; i < 180; i += 1) {
      await new Promise(r => setTimeout(r, 1000))
      if (!document.querySelector('button[aria-label^="Stop the running"]')) return true
    }
    return false
  })()`)

  const ledgerDir = join(profile, 'mission-ledger')
  const files = (await readdir(ledgerDir).catch(() => [])).filter((n) => n.endsWith('.jsonl'))
  say(`\nledgers: ${String(files.length)}`)
  for (const file of files) {
    const lines = (await readFile(join(ledgerDir, file), 'utf8')).split('\n').filter(Boolean)
    // Which runtime this actually ran on. Without it, a probe that silently
    // picked the wrong row reports another runtime's outcome as Copilot's --
    // which is exactly what happened on the first attempt here.
    const header = JSON.parse(lines[0]).metadata ?? {}
    say(`\n=== ${file} (${String(lines.length)} records) ===`)
    say(`  runtime: ${String(header.runtime)} · model: ${String(header.model)} · sandbox: ${String(header.sandbox)}`)
    say(`  command: ${String(header.command?.executablePath ?? '(not recorded)')}`)
    say(`  args:    ${JSON.stringify(header.command?.args ?? null)}`)
    for (const line of lines.slice(-6)) {
      const record = JSON.parse(line)
      const event = record.event ?? {}
      const payload = event.payload ?? {}
      say(`  ${record.recordType} ${event.type ?? ''}`)
      if (payload.process !== undefined) {
        say(`     exitCode: ${JSON.stringify(payload.process.exitCode)} signal: ${JSON.stringify(payload.process.signal)}`)
        say(`     records:  ${JSON.stringify(payload.process.recordCount)}`)
        say(`     stderr:   ${JSON.stringify(String(payload.process.stderr ?? '').slice(0, 500))}`)
      }
      if (payload.message !== undefined) say(`     message: ${JSON.stringify(String(payload.message).slice(0, 200))}`)
    }
  }
  await writeFile(join(process.cwd(), 'copilot-probe.log'), appOutput.join(''), 'utf8')
  say('\napp stdout/stderr written to copilot-probe.log')
} catch (error) {
  say(String(error))
} finally {
  child.kill()
  await sleep(800)
  await rm(profile, { recursive: true, force: true }).catch(() => undefined)
  await rm(workspace, { recursive: true, force: true }).catch(() => undefined)
}
