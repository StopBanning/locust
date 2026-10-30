// Grok, passes 12 and 14: a room post finishes with everyone answered, and
// `rooms.json` still says someone is `queued`.
//
//   node _tools/room-queue-drive.mjs [--members 3] [--keep]
//
// Seeds N teammates on the free OpenCode model and one room holding them,
// posts once through the same bridge the room screen uses, waits until every
// member's mission has ended on the record, and then reads the room file the
// way Grok did. The post must list N missions and carry no `queued` at all.
// Live: N real runs on the free model. No quota.

import '../_tools/scratch-root.mjs'

import { spawn } from 'node:child_process'
import { mkdtemp, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { join } from 'node:path'

import { localPath, FREE_ROUTE } from './drive-lib.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const NPM_DIR = join(homedir(), 'AppData', 'Roaming', 'npm')
const PORT = 9497
const FREE_MODEL = FREE_ROUTE.model
const MEMBERS = process.argv.includes('--members') ? Number(process.argv[process.argv.indexOf('--members') + 1]) : 3
const KEEP = process.argv.includes('--keep')
const WAIT_MS = 6 * 60 * 1000

let failures = 0
const check = (label, ok, detail) => {
  if (!ok) failures += 1
  console.error(`  [${ok ? 'PASS' : 'FAIL'}] ${label}${detail === undefined ? '' : ` -- ${detail}`}`)
}
const say = (line) => console.error(line)
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))

const workspace = await mkdtemp(join(tmpdir(), 'locust-roomq-ws-'))
await writeFile(join(workspace, 'README.md'), '# room queue\n', 'utf8')
const profile = await mkdtemp(join(tmpdir(), 'locust-roomq-'))
await mkdir(join(profile, 'mission-ledger'), { recursive: true })
const T0 = '2026-09-05T05:00:00.000Z'
const route = { runtime: 'opencode', model: FREE_MODEL, mode: 'ask' }
const NAMES = ['Wren', 'Booty', 'Quill', 'Atlas', 'Marlow', 'Sable', 'Pip', 'Juniper']
const teammates = NAMES.slice(0, MEMBERS).map((name, i) => ({
  teammateId: `tm_${name.toLowerCase()}`,
  name,
  hue: ['lime', 'blue', 'violet', 'clay'][i % 4],
  role: i === 2 ? 'Custom' : 'Code & Migrations',
  ...(i === 2 ? { roleTitle: 'Security reviewer' } : {}),
  createdAt: T0,
  route
}))
await writeFile(
  join(profile, 'teammates.json'),
  JSON.stringify({ schemaVersion: 1, teammates, missionOwners: {}, settings: { swarm: false, relay: true, relayHopCap: 6 } })
)
await writeFile(
  join(profile, 'rooms.json'),
  JSON.stringify({
    schemaVersion: 1,
    rooms: [{ roomId: 'room_standup', name: 'Standup', teammateIds: teammates.map((t) => t.teammateId), createdAt: T0, posts: [], tasks: [] }]
  })
)

const child = spawn(ELECTRON, [APP_DIR, `--remote-debugging-port=${String(PORT)}`, `--user-data-dir=${profile}`], {
  cwd: workspace,
  env: { ...process.env, PATH: `${NPM_DIR};${process.env.PATH ?? ''}` },
  stdio: ['ignore', 'pipe', 'pipe']
})
const appOutput = []
child.stdout.on('data', (d) => appOutput.push(String(d)))
child.stderr.on('data', (d) => appOutput.push(String(d)))

