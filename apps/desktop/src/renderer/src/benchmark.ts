import type { MissionRuntimeId } from '@teammate/runtime-adapters'
import type { RunCost } from './cost.js'

/**
 * YOUR OWN BENCHMARK, ON SCREEN (0.763, main/compare-reruns.ts): every run of one comparison -- the first and each
 * run again -- read as one table, a row a model. After Artificial Analysis's Optima: score, time a task and cost a task
 * side by side, and score against cost on a chart. Nothing here is estimated: a score is the judge's points met, a time
 * is the run's own clock, a cost is what the runtime reported, in its own unit.
 */

/** One model's answer in one run. */
export interface BenchmarkAnswer {
  /** `runtime:model`, the picker's own key. */
  readonly key: string
  readonly runtime: MissionRuntimeId
  readonly model: string
  readonly effort?: string
  readonly name: string
  /** It finished an answer in this run. */
  readonly answered: boolean
  /** The judge's points it met, of how many it scored; absent when this run was not scored. */
  readonly met?: number
  readonly of?: number
  readonly spanMs?: number
  readonly cost?: RunCost
  /** What your check said in its copy after its turn (0.810), when one ran. */
  readonly check?: 'passed' | 'failed' | 'timed-out' | 'could-not-run'
}

export interface BenchmarkRunInput {
  readonly compareId: string
  readonly answers: readonly BenchmarkAnswer[]
}

export interface BenchmarkRow {
  readonly key: string
  readonly runtime: MissionRuntimeId
  readonly model: string
  readonly effort?: string
  readonly name: string
  /** Runs it finished an answer in. */
  readonly runs: number
  /** Of those, the runs the judge scored. */
  readonly scored: number
  /** 0-100: the points met over the points scored, averaged over its scored runs. */
  readonly score?: number
  /** Scored runs in which it met every point. */
  readonly everyPoint: number
  /** The middle run's time. */
  readonly spanMs?: number
  /** What a run cost on average, in the unit its runtime reports. */
  readonly cost?: RunCost
  /** Runs your check ran in its copy after (0.810), and of those, the runs it passed. */
  readonly checked: number
  readonly checkPassed: number
}

/** The middle of a list of numbers: the mean of the two middle ones for an even count. */
export function median(values: readonly number[]): number | undefined {
  if (values.length === 0) return undefined
  const sorted = [...values].sort((left, right) => left - right)
  const middle = Math.floor(sorted.length / 2)
  return sorted.length % 2 === 1 ? sorted[middle] : (sorted[middle - 1]! + sorted[middle]!) / 2
}

/**
 * What one run cost on average, in each unit the runs reported: dollars over the runs that were priced, premium
 * requests over the runs that spent some, tokens over the runs that counted them. "In your plan" when every run was.
 */
export function averageCost(costs: readonly (RunCost | undefined)[]): RunCost | undefined {
  const reported = costs.filter((cost): cost is RunCost => cost !== undefined)
  if (reported.length === 0) return undefined
  const mean = (pick: (cost: RunCost) => number | undefined): number | undefined => {
    const values = reported.map(pick).filter((value): value is number => value !== undefined)
    return values.length === 0 ? undefined : values.reduce((sum, value) => sum + value, 0) / values.length
  }
  const usd = mean((cost) => cost.usd)
  const premiumRequests = mean((cost) => cost.premiumRequests)
  const inputTokens = mean((cost) => cost.inputTokens)
  const outputTokens = mean((cost) => cost.outputTokens)
  return {
    ...(usd === undefined ? {} : { usd }),
    ...(premiumRequests === undefined ? {} : { premiumRequests: Math.round(premiumRequests * 10) / 10 }),
    ...(inputTokens === undefined ? {} : { inputTokens: Math.round(inputTokens) }),
    ...(outputTokens === undefined ? {} : { outputTokens: Math.round(outputTokens) }),
    ...(reported.every((cost) => cost.plan === true) ? { plan: true as const } : {})
  }
}

