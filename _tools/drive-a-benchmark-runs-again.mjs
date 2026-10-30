// YOUR OWN BENCHMARK (0.763): a comparison the judge has scored, run again, read as one table.
//
//   LOCUST_SPEND=1 node _tools/drive-a-benchmark-runs-again.mjs [--packaged <exe>] [--tag <name>]
//
// The canvas "Your Own Benchmark" (56riCzZ3ZzR5AQ2sg6ssJm), after Artificial Analysis's Optima: one run can be luck.
// A seeded comparison of two free models, its judge's scorecard recorded, offers to run it again; "Run once more" runs
// both models again for real (free) and the same judge again for real (one short read-only Codex run, which may be
// spent: the cheapest that writes the scorecard's form reliably), then reads both runs as one table with a chart.
// The run again is never a conversation of its own; it opens, and goes back to its benchmark; a row sets a teammate.

import { createHash } from 'node:crypto'
import { mkdtemp, readFile, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

import { localPath, FREE_ROUTE, FREE_ROW, recordRoot, say, scratchRepository, sleep, startDrive } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
const tag = arg('--tag') ?? (packaged === undefined ? 'local' : 'packaged')

const root = localPath(new URL('..', import.meta.url))
const store = await import(pathToFileURL(join(root, 'packages', 'mission-store', 'dist', 'index.js')).href)
const workspace = await scratchRepository('locust-drive-benchmark-ws-')
const profilePath = await mkdtemp(join(process.env.LOCUST_SCRATCH ?? tmpdir(), 'locust-drive-benchmark-profile-'))
const workspaceId = `ws_${createHash('sha256').update(workspace, 'utf8').digest('hex').slice(0, 32)}`
const ledger = store.createFileMissionLedger({ rootDirectory: join(profilePath, 'mission-ledger') })
const TEAMMATE = { teammateId: 'tm_atlas0000000000000000', name: 'Atlas', hue: 'blue', role: 'Custom', roleTitle: 'Research', createdAt: '2026-09-05T05:00:00.000Z', route: { runtime: 'codex', model: 'account-default', mode: 'ask' } }
const ASK = 'Which CRM should a five-person team buy? Answer in one sentence that names it and its price.'
const CRITERIA = 'Names a price for five seats\nSays which CRM to buy'
const FREE_LABEL = FREE_ROUTE.model.replace(/^opencode\//, '').replace(/-free$/, '').split(/[-.]+/).map((word) => word.charAt(0).toUpperCase() + word.slice(1)).join(' ')
const FREE_NAME = new RegExp(FREE_ROW.slice(1, FREE_ROW.lastIndexOf('/')), 'i')
const ANSWERS = {
  a: ['opencode/space-bunny-free', 'Space Bunny', 'HubSpot Starter: $20 per seat a month, so $100 for five, and it covers email and deals.'],
  b: [FREE_ROUTE.model, FREE_LABEL, 'Some CRM would probably be good for a small team.']
}
const JUDGED = [
  'Answer A names a product and a price; Answer B names neither.',
  'SCORE A 1: met. $20 a seat, $100 for five',
  'SCORE B 1: not met. no price at all',
  'SCORE A 2: met. says HubSpot Starter',
  'SCORE B 2: not met. names no CRM',
  'I would keep Answer A.'
].join('\n')
const at = new Date(Date.now() - 900_000).toISOString()
let n = 0
const recorded = async (runtime, model, prompt, text) => {
  n += 1
  const missionId = `mission_5e000000-0000-4000-8000-0763${String(n).padStart(8, '0')}`
  const runId = `run_5e0763${String(n)}`
  await ledger.createMission({
    missionId, runId, prompt,
    runtime, model, requestedRouteId: runtime, resolvedRouteId: `${runtime}-account:default`, cliVersion: null,
    workspaceId, sandbox: 'read-only', executionPolicyVersion: 1, createdAt: at
  })
  const event = (sequence, type, payload) => ({ id: `event_${String(n)}_${String(sequence)}`, runId, missionId, sequence, occurredAt: at, sourceAdapter: runtime, type, payload: { ...payload, evidence: { redacted: true } } })
  await ledger.appendEvents(missionId, [
    event(1, 'message.delta', { itemId: 'answer', operation: 'append', text, final: true }),
    event(2, 'run.completed', { usage: { inputTokens: 900, outputTokens: 60, ...(runtime === 'opencode' ? { usd: 0 } : {}) }, process: { exitCode: 0, signal: null, stderr: '', stderrTruncated: false, recordCount: 2, inputDeliveryFailed: false, outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, startedAt: at, finishedAt: at } })
  ])
  return missionId
}
const slots = []
const owners = {}
for (const [slot, [model, label, text]] of Object.entries(ANSWERS)) {
  const missionId = await recorded('opencode', model, ASK, text)
  owners[missionId] = TEAMMATE.teammateId
  slots.push({ slot, route: { runtime: 'opencode', model, label }, missionIds: [missionId] })
}
const judgeMission = await recorded('codex', 'account-default', 'Judge the answers', JUDGED)
await ledger.flush?.()
await writeFile(join(profilePath, 'compares.json'), JSON.stringify({
  schemaVersion: 1,
  compares: [{ compareId: 'cmp_bench', teammateId: TEAMMATE.teammateId, prompt: ASK, createdAt: at, slots, judge: { route: { runtime: 'codex', model: 'account-default', label: 'Codex' }, missionIds: [judgeMission], criteria: CRITERIA } }]
}), 'utf8')

const drive = await startDrive({
  ...(packaged === undefined ? {} : { packaged }),
  name: `benchmark-${tag}`,
  port: 9844,
  workspace,
  profilePath,
  spends: true,
  outPath: join(recordRoot('a-benchmark-runs-again-2026-10-12'), tag),
  seed: { schemaVersion: 1, teammates: [TEAMMATE], missionOwners: owners, settings: { swarm: false, relay: false, relayHopCap: 2, memoryMode: 'off' } }
})
let failures = 0
const check = (what, ok, detail) => {
  if (!ok) failures += 1
  say(`  [${ok ? 'PASS' : 'FAIL'}] ${what}${detail === undefined ? '' : ` -- ${String(detail).slice(0, 500)}`}`)
}
const state = `JSON.stringify({
  block: document.querySelector('.lc-benchmark')?.innerText.replace(/\\s+/g, ' ').trim() ?? '',
  label: document.querySelector('.lc-benchmark .lc-compare__recordlabel')?.innerText.trim() ?? '',
  progress: document.querySelector('.lc-benchmark__progress')?.innerText.replace(/\\s+/g, ' ').trim() ?? '',
  rows: [...document.querySelectorAll('.lc-benchmark__table tbody tr')].map((tr) => [...tr.querySelectorAll('th, td')].map((cell) => cell.innerText.replace(/\\s+/g, ' ').trim())),
  runs: [...document.querySelectorAll('.lc-benchmark__run')].map((b) => b.innerText.replace(/\\s+/g, ' ').trim()),
  chart: document.querySelector('.lc-benchmark__chart svg')?.getAttribute('aria-label') ?? '',
  bar: document.querySelector('.lc-compare__bar')?.innerText.replace(/\\s+/g, ' ').trim() ?? '',
  keeps: [...document.querySelectorAll('.lc-compare button')].filter((b) => b.innerText.trim() === 'Keep this one').length,
  sidebar: [...document.querySelectorAll('.lc-convrow button.lc-conv')].map((b) => b.innerText.replace(/\\s+/g, ' ').trim())
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
  say(`  open: ${String(await drive.evaluate(`(() => { const row = [...document.querySelectorAll('.lc-convrow button.lc-conv')].find((r) => /CRM should/i.test(r.innerText)); if (!row) return 'no row'; row.click(); return 'opened' })()`))}`)
  for (let i = 0; i < 40 && (await read()).block === ''; i += 1) await sleep(500)
  const offered = await drive.capture('a scored comparison offers to run it again', async () => {
    await drive.evaluate(`document.querySelector('.lc-benchmark')?.scrollIntoView({ block: 'center' }); 1`)
    return read()
  })
  check('a comparison its judge has scored offers to run it again, once or three times more', /YOUR BENCHMARK/.test(offered.block) && /One run can be luck/.test(offered.block) && /Run 3 more times/.test(offered.block) && /Run once more/.test(offered.block), offered.block)
  const sidebarBefore = offered.sidebar.length

  say(`  run once more: ${String(await press('.lc-benchmark button', 'Run once more'))}`)
  let seen = { answering: '', judging: '' }
  let now = await read()
  for (let waited = 0; waited < 480_000; waited += 2000) {
    await sleep(2000)
    now = await read()
    if (/models are answering/.test(now.progress) && seen.answering === '') seen = { ...seen, answering: now.progress }
    if (/judge is reading/.test(now.progress) && seen.judging === '') seen = { ...seen, judging: now.progress }
    if (now.progress === '' && /2 RUNS/.test(now.label) && now.runs.some((one) => /Run 2/.test(one) && !/answering|judging|waiting/.test(one))) break
  }
  check('while it runs, it says which run and what it is doing', seen.answering.length > 0 && seen.judging.length > 0, JSON.stringify(seen))
  const table = await drive.capture('both runs, read as one table', async () => {
    await drive.evaluate(`document.querySelector('.lc-benchmark')?.scrollIntoView({ block: 'start' }); 1`)
    return read()
  })
  check('it reads both runs as one table, a row a model with its score', table.label === 'YOUR BENCHMARK · 2 RUNS' && table.rows.length === 2 && table.rows.every((row) => /^\d{1,3}$/.test(row[1] ?? '')), JSON.stringify(table.rows))
  check('the second run was scored by the same judge on the same points', table.runs.some((one) => /^Run 2 scored$/.test(one)), JSON.stringify(table.runs))
  check('each row says the runs that met every point, its time and its cost', table.rows.every((row) => /^\d of \d$/.test(row[2] ?? '') && /\d/.test(row[3] ?? '') && (row[4] ?? '').length > 0), JSON.stringify(table.rows))
  check('it charts score against cost a task', /^Score against (cost|time) a task: .+/.test(table.chart), table.chart)
  check('the run again is not a conversation of its own', table.sidebar.length === sidebarBefore, JSON.stringify(table.sidebar))

  say(`  open run 2: ${String(await press('.lc-benchmark__run', 'Run 2'))}`)
  await sleep(1200)
  const rerun = await drive.capture('the run again, opened', () => read())
  check('opened, the run says it is a run of the benchmark, and keeps nothing', /^A run of your benchmark, comparing/.test(rerun.bar) && /Back to the benchmark/.test(rerun.bar) && rerun.keeps === 0, `${rerun.bar} · keeps ${String(rerun.keeps)}`)
  say(`  back: ${String(await press('.lc-compare__bar button', 'Back to the benchmark'))}`)
  await sleep(1200)
  const back = await read()
  check('and goes back to its benchmark', back.label === 'YOUR BENCHMARK · 2 RUNS', back.label)

  const first = back.rows[0]?.[0] ?? ''
  say(`  use for Atlas: ${String(await drive.evaluate(`(() => { const button = document.querySelector('.lc-benchmark__table tbody tr .lc-benchmark__use button'); if (!button) return 'no button'; button.click(); return 'pressed' })()`))}`)
  let used = back
  for (let i = 0; i < 30; i += 1) {
    await sleep(500)
    used = await read()
    if (/Atlas's model/.test(used.rows[0]?.join(' ') ?? '')) break
  }
  await drive.capture('a row set as the teammate\'s model', async () => used)
  const teammates = JSON.parse(await readFile(join(profilePath, 'teammates.json'), 'utf8').catch(() => '{}'))
  const atlas = (teammates.teammates ?? []).find((one) => one.teammateId === TEAMMATE.teammateId)
  check('a row sets its model as the teammate\'s own, in one press', /Atlas's model/.test(used.rows[0]?.join(' ') ?? '') && atlas?.route?.runtime === 'opencode' && (/space-bunny/.test(atlas?.route?.model ?? '') || FREE_NAME.test(atlas?.route?.model ?? '')), `${first} -> ${JSON.stringify(atlas?.route)}`)
} catch (error) {
  failures += 1
  say(`drive failed: ${error instanceof Error ? error.message : String(error)}`)
} finally {
  await drive.finish({ intro: `Build: ${packaged ?? 'out/'}. A seeded comparison of two free models, its judge's scorecard recorded, run once more for real.`, extra: `Checks failed: ${String(failures)}` })
}
say(failures === 0 ? 'ALL CHECKS PASSED' : `${String(failures)} CHECK(S) FAILED`)
process.exit(failures === 0 ? 0 : 1)