const roomFile = async () => JSON.parse(await readFile(join(profile, 'rooms.json'), 'utf8'))
const endedMissions = async () => {
  const names = (await readdir(join(profile, 'mission-ledger')).catch(() => [])).filter((n) => n.endsWith('.jsonl'))
  let ended = 0
  for (const name of names) {
    const text = await readFile(join(profile, 'mission-ledger', name), 'utf8').catch(() => '')
    if (/"type":"run\.(completed|failed|cancelled)"/.test(text)) ended += 1
  }
  return { files: names.length, ended }
}

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
  if (page === undefined) { say(appOutput.join('')); throw new Error('renderer never came up') }
  const socket = new WebSocket(page.webSocketDebuggerUrl)
  await new Promise((res) => socket.addEventListener('open', res, { once: true }))
  let id = 0
  const pending = new Map()
  socket.addEventListener('message', (e) => {
    const m = JSON.parse(e.data)
    const w = pending.get(m.id)
    if (w) { pending.delete(m.id); w(m) }
  })
  const send = (method, params = {}) => new Promise((res) => { const n = ++id; pending.set(n, res); socket.send(JSON.stringify({ id: n, method, params })) })
  const evaluate = async (expression) => {
    const m = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })
    if (m.result?.exceptionDetails) throw new Error(m.result.exceptionDetails.exception?.description ?? 'evaluate threw')
    return m.result?.result?.value
  }
  await send('Runtime.enable')
  // The route control names whatever route is picked (Codex by default),
  // so "is OpenCode up" is asked of discovery, not of that control's text.
  const ready = await evaluate(`(async () => {
    for (let i = 0; i < 240; i += 1) {
      const r = await window.desktop.getLocalRuntimes().catch(() => undefined)
      const oc = r && r.ok ? r.data.runtimes.find(x => x.id === 'opencode') : undefined
      if (oc && oc.installed && /\\d+[.]\\d+/.test(oc.version || '')) return true
      await new Promise(r => setTimeout(r, 500))
    }
    return false
  })()`)
  check('OpenCode is connected', ready === true)

  say(`1. post once to a room of ${String(MEMBERS)}`)
  const posted = await evaluate(`window.desktop.postToRoom({ roomId: 'room_standup', text: 'Reply with only the word CEDAR.' }).then(r => JSON.stringify(r))`)
  say(`   ${posted.slice(0, 400)}`)
  const reply = JSON.parse(posted)
  check('the post was accepted', reply.ok === true, posted.slice(0, 200))
  const afterPost = await roomFile()
  say(`   post as written: ${JSON.stringify(afterPost.rooms[0].posts[0])}`)

  say('2. wait until every member has ended on the record')
  const startedAt = Date.now()
  let seen = { files: 0, ended: 0 }
  while (Date.now() - startedAt < WAIT_MS) {
    await sleep(3000)
    seen = await endedMissions()
    const file = await roomFile()
    const post = file.rooms[0].posts[0]
    const seconds = Math.round((Date.now() - startedAt) / 1000)
    if (seconds % 15 === 0) say(`   ${String(seconds)}s: ledgers=${String(seen.files)} ended=${String(seen.ended)} missions=${String(Object.keys(post.missions).length)} queued=${JSON.stringify(post.queued ?? null)}`)
    if (seen.ended >= MEMBERS && Object.keys(post.missions).length >= MEMBERS) break
  }
  // Give the host's own run-ended work (the drain, the room note) a moment.
  await sleep(4000)

  say('3. the room file, the way Grok read it')
  const file = await roomFile()
  const post = file.rooms[0].posts[0]
  say(`   ${JSON.stringify(post)}`)
  check(`the post lists ${String(MEMBERS)} missions`, Object.keys(post.missions).length === MEMBERS, JSON.stringify(Object.keys(post.missions)))
  check('every member ended on the record', seen.ended >= MEMBERS, JSON.stringify(seen))
  check('nobody is still queued', post.queued === undefined, JSON.stringify(post.queued))
  check('nobody was refused', post.refused === undefined, JSON.stringify(post.refused))
  const shown = await evaluate(`window.desktop.listRooms().then(r => JSON.stringify(r.ok ? r.data.rooms[0].posts[0] : r))`)
  say(`   as the app reads it back: ${shown.slice(0, 400)}`)
} finally {
  child.kill()
  await sleep(1000)
  if (KEEP) say(`profile kept at ${profile}`)
  else await rm(profile, { recursive: true, force: true }).catch(() => undefined)
  await rm(workspace, { recursive: true, force: true }).catch(() => undefined)
}
console.error(failures === 0 ? 'ROOM QUEUE DRIVE PASSED' : `${String(failures)} check(s) failed`)
process.exitCode = failures === 0 ? 0 : 1
