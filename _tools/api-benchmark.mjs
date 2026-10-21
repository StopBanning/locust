// USD / million tokens, standard global API, checked 2026-10-21:
// https://platform.claude.com/docs/en/about-claude/pricing
// Estimates, not invoices. Haiku uses its >100k tier conservatively because a run aggregates many prompts.
// Cache writes use the dearer 1h rate; cache tokens are added, never subtracted from an ambiguous input count.
export const PRICES = Object.freeze({
  'claude-opus-5-5': { input: 4, output: 20, read: 0.20, write: 8 },
  'claude-sonnet-5-5': { input: 2, output: 10, read: 0.10, write: 4 },
  'claude-haiku-5-5': { input: 0.50, output: 2.50, read: 0.05, write: 1, under100k: { input: 0.10, output: 0.50 } },
  'claude-fable-5-1': { input: 10, output: 50, read: 0.25, write: 20 },
  'claude-opus-5': { input: 5, output: 25, read: 0.50, write: 10 },
  'claude-sonnet-5': { input: 2, output: 10, read: 0.20, write: 4 }
})
export const CONTESTANTS = ['claude-opus-5-5', 'claude-sonnet-5-5', 'claude-haiku-5-5']
export const API_JUDGES = ['claude-fable-5-1', 'claude-opus-5', 'claude-sonnet-5']
export const ESTIMATE_LABEL = 'Estimate, not an invoice; standard API rates; Haiku high tier; cache writes 1h; cache counted separately.'
export const ownModelRoute = (model) => ({ runtime: 'opencode', model: `own-${model.ownId}/${model.model}`, label: model.name })

/** The same append/replace contract the conversation reads, not raw deltas concatenated into duplicate scorecards. */
export function answerOf(mission) {
  const items = new Map()
  for (const event of mission?.events ?? []) {
    if (event.type !== 'message.delta' || typeof event.payload.text !== 'string') continue
    const { itemId = 'answer', text, operation } = event.payload
    items.set(itemId, operation === 'replace' ? text : (items.get(itemId) ?? '') + text)
  }
  return [...items.values()].join('\n')
}

const finite = (value) => typeof value === 'number' && Number.isFinite(value) && value >= 0
export function estimateCost(model, usage) {
  if (model.endsWith('-free')) return 0
  const rate = PRICES[model.split('/').at(-1)]
  if (!rate || !finite(usage?.inputTokens) || !finite(usage?.outputTokens)) return null
  const read = usage.cacheReadTokens ?? 0, write = usage.cacheWriteTokens ?? 0
  if (!finite(read) || !finite(write)) return null
  return (usage.inputTokens * rate.input + usage.outputTokens * rate.output + read * rate.read + write * rate.write) / 1_000_000
}

// A checkpoint between groups of concurrent turns, not a hard cap on a group already in flight.
export function spendStop(charges, maximum) {
  if (!finite(maximum) || maximum === 0) return 'The estimated-spend limit must be a positive number.'
  if (charges.some((charge) => !finite(charge))) return 'Usage was not reported for a paid turn; estimated spend is unknown. No new turns started.'
  const total = charges.reduce((sum, charge) => sum + charge, 0)
  return total >= maximum ? `Estimated spend reached $${total.toFixed(4)} (limit $${maximum}). No new turns started.` : undefined
}

export function receipt(mission, model) {
  const events = mission?.events ?? []
  const ended = [...events].reverse().find((event) => ['run.completed', 'run.failed', 'run.cancelled'].includes(event.type))
  const started = events.find((event) => event.type === 'run.started')
  const usage = ended?.payload?.usage
  const start = Date.parse(started?.occurredAt ?? mission?.createdAt ?? ''), end = Date.parse(ended?.occurredAt ?? '')
  return {
    phase: mission?.phase ?? 'not started',
    inputTokens: finite(usage?.inputTokens) ? usage.inputTokens : null,
    outputTokens: finite(usage?.outputTokens) ? usage.outputTokens : null,
    cacheReadTokens: finite(usage?.cacheReadTokens) ? usage.cacheReadTokens : null,
    cacheWriteTokens: finite(usage?.cacheWriteTokens) ? usage.cacheWriteTokens : null,
    wallMs: Number.isFinite(start) && Number.isFinite(end) && end >= start ? end - start : null,
    estimatedDollars: estimateCost(model, usage),
    estimateLabel: ESTIMATE_LABEL
  }
}

export function resultRow({ task, model, actualModel, run, compareId, mission, check, points = null, pick = null, standIn }) {
  const took = receipt(mission, actualModel)
  return {
    task, model, actualModel, run, standIn,
    checkPassed: check?.passed ?? null, checkExitCode: check?.exitCode ?? null,
    checkSaid: check?.said ?? 'The column did not start.',
    pointsMet: took.phase === 'completed' ? points : null,
    panelPick: took.phase === 'completed' ? pick : null,
    ...took, compareId, missionId: mission?.missionId ?? null
  }
}

