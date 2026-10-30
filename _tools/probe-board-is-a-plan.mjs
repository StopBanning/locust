// Photograph the room's task board, now that it is drawn as a plan.
//
//   node _tools/probe-board-is-a-plan.mjs --shot board.png
//
// Colin, 2026-09-13: "I just really enjoyed that UI over the task bar we
// setup. Would there be a way to have at least replace that?" The rows had
// already been rebuilt on the plan step; everything around them had not --
// a bordered raised card with a titled head, above every room, drawn at full
// size even with nothing on it.
//
// This asks the RENDERED page rather than the stylesheet: what the board's
// computed border and background actually are, whether its header is the
// plan's own counter line, whether a row still carries a rule, and whether
// the per-row controls are quiet until reached for. Then it opens a room
// with no tasks at all, because "an empty board is one quiet line rather
// than a panel" is the half a screenshot of a full board would never show.
//
// It spends nothing: every mission it reads is seeded on disk and no runtime
// is ever started.

import { spawn } from 'node:child_process'
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises'
import { homedir, tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { createHash } from 'node:crypto'

import { localPath, FREE_ROUTE } from './drive-lib.mjs'

const APP_DIR = localPath(new URL('../apps/desktop/', import.meta.url))
const ELECTRON = join(APP_DIR, 'node_modules', 'electron', 'dist', 'electron.exe')
const NPM_DIR = join(homedir(), 'AppData', 'Roaming', 'npm')
// A `_tools` drive port, from the 9490-9599 range `_smoke/ports.mjs` reserves
// for them, and not one any other drive in this folder already takes.
const PORT = 9514
const args = process.argv.slice(2)
const shotAt = args.indexOf('--shot')
const SHOT = shotAt === -1 ? undefined : resolve(args[shotAt + 1])
const FREE_MODEL = FREE_ROUTE.model

let failures = 0
function check(label, ok, detail) {
  if (!ok) failures += 1
  console.error(`  [${ok ? 'PASS' : 'FAIL'}] ${label}${detail === undefined ? '' : ` -- ${detail}`}`)
}
const say = (line) => console.error(line)
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))

// The page scripts, named rather than inlined at the call: every one of them
// is read by `embedded-scripts-parse`, and a script built inside an argument
// list is where this repo has lost escapes before.
const READY = `(async () => {
  for (let i = 0; i < 240; i += 1) {
    const f = document.querySelector('form.command-dock textarea')
    if (f && !/Checking local runtimes/.test(f.placeholder)) return true
    await new Promise(r => setTimeout(r, 250))
  }
  return false
})()`

const openRoom = (name) => `(async () => {
  const wanted = ${JSON.stringify(name)}
  const rows = [...document.querySelectorAll('button, .lc-roomrow, .lc-roomcard')]
  const row = rows.find((r) => (r.innerText || '').trim().split('\\n')[0] === wanted)
  if (row) row.click()
  await new Promise(r => setTimeout(r, 800))
  return row !== undefined
})()`

const BOARD_FACTS = `(() => {
  const el = document.querySelector('.lc-board')
  if (!el) return { found: false }
  const style = getComputedStyle(el)
  const head = el.querySelector('.lc-board__head')
  const rows = [...el.querySelectorAll('.lc-task')]
  const actions = rows[0] ? rows[0].querySelector('.lc-task__actions') : null
  return {
    found: true,
    head: head ? head.innerText.trim() : null,
    border: style.borderTopWidth,
    background: style.backgroundColor,
    rows: rows.length,
    rowBorder: rows[0] ? getComputedStyle(rows[0]).borderTopWidth : null,
    usesPlanStep: rows.length > 0 && rows.every((r) => r.classList.contains('lc-plan__step')),
    actionsHiddenAtRest: actions ? getComputedStyle(actions).opacity : null,
    restText: rows[0] ? rows[0].innerText.replace(/\\s+/g, ' ').trim() : null
  }
})()`

