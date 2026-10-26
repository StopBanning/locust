import { dollars } from '../../shared/spend.js'
import { cachedPercent, totalTokens } from '../../shared/usage.js'
import type { UsageDay, UsageModel, UsagePay, UsageRange, UsageSummary } from '../../shared/usage.js'

/**
 * USAGE, IN WORDS AND SHAPES (0.714): what the Home line and the Usage dialog
 * say about a summary the host added up (shared/usage.ts). Pure, so every
 * word is tested without drawing anything (usage-is-counted.test.ts).
 */

const COUNT_UNITS = [[1_000_000_000, 'B'], [1_000_000, 'M'], [1_000, 'k']] as const

/**
 * 1.1B, 3.1M, 410k, 920: how the app writes a token count everywhere.
 * BILLIONS (2026-10-26): Colin's all-time Usage read "1133M"; past a billion it
 * is 1.1B. A count that would round to 1000 of one unit is written as the next
 * (999.6M is 1.0B, never "1000M"), and one that rounds to 10 drops its decimal.
 */
export function compactCount(value: number): string {
  for (const [index, [size, unit]] of COUNT_UNITS.entries()) {
    if (value < size) continue
    const scaled = value / size
    const text = scaled >= 9.95 ? scaled.toFixed(0) : scaled.toFixed(1)
    const larger = COUNT_UNITS[index - 1]
    return text === '1000' && larger !== undefined ? `1.0${larger[1]}` : `${text}${unit}`
  }
  return String(Math.round(value))
}

export function plural(count: number, one: string, many = `${one}s`): string {
  return `${count.toLocaleString('en-US')} ${count === 1 ? one : many}`
}

/** "Oct 3", from `2026-10-03`. */
export function dayLabel(date: string): string {
  const [year, month, day] = date.split('-').map(Number)
  return new Date(year!, month! - 1, day!).toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
}

export const RANGE_NAMES: Readonly<Record<UsageRange, string>> = { '7d': '7 days', '30d': '30 days', all: 'All time' }

/** "the last 30 days", "all time": the range in a sentence. */
export function rangePhrase(range: UsageRange): string {
  return range === 'all' ? 'all time' : `the last ${RANGE_NAMES[range]}`
}

/**
 * The Home line's facts: "142 turns · 3.1M tokens · $0.31 spent". Tokens only
 * when some turn reported them; money only when a runtime priced something.
 */
export function usageLineFacts(summary: UsageSummary): string {
  const parts = [plural(summary.turns, 'turn')]
  const tokens = totalTokens(summary.tokens)
  if (tokens > 0) parts.push(`${compactCount(tokens)} tokens`)
  if (summary.pay.usd !== undefined && summary.pay.usd > 0) parts.push(`${dollars(summary.pay.usd)} spent`)
  return parts.join(' · ')
}

export interface SparkBar {
  readonly date: string
  readonly turns: number
  /** 0-1 of the busiest day; a day with any turn is never drawn as nothing. */
  readonly height: number
  readonly today: boolean
}

/** One bar a day for the Home line, oldest first. */
export function sparkBars(days: readonly UsageDay[], today: string): readonly SparkBar[] {
  const most = Math.max(1, ...days.map((day) => day.turns))
  return days.map((day) => ({ date: day.date, turns: day.turns, today: day.date === today, height: day.turns === 0 ? 0 : Math.max(0.16, day.turns / most) }))
}

/**
 * A heatmap cell's shade, 0-4, as GitHub and Claude Code's `/stats` draw
 * them: nothing, then four steps up to the busiest day -- steps of the
 * busiest day rather than fixed counts, so a light quarter still has a shape.
 */
export function heatLevel(turns: number, busiest: number): 0 | 1 | 2 | 3 | 4 {
  if (turns <= 0 || busiest <= 0) return 0
  const share = turns / busiest
  return share > 0.75 ? 4 : share > 0.5 ? 3 : share > 0.25 ? 2 : 1
}

export interface HeatColumn {
  /** Sunday to Saturday; the days after today are absent. */
  readonly days: readonly (UsageDay | undefined)[]
  /** The month named over this column: the one whose first day it holds. */
  readonly month?: string
}

/** How many columns a month's name needs to itself: "Sep" is wider than one week. */
const MONTH_COLUMNS = 3