/** Every model across every run, best score first; a model never scored comes after the scored ones. */
export function benchmarkRows(runs: readonly BenchmarkRunInput[]): readonly BenchmarkRow[] {
  const byKey = new Map<string, BenchmarkAnswer[]>()
  for (const run of runs) {
    for (const answer of run.answers) {
      const held = byKey.get(answer.key) ?? []
      held.push(answer)
      byKey.set(answer.key, held)
    }
  }
  const rows: BenchmarkRow[] = []
  for (const [key, answers] of byKey) {
    const first = answers[0]!
    const finished = answers.filter((answer) => answer.answered)
    const scored = finished.filter((answer) => answer.met !== undefined && answer.of !== undefined && answer.of > 0)
    const score = scored.length === 0 ? undefined : Math.round((scored.reduce((sum, answer) => sum + answer.met! / answer.of!, 0) / scored.length) * 100)
    const spanMs = median(finished.flatMap((answer) => (answer.spanMs === undefined ? [] : [answer.spanMs])))
    const cost = averageCost(finished.map((answer) => answer.cost))
    const checked = finished.filter((answer) => answer.check !== undefined)
    rows.push({
      key,
      runtime: first.runtime,
      model: first.model,
      ...(first.effort === undefined ? {} : { effort: first.effort }),
      name: first.name,
      runs: finished.length,
      scored: scored.length,
      ...(score === undefined ? {} : { score }),
      everyPoint: scored.filter((answer) => answer.met === answer.of).length,
      ...(spanMs === undefined ? {} : { spanMs }),
      ...(cost === undefined ? {} : { cost }),
      checked: checked.length,
      checkPassed: checked.filter((answer) => answer.check === 'passed').length
    })
  }
  return rows.sort((left, right) => (right.score ?? -1) - (left.score ?? -1) || (left.spanMs ?? Infinity) - (right.spanMs ?? Infinity))
}

/**
 * Who leads a benchmark (0.764, the Benchmarks screen): the best score, and every model at it, so a tie is said as a
 * tie. Undefined until the judge has scored one.
 */
export function benchmarkLeader(rows: readonly BenchmarkRow[]): { readonly names: readonly string[]; readonly score: number } | undefined {
  const scored = rows.filter((row) => row.score !== undefined)
  if (scored.length === 0) return undefined
  const score = Math.max(...scored.map((row) => row.score!))
  return { names: scored.filter((row) => row.score === score).map((row) => row.name), score }
}

/** What a dollar figure there is for a row: its average, or nothing when its runtime does not price its runs. */
const dollarsOf = (row: BenchmarkRow): number | undefined => (row.cost?.usd !== undefined ? row.cost.usd : undefined)

export interface BenchmarkChart {
  /**
   * Dollars a task when at least two scored models were priced (a free model at $0) at different prices, else time a
   * task: two free models charted by cost stood on one line, the chart saying nothing the table did not (0.763 drive).
   */
  readonly axis: 'cost' | 'time'
  /** Why it is charted against time: too few were priced in dollars, or every one cost the same. */
  readonly because?: 'unpriced' | 'same-price'
  readonly points: readonly { readonly key: string; readonly name: string; readonly score: number; readonly x: number; readonly free: boolean }[]
  /** Scored models the chart cannot place: no price on a dollar chart. */
  readonly left: readonly string[]
}

/** Score against cost a task, or against time when too few are priced; undefined with fewer than two scored. */
export function benchmarkChart(rows: readonly BenchmarkRow[]): BenchmarkChart | undefined {
  const scored = rows.filter((row) => row.score !== undefined)
  if (scored.length < 2) return undefined
  const priced = scored.filter((row) => dollarsOf(row) !== undefined)
  const prices = new Set(priced.map((row) => dollarsOf(row)!.toFixed(4)))
  if (priced.length >= 2 && prices.size >= 2) {
    return {
      axis: 'cost',
      points: priced.map((row) => ({ key: row.key, name: row.name, score: row.score!, x: dollarsOf(row)!, free: dollarsOf(row) === 0 })),
      left: scored.filter((row) => dollarsOf(row) === undefined).map((row) => row.name)
    }
  }
  const timed = scored.filter((row) => row.spanMs !== undefined)
  if (timed.length < 2) return undefined
  return {
    axis: 'time',
    because: priced.length >= 2 ? 'same-price' : 'unpriced',
    points: timed.map((row) => ({ key: row.key, name: row.name, score: row.score!, x: row.spanMs!, free: false })),
    left: scored.filter((row) => row.spanMs === undefined).map((row) => row.name)
  }
}
