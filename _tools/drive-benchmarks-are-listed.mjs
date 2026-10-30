// YOUR BENCHMARKS, IN ONE PLACE (0.764): every comparison run again, listed, a row opening each.
//
//   node _tools/drive-benchmarks-are-listed.mjs [--packaged <exe>] [--tag <name>]
//
// Sends nothing and spends nothing. Seeded: two benchmarks -- a comparison its judge scored and its runs again, each
// run's answers and scorecard recorded -- and a plain comparison never run again, which is no benchmark. The sidebar
// gains a Benchmarks place; it lists the two, newest first, each with its models, its runs, who leads and by how much;
// a row opens its comparison, whose Back goes to the list. Read at three window sizes.

import { createHash } from 'node:crypto'
import { mkdtemp, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')

const root = localPath(new URL('..', import.meta.url))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-benchlist-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-benchlist-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const ATLAS = { teammateId: 'tm_atlas0000000000000000', name: 'Atlas', hue: 'blue', role: 'Custom', roleTitle: 'Research', createdAt: '2026-09-05T05:00:00.000Z', route: { runtime: 'opencode', model: 'opencode/space-bunny-free', mode: 'ask' } }
const BUNNY = { runtime: 'opencode', model: 'opencode/space-bunny-free', label: 'Space Bunny' }
const MUSE = { runtime: 'opencode', model: 'opencode/muse-spark-1.3-contributor-free', label: 'Muse Spark' }
const LING = { runtime: 'opencode', model: 'opencode/ling-3.1-flash-free', label: 'Ling 3.1' }
const JUDGE = { runtime: 'codex', model: 'account-default', label: 'Codex' }

let n = 0
const recorded = async (route, prompt, text, at) => {
  n += 1
  const missionId = `mission_5e000000-0000-4000-8000-0764${String(n).padStart(8, '0')}`
  const runId = `run_5e0764${String(n)}`
  await ledger.createMission({
    missionId, runId, prompt,
    runtime: route.runtime, model: route.model, requestedRouteId: route.runtime, resolvedRouteId: `${route.runtime}-account:default`, cliVersion: null,
    workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at
  })
  const event = (sequence, type, payload) => ({ id: `event_${String(n)}_${String(sequence)}`, runId, missionId, sequence, occurredAt: at, sourceAdapter: route.runtime, type, payload: { ...payload, evidence: { redacted: true } } })
  await ledger.appendEvents(missionId, [
    event(1, 'message.delta', { itemId: 'answer', operation: 'append', text, final: true }),
    event(2, 'run.completed', { usage: { inputTokens: 900, outputTokens: 60, ...(route.runtime === 'opencode' ? { usd: 0 } : {}) }, process: { exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 2, inputDeliveryFailed: false, outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at, finishedAt: at } })
  ])
  return missionId
}
const owners = {}
/** One run of a comparison: its answers and its judge's scorecard, `met[slot]` of the two points. */
const aRun = async ({ compareId, prompt, routes, met, at, teammateId, extra = {} }) => {
  const slots = []
  for (const [index, route] of routes.entries()) {
    const slot = 'abcd'[index]
    const missionId = await recorded(route, prompt, `${route.label}'s answer.`, at)
    if (teammateId !== undefined) owners[missionId] = teammateId
    slots.push({ slot, route, missionIds: [missionId] })
  }
  const lines = ['The answers, read.']
  for (const [index] of routes.entries()) {
    const letter = 'ABCD'[index]
    for (const point of [1, 2]) lines.push(`SCORE ${letter} ${String(point)}: ${point <= met[index] ? 'met' : 'not met'}. read`)
  }
  lines.push('I would keep Answer A.')
  const judgeMission = await recorded(JUDGE, 'Judge the answers', lines.join('\n'), at)
  return { compareId, ...(teammateId === undefined ? {} : { teammateId }), prompt, createdAt: at, slots, judge: { route: JUDGE, missionIds: [judgeMission], criteria: 'Names a price\nSays which to buy' }, ...extra }
}
const ago = (minutes) => new Date(Date.now() - minutes * 60_000).toISOString()
const CRM = 'Which CRM should a five-person team buy? Answer in one sentence that names it and its price.'
const FIX = 'Why does the invoice date test fail on the first of the month?'
const compares = [
  // The newer benchmark: three runs, Space Bunny leading.
  await aRun({ compareId: 'cmp_crm', prompt: CRM, routes: [BUNNY, MUSE], met: [2, 0], at: ago(300), teammateId: ATLAS.teammateId, extra: { kept: { slot: 'a', at: ago(299) }, reruns: ['cmp_crm_2', 'cmp_crm_3'] } }),
  await aRun({ compareId: 'cmp_crm_2', prompt: CRM, routes: [BUNNY, MUSE], met: [2, 1], at: ago(40), teammateId: ATLAS.teammateId, extra: { rerunOf: 'cmp_crm', settled: true } }),
  await aRun({ compareId: 'cmp_crm_3', prompt: CRM, routes: [BUNNY, MUSE], met: [1, 0], at: ago(30), teammateId: ATLAS.teammateId, extra: { rerunOf: 'cmp_crm', settled: true } }),
  // The older one: three models, two runs, a tie at the top.
  await aRun({ compareId: 'cmp_fix', prompt: FIX, routes: [BUNNY, MUSE, LING], met: [2, 2, 1], at: ago(3000), teammateId: ATLAS.teammateId, extra: { kept: { slot: 'b', at: ago(2999) }, reruns: ['cmp_fix_2'] } }),
  await aRun({ compareId: 'cmp_fix_2', prompt: FIX, routes: [BUNNY, MUSE, LING], met: [1, 1, 0], at: ago(2900), teammateId: ATLAS.teammateId, extra: { rerunOf: 'cmp_fix', settled: true } }),
  // Scored once and never run again: a comparison, not a benchmark.
  await aRun({ compareId: 'cmp_once', prompt: 'Which spreadsheet formula sums a column by month?', routes: [BUNNY, MUSE], met: [1, 1], at: ago(100), teammateId: ATLAS.teammateId, extra: { kept: { slot: 'a', at: ago(99) } } })
]
await ledger.flush?.()
await writeFile(join(profilePath, 'compares.json'), JSON.stringify({ schemaVersion: 1, compares }), 'utf8')

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `benchlist-${tag}`,
  port: 9845,
  workspace,
  profilePath,
  outPath: join(recordRoot('a-benchmark-list-2026-10-12'), tag),
  seed: { schemaVersion: 1, teammates: [ATLAS], missionOwners: owners, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 500)}`}`)
}
const state = `JSON.stringify({
  place: [...document.querySelectorAll('.lc-sidebar__places--pinned button')].map((b) => b.innerText.replace(/\\s+/g, ' ').trim()).find((t) => /^Benchmarks/.test(t)) ?? '',
  screen: document.querySelector('.lc-benchmarks')?.getAttribute('aria-label') ?? '',
  meta: document.querySelector('.lc-benchmarks .lc-screen__meta')?.innerText.trim() ?? '',
  rows: [...document.querySelectorAll('.lc-benchmarks__row')].map((r) => ({
    question: r.querySelector('.lc-benchmarks__question')?.innerText.trim() ?? '',
    meta: r.querySelector('.lc-benchmarks__meta')?.innerText.trim() ?? '',
    chip: r.querySelector('.lc-benchmarks__chip')?.innerText.replace(/\\s+/g, ' ').trim() ?? '',
    fill: r.querySelector('.lc-benchmarks__fill')?.style.width ?? '',
    at: r.querySelector('.lc-benchmarks__at')?.innerText.trim() ?? '',
    width: Math.round(r.getBoundingClientRect().width),
    height: Math.round(r.getBoundingClientRect().height),
    overflowing: [...r.children].some((c) => c.getBoundingClientRect().right > r.getBoundingClientRect().right + 0.5)
  })),
  label: document.querySelector('.lc-benchmark .lc-compare__recordlabel')?.innerText.trim() ?? '',
  // Where the benchmark sits in the window: opened for it, it is scrolled to rather than left below the columns.
  benchmarkTop: Math.round(document.querySelector('.lc-benchmark')?.getBoundingClientRect().top ?? -1),
  bar: document.querySelector('.lc-compare__bar')?.innerText.replace(/\\s+/g, ' ').trim() ?? '',
  pageScrollsSideways: document.documentElement.scrollWidth > document.documentElement.clientWidth
})`
const read = async () => JSON.parse(String(await drive.evaluate(state)))
const press = (selector, words) => drive.evaluate(`(() => {
  const found = [...document.querySelectorAll(${JSON.stringify(selector)})].find((one) => one.innerText.replace(/\\s+/g, ' ').trim().startsWith(${JSON.stringify(words)}))
  if (!found) return 'no ' + ${JSON.stringify(words)}
  found.click()
  return 'pressed'
})()`)

