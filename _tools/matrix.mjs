// A workflow matrix: each scenario on each runtime, PASS / FAIL / NOT RUN (W9, 0.568).
//
//   node _tools/matrix.mjs --packaged <exe> [--runtimes opencode,codex] [--only a,b] [--limit-min 15]
//   LOCUST_SPEND=1 node _tools/matrix.mjs --packaged <exe> --runtimes opencode,codex:gpt-6-luna:low
//
// From the survey's Promptfoo idea, with no new dependency: a scenario is an
// existing drive (named below), run once per runtime the way
// `sweep-drives.mjs` runs a drive -- alone, with a time limit, its record
// out of the repository -- and read the same way: its exit code and its
// PASS / FAIL lines. Nothing is graded by a model.
//
// Only the free OpenCode route runs unless asked. Any other runtime is a
// column only with LOCUST_SPEND=1 AND Colin's go: it spends that account.
// A cell that was not run says why, and is never counted as failed: a
// scenario whose drive runs on OpenCode only, a paid column without
// LOCUST_SPEND=1, or a scenario left out with --only.
//
// Writes docs/matrix/<stamp>.md (rewritten after every cell, so a stopped
// matrix still says what it found), with each failed cell's FAIL lines, and
// the drives' logs and captures beside it under docs/matrix/<stamp>/.

import { execFile, spawn } from 'node:child_process'
import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import { localPath, FREE_ROUTE } from './drive-lib.mjs'

const arg = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined)
const packaged = arg('--packaged')
if (packaged === undefined) {
  console.error('usage: node _tools/matrix.mjs --packaged <exe> [--runtimes opencode,codex[:model[:effort]]] [--only a,b] [--limit-min 15]')
  process.exit(2)
}
const LIMIT_MS = Number(arg('--limit-min') ?? '15') * 60_000
const only = arg('--only')?.split(',').map((id) => id.trim())

/**
 * The scenarios, each mapped to the drive that already tests it. `anyRuntime`
 * drives take LOCUST_RUNTIME / LOCUST_MODEL / LOCUST_EFFORT; the others run on
 * the free OpenCode route only, and say so in every other column.
 */
const SCENARIOS = [
  { id: 'a-turn-and-a-follow-up', says: 'A turn reads a file and answers; a follow-up continues it', drive: 'drive-a-turn-round-trip', anyRuntime: true },
  { id: 'two-conversations-stay-apart', says: 'Two conversations with one teammate run at once and stay apart', drive: 'drive-two-conversations-one-teammate', anyRuntime: false },
  { id: 'a-stopped-run-says-so', says: 'A run stopped part way through says the person stopped it, and nothing lands after', drive: 'drive-stop-a-live-run', anyRuntime: true },
  { id: 'a-stopped-tool-reads-as-stopped', says: 'A command stopped before it reported is not called "nothing ran"', drive: 'drive-stop-before-a-tool-reports', anyRuntime: false },
  { id: 'a-run-survives-a-relaunch', says: 'A finished run comes back after Locust closes, and a follow-up continues it', drive: 'drive-a-run-survives-a-relaunch', anyRuntime: true },
  { id: 'an-emails-instructions-stay-inert', says: 'An email that says "do this" is summarised, not obeyed (measures the model too)', drive: 'drive-an-email-that-says-do-this', anyRuntime: true }
]

/** The cheapest sensible route per runtime when a column names none. */
const DEFAULTS = {
  opencode: { model: FREE_ROUTE.model },
  codex: { model: 'gpt-6-luna', effort: 'low' },
  claude: { model: 'haiku' },
  antigravity: { model: 'gemini-3.8-flash', effort: 'low' }
}
const columns = (arg('--runtimes') ?? 'opencode').split(',').map((spec) => {
  const [runtime, model, effort] = spec.trim().split(':')
  return { runtime, model: model || DEFAULTS[runtime]?.model, effort: effort || (model ? undefined : DEFAULTS[runtime]?.effort) }
})
const spendAllowed = process.env.LOCUST_SPEND === '1'

const repo = localPath(new URL('../', import.meta.url))
const stamp = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19)
const folder = join(repo, 'docs', 'matrix', stamp)
await mkdir(join(folder, 'logs'), { recursive: true })
await mkdir(join(folder, 'captures'), { recursive: true })
const report = join(repo, 'docs', 'matrix', `${stamp}.md`)

/** The whole tree a drive started, and nothing else: its own app included. */
const killTree = (pid) => new Promise((resolve) => execFile('taskkill', ['/PID', String(pid), '/T', '/F'], { windowsHide: true }, () => resolve()))

