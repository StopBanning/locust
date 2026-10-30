// Does a teammate reach an OLD memory -- one the brief no longer pastes?
//
//   node _tools/memory-recall-drive.mjs [--keep]
//
// The brief pastes the newest 8 memories and names `.locust/memory.md` for
// the rest. Nobody had verified that a teammate opens the file. This seeds
// TWELVE folder memories, the OLDEST of which holds the only answer to the
// question asked ("the secret word is PELICAN"), asks Wren on the free
// OpenCode model, and reports three facts from the record:
//
//   1. whether the reply carries PELICAN at all;
//   2. whether any tool event in the ledger touched memory.md;
//   3. what the reply actually said.
//
// Nothing is asserted. Run it before and after the brief learns to rank by
// relevance (docs/PLAN-2026-09-21-COMPACT.md, item 3).

import { spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

import { localPath, FREE_ROUTE } from './drive-lib.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const NPM_DIR = join(homedir(), 'AppData', 'Roaming', 'npm')
const PORT = 9278
const FREE_MODEL = FREE_ROUTE.model
const KEEP = process.argv.includes('--keep')
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const say = (line) => console.error(line)

const workspace = await mkdtemp(join(tmpdir(), 'locust-recall-ws-'))
await writeFile(join(workspace, 'README.md'), '# scratch\n', 'utf8')
const WORKSPACE_ID = `ws_${createHash('sha256').update(resolve(workspace), 'utf8').digest('hex').slice(0, 32)}`
const profile = await mkdtemp(join(tmpdir(), 'locust-recall-'))
await mkdir(join(profile, 'mission-ledger'), { recursive: true })
const route = { runtime: 'opencode', model: FREE_MODEL, mode: 'ask' }
await writeFile(
  join(profile, 'teammates.json'),
  JSON.stringify({
    schemaVersion: 1,
    teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: '2026-09-01T05:00:00.000Z', route }],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 6, memoryMode: 'auto' }
  })
)
// Twelve memories, one a day; the oldest is the one that answers.
const FILLER = [
  'The build command is pnpm build.',
  'Tests run with pnpm test, never npm.',
  'The person prefers diffs to prose.',
  'Docs live under docs/ and are Markdown.',
  'The API listens on port 4000 in development.',
  'Commits are written in the imperative.',
  'The release script is _tools/ship.mjs.',
  'CSS tokens are in shell.css and nowhere else.',
  'The sidebar lists conversations newest first.',
  'Rooms are for more than one teammate at once.',
  'The changelog is written for people, not engineers.'
]
const memories = [
  { text: 'The secret word for this project is PELICAN.', at: '2026-09-01T05:00:00.000Z' },
  ...FILLER.map((text, index) => ({ text, at: `2026-09-${String(2 + index).padStart(2, '0')}T05:00:00.000Z` }))
].map((entry, index) => ({
  memoryId: `mem_${String(index)}`, text: entry.text, scope: 'workspace', workspaceId: WORKSPACE_ID, workspaceName: 'scratch',
  by: { name: 'you' }, createdAt: entry.at, status: 'kept', enabled: true
}))
await writeFile(join(profile, 'memories.json'), JSON.stringify({ schemaVersion: 1, memories }))