const EMPTY_FACTS = `(() => {
  const el = document.querySelector('.lc-board')
  if (!el) return { found: false }
  const add = el.querySelector('.lc-board__add')
  return {
    found: true,
    head: el.querySelector('.lc-board__head') === null ? 'none' : 'present',
    rows: el.querySelectorAll('.lc-task').length,
    addOpacity: add ? getComputedStyle(add).opacity : null,
    height: Math.round(el.getBoundingClientRect().height)
  }
})()`

const workspace = await mkdtemp(join(tmpdir(), 'locust-board-ws-'))
await writeFile(join(workspace, 'README.md'), '# board\n', 'utf8')
const WORKSPACE_ID = `ws_${createHash('sha256').update(resolve(workspace), 'utf8').digest('hex').slice(0, 32)}`
const profile = await mkdtemp(join(tmpdir(), 'locust-board-'))
const LEDGER_DIR = join(profile, 'mission-ledger')
await mkdir(LEDGER_DIR, { recursive: true })
const ROOMS = join(profile, 'rooms.json')
const T0 = '2026-09-05T05:00:00.000Z'
const T1 = '2026-09-05T05:01:00.000Z'
const route = { runtime: 'opencode', model: FREE_MODEL, mode: 'ask' }

await writeFile(
  join(profile, 'teammates.json'),
  JSON.stringify({
    schemaVersion: 1,
    teammates: [
      { teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: T0, route },
      { teammateId: 'tm_booty', name: 'Booty', hue: 'blue', role: 'Custom', createdAt: T0, route }
    ],
    missionOwners: { mission_wren1: 'tm_wren' },
    settings: { swarm: false, relay: true, relayHopCap: 6 }
  })
)

await writeFile(
  ROOMS,
  JSON.stringify({
    schemaVersion: 1,
    rooms: [
      {
        roomId: 'room_release',
        name: 'Release',
        teammateIds: ['tm_wren', 'tm_booty'],
        createdAt: T0,
        posts: [{ postId: 'post_1', text: 'Which files mention the release date?', at: T0, missions: { tm_wren: 'mission_wren1' } }],
        tasks: [
          { taskId: 'task_notes', text: 'Write the release notes', ownerId: 'tm_wren', state: 'in-hand', missionId: 'mission_wren1', at: T1 },
          { taskId: 'task_version', text: 'Check the version string', state: 'open', at: T0 },
          { taskId: 'task_tag', text: 'Tag the release and push it', ownerId: 'tm_booty', state: 'done', at: T1 }
        ]
      },
      // A room with no tasks at all: the half a full board cannot show.
      { roomId: 'room_fresh', name: 'Fresh', teammateIds: ['tm_wren'], createdAt: T0, posts: [], tasks: [] }
    ]
  })
)

const PROCESS = {
  exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 3, inputDeliveryFailed: false,
  outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: T0, finishedAt: T1
}
const metadata = {
  missionId: 'mission_wren1', runId: 'run_wren1', prompt: 'Which files mention the release date?',
  runtime: 'opencode', model: FREE_MODEL, requestedRouteId: 'opencode', resolvedRouteId: 'opencode-account:default',
  cliVersion: '1.18.27', workspaceId: WORKSPACE_ID, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: T0
}
const events = [
  { type: 'run.started', occurredAt: T0, payload: { runtimeThreadId: 'thread-1', evidence: { redacted: true } } },
  { type: 'message.delta', occurredAt: T1, payload: { itemId: 'answer', operation: 'append', text: 'README.md and CHANGELOG.md mention it.', final: true, evidence: { redacted: true } } },
  { type: 'run.completed', occurredAt: T1, payload: { process: PROCESS, evidence: { redacted: true } } }
]
const lines = [JSON.stringify({ schemaVersion: 13, recordType: 'mission.created', ledgerSequence: 1, occurredAt: T0, metadata })]
events.forEach((event, index) => {
  lines.push(JSON.stringify({
    schemaVersion: 13, recordType: 'mission.event', ledgerSequence: index + 2, occurredAt: event.occurredAt,
    event: { ...event, id: `mission_wren1:${String(index + 1)}`, runId: 'run_wren1', missionId: 'mission_wren1', sequence: index + 1, sourceAdapter: 'opencode' }
  }))
})
await writeFile(join(LEDGER_DIR, 'mission_wren1.jsonl'), `${lines.join('\n')}\n`, 'utf8')