try {
  await drive.capture('launch', () => drive.ready())
  await drive.resize(1440, 900)
  let now = await read()
  for (let i = 0; i < 40 && now.place === ''; i += 1) {
    await sleep(500)
    now = await read()
  }
  check('the sidebar has a Benchmarks place, counting the two benchmarks and not the comparison never run again', now.place === 'Benchmarks 2', now.place)
  say(`  open Benchmarks: ${String(await press('.lc-sidebar__places--pinned button', 'Benchmarks'))}`)
  await sleep(800)
  const list = await drive.capture('the benchmarks listed', () => read())
  check('it lists each benchmark once, the newest first', list.screen === 'Benchmarks' && list.rows.length === 2 && list.rows[0]?.question === CRM && list.rows[1]?.question === FIX, JSON.stringify(list.rows.map((r) => r.question)))
  check('the header counts the benchmarks and every run of them', list.meta === '2 benchmarks · 5 runs', list.meta)
  check('a row says its models, its runs and its judge', list.rows[0]?.meta === 'Space Bunny vs Muse Spark · 3 runs · judged by Codex' && list.rows[1]?.meta === 'Space Bunny vs Muse Spark vs Ling 3.1 · 2 runs · judged by Codex', JSON.stringify(list.rows.map((r) => r.meta)))
  // CRM: Space Bunny met 2, 2, 1 of 2 -> 83; Muse Spark 0, 1, 0 -> 17. FIX: Bunny and Muse 2, 1 -> 75 each.
  check('a row names who leads and by how much, a tie as a tie', list.rows[0]?.chip === 'Space Bunny 83' && list.rows[0]?.fill === '83%' && list.rows[1]?.chip === 'Space Bunny and Muse Spark tie 75', JSON.stringify(list.rows.map((r) => `${r.chip} / ${r.fill}`)))
  check('a row says when it last ran', list.rows[0]?.at === '30m' && list.rows[1]?.at === '2d', JSON.stringify(list.rows.map((r) => r.at)))
  // ACROSS YOUR BENCHMARKS (0.811): the two models in both, each benchmark counted once -- Bunny (83 + 75) / 2 = 79 and
  // Muse (17 + 75) / 2 = 46, their runs that met every point counted over all five -- and Ling, in one, left out.
  const across = JSON.parse(String(await drive.evaluate(`JSON.stringify({
    rows: [...document.querySelectorAll('.lc-benchmarks__across tbody tr')].map((tr) => [...tr.children].slice(0, 4).map((cell) => cell.textContent.trim())),
    before: (() => { const table = document.querySelector('.lc-benchmarks__across'); const list = document.querySelector('.lc-benchmarks__list'); return table !== null && list !== null && table.getBoundingClientRect().bottom <= list.getBoundingClientRect().top + 1 })()
  })`)))
  check('across your benchmarks, the models both ran in, each benchmark counted once, over the list', JSON.stringify(across.rows) === JSON.stringify([['Space Bunny', '2', '79', '3 of 5'], ['Muse Spark', '2', '46', '1 of 5']]) && across.before, across)

  say(`  open the CRM benchmark: ${String(await press('.lc-benchmarks__row', ''))}`)
  await sleep(1500)
  const opened = await drive.capture('a benchmark opened from the list', () => read())
  check('a row opens its comparison, at its benchmark', opened.label === 'YOUR BENCHMARK · 3 RUNS' && opened.benchmarkTop >= 0 && opened.benchmarkTop < 260, `${opened.label} · its top at ${String(opened.benchmarkTop)}px`)
  check('whose Back goes to the list', /Back to Benchmarks/.test(opened.bar), opened.bar)
  say(`  back: ${String(await press('.lc-compare__bar button', 'Back to Benchmarks'))}`)
  await sleep(800)
  const back = await read()
  check('and comes back to it', back.screen === 'Benchmarks' && back.rows.length === 2, back.screen)

  // A run of it, opened and gone back from: back at the benchmark, not at the top of the comparison.
  say(`  open again: ${String(await press('.lc-benchmarks__row', ''))}`)
  await sleep(1500)
  say(`  open run 2: ${String(await press('.lc-benchmark__run', 'Run 2'))}`)
  await sleep(1200)
  say(`  back to the benchmark: ${String(await press('.lc-compare__bar button', 'Back to the benchmark'))}`)
  await sleep(1200)
  const returned = await drive.capture('back from a run, at the benchmark', () => read())
  check('back from one of its runs, it is at the benchmark again, and its Back still goes to the list', returned.label === 'YOUR BENCHMARK · 3 RUNS' && returned.benchmarkTop >= 0 && returned.benchmarkTop < 260 && /Back to Benchmarks/.test(returned.bar), `top ${String(returned.benchmarkTop)} · ${returned.bar}`)
  say(`  back: ${String(await press('.lc-compare__bar button', 'Back to Benchmarks'))}`)
  await sleep(800)

  for (const [width, height] of [[1100, 720], [860, 640]]) {
    await drive.resize(width, height)
    await sleep(600)
    const sized = await drive.capture(`the list at ${String(width)} by ${String(height)}`, () => read())
    check(`at ${String(width)} by ${String(height)} the table across them stays inside the window`, await drive.evaluate(`(() => {
      const table = document.querySelector('.lc-benchmarks__across')
      return table !== null && table.getBoundingClientRect().right <= innerWidth && document.documentElement.scrollWidth <= innerWidth
    })()`))
    check(`at ${String(width)} by ${String(height)} every row holds its parts on one line, inside the window`, sized.rows.length === 2 && sized.rows.every((r) => !r.overflowing && r.height < 70) && !sized.pageScrollsSideways, JSON.stringify(sized.rows.map((r) => `${String(r.width)}x${String(r.height)}${r.overflowing ? ' over' : ''}`)))
  }
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. Two seeded benchmarks and a comparison never run again; nothing sent.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