/** Runs one drive on one route; reads its exit and its PASS / FAIL lines. */
async function runCell(scenario, column) {
  const began = Date.now()
  const child = spawn(process.execPath, [join(repo, '_tools', `${scenario.drive}.mjs`), '--packaged', packaged, '--tag', `matrix-${column.runtime}`], {
    cwd: repo,
    env: {
      ...process.env,
      LOCUST_DRIVE_OUT: join(folder, 'captures'),
      LOCUST_RUNTIME: column.runtime,
      ...(column.model === undefined ? {} : { LOCUST_MODEL: column.model }),
      ...(column.effort === undefined ? {} : { LOCUST_EFFORT: column.effort }),
      ...(column.runtime === 'opencode' ? { LOCUST_FREE_MODEL: column.model ?? FREE_ROUTE.model } : {})
    },
    windowsHide: true
  })
  let text = ''
  child.stdout.on('data', (chunk) => { text += String(chunk) })
  child.stderr.on('data', (chunk) => { text += String(chunk) })
  let timedOut = false
  const timer = setTimeout(() => {
    timedOut = true
    void killTree(child.pid)
  }, LIMIT_MS)
  const code = await new Promise((resolve) => child.on('close', (value) => resolve(value ?? -1)))
  clearTimeout(timer)
  await writeFile(join(folder, 'logs', `${scenario.id}--${column.runtime}.log`), text, 'utf8')
  const lines = text.split(/\r?\n/)
  const passes = lines.filter((line) => /\bPASS\b/.test(line)).length
  const failLines = lines.filter((line) => /\bFAIL\b|(drive|probe) failed:|refusing to/.test(line)).map((line) => line.trim())
  const verdict = !timedOut && code === 0 && failLines.length === 0 && passes > 0 ? 'PASS' : 'FAIL'
  return { verdict, passes, failLines: timedOut ? [`timed out after ${String(LIMIT_MS / 60_000)} min`, ...failLines] : code !== 0 && failLines.length === 0 ? [`exited ${String(code)} with no FAIL line`] : failLines, seconds: Math.round((Date.now() - began) / 1000) }
}

/** Why a cell is not run, or undefined when it should be. */
function notRun(scenario, column) {
  if (only !== undefined && !only.includes(scenario.id)) return 'left out with --only'
  if (!scenario.anyRuntime && column.runtime !== 'opencode') return 'this drive runs on the free OpenCode route only'
  if (column.runtime !== 'opencode' && !spendAllowed) return 'spends this account: needs LOCUST_SPEND=1 and Colin\'s go'
  return undefined
}

const cells = new Map()
const label = (column) => `${column.runtime}${column.model === undefined ? '' : ` / ${column.model}`}${column.effort === undefined ? '' : ` (${column.effort})`}`
async function write() {
  const head = `| scenario | ${columns.map(label).join(' | ')} |`
  const rule = `| --- | ${columns.map(() => '---').join(' | ')} |`
  const rows = SCENARIOS.map((scenario) => `| **${scenario.id}** -- ${scenario.says} | ${columns.map((column) => {
    const cell = cells.get(`${scenario.id}|${column.runtime}`)
    if (cell === undefined) return '...'
    if (cell.verdict === 'NOT RUN') return `NOT RUN (${cell.why})`
    return `${cell.verdict} (${String(cell.passes)} checks passed, ${String(cell.seconds)}s)`
  }).join(' | ')} |`)
  const counted = [...cells.values()]
  const failed = SCENARIOS.flatMap((scenario) => columns.map((column) => ({ scenario, column, cell: cells.get(`${scenario.id}|${column.runtime}`) })))
    .filter((entry) => entry.cell?.verdict === 'FAIL')
  const md = [
    `# Workflow matrix ${stamp}`,
    '',
    `Build: \`${packaged}\``,
    '',
    `${String(counted.filter((cell) => cell.verdict === 'PASS').length)} passed, ${String(counted.filter((cell) => cell.verdict === 'FAIL').length)} failed, ${String(counted.filter((cell) => cell.verdict === 'NOT RUN').length)} not run (never counted as failed).`,
    '',
    head,
    rule,
    ...rows,
    '',
    ...(failed.length === 0 ? [] : ['## What failed', '', ...failed.flatMap(({ scenario, column, cell }) => [`### ${scenario.id} on ${label(column)}`, '', ...cell.failLines.slice(0, 12).map((line) => `- ${line.replace(/\|/g, '/').slice(0, 400)}`), `- log: \`docs/matrix/${stamp}/logs/${scenario.id}--${column.runtime}.log\``, ''])]),
    ''
  ].join('\n')
  await writeFile(report, md, 'utf8')
}

for (const scenario of SCENARIOS) {
  for (const column of columns) {
    const why = notRun(scenario, column)
    if (why !== undefined) {
      cells.set(`${scenario.id}|${column.runtime}`, { verdict: 'NOT RUN', why })
      await write()
      continue
    }
    console.log(`${scenario.id} on ${label(column)}...`)
    const cell = await runCell(scenario, column)
    cells.set(`${scenario.id}|${column.runtime}`, cell)
    await write()
    console.log(`  ${cell.verdict} (${String(cell.passes)} PASS, ${String(cell.seconds)}s)${cell.failLines.length === 0 ? '' : ` -- ${cell.failLines[0].slice(0, 160)}`}`)
  }
}
console.log(`MATRIX DONE: ${report}`)