const child = spawn(ELECTRON, [APP_DIR, `--remote-debugging-port=${String(PORT)}`, `--user-data-dir=${profile}`], {
  cwd: workspace,
  env: { ...process.env, PATH: `${NPM_DIR};${process.env.PATH ?? ''}` },
  stdio: ['ignore', 'ignore', 'ignore']
})
try {
  let page
  for (let attempt = 0; attempt < 80 && page === undefined; attempt += 1) {
    await sleep(500)
    if (child.exitCode !== null) throw new Error(`app exited ${String(child.exitCode)}`)
    try {
      const list = await (await fetch(`http://127.0.0.1:${String(PORT)}/json/list`)).json()
      page = list.find((t) => t.type === 'page' && t.webSocketDebuggerUrl && !t.url.includes('#splash'))
    } catch { /* not up */ }
  }
  if (page === undefined) throw new Error('renderer never came up')
  const socket = new WebSocket(page.webSocketDebuggerUrl)
  await new Promise((res, rej) => { socket.addEventListener('open', res, { once: true }); socket.addEventListener('error', rej, { once: true }) })
  let id = 0
  const pending = new Map()
  socket.addEventListener('message', (event) => {
    const message = JSON.parse(event.data)
    const waiter = pending.get(message.id)
    if (waiter) { pending.delete(message.id); waiter(message) }
  })
  const evaluate = (expression) => new Promise((resolve_) => {
    const next = ++id
    const gaveUp = setTimeout(() => { if (pending.delete(next)) resolve_(undefined) }, 600_000)
    pending.set(next, (message) => { clearTimeout(gaveUp); resolve_(message?.result?.result?.value) })
    socket.send(JSON.stringify({ id: next, method: 'Runtime.evaluate', params: { expression, awaitPromise: true, returnByValue: true } }))
  })
  const ready = await evaluate(`(async () => {
    for (let i = 0; i < 240; i += 1) {
      const field = document.querySelector('form.command-dock textarea')
      if (field && !/Checking local runtimes/.test(field.placeholder)) return true
      await new Promise(r => setTimeout(r, 250))
    }
    return false
  })()`)
  say(`discovery finished: ${String(ready)}`)
  const outcome = await evaluate(`(async () => {
    let face
    for (let i = 0; i < 40 && !face; i += 1) {
      face = [...document.querySelectorAll('.lc-faces__one')].find(b => (b.getAttribute('aria-label') || '').startsWith('Wren '))
      if (!face) await new Promise(r => setTimeout(r, 250))
    }
    if (!face) return 'no face'
    face.click()
    await new Promise(r => setTimeout(r, 600))
    const field = document.querySelector('form.command-dock textarea')
    const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set
    setter.call(field, 'What is the secret word for this project? Answer with the word alone. Your team memory knows it; do not guess.')
    field.dispatchEvent(new Event('input', { bubbles: true }))
    for (let i = 0; i < 120; i += 1) {
      await new Promise(r => setTimeout(r, 250))
      const button = document.querySelector('button[aria-label="Send"]')
      if (button && !button.disabled) { button.click(); break }
    }
    let sawRunning = false
    for (let i = 0; i < 720; i += 1) {
      await new Promise(r => setTimeout(r, 500))
      const stop = document.querySelector('button[aria-label^="Stop the running"]')
      if (stop) sawRunning = true
      if (sawRunning && !stop) return 'finished'
    }
    return 'still running'
  })()`)
  say(`run: ${String(outcome)}`)
  await sleep(2000)
  const dir = join(profile, 'mission-ledger')
  const names = (await readdir(dir)).filter((n) => n.endsWith('.jsonl'))
  for (const name of names) {
    const text = await readFile(join(dir, name), 'utf8')
    const buffers = new Map()
    const touched = []
    for (const line of text.split('\n')) {
      if (line.trim().length === 0) continue
      let record
      try { record = JSON.parse(line) } catch { continue }
      const event = record.event
      if (event === undefined) continue
      if (event.type === 'message.delta') {
        const { itemId, operation, text: delta } = event.payload
        buffers.set(itemId, operation === 'replace' ? delta : `${buffers.get(itemId) ?? ''}${delta}`)
      }
      if (/^tool\./.test(event.type) && /memory/i.test(JSON.stringify(event.payload))) touched.push(`${event.type}: ${JSON.stringify(event.payload).slice(0, 200)}`)
    }
    const reply = [...buffers.values()].at(-1) ?? ''
    say(`reply carries PELICAN: ${String(/PELICAN/i.test(reply))}`)
    say(`tool events touching memory: ${String(touched.length)}`)
    for (const line of touched) say(`  ${line}`)
    say(`reply: ${JSON.stringify(reply.slice(0, 400))}`)
  }
  const file = await readFile(join(workspace, '.locust', 'memory.md'), 'utf8').catch(() => undefined)
  say(`memory.md written: ${String(file !== undefined)}; carries PELICAN: ${String(file !== undefined && /PELICAN/.test(file))}`)
} finally {
  child.kill()
  await sleep(500)
  if (KEEP) say(`profile kept at ${profile}`)
  else await rm(profile, { recursive: true, force: true }).catch(() => undefined)
  await rm(workspace, { recursive: true, force: true }).catch(() => undefined)
}