const child = spawn(ELECTRON, [APP_DIR, `--remote-debugging-port=${String(PORT)}`, `--user-data-dir=${profile}`], {
  cwd: workspace,
  env: { ...process.env, PATH: `${NPM_DIR};${process.env.PATH ?? ''}` },
  stdio: ['ignore', 'pipe', 'pipe']
})
child.stdout.on('data', () => undefined)
child.stderr.on('data', () => undefined)

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
  await new Promise((res) => socket.addEventListener('open', res, { once: true }))
  let id = 0
  const pending = new Map()
  socket.addEventListener('message', (e) => {
    const m = JSON.parse(e.data)
    const w = pending.get(m.id)
    if (w) { pending.delete(m.id); w(m) }
  })
  const send = (method, params = {}) =>
    Promise.race([
      new Promise((res) => { const n = ++id; pending.set(n, res); socket.send(JSON.stringify({ id: n, method, params })) }),
      new Promise((res) => { const t = setTimeout(() => res({ error: { message: 'cdp timeout' } }), 120_000); t.unref() })
    ])
  const evaluate = async (expression) => {
    const m = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })
    if (m.error) throw new Error(JSON.stringify(m.error))
    if (m.result?.exceptionDetails) throw new Error(m.result.exceptionDetails.exception?.description ?? 'evaluate threw')
    return m.result?.result?.value
  }
  await send('Runtime.enable')
  await evaluate(READY)
  await evaluate(openRoom('Release'))

  const board = await evaluate(BOARD_FACTS)
  say(`       ${JSON.stringify(board)}`)
  check(
    'the board is a plan, not a card',
    board.found === true && board.border === '0px' && /rgba\(0, 0, 0, 0\)|transparent/.test(String(board.background)),
    `border ${String(board.border)} / background ${String(board.background)}`
  )
  check('its header is the plan-s own counter line', /^TASKS . \d+ of \d+ done$/.test(String(board.head)), String(board.head))
  check(
    'every row is a plan step with no rule above it',
    board.usesPlanStep === true && board.rowBorder === '0px',
    `${String(board.rows)} rows, rule ${String(board.rowBorder)}`
  )
  check('the controls are quiet until reached for', board.actionsHiddenAtRest === '0', String(board.actionsHiddenAtRest))
  check('and a row at rest is the marker, the words and who has it', /Write the release notes/.test(String(board.restText)), String(board.restText))

  if (SHOT !== undefined) {
    const shot = await send('Page.captureScreenshot', { format: 'png' })
    if (typeof shot.result?.data === 'string') await writeFile(SHOT, Buffer.from(shot.result.data, 'base64'))
    say(`       wrote ${SHOT}`)
  }

  await evaluate(openRoom('Fresh'))
  const empty = await evaluate(EMPTY_FACTS)
  say(`       ${JSON.stringify(empty)}`)
  check('an empty board draws no header and no rows', empty.found === true && empty.head === 'none' && empty.rows === 0, JSON.stringify(empty))
  check(
    'and is one quiet line rather than a panel',
    Number(empty.addOpacity) < 1 && Number(empty.height) < 80,
    `${String(empty.addOpacity)} opacity, ${String(empty.height)}px tall`
  )

  if (SHOT !== undefined) {
    const shot = await send('Page.captureScreenshot', { format: 'png' })
    const path = SHOT.replace(/\.png$/, '-empty.png')
    if (typeof shot.result?.data === 'string') await writeFile(path, Buffer.from(shot.result.data, 'base64'))
    say(`       wrote ${path}`)
  }
} catch (error) {
  failures += 1
  say(`  [FAIL] ${error instanceof Error ? error.message : String(error)}`)
} finally {
  try { child.kill() } catch { /* gone */ }
  await sleep(1500)
  await rm(profile, { recursive: true, force: true }).catch(() => undefined)
  await rm(workspace, { recursive: true, force: true }).catch(() => undefined)
}

console.error(failures === 0 ? '\nBOARD PROBE PASSED' : `\nBOARD PROBE FAILED (${String(failures)})`)
process.exit(failures === 0 ? 0 : 1)