/**
 * The heatmap's weeks, oldest first, each named by the month that starts in
 * it -- and the first column by its own month, so the grid never opens
 * unnamed. A name is left out where it would run into the one before it: the
 * first draft wrote "JuAug" over the first two weeks.
 */
export function heatColumns(heatmap: readonly UsageDay[]): readonly HeatColumn[] {
  const weeks: { days: (UsageDay | undefined)[]; opening?: UsageDay }[] = []
  for (let start = 0; start < heatmap.length; start += 7) {
    const days: (UsageDay | undefined)[] = heatmap.slice(start, start + 7)
    while (days.length < 7) days.push(undefined)
    const opening = days.find((day) => day !== undefined && day.date.endsWith('-01'))
    weeks.push({ days, ...(opening === undefined ? {} : { opening }) })
  }
  const monthOf = (day: UsageDay): string => new Date(`${day.date}T12:00:00`).toLocaleDateString('en-US', { month: 'short' })
  let lastNamed = -MONTH_COLUMNS
  return weeks.map((week, index) => {
    const opening = week.opening
    const first = week.days[0]
    // The first column's own month, unless another month starts too soon after it.
    const nextOpening = weeks.findIndex((later, at) => at > index && later.opening !== undefined)
    const named =
      opening ??
      (index === 0 && first !== undefined && (nextOpening === -1 || nextOpening >= MONTH_COLUMNS) ? first : undefined)
    if (named === undefined || index - lastNamed < MONTH_COLUMNS) return { days: week.days }
    lastNamed = index
    return { days: week.days, month: monthOf(named) }
  })
}

/** Runtimes that run on the account they are signed in to and price nothing per turn. */
const ACCOUNT_RUNTIMES: ReadonlySet<string> = new Set(['codex', 'cursor', 'gemini', 'antigravity', 'muse'])

/**
 * How a model's turns were paid for, in the receipts' words: dollars a runtime
 * priced, Copilot's premium requests, a subscription's turns, a free model's.
 * A runtime that runs on the person's own account and prices nothing says so,
 * rather than "not reported" -- the account is what paid.
 */
export function paidBy(model: Pick<UsageModel, 'runtime' | 'turns'> & UsagePay): string {
  const parts: string[] = []
  if (model.usd !== undefined && model.usd > 0) parts.push(dollars(model.usd))
  if (model.premiumRequests !== undefined && model.premiumRequests > 0) parts.push(plural(model.premiumRequests, 'premium request'))
  if (model.planTurns > 0) parts.push(model.planTurns === model.turns ? 'your plan' : `${String(model.planTurns)} in your plan`)
  if (model.freeTurns > 0) parts.push(model.freeTurns === model.turns ? 'free' : `${String(model.freeTurns)} free`)
  if (parts.length > 0) return parts.join(' · ')
  return ACCOUNT_RUNTIMES.has(model.runtime) ? 'your account' : 'not reported'
}

/** "71%" of what the model read came from its cache; a dash where it read nothing. */
export function cachedText(model: Pick<UsageModel, 'tokens'>): string {
  const percent = cachedPercent(model.tokens)
  return percent === undefined ? '—' : `${String(percent)}%`
}

/** A model's share of the range's turns, 0-100, rounded; never 0 for a model that ran. */
export function shareOf(model: Pick<UsageModel, 'turns'>, summary: Pick<UsageSummary, 'turns'>): number {
  if (summary.turns === 0 || model.turns === 0) return 0
  return Math.max(1, Math.round((model.turns / summary.turns) * 100))
}

/** What the dialog's foot says about money in the range. */
export function spentSentence(summary: UsageSummary): string {
  const { usd, premiumRequests } = summary.pay
  const parts: string[] = []
  if (usd !== undefined && usd > 0) parts.push(`${dollars(usd)} on API keys and paid models`)
  if (premiumRequests !== undefined && premiumRequests > 0) parts.push(plural(premiumRequests, 'Copilot premium request'))
  return parts.length === 0 ? 'Nothing spent beyond your plans and free models.' : `Spent ${parts.join(' and ')}.`
}

/** "6 days", "1 day", "none": a streak's length. */
export function streakText(days: number): string {
  return days === 0 ? 'none' : plural(days, 'day')
}
