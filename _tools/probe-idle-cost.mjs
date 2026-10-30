// What does Locust cost the machine, screen by screen? Measured on the built app.
//
//   node _tools/probe-idle-cost.mjs [--packaged <exe>] [--tag <name>] [--seconds <n>]
//
// A beta tester, 2026-09-23 (passed on by Colin): "Lowkey my computer feels
// noticeably slower while running locust". Colin's own installed copy, open
// for hours and sitting behind other windows, measured ~0% CPU -- so the cost
// is in what a person is LOOKING at, or doing. This holds each state still
// for a while and reads, for every process the app started (main, renderer,
// GPU, utility): CPU time used, as a share of one core, and the GPU's own
// engines' busy time for those processes. And from inside the renderer, how
// much of each second went to script, style, layout and paint (CDP
// Performance metrics), and how many animation frames it drew.
//
// States: the home screen in front; the same, left alone for 35 s; the home
// screen behind another window;
// a teammate's empty conversation in front; Settings in front; the window
// minimised. Sends nothing.

import { execFile } from 'node:child_process'
import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import { localPath, say, scratchRepository, sleep, startDrive, teammateFace } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')
const SECONDS = Number(arg('--seconds') ?? '15')
const OUT = join(localPath(new URL('../docs/beta-fixes-2026-09-23/', import.meta.url)), `idle-cost-${tag}`)
await mkdir(OUT, { recursive: true })

const mate = (id, name, hue, shape) => ({
  teammateId: id,
  name,
  hue,
  role: 'Code & Migrations',
  avatar: { headwear: 0, accessory: 0, mouth: 0, bot: { shape, face: 'eyes' } },
  createdAt: '2026-09-05T05:00:00.000Z'
})
const workspace = await scratchRepository('locust-idle-cost-ws-')
const drive = await startDrive({
  name: `idle-cost-${tag}`,
  port: 9457,
  workspace,
  outPath: OUT,
  sendsNothing: true,
  ...(packaged === undefined ? {} : { packaged }),
  seed: {
    schemaVersion: 1,
    teammates: [mate('tm_wren', 'Wren', 'lime', 'droid'), mate('tm_pip', 'Pip', 'violet', 'ghost'), mate('tm_sable', 'Sable', 'teal', 'hopper')],
    missionOwners: {},
    settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off', autoMode: false }
  }
})

const powershell = (script) =>
  new Promise((resolve) => {
    execFile('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', script], { windowsHide: true, maxBuffer: 8 * 1024 * 1024 }, (error, stdout) => resolve(error === null ? stdout : ''))
  })

// Every process in the app's tree: its id, what it is, CPU seconds so far.
const TREE = (root) => `
$all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId, CommandLine
$ids = @(${String(root)}); $grew = $true
while ($grew) { $grew = $false; foreach ($p in $all) { if (($ids -contains $p.ParentProcessId) -and -not ($ids -contains $p.ProcessId)) { $ids += $p.ProcessId; $grew = $true } } }
$rows = foreach ($id in $ids) { $proc = Get-Process -Id $id -ErrorAction SilentlyContinue; if ($proc) { $cmd = ($all | Where-Object { $_.ProcessId -eq $id }).CommandLine; $type = if ($cmd -match '--type=(\\S+)') { $Matches[1] } else { 'main' }; [pscustomobject]@{ id = $id; type = $type; cpu = $proc.TotalProcessorTime.TotalSeconds } } }
$rows | ConvertTo-Json -Compress`
const GPU = (ids) => `
$c = Get-Counter '\\GPU Engine(*)\\Running Time' -ErrorAction SilentlyContinue
$want = @(${ids.join(',')})
$total = 0
foreach ($s in $c.CounterSamples) { if ($s.InstanceName -match 'pid_(\\d+)_') { if ($want -contains [int]$Matches[1]) { $total += $s.RawValue } } }
$total`

const sample = async () => {
  const rows = JSON.parse((await powershell(TREE(drive.pid))) || '[]')
  const list = Array.isArray(rows) ? rows : [rows]
  const gpu = Number((await powershell(GPU(list.map((row) => row.id)))).trim() || '0')
  const metrics = await drive.send('Performance.getMetrics', {})
  const named = Object.fromEntries((metrics?.result?.metrics ?? []).map((m) => [m.name, m.value]))
  return { at: Date.now(), rows: list, gpu, named }
}

