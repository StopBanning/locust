// A fair measurement: Electron and the Rust app, each showing the SAME conversation, at the same size, measured the same
// way over the same idle minute. Spends nothing; no agent runs.
//
//   node apps/native/scripts/bench-pair.mjs --out <folder> [--seconds 60]
//
// Electron runs as every drive does (_tools/drive-lib.mjs): a throwaway profile, its window behind other windows and
// never focused. The Rust app runs its quiet bench window (`--bench`, below other windows, click-through) with HOME
// pointed at a copy of that same profile, so it reads exactly the ledger Electron read, and none of the person's own.
// Nothing here touches the person's keyboard, mouse or screen.

import { execFileSync, spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import { cp, mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises'
import { statSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'

// The checkout whose Electron is built (the drive launches it, and its ledger writer seeds the conversation).
const ROOT = process.env.LOCUST_ROOT ?? join(process.env.HOME, 'work/locust')
const { FREE_ROUTE, openTeammateScript, say, scratchRepository, sleep, startDrive } = await import(join(ROOT, '_tools/drive-lib.mjs'))

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const OUT = resolve(arg('--out') ?? 'bench-pair')
const SECONDS = Number(arg('--seconds') ?? '60')
const WARM = 10
const SIZE = [1100, 780]
const CONVERSATION = 'mission_polish_second'
const BINARY = process.env.LOCUST_NATIVE_BINARY ?? join(process.env.HOME, 'work/scratch/native-target/release/locust-native')
await mkdir(OUT, { recursive: true })

/** The polish drive's conversation (_tools/polish-frames.mjs): a finished two-turn one with Wren, and seven more. */
async function seedConversation(folder) {
  const { createFileMissionLedger } = await import(join(ROOT, 'packages/mission-store/dist/index.js'))
  const profile = await mkdtemp(join(tmpdir(), 'locust-benchpair-profile-'))
  const ledger = createFileMissionLedger({ rootDirectory: join(profile, 'mission-ledger') })
  const workspaceId = `ws_${createHash('sha256').update(folder).digest('hex').slice(0, 32)}`
  const answer = '## Acme Storefront\n\nA small shop: a cart, a checkout and a tax table.\n\n- `src/cart.py` holds the cart\n- the open issue is a total that counts an item twice\n\n| Area | State |\n| --- | --- |\n| Cart | bug |\n| Checkout | fine |\n\n```python\ndef total(items):\n    return sum(i.price for i in items)\n```'
  const turns = [
    { missionId: 'mission_polish_first', owner: 'tm_wren', prompt: 'Look at the files in this folder and tell me what this project is, then sketch a kickoff note.', tools: [{ command: 'ls -la', title: 'Listed the files in the folder', output: 'README.md\nissue.md\nsrc\ntests' }, { command: 'cat README.md issue.md', title: 'Read the README and the open issue', output: '# Acme Storefront ...' }], answer },
    { missionId: 'mission_polish_second', owner: 'tm_wren', follows: 'mission_polish_first', prompt: 'Yes, start on the fix. Keep it to the one function.', tools: [{ command: 'python -m pytest -q', title: 'Ran the tests', output: '3 passed in 0.12s' }], answer: 'Fixed. `total()` now adds each item once, and the three tests pass.\n\nThe change is one line in `src/cart.py`.' },
    { missionId: 'mission_polish_pay', owner: 'tm_atlas', prompt: 'Compare the three payment providers we could use for checkout.', tools: [], answer: 'Stripe is the quickest to wire in; Adyen costs less at volume; Square fits if the shop sells in person too.', at: '2026-10-31T17:20:00.000Z' },
    { missionId: 'mission_polish_readme', owner: 'tm_sable', prompt: 'Write the README section on running the tests.', tools: [], answer: 'Added a Testing section.', at: '2026-10-31T15:05:00.000Z' },
    { missionId: 'mission_polish_tax', owner: 'tm_wren', prompt: 'Why does the tax test fail on Windows only?', tools: [], answer: 'The fixture file has CRLF line endings.', at: '2026-10-30T16:10:00.000Z' }
  ]
  for (const [index, turn] of turns.entries()) {
    const runId = `run_benchpair_${String(index)}`
    const at = turn.at ?? `2026-11-01T1${String(index)}:00:00.000Z`
    await ledger.createMission({
      missionId: turn.missionId, runId, prompt: turn.prompt, runtime: 'opencode', model: FREE_ROUTE.model,
      requestedRouteId: 'opencode', resolvedRouteId: 'opencode-account:default', cliVersion: null, workspaceId,
      sandbox: 'workspace-write', executionPolicyVersion: 1, createdAt: at,
      ...(turn.follows === undefined ? {} : { continuesFrom: { missionId: turn.follows, checkpointEpoch: 1, reason: 'follow-up' } })
    })
    let sequence = 0
    const event = (type, payload) => ({ id: `event_benchpair_${String(index)}_${String(++sequence)}`, missionId: turn.missionId, runId, sequence, occurredAt: at, sourceAdapter: 'opencode', type, payload: { ...payload, evidence: { redacted: true } } })
    const events = []
    for (const [n, tool] of turn.tools.entries()) {
      const itemId = `tool_${String(index)}_${String(n)}`
      events.push(event('tool.started', { itemId, toolKind: 'command', name: 'bash', command: tool.command, title: tool.title, phase: 'started' }))
      events.push(event('tool.completed', { itemId, toolKind: 'command', name: 'bash', command: tool.command, title: tool.title, output: tool.output, exitCode: 0, phase: 'completed' }))
    }
    events.push(event('message.delta', { itemId: 'answer', operation: 'append', text: turn.answer, final: true }))
    events.push(event('run.completed', { process: { exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: events.length + 1, inputDeliveryFailed: false, outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at, finishedAt: at } }))
    await ledger.appendEvents(turn.missionId, events)
  }
  await ledger.flush()
  return { profile, owners: Object.fromEntries(turns.map((turn) => [turn.missionId, turn.owner])) }
}

/** Every live process under `root`, with resident memory (KiB) and CPU seconds used so far. */
function tree(root) {
  const rows = execFileSync('/bin/ps', ['-axo', 'pid=,ppid=,rss=,time=']).toString().trim().split('\n').map((line) => {
    const [pid, ppid, rss, time] = line.trim().split(/\s+/)
    const parts = time.split(':').map(Number)
    const seconds = parts.length === 3 ? parts[0] * 3600 + parts[1] * 60 + parts[2] : parts[0] * 60 + parts[1]
    return { pid: Number(pid), ppid: Number(ppid), rss: Number(rss), cpu: seconds }
  })
  const keep = new Set([root])
  let grew = true
  while (grew) {
    grew = false
    for (const row of rows) if (!keep.has(row.pid) && keep.has(row.ppid)) { keep.add(row.pid); grew = true }
  }
  const mine = rows.filter((row) => keep.has(row.pid) && row.rss > 0)
  return { rssKiB: mine.reduce((sum, row) => sum + row.rss, 0), cpuSeconds: mine.reduce((sum, row) => sum + row.cpu, 0), processes: mine.length }
}

/** One sample a second for `seconds`: median and peak memory, and CPU over the whole interval. */
async function measure(root, seconds) {
  const first = tree(root)
  const started = Date.now()
  const rss = []
  for (let i = 0; i < seconds; i += 1) {
    await sleep(1000)
    rss.push(tree(root).rssKiB)
  }
  const last = tree(root)
  const wall = (Date.now() - started) / 1000
  rss.sort((a, b) => a - b)
  return {
    medianMiB: Math.round(rss[Math.floor(rss.length / 2)] / 1024),
    peakMiB: Math.round(rss[rss.length - 1] / 1024),
    cpuPercent: Number((((last.cpuSeconds - first.cpuSeconds) / wall) * 100).toFixed(2)),
    processes: last.processes
  }
}

// ---- Electron -------------------------------------------------------------------------------------------------------
const workspace = await scratchRepository('locust-benchpair-ws-')
const seeded = await seedConversation(workspace)
const at = '2026-10-20T09:00:00.000Z'
// The built Electron from this checkout by default. The installed app (LOCUST_PACKAGED=<exe>) opens a fresh profile on
// its big-update splash, which covers the conversation, so the pair would not be showing the same thing.
const PACKAGED = process.env.LOCUST_PACKAGED
const drive = await startDrive({
  name: 'bench-pair', port: 9471, workspace, outPath: join(OUT, 'drive'), sendsNothing: true, keep: true, ...(PACKAGED === undefined ? {} : { packaged: PACKAGED }),
  profilePath: seeded.profile,
  seed: {
    schemaVersion: 1,
    teammates: [
      { teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: at, route: FREE_ROUTE },
      { teammateId: 'tm_atlas', name: 'Atlas', hue: 'blue', role: 'Research & Briefs', createdAt: at, route: FREE_ROUTE },
      { teammateId: 'tm_sable', name: 'Sable', hue: 'clay', role: 'Docs & QA', createdAt: at, route: FREE_ROUTE }
    ],
    missionOwners: seeded.owners,
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false }
  }
})
say(await drive.ready())
// Opened while the window is wide (at 1100 points Electron folds its sidebar to the rail, where the drive's step finds
// no name), then made the measured size; the conversation stays open.
await drive.evaluate(openTeammateScript('Wren'))
await sleep(3000)
await drive.resize(SIZE[0], SIZE[1])
await sleep(WARM * 1000)
const electronShown = await drive.evaluate(`document.body.innerText.includes('total()')`)
say(`Electron shows the conversation: ${String(electronShown)}`)
await drive.capture('electron-measured', async () => 'the window as measured')
// In front (the drive tells the page it is focused, so its faces move), then behind other windows (the window's own
// blur, as the app hears it when the person clicks elsewhere). The Rust bench window is behind other windows too, so
// the behind figures are the pair to compare.
const electronFront = await measure(drive.pid, Math.round(SECONDS / 2))
say(`Electron in front: ${JSON.stringify(electronFront)}`)
await drive.evaluate(`window.dispatchEvent(new Event('blur')), true`)
await sleep(3000)
const electron = await measure(drive.pid, SECONDS)
say(`Electron behind: ${JSON.stringify(electron)}`)
await drive.finish({ intro: 'Bench pair: Electron with the conversation open, measured at rest beside the Rust app.' })

// ---- Rust -----------------------------------------------------------------------------------------------------------
const home = await mkdtemp(join(tmpdir(), 'locust-benchpair-home-'))
const electronData = join(home, 'Library/Application Support/@teammate/desktop')
await mkdir(dirname(electronData), { recursive: true })
await cp(seeded.profile, electronData, { recursive: true })
const frame = join(OUT, 'native-frame.json')
const child = spawn(BINARY, ['--bench', frame, '--bench-seconds', String(WARM + SECONDS + 5), '--bench-conversation', CONVERSATION], {
  env: { ...process.env, HOME: home, LOCUST_NATIVE_DATA: join(home, 'native') },
  stdio: 'ignore'
})
await sleep(WARM * 1000)
const native = await measure(child.pid, SECONDS)
say(`Rust: ${JSON.stringify(native)}`)
await new Promise((done) => child.once('exit', done))
const firstFrame = JSON.parse(await readFile(frame, 'utf8')).first_frame_ms
// The Rust bench's proof that it showed the conversation (written once it opened, with how many items it showed).
const rustOpened = await readFile(join(OUT, 'native-frame.opened'), 'utf8').then(JSON.parse).catch(() => null)
say(`Rust shows the conversation: ${JSON.stringify(rustOpened)}`)

const electronApp = '/Applications/Locust.app'
const du = (path) => Number(execFileSync('/usr/bin/du', ['-sk', path]).toString().split(/\s+/)[0])
const result = {
  date: new Date().toISOString(),
  conversation: CONVERSATION,
  size: SIZE,
  idleSeconds: SECONDS,
  electron: { behind: electron, inFront: electronFront, appMiB: Math.round(du(electronApp) / 1024), showsConversation: electronShown, packaged: PACKAGED },
  rust: { ...native, firstFrameMs: Math.round(firstFrame), binaryMiB: Math.round(statSync(BINARY).size / 1048576), showsConversation: rustOpened }
}
await writeFile(join(OUT, 'result.json'), JSON.stringify(result, null, 2))
say(JSON.stringify(result, null, 2))
// The throwaway profile and HOME are scratch: gone once measured.
const { rm } = await import('node:fs/promises')
await rm(seeded.profile, { recursive: true, force: true })
await rm(home, { recursive: true, force: true })
await rm(workspace, { recursive: true, force: true })