export function csv(rows) {
  const columns = ['task', 'model', 'actualModel', 'run', 'standIn', 'checkPassed', 'checkExitCode', 'checkSaid', 'phase', 'pointsMet', 'panelPick', 'inputTokens', 'outputTokens', 'cacheReadTokens', 'cacheWriteTokens', 'wallMs', 'estimatedDollars', 'estimateLabel', 'compareId', 'missionId']
  const cell = (value) => '"' + String(value ?? '').replaceAll('"', '""') + '"'
  return columns.join(',') + '\n' + rows.map((row) => columns.map((key) => cell(row[key])).join(',')).join('\n') + '\n'
}

export function summary(rows, meta) {
  const mean = (values) => values.length ? (values.reduce((sum, value) => sum + value, 0) / values.length).toFixed(2) : 'not reported'
  const lines = ['# API benchmark', '', meta.standIn ? '**Free stand-in only. These are not Claude quality, latency or cost measurements.**' : 'API contestant measurements.', '', meta.stop ?? 'Requested runs finished.', '',
    '| Model (actual route) | Objective checks passed / attempted | Mean points / 3 (scored rows) | Panel picks | Mean input / output tokens (reported rows) | Estimated contestant cost |',
    '| --- | --- | --- | --- | --- | --- |']
  for (const model of CONTESTANTS) {
    const own = rows.filter((row) => row.model === model)
    const points = own.filter((row) => row.pointsMet !== null)
    const cost = own.map((row) => row.estimatedDollars)
    lines.push(`| ${model} (${own[0]?.actualModel ?? 'not run'}) | ${own.filter((row) => row.checkPassed === true).length} / ${own.length} | ${mean(points.map((row) => row.pointsMet))} (${points.length}) | ${own.filter((row) => row.panelPick === true).length} | ${mean(own.flatMap((row) => row.inputTokens === null ? [] : [row.inputTokens]))} / ${mean(own.flatMap((row) => row.outputTokens === null ? [] : [row.outputTokens]))} | ${cost.some((value) => value === null) ? 'unknown' : '$' + cost.reduce((sum, value) => sum + value, 0).toFixed(6)} |`)
  }
  lines.push('', ESTIMATE_LABEL, '', 'Prices: https://platform.claude.com/docs/en/about-claude/pricing (checked 2026-10-21).',
    'Spend includes contestants, judges and a conservative $0.01 allowance per Settings Test (its usage is not exposed). A group already started can finish above the cap. Unknown paid usage stops further turns.',
    `Total estimated spend including panel/setup: ${meta.charges.some((value) => value === null) ? 'unknown' : '$' + meta.charges.reduce((sum, value) => sum + value, 0).toFixed(6)}.`, '',
    'Failed or timed-out turns remain rows; missing tokens, points and times are null, never a fabricated zero. Objective checks are independent of panel opinions. Partial scorecards do not get a total.', '',
    '## Fixed bases', '', ...meta.bases.map((base) => `- ${base.task}: ${base.commit}; ${base.hash}`), '',
    '## Frames', '', ...meta.frames.map((frame) => `- ${frame}`), '', `Secret scan: ${meta.scan ?? 'pending'}.`, '')
  return lines.join('\n')
}

export function options(args) {
  const flags = new Set(['--stand-in', '--tasks', '--runs', '--max-dollars', '--judges', '--tag', '--packaged'])
  const picked = {}
  for (let i = 0; i < args.length; i += 1) {
    const flag = args[i]
    if (!flags.has(flag)) throw new Error(`Unknown option ${flag}`)
    if (flag === '--stand-in') picked.standIn = true
    else {
      if (args[i + 1] === undefined || args[i + 1].startsWith('--')) throw new Error(`Missing value for ${flag}`)
      picked[flag.slice(2)] = args[++i]
    }
  }
  const runs = Number(picked.runs ?? 3), maximum = Number(picked['max-dollars'] ?? 20)
  if (!Number.isInteger(runs) || runs < 1 || runs > 20 || !finite(maximum) || maximum === 0) throw new Error('Runs must be 1..20 and max-dollars must be positive.')
  const judges = picked.judges ?? 'free'
  if (!['free', 'api'].includes(judges) || (picked.standIn && judges !== 'free')) throw new Error('Choose free or api judges; stand-in only permits free judges.')
  return { ...picked, runs, maximum, judges, tasks: (picked.tasks ?? 'bug,feature,refactor,tests,html').split(',') }
}
