import type { CompareRoute, CompareSlotId, PublicCompare } from '../../shared/compare.js'
import { hasColumnInstructions } from '../../shared/compare.js'
import { costLine } from './cost.js'
import type { RunCost } from './cost.js'
import { durationText } from './missionView.js'

/**
 * YOUR RECORD, AS A TABLE (0.519), after Artificial Analysis's Optima: each
 * model with its score, cost per task and time per task, so "strong and
 * cheap" reads at a glance. Here the score is yours -- how many of your
 * decided comparisons it answered in, and how many of those you kept it
 * (`compareRecord`, 0.449) -- and beside it the typical time and cost of its
 * answers, each the middle one of its columns, as their runtimes reported
 * them. Nothing is estimated: a model none of whose answers reported a cost
 * has no cost.
 */
export interface CompareRecordRow {
  /** `runtime:model`, the picker's own key. */
  readonly key: string
  readonly name: string
  readonly kept: number
  readonly compared: number
  /** "41s": the middle of its answers' times. */
  readonly time?: string
  /** "$0.02", "12k in · 900 out", "in your plan": the middle of its answers' costs. */
  readonly cost?: string
}

/** What one column of one comparison took, from its runs' events. */
export interface ColumnTook {
  readonly ms?: number
  readonly cost?: RunCost
}

function middle<T>(values: readonly T[], weight: (value: T) => number): T | undefined {
  const sorted = [...values].sort((left, right) => weight(left) - weight(right))
  return sorted[Math.floor((sorted.length - 1) / 2)]
}

/** A cost's size, for finding the middle one: dollars, then premium requests, then tokens. */
function costWeight(cost: RunCost): number {
  return cost.usd ?? cost.premiumRequests ?? (cost.inputTokens ?? 0) + (cost.outputTokens ?? 0)
}

export function compareRecordRows(
  compares: readonly PublicCompare[],
  took: (compare: PublicCompare, slot: CompareSlotId) => ColumnTook | undefined,
  nameOf: (route: CompareRoute) => string
): readonly CompareRecordRow[] {
  const rows = new Map<string, { name: string; kept: number; compared: number; times: number[]; costs: RunCost[] }>()
  for (const compare of compares) {
    // Only decided comparisons, and only columns that answered: the same rule as the picker's "kept 2 of 3".
    if (compare.kept === undefined || hasColumnInstructions(compare)) continue
    for (const column of compare.slots) {
      if (column.missionIds.length === 0) continue
      const key = `${column.route.runtime}:${column.route.model}`
      const row = rows.get(key) ?? { name: nameOf(column.route), kept: 0, compared: 0, times: [], costs: [] }
      row.compared += 1
      if (column.slot === compare.kept.slot) row.kept += 1
      const spent = took(compare, column.slot)
      if (spent?.ms !== undefined && spent.ms > 0) row.times.push(spent.ms)
      if (spent?.cost !== undefined) row.costs.push(spent.cost)
      rows.set(key, row)
    }
  }
  return [...rows.entries()]
    .map(([key, row]): CompareRecordRow => {
      const time = middle(row.times, (ms) => ms)
      const cost = costLine(middle(row.costs, costWeight))
      return { key, name: row.name, kept: row.kept, compared: row.compared, ...(time === undefined ? {} : { time: durationText(time) }), ...(cost === undefined ? {} : { cost }) }
    })
    .sort((left, right) => right.kept / right.compared - left.kept / left.compared || right.compared - left.compared || left.name.localeCompare(right.name))
}