const measure = async (label) => {
  await sleep(2500)
  const running = String(await drive.evaluate(`JSON.stringify(document.getAnimations().filter((a) => a.playState === 'running').map((a) => (a.animationName ?? a.constructor.name) + ' @ ' + String(a.effect?.target?.className ?? '').slice(0, 40)))`))
  const a = await sample()
  await sleep(SECONDS * 1000)
  const b = await sample()
  const wall = (b.at - a.at) / 1000
  const byType = {}
  for (const row of b.rows) {
    const before = a.rows.find((entry) => entry.id === row.id)
    if (before === undefined) continue
    byType[row.type] = (byType[row.type] ?? 0) + (row.cpu - before.cpu)
  }
  const cpu = Object.fromEntries(Object.entries(byType).map(([type, seconds]) => [type, Math.round((1000 * seconds) / wall) / 10]))
  const total = Math.round(Object.values(cpu).reduce((sum, value) => sum + value, 0) * 10) / 10
  // GPU engine running time is in 100 ns units.
  const gpu = Math.round((((b.gpu - a.gpu) / 1e7) / wall) * 1000) / 10
  const share = (name) => Math.round((1000 * ((b.named[name] ?? 0) - (a.named[name] ?? 0))) / wall) / 10
  const row = { state: label, cpuTotal: total, cpu, gpuPercentOfOneEngine: gpu, rendererBusy: { script: share('ScriptDuration'), style: share('RecalcStyleDuration'), layout: share('LayoutDuration'), task: share('TaskDuration') }, running: JSON.parse(running) }
  say(`${label}: CPU ${String(total)}% of one core ${JSON.stringify(cpu)}; GPU ${String(gpu)}%; renderer busy ${JSON.stringify(row.rendererBusy)} (% of each second)`)
  say(`    running: ${running}`)
  return row
}

const results = []
try {
  await drive.ready()
  await drive.resize(1215, 800)
  await drive.send('Performance.enable', {})
  // No frame counter: a rAF loop of the probe's own would keep the compositor
  // producing frames and be measured as the app's cost. What runs is listed
  // instead -- every running animation, and every canvas that is redrawn.
  await drive.waitFor(`document.querySelector('.lc-cover__face .lc-bot[data-bot="hopper"]')?.dataset.state !== 'still'`, { timeoutMs: 60_000, what: 'the cover on' })
  await sleep(4000)

  await drive.send('Emulation.setFocusEmulationEnabled', { enabled: true })
  await drive.evaluate(`window.dispatchEvent(new Event('focus'))`)
  results.push(await measure('home screen, in front'))

  // Left alone: nothing touches the window for longer than the cover waits
  // before it rests (HomeCover's REST_AFTER_MS, 30 s, from 0.305).
  await sleep(35_000)
  results.push(await measure('home screen, in front, left alone'))

  await drive.send('Emulation.setFocusEmulationEnabled', { enabled: false })
  await drive.evaluate(`window.dispatchEvent(new Event('blur'))`)
  results.push(await measure('home screen, behind another window'))

  await drive.send('Emulation.setFocusEmulationEnabled', { enabled: true })
  await drive.evaluate(`window.dispatchEvent(new Event('focus'))`)
  await drive.evaluate(`(async () => { ${teammateFace('Wren')}?.click(); await new Promise((r) => setTimeout(r, 1200)) })()`)
  results.push(await measure("a teammate's empty conversation, in front"))

  await drive.evaluate(`(async () => { [...document.querySelectorAll('button')].find((b) => (b.getAttribute('title') ?? '').startsWith('Settings'))?.click(); await new Promise((r) => setTimeout(r, 1200)) })()`)
  results.push(await measure('Settings, in front'))

  await drive.send('Browser.setWindowBounds', { windowId: (await drive.send('Browser.getWindowForTarget', {}))?.result?.windowId, bounds: { windowState: 'minimized' } }).catch(() => undefined)
  results.push(await measure('minimised'))

  await writeFile(join(OUT, 'cost.json'), JSON.stringify(results, null, 1))
} catch (error) {
  say(`probe failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: 'Each screen held still and the app measured: CPU per process, the GPU, and the renderer from inside. Sends nothing.' })
}
