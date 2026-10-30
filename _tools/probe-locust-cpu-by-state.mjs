// HOW MUCH CPU LOCUST ITSELF TAKES, BY SCREEN AND STATE (2026-10-04).
//
//   node _tools/probe-locust-cpu-by-state.mjs --packaged <exe>
//
// Colin's Task Manager, while Sonnet's executor ran: Locust (11) at 32-41 % of a 12-core
// machine, its GPU process ~27 % and renderer ~15 % by a 5-second sample -- for an app showing
// one run. Is that the app, and in which state? This measures this instance's own processes
// (matched by its profile folder) over 5 s in four states -- Home idle, Home while a teammate
// works, the conversation while it streams, both idle after -- with the renderer's own counts
// (CDP Performance: style recalcs, layouts, script and task time) and the animations running.
// Free model by default. --buddy dresses the working teammate as Codex Buddy. --codex uses the subscription's actual streaming
// transport (gpt-6.1-sol, low effort; --model overrides it) and requires LOCUST_SPEND=1.

import { execFileSync } from 'node:child_process'
import { existsSync, readFileSync } from 'node:fs'
import { copyFile, mkdir, mkdtemp, readdir, readFile, writeFile } from 'node:fs/promises'
import { cpus, tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

import { FREE_ROUTE, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const terminalOnly = process.argv.includes('--terminal-only')
const packaged = arg('--packaged')
// A short quiet slot can still measure the streaming budget. Same launch,
// route, prompt and five-second sample; skip the unrelated Home rest waits.
const streamingOnly = process.argv.includes('--streaming-only')
const codex = process.argv.includes('--codex')
if (codex && process.env.LOCUST_SPEND !== '1') throw new Error('--codex requires LOCUST_SPEND=1')
/*
 * --fake-claude (0.637): the reply streams from _tools/fake-claude, a stand-in
 * `claude` that asks no model and spends nothing -- OpenCode's free models
 * cannot stream (each message arrives whole), so without it the streaming
 * state could only be measured on a paid route. The app is handed a PATH with
 * the stand-in's folder first and no folder that holds a real `claude` (the
 * locator takes any `.exe` on PATH before any `.cmd`); its route is Claude
 * Code's, so the app's free-only guard has to be lifted (LOCUST_SPEND=1), and
 * nothing is sent until the app reports the stand-in's own version.
 */
const fakeClaude = process.argv.includes('--fake-claude')
const longTurn = process.argv.includes('--long-turn')
if (longTurn && !fakeClaude) throw new Error('--long-turn requires the no-spend stand-in --fake-claude')
if (fakeClaude && process.env.LOCUST_SPEND !== '1') throw new Error('--fake-claude lifts the free-only guard, so it requires LOCUST_SPEND=1; it spends nothing, and checks that before sending')
const FAKE_DIR = join(dirname(fileURLToPath(import.meta.url)), 'fake-claude')
const FAKE_VERSION = '0.0.0-locust-fake'
const route = codex
  ? { runtime: 'codex', model: arg('--model') ?? 'gpt-6.1-sol', effort: 'low', mode: 'ask' }
  : fakeClaude ? { runtime: 'claude', model: 'claude-haiku-4-5', mode: 'ask' } : FREE_ROUTE
const fakeEnv = fakeClaude
  ? { PATH: [FAKE_DIR, ...(process.env.PATH ?? '').split(';').filter((dir) => dir !== '' && !existsSync(join(dir, 'claude.exe')) && !existsSync(join(dir, 'claude.cmd')))].join(';') }
  : {}
/*
 * --buddy (2026-10-05): Wren, the teammate who works, wears Codex Buddy with
 * his screen (his own rig since 2026-10-05), so his cost at rest and at work
 * is in the sample. His sheet is his maker's and never in this repository: it
 * is copied from this computer's Locust (%APPDATA%\@teammate\desktop\pets,
 * or --sheet <folder>) into the probe's profile, with a scratch CODEX_HOME so
 * the real ~/.codex/pets is never read.
 */
const buddy = process.argv.includes('--buddy')
const port = Number(arg('--port') ?? 9877)
// --seconds <n>: a longer sample (2026-10-07). Five seconds on the same build read 7.5% and 11.3%; an
// experiment worth 1% needs longer. Pair it with --long-turn, so the reply is still streaming at the end.
const SAMPLE_SECONDS = Number(arg('--seconds') ?? 5)
const restMs = 46_000 // Sample after the 45-second rest deadline, finishing within 60 seconds.
/*
 * OFF WINDOWS (the first Mac run, 2026-10-30): this probe read every process through PowerShell, so the CPU budget
 * could not be probed on a Mac at all. There `ps` lists the processes, and the CPU each used is read twice around the
 * same sleep: `ps -o time=` on a Mac (to a hundredth of a second), /proc/<pid>/stat on Linux. Windows is unchanged.
 */
const POSIX = process.platform !== 'win32'
/** Every process with its parent and command line: BSD's spelling on a Mac, POSIX's on Linux. */
const processTable = () =>
  execFileSync('ps', process.platform === 'darwin' ? ['-axo', 'pid=,ppid=,command='] : ['-eo', 'pid=,ppid=,args='], { encoding: 'utf8' })
    .split('\n')
    .map((line) => /^\s*(\d+)\s+(\d+)\s+(.*)$/.exec(line))
    .filter((match) => match !== null)
    .map((match) => ({ pid: Number(match[1]), ppid: Number(match[2]), command: match[3] }))
/** Seconds of CPU a process has used so far ([dd-][hh:]mm:ss.ss on a Mac); undefined once it is gone. */
const cpuSeconds = (pid) => {
  try {
    if (process.platform === 'linux') {
      const stat = readFileSync(`/proc/${String(pid)}/stat`, 'utf8')
      const fields = stat.slice(stat.lastIndexOf(')') + 2).split(' ')
      return (Number(fields[11]) + Number(fields[12])) / 100
    }
    const text = execFileSync('ps', ['-o', 'time=', '-p', String(pid)], { encoding: 'utf8' }).trim()
    const [days, clock] = text.includes('-') ? text.split('-') : ['0', text]
    return Number(days) * 86_400 + clock.split(':').reduce((sum, part) => sum * 60 + Number(part), 0)
  } catch {
    return undefined
  }
}
const sleepSync = (ms) => Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms)
// An arena or another suite starting mid-probe invalidates the comparison too.
const assertQuiet = () => {
  if (POSIX) {
    const busy = processTable().filter((row) => row.pid !== process.pid && /(^|\/)node$/.test(row.command.split(' ')[0] ?? '')
      && /vitest|electron-builder|_tools\/(drive|probe|look|capture)[-/]/.test(row.command))
    if (busy.length > 0) throw new Error(`Machine is not quiet: other test/build/UI-drive node processes ${busy.map((row) => row.pid).join(', ')}. Nothing here is a valid CPU comparison.`)
    return
  }
  const script = `$rows = @(Get-CimInstance Win32_Process -Filter "Name='node.exe'" | Where-Object { $_.ProcessId -ne ${String(process.pid)} -and ($_.CommandLine -match 'vitest|electron-builder' -or ([string]$_.CommandLine).Replace([char]92, [char]47) -match '_tools/(drive|probe|look|capture)[-/]') }); @($rows | ForEach-Object { [int]$_.ProcessId }) | ConvertTo-Json -Compress`
  const raw = execFileSync('powershell', ['-NoProfile', '-Command', script], { encoding: 'utf8', windowsHide: true }).trim()
  const pids = raw === '' ? [] : [JSON.parse(raw)].flat()
  if (pids.length > 0) throw new Error(`Machine is not quiet: other test/build/UI-drive node processes ${pids.join(', ')}. Nothing here is a valid CPU comparison.`)
}
assertQuiet()
const OUT = join(recordRoot('locust-cpu-by-state'), new Date().toISOString().replace(/[:.]/g, '-'))
await mkdir(OUT, { recursive: true })
const workspace = await scratchRepository('locust-cpu-ws-')
const at = '2026-09-10T09:00:00.000Z'
const wearingBuddy = { headwear: 1, accessory: 0, mouth: 0, bot: { shape: 'droid', face: 'eyes' }, pet: { source: 'gallery', id: 'codex-buddy' } }
const team = [
  { teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', createdAt: at, route: { ...route, mode: 'ask' }, ...(buddy ? { avatar: wearingBuddy } : {}) },
  { teammateId: 'tm_atlas', name: 'Atlas', hue: 'blue', role: 'Research & Briefs', createdAt: at, route: FREE_ROUTE },
  { teammateId: 'tm_juno', name: 'Juno', hue: 'violet', role: 'Docs & QA', createdAt: at, route: FREE_ROUTE }
]
const buddyProfile = buddy ? await mkdtemp(join(tmpdir(), 'locust-cpu-buddy-')) : undefined
const buddyEnv = {}
if (buddyProfile !== undefined) {
  const sheet = arg('--sheet') ?? join(process.env.APPDATA ?? '', '@teammate', 'desktop', 'pets', 'codex-buddy')
  await mkdir(join(buddyProfile, 'pets', 'codex-buddy'), { recursive: true })
  for (const file of ['spritesheet.webp', 'pet.json']) await copyFile(join(sheet, file), join(buddyProfile, 'pets', 'codex-buddy', file))
  buddyEnv.CODEX_HOME = await mkdtemp(join(tmpdir(), 'locust-cpu-buddy-codex-'))
}
const drive = await startDrive({
  sendsNothing: terminalOnly,
  focused: terminalOnly,
  name: 'cpu-by-state', port, workspace, outPath: OUT, spends: codex || fakeClaude, env: { ...fakeEnv, ...buddyEnv },
  ...(buddyProfile === undefined ? {} : { profilePath: buddyProfile }),
  ...(packaged === undefined ? {} : { packaged }),
  seed: { schemaVersion: 1, teammates: team, missionOwners: {}, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false } }
})
say(`CPU probe requested route: ${JSON.stringify(route)}`)

/** Off Windows: the same measure from `ps` and the CPU time each process used across the same sleep. */
const cpuPosix = (seconds) => {
  const table = processTable()
  let mine = table.filter((row) => row.command.includes(drive.profile))
  if (terminalOnly) {
    const ids = new Set([drive.pid])
    for (let added = true; added;) {
      added = false
      for (const row of table) if (ids.has(row.ppid) && !ids.has(row.pid)) { ids.add(row.pid); added = true }
    }
    mine = table.filter((row) => ids.has(row.pid))
  }
  const before = new Map(mine.map((row) => [row.pid, cpuSeconds(row.pid)]))
  sleepSync(seconds * 1000)
  const cores = cpus().length
  return mine.flatMap((row) => {
    const start = before.get(row.pid)
    const end = cpuSeconds(row.pid)
    if (start === undefined || end === undefined) return []
    return [{ type: /--type=([a-z-]+)/.exec(row.command)?.[1] ?? 'main', pct: Math.round(((end - start) / seconds) * 100 / cores * 10) / 10 }]
  })
}

/** This instance's processes' CPU over `seconds`, as a share of the whole machine (Task Manager's measure). */
const cpu = (seconds) => {
  if (POSIX) {
    const rows = cpuPosix(seconds)
    if (rows.length === 0) throw new Error('No Locust processes matched this profile; CPU was not measured')
    const by = {}
    for (const row of rows) by[row.type] = Math.round(((by[row.type] ?? 0) + row.pct) * 10) / 10
    return { total: Math.round(rows.reduce((sum, row) => sum + row.pct, 0) * 10) / 10, by }
  }
  const key = drive.profile.replace(/'/g, "''")
  const script = `
$ps = @(Get-CimInstance Win32_Process -Filter "Name='Locust.exe' OR Name='electron.exe'" | Where-Object { $_.CommandLine -like ('*' + '${key}' + '*') })
${terminalOnly ? `$all = @(Get-CimInstance Win32_Process); $ids = @{}; $ids[${String(drive.pid)}] = $true; do { $added = $false; foreach ($p in $all) { if ($ids.ContainsKey([int]$p.ParentProcessId) -and !$ids.ContainsKey([int]$p.ProcessId)) { $ids[[int]$p.ProcessId] = $true; $added = $true } } } while ($added); $ps = @($all | Where-Object { $ids.ContainsKey([int]$_.ProcessId) })` : ''}
$a = @{}; foreach ($p in $ps) { $g = Get-Process -Id $p.ProcessId -ErrorAction SilentlyContinue; if ($g) { $a[[int]$p.ProcessId] = $g.TotalProcessorTime.TotalMilliseconds } }
Start-Sleep -Seconds ${String(seconds)}
$cores = [Environment]::ProcessorCount
$out = foreach ($p in $ps) { $g = Get-Process -Id $p.ProcessId -ErrorAction SilentlyContinue; if ($g -and $a.ContainsKey([int]$p.ProcessId)) { $type = 'main'; if ($p.CommandLine -match '--type=([a-z-]+)') { $type = $Matches[1] }; [pscustomobject]@{ type = $type; pct = [Math]::Round((($g.TotalProcessorTime.TotalMilliseconds - $a[[int]$p.ProcessId]) / (${String(seconds)} * 1000)) * 100 / $cores, 1) } } }
@($out) | ConvertTo-Json -Compress`
  const raw = execFileSync('powershell', ['-NoProfile', '-Command', script], { encoding: 'utf8', windowsHide: true }).trim()
  const rows = raw === '' ? [] : [JSON.parse(raw)].flat()
  const by = {}
  for (const row of rows) by[row.type] = Math.round(((by[row.type] ?? 0) + row.pct) * 10) / 10
  if (rows.length === 0) throw new Error('No Locust processes matched this profile; CPU was not measured')
  return { total: Math.round(rows.reduce((sum, row) => sum + row.pct, 0) * 10) / 10, by }
}
const metrics = async () => {
  const answer = await drive.send('Performance.getMetrics', {})
  return Object.fromEntries((answer?.result?.metrics ?? []).map((m) => [m.name, m.value]))
}
const results = []
const showOpening = async () => {
  // Stop its animated follow before framing the opening. The first scroll
  // consumes any outstanding "our own scroll" flag; the second moves up.
  await drive.evaluate(`(() => {
    const thread = document.querySelector('.lc-thread')
    thread.scrollTop = Math.max(0, thread.scrollTop - 2)
    thread.dispatchEvent(new Event('scroll', { bubbles: true }))
    thread.scrollTop = 0
    thread.dispatchEvent(new Event('scroll', { bubbles: true }))
  })()`)
  for (let i = 0; i < 3; i += 1) {
    await drive.send('Input.dispatchMouseEvent', { type: 'mouseWheel', x: 700, y: 250, deltaX: 0, deltaY: -10000 })
    await sleep(150)
  }
}
const eventCounts = async () => {
  const counts = { events: 0, calls: 0 }
  for (const name of (await readdir(join(drive.profile, 'mission-ledger'))).filter((name) => name.endsWith('.jsonl'))) {
    for (const line of (await readFile(join(drive.profile, 'mission-ledger', name), 'utf8')).split('\n')) {
      try {
        const event = JSON.parse(line).event
        if (!event) continue
        counts.events += 1
        if (event.type === 'tool.completed') counts.calls += 1
      } catch { /* an append in progress is read again */ }
    }
  }
  return counts
}
const streamedFragments = async () => {
  let count = 0
  const directory = join(drive.profile, 'mission-ledger')
  for (const name of (await readdir(directory)).filter((name) => name.endsWith('.jsonl'))) {
    const lines = (await readFile(join(directory, name), 'utf8')).split('\n')
    for (const line of lines) {
      try {
        const record = JSON.parse(line)
        if (record.event?.type === 'message.delta' && record.event.payload.operation === 'append'
          && record.event.payload.final === false && record.event.payload.text.length > 0) count += 1
      } catch { /* an append in progress is read again after the sample */ }
    }
  }
  return count
}
const measure = async (state) => {
  assertQuiet()
  const streaming = state === 'the conversation, streaming'
  const fragmentsBefore = streaming ? await streamedFragments() : undefined
  const visibleCharacters = () => drive.evaluate(`[...document.querySelectorAll('.lc-agentline__body')].reduce((sum, element) => sum + element.innerText.length, 0)`)
  const visibleBefore = streaming ? await visibleCharacters() : undefined
  const canvasFrames = () => drive.evaluate(`JSON.stringify([...document.querySelectorAll('.lc-cover canvas')].map((canvas) => canvas.toDataURL()))`)
  const drawing = await canvasFrames()
  const running = await drive.evaluate(`Boolean(document.querySelector('button[aria-label^="Stop the running"]')) || [...document.querySelectorAll('.lc-hometeam__state')].some((s) => /working|thinking|replying/i.test(s.innerText))`)
  const animations = JSON.parse(String(await drive.evaluate(`JSON.stringify(document.getAnimations().filter((a) => a.playState === 'running').map((a) => (a.animationName ?? a.constructor.name) + ' @ ' + String(a.effect?.target?.className?.baseVal ?? a.effect?.target?.className ?? a.effect?.target?.tagName ?? '').slice(0, 50)))`)))
  const before = await metrics()
  if (longTurn && streaming) await drive.evaluate(`(() => {
    window.locustProbeFrames = { active: true, gaps: [], last: performance.now() }
    const tick = (now) => { const frames = window.locustProbeFrames; if (!frames.active) return; frames.gaps.push(now - frames.last); frames.last = now; requestAnimationFrame(tick) }
    requestAnimationFrame(tick)
  })()`)
  // --draws: which canvases drew during the sample, by where they sit (each draw begins with a clearRect).
  if (arg('--draws') !== undefined || process.argv.includes('--draws')) await drive.evaluate(`(() => {
    const where = (c) => { for (let n = c; n; n = n.parentElement) { const k = String(n.className?.baseVal ?? n.className ?? '').split(' ').find((x) => /^lc-(cover|hometeam|faces|sidebar|thread|livestep|rail|workroom|agentline|activity|composer|plancard)/.test(x)); if (k) return k } return c.isConnected ? 'other' : 'offscreen' }
    const tally = {}; const proto = CanvasRenderingContext2D.prototype; const original = proto.clearRect
    window.locustProbeDraws = { tally, restore: () => { proto.clearRect = original } }
    // Each canvas counted on its own (#n, its size after), so one canvas clearing twice a draw reads apart from two canvases.
    const ids = new WeakMap(); let next = 0
    proto.clearRect = function (...a) { if (!ids.has(this.canvas)) ids.set(this.canvas, next++); const k = where(this.canvas) + '#' + ids.get(this.canvas) + ' ' + this.canvas.width + 'x' + this.canvas.height + (this.canvas.isConnected ? '' : ' detached'); tally[k] = (tally[k] ?? 0) + 1; return original.apply(this, a) }
  })()`)
  const share = cpu(SAMPLE_SECONDS)
  const draws = process.argv.includes('--draws') ? JSON.parse(String(await drive.evaluate(`(() => { const d = window.locustProbeDraws; d.restore(); return JSON.stringify(Object.fromEntries(Object.entries(d.tally).map(([k, v]) => [k, Math.round(v / ${String(SAMPLE_SECONDS)})]))) })()`))) : undefined
  const after = await metrics()
  const frames = longTurn && streaming ? JSON.parse(String(await drive.evaluate(`(() => {
    const frames = window.locustProbeFrames; frames.active = false
    const gaps = frames.gaps.slice(1).sort((a, b) => a - b)
    return JSON.stringify({ count: gaps.length, medianMs: gaps[Math.floor(gaps.length / 2)], p95Ms: gaps[Math.floor(gaps.length * 0.95)], maxMs: gaps.at(-1) })
  })()`))) : undefined
  assertQuiet()
  const nextDrawing = await canvasFrames()
  const frozenDrawing = drawing === '[]' ? null : drawing === nextDrawing
  const focused = await drive.evaluate(`document.hasFocus() && !document.hidden`)
  const stillRunning = await drive.evaluate(`Boolean(document.querySelector('button[aria-label^="Stop the running"]')) || [...document.querySelectorAll('.lc-hometeam__state')].some((s) => /working|thinking|replying/i.test(s.innerText))`)
  const fragmentsAfter = streaming ? await streamedFragments() : undefined
  const visibleAfter = streaming ? await visibleCharacters() : undefined
  const expectedRunning = /working|streaming/.test(state)
  const valid = focused === true && running === expectedRunning && stillRunning === expectedRunning
    && (!streaming || (fragmentsBefore > 0 && fragmentsAfter > fragmentsBefore && visibleBefore > 0 && visibleAfter > visibleBefore))
  const per = (name) => Math.round(((after[name] ?? 0) - (before[name] ?? 0)) / 5 * 10) / 10
  const row = {
    state, running, stillRunning, focused, valid, frozenDrawing, cpu: share,
    ...(frames === undefined ? {} : { frames }),
    ...(streaming ? { fragmentsBefore, fragmentsAfter, visibleBefore, visibleAfter } : {}),
    perSecond: { styleRecalcs: per('RecalcStyleCount'), layouts: per('LayoutCount'), scriptMs: Math.round(per('ScriptDuration') * 1000), taskMs: Math.round(per('TaskDuration') * 1000) },
    animations: animations.length, animationNames: [...new Set(animations)].slice(0, 12),
    ...(draws === undefined ? {} : { drawsPerSecond: draws })
  }
  results.push(row)
  say(`  ${state}: ${JSON.stringify(row)}`)
  if (!valid) throw new Error(`${state} changed during the sample; this is not a valid measurement of that state`)
}
const cards = async () => JSON.parse(String(await drive.evaluate(`JSON.stringify([...document.querySelectorAll('.lc-hometeam__card')].map((card) => ({ name: card.querySelector('.lc-hometeam__name')?.firstChild?.textContent?.trim() ?? '', word: card.querySelector('.lc-hometeam__state')?.innerText.trim() ?? null })))`)))
try {
  await drive.ready()
  if (terminalOnly) {
    const guardIdle = async (withTerminal) => {
      assertQuiet()
      if (await drive.evaluate(`document.hasFocus() && !document.hidden`) !== true) throw new Error('The CPU sample needs a focused, visible window')
      if (withTerminal && await drive.evaluate(`(() => { const text = document.querySelector('.lc-terminal:not([hidden]) .xterm-accessibility-tree')?.innerText ?? ''; return text.trim().length > 0 && !text.includes('[Shell exited:') })()`) !== true) throw new Error('The CPU sample needs an open, live terminal')
    }
    await drive.resize(1200, 720)
    await drive.send('Page.bringToFront', {})
    await sleep(restMs)
    await guardIdle(false)
    const before = cpu(SAMPLE_SECONDS)
    await guardIdle(false)
    await drive.evaluate(`document.querySelector('button[aria-label="Terminal"]').click()`)
    await drive.waitFor(`Boolean(document.querySelector('.lc-terminal__screen .xterm-helper-textarea'))`, { timeoutMs: 20000, what: 'idle terminal' })
    await sleep(restMs)
    await guardIdle(true)
    const after = cpu(SAMPLE_SECONDS)
    await guardIdle(true)
    results.push({ state: 'Home at rest, no terminal', ...before }, { state: 'Home at rest, terminal open', ...after })
    await drive.capture('idle terminal CPU', async () => JSON.stringify(results))
    if (before.total >= 1 || after.total >= 1) throw new Error('Home or idle terminal exceeded 1% CPU')
    console.log(JSON.stringify(results, null, 2))
    // Runs no model turns; the finally below records and ends this instance.
  } else {
  await drive.resize(longTurn ? 1200 : 1440, longTurn ? 720 : 900)
  await drive.send('Page.bringToFront', {})
  await drive.waitFor(`document.hasFocus() && !document.hidden`, { timeoutMs: 15_000, what: 'probe window in front' })
  await drive.send('Performance.enable', {})
  for (let i = 0; i < 60; i += 1) {
    if (/^\d+ ready$/.test(String(await drive.evaluate(`document.querySelector('.lc-agenthead__note')?.innerText.trim() ?? ''`)))) break
    await sleep(500)
  }
  await drive.evaluate(`window.dispatchEvent(new Event('keydown'))`)
  await sleep(3000)
  if (!streamingOnly) {
    await measure('home, idle before rest')
    await drive.evaluate(`window.dispatchEvent(new Event('keydown'))`)
    await sleep(restMs)
    await measure('home, idle at rest')
  }
  // --css <rules>: an experiment's style sheet, added before the send (what costs what, by turning it off).
  const extraCss = arg('--css')
  if (extraCss !== undefined) {
    await drive.evaluate(`(() => { const style = document.createElement('style'); style.dataset.probe = 'css'; style.textContent = ${JSON.stringify(extraCss)}; document.head.appendChild(style); return 'added' })()`)
    say(`  experiment css: ${extraCss}`)
  }
  // --js <script>: an experiment run in the page before the send, for a cost no style sheet can reach
  // (2026-10-07: the faces' canvas glow, by setting shadowBlur to nothing).
  const extraJs = arg('--js')
  if (extraJs !== undefined) {
    await drive.evaluate(`(() => { ${extraJs}; return 'ran' })()`)
    say(`  experiment js: ${extraJs}`)
  }
  if (fakeClaude) {
    // The stand-in or nothing: the version the app's own discovery read for Claude Code.
    const seen = String(await drive.evaluate(`window.desktop.discoveryLog().then((log) => JSON.stringify(log.filter((e) => e.kind === 'probe.finished' && e.id === 'claude').map((e) => e.version ?? null)))`))
    say(`  Claude Code as the app found it: ${seen}`)
    if (!seen.includes(FAKE_VERSION)) throw new Error(`the app did not find the stand-in claude (it read ${seen}); nothing was sent`)
  }
  const sent = String(await drive.evaluate(`(async () => {
    const card = [...document.querySelectorAll('.lc-hometeam__card')].find((c) => c.getAttribute('aria-label')?.startsWith('Message Wren'))
    if (!card) return 'no Wren card'
    card.click()
    await new Promise((r) => setTimeout(r, 800))
    const field = document.querySelector('form.command-dock textarea')
    if (!field) return 'no composer'
    Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set.call(field, ${JSON.stringify(longTurn ? '[locust-fake-long-turn] Simulate 600 Read calls with progress messages, then stream the story. Do not read or change files.' : 'Reply in chat with about 1800 words of a fictional story about a swarm of locusts travelling across a field. Use plain paragraphs with no headings. Start the story immediately. Do not use tools, research, a plan, a preface, or explanations. Do not create or edit any file.')})
    field.dispatchEvent(new Event('input', { bubbles: true }))
    for (let i = 0; i < 120; i += 1) {
      await new Promise((r) => setTimeout(r, 250))
      const button = document.querySelector('button[aria-label="Send"]')
      if (button && !button.disabled) { button.click(); return 'sent' }
    }
    return 'Send never enabled'
  })()`))
  say(`  ${sent}`)
  if (sent !== 'sent') throw new Error(sent)
  if (longTurn) {
    for (let i = 0; i < 400 && (await eventCounts()).calls < 110; i += 1) await sleep(100)
    await showOpening()
    await drive.capture('over 500 events, still running', async () => JSON.stringify(await eventCounts()))
    for (let i = 0; i < 400 && (await eventCounts()).calls < 600; i += 1) await sleep(100)
    const counts = await eventCounts()
    if (counts.events <= 3000 || counts.calls < 600) throw new Error(`Long turn was not reached: ${JSON.stringify(counts)}`)
    await showOpening()
    await drive.capture('past 3000 events, still running', async () => JSON.stringify(counts))
    await drive.evaluate(`document.querySelector('button[aria-label="Go to the newest message"]')?.click()`)
    say(`  long-turn counts: ${JSON.stringify(counts)}`)
  }
  if (!streamingOnly) {
    await sleep(1500)
    await drive.evaluate(`document.querySelector('.lc-brand__lockup')?.click()`)
    for (let i = 0; i < 60; i += 1) {
      await sleep(500)
      if ((await cards()).some((card) => /working|thinking|replying/i.test(card.word ?? ''))) break
    }
    await measure('home, a teammate working')
    await drive.evaluate(`[...document.querySelectorAll('.lc-hometeam__card')].find((c) => c.getAttribute('aria-label')?.startsWith('Message Wren'))?.click()`)
  }
  await sleep(1200)
  // A running process may still be thinking. Sample actual text arriving.
  // Skip a short preface that finishes before the real answer begins.
  const minimum = codex ? 100 : 1
  for (let i = 0; i < 300 && await streamedFragments() < minimum; i += 1) {
    if (i % 25 === 0) assertQuiet()
    await sleep(200)
  }
  await measure('the conversation, streaming')
  if (longTurn) await drive.capture('after the streaming CPU sample', async () => JSON.stringify(await eventCounts()))
  if (streamingOnly) {
    say('  streaming-only: Home and idle-after states were not measured')
  } else {
    for (let i = 0; i < 240; i += 1) {
      if (await drive.evaluate(`Boolean(document.querySelector('button[aria-label^="Stop the running"]'))`) !== true) break
      await sleep(1000)
    }
    await sleep(3000)
    await measure('the conversation, idle after')
    await drive.evaluate(`document.querySelector('.lc-brand__lockup')?.click()`)
    await sleep(restMs)
    await measure('home, idle after')
  }
  }
} catch (error) {
  say(`  probe failed: ${error instanceof Error ? error.message : String(error)}`)
  process.exitCode = 1
} finally {
  await writeFile(join(OUT, 'cpu-by-state.json'), JSON.stringify(results, null, 2))
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. Locust's own CPU by state.`, extra: JSON.stringify(results, null, 2) })
}
