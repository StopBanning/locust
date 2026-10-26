import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'

import { dollars, limitReached, moneyOfUsage } from '../../shared/spend.js'
import type { Spend } from '../../shared/spend.js'
import { compactCount } from './usageView.js'

/**
 * What a run cost, read off its receipt and never estimated.
 *
 * Every runtime that reports usage puts it on the `run.completed` event, each
 * in its own vocabulary: token counts from Codex, Cursor and OpenCode; a
 * dollar figure from Claude Code; premium-request counts from Copilot. The
 * numbers a person sees here are exactly those, in the runtime's own unit.
 * Where a runtime reports nothing, the line says so instead of showing a
 * zero -- a zero reads as "free", and free is a claim only the runtime can
 * make. OpenCode does make it, with an explicit cost of 0 on its free models.
 */

export interface RunCost {
  readonly inputTokens?: number
  readonly outputTokens?: number
  /** Dollars, only when the runtime itself priced the run. */
  readonly usd?: number
  /** Copilot's own unit: premium requests spent from the plan's allowance. */
  readonly premiumRequests?: number
  /**
   * What the last turn's prompt occupied, cached or not, and how big the
   * model says its window is. Both come from the runtime -- Claude Code's
   * result states `contextWindow` per model -- so a reading exists only
   * where one was reported, and no denominator is ever assumed.
   */
  readonly cacheReadTokens?: number
  readonly cacheWriteTokens?: number
  readonly contextWindow?: number
  /**
   * What the conversation held after the run's last model call. The token
   * counts above are the run's TOTALS, every call added, and a call re-reads
   * the whole conversation -- so they are not a measure of how full it is.
   */
  readonly contextTokens?: number
  /**
   * The run was covered by the person's subscription: Claude Code reported
   * its usage windows and no paid extra usage. Its dollar figure is then
   * what the same tokens would have cost on the API, which nobody paid, and
   * it is not carried here. Claude Code shows subscribers no cost either.
   */
  readonly plan?: true
}

function count(value: unknown): number | undefined {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : undefined
}

/** The cost a run's receipt carries, or undefined when it carries none. */
export function runCostOf(events: readonly NormalizedRuntimeEvent[]): RunCost | undefined {
  const completed = events.find((event) => event.type === 'run.completed')
  if (completed === undefined) return undefined
  const usage = (completed.payload as { readonly usage?: unknown }).usage
  if (typeof usage !== 'object' || usage === null) return undefined
  const record = usage as Record<string, unknown>
  // The host totals money by the same rule (shared/spend.ts), so a card and
  // a receipt can never price one run two ways.
  const { usd, premiumRequests, plan } = moneyOfUsage(record)
  const cost: RunCost = {
    ...(count(record.inputTokens ?? record.input_tokens) === undefined ? {} : { inputTokens: count(record.inputTokens ?? record.input_tokens) }),
    ...(count(record.outputTokens ?? record.output_tokens) === undefined ? {} : { outputTokens: count(record.outputTokens ?? record.output_tokens) }),
    ...(usd === undefined ? {} : { usd }),
    ...(premiumRequests === undefined ? {} : { premiumRequests }),
    ...(count(record.cacheReadTokens) === undefined ? {} : { cacheReadTokens: count(record.cacheReadTokens) }),
    ...(count(record.cacheWriteTokens) === undefined ? {} : { cacheWriteTokens: count(record.cacheWriteTokens) }),
    ...(count(record.contextWindow) === undefined ? {} : { contextWindow: count(record.contextWindow) }),
    ...(count(record.contextTokens) === undefined ? {} : { contextTokens: count(record.contextTokens) }),
    ...(plan ? { plan: true as const } : {})
  }
  return Object.keys(cost).length === 0 ? undefined : cost
}

/**
 * A conversation turn's cost: its receipt when the window holds its events,
 * and otherwise the MONEY the host read from the whole record -- a row sent
 * without its events still says what it cost. Its token counts are the one
 * thing such a row cannot give back, and money is never lost (2026-09-26).
 */
export function missionCost(mission: { readonly events: readonly NormalizedRuntimeEvent[]; readonly money?: Spend }): RunCost | undefined {
  return runCostOf(mission.events) ?? (mission.money === undefined ? undefined : { ...mission.money })
}

/** A receipt's token counts are written as every other count is (usageView). */
const tokens = compactCount

/**
 * One short line, in the runtime's own unit. Dollars win when the runtime
 * priced the run; premium requests are Copilot's unit and are said as such;
 * tokens are shown as in/out. `undefined` means the receipt said nothing.
 */
/**
 * Whether a number is MONEY or merely a measurement.
 *
 * `costLine` falls back to a token count when a runtime reports no price, so
 * the same function returns "$0.04" and "12k in · 3k out" -- and everything
 * downstream called both of them a cost. Sol's beta review of 0.225.0, after
 * four runs on a model the picker labels "Free · no sign-in": Missions said
 * *"across 4 priced"* and the Team card put the token counts under a COST
 * heading. *"No currency charge was displayed, but 'priced' and 'cost'
 * contradict the route's free promise."*
 *
 * Right, and it is a truth problem rather than a wording preference: a person
 * choosing a free route is choosing it to avoid being charged, and the app
 * telling them their free runs were priced is the app appearing to contradict
 * its own first screen.
 *
 * `undefined` for a run that reported nothing stays `undefined`. Nothing here
 * decides a route is free -- it decides only what UNIT was reported, which is
 * the one thing the receipt actually says.
 */
export function costUnit(cost: RunCost | undefined): 'money' | 'plan' | 'usage' | undefined {
  if (cost === undefined) return undefined
  if (cost.usd !== undefined || cost.premiumRequests !== undefined) return 'money'
  // Before tokens: a subscription run reports tokens too, and what a person
  // wants to know about it is that it cost them nothing extra.
  if (cost.plan === true) return 'plan'
  if (cost.inputTokens !== undefined || cost.outputTokens !== undefined) return 'usage'
  return undefined
}

/** What a receipt calls the row: money and a plan both answer "what did it cost". */
export function costLabel(cost: RunCost | undefined): 'Cost' | 'Usage' {
  const unit = costUnit(cost)
  return unit === 'money' || unit === 'plan' ? 'Cost' : 'Usage'
}

/** A subscription run's cost, short enough for a table cell. */
export const COST_IN_PLAN = 'in your plan'

export function costLine(cost: RunCost | undefined): string | undefined {
  if (cost === undefined) return undefined
  if (cost.usd !== undefined) return cost.usd === 0 ? '$0.00' : cost.usd < 0.01 ? '< $0.01' : `$${cost.usd.toFixed(2)}`
  if (cost.premiumRequests !== undefined) {
    return `${String(cost.premiumRequests)} premium request${cost.premiumRequests === 1 ? '' : 's'}`
  }
  if (cost.plan === true) return COST_IN_PLAN
  if (cost.inputTokens !== undefined || cost.outputTokens !== undefined) {
    return `${tokens(cost.inputTokens ?? 0)} in · ${tokens(cost.outputTokens ?? 0)} out`
  }
  return undefined
}

/**
 * A list's total, in one unit, over the runs that reported that unit.
 *
 * "$8.75 across 3 priced" counted every run that reported ANYTHING as priced,
 * so free runs beside priced ones inflated the count. And a subscription's
 * runs are not priced one by one, so a list of only those has no total to
 * state: saying "in your plan across 3" is a sentence about nothing.
 */
export function costTotal(costs: readonly (RunCost | undefined)[]): { readonly line: string; readonly runs: number; readonly word: 'priced' | 'measured' } | undefined {
  const summed = sumCosts(costs)
  const unit = costUnit(summed)
  const line = costLine(summed)
  if (unit === undefined || unit === 'plan' || line === undefined) return undefined
  return {
    line,
    runs: costs.filter((cost) => costUnit(cost) === unit).length,
    word: unit === 'money' ? 'priced' : 'measured'
  }
}

/**
 * What a conversation has cost so far, across every turn that reported one.
 *
 * Cost only existed on a receipt, and a receipt belongs to one mission -- so a
 * five-turn conversation kept its cost in five places and showed it in none of
 * them while any of it was happening. MEASURED 2026-09-03: the moment a person
 * most wants the number is while a run is going, and that was the one moment
 * it was never on screen.
 *
 * Nothing here is estimated. A turn that reported no usage contributes
 * nothing, and a conversation where no turn reported any returns undefined --
 * which the caller must render as silence, not as zero.
 */
export function conversationCost(
  earlierTurns: readonly { readonly events: readonly NormalizedRuntimeEvent[]; readonly money?: Spend }[],
  events: readonly NormalizedRuntimeEvent[]
): RunCost | undefined {
  return sumCosts([...earlierTurns.map((turn) => missionCost(turn)), runCostOf(events)])
}

/** Sum what can be summed; a mixed list keeps every unit it saw. */
export function sumCosts(costs: readonly (RunCost | undefined)[]): RunCost | undefined {
  let total: RunCost | undefined
  for (const cost of costs) {
    if (cost === undefined) continue
    const held: RunCost = total ?? {}
    total = {
      ...held,
      ...(cost.inputTokens === undefined ? {} : { inputTokens: (held.inputTokens ?? 0) + cost.inputTokens }),
      ...(cost.outputTokens === undefined ? {} : { outputTokens: (held.outputTokens ?? 0) + cost.outputTokens }),
      ...(cost.usd === undefined ? {} : { usd: (held.usd ?? 0) + cost.usd }),
      ...(cost.premiumRequests === undefined ? {} : { premiumRequests: (held.premiumRequests ?? 0) + cost.premiumRequests }),
      ...(cost.plan === true ? { plan: true as const } : {})
    }
  }
  return total
}

/**
 * How full the model's context is, when the runtime said how big it is.
 *
 * The occupied part is what the conversation held after the run's last model
 * call: that call's whole prompt -- written fresh, written to cache, read back
 * from it -- and what it wrote. The adapter measures it as `contextTokens`.
 *
 * NOT the run's token totals. This read them, on a measurement of a one-call
 * run where the two are the same number; a run with tools makes a call per
 * step and every call re-reads the conversation, so the totals count it once
 * per call. Colin's ring read "5M of 1M" on a 23-call run that never held
 * more than 240k (2026-09-23). A receipt written before the adapter measured
 * the last call cannot say what it held, and gets no reading.
 *
 * Returns nothing at all where no window was reported. Every other runtime
 * is in that position today, and a ring drawn against a guessed denominator
 * would be a number the app made up about the person's own quota.
 */
export interface ContextReading {
  readonly usedTokens: number
  readonly windowTokens: number
  /** 0-100, rounded, and never above 100. */
  readonly percent: number
}

/**
 * The newest turn that reported a context reading.
 *
 * NOT the conversation's summed cost: tokens add up across turns, but the
 * window holds one prompt. Summing them would report a five-turn chat as
 * five times as full as it is. So this walks back from the latest turn and
 * takes the first reading it finds -- the live turn once it has settled,
 * otherwise the one before it.
 */
export function latestContext(
  earlierTurns: readonly { readonly events: readonly NormalizedRuntimeEvent[] }[],
  events: readonly NormalizedRuntimeEvent[]
): ContextReading | undefined {
  const newestFirst = [events, ...[...earlierTurns].reverse().map((turn) => turn.events)]
  for (const turn of newestFirst) {
    const reading = contextReading(runCostOf(turn))
    if (reading !== undefined) return reading
    // A compaction since, with no reading after it (0.797): an older reading
    // describes a conversation that is no longer there. A `/compact` turn
    // makes no model call of its own to read, so the ring waits for the next.
    if (compactedIn(turn)) return undefined
  }
  return undefined
}

/** Whether a turn's runtime summarized the conversation, asked or on its own. */
function compactedIn(events: readonly NormalizedRuntimeEvent[]): boolean {
  return events.some((event) => event.type === 'adapter.diagnostic' && event.payload.code.endsWith('.context_compacted'))
}

export function contextReading(cost: RunCost | undefined): ContextReading | undefined {
  const windowTokens = cost?.contextWindow
  if (cost === undefined || windowTokens === undefined || windowTokens <= 0) return undefined
  const usedTokens = cost.contextTokens
  if (usedTokens === undefined || usedTokens <= 0) return undefined
  return {
    usedTokens,
    windowTokens,
    percent: Math.min(100, Math.round((usedTokens / windowTokens) * 100))
  }
}

/** The reading in the words the tooltip uses. */
export function contextSentence(reading: ContextReading): string {
  return `Context: ${tokens(reading.usedTokens)} of ${tokens(reading.windowTokens)} used, ${String(reading.percent)}%`
}

/**
 * What to say when the runtime reported no cost at all.
 *
 * Four surfaces each answered this themselves and each answered differently:
 * the Missions row drew `—`, the Team card said `not reported`, the thread's
 * receipt and the inspector said `not reported by the runtime`, and the
 * inspector alone knew to say something else while a run was still going.
 * One absence, four spellings, three of which are in view at the same time
 * (Grok's finding 6, 2026-09-13).
 *
 * `running` is the one real distinction and it is kept: a run that has not
 * finished has not reported a cost YET, which is a different fact from a
 * runtime that does not report costs. Everything else is the same sentence.
 */
export function costLineOrWhyNot(cost: RunCost | undefined, running = false): string {
  return costLine(cost) ?? (running ? 'reported when the run ends' : 'not reported by the runtime')
}

/**
 * The same fact where only a few characters fit -- a table cell, a row's
 * trailing column. It claims nothing about the price, which is the thing a
 * `$0.00` would claim.
 *
 * It was an em dash, and a dash on its own read as a column nobody filled
 * in (Yurt's beta report, 2026-09-23, #14: 'a bare "—" column'). Two words
 * say what the dash meant, in the width "in your plan" already takes.
 */
export const COST_NOT_REPORTED_SHORT = 'not reported'

/**
 * A row's cost cell: what the run reported, else why there is nothing.
 *
 * A model whose own id says it is free (OpenCode's `...-free` models, the
 * ones the route chip tags Free) reads "free" rather than "not reported":
 * the price is known, it is nothing, and the runtime simply does not send a
 * number for it.
 */
export function costCell(cost: RunCost | undefined, model: string): string {
  return costLine(cost) ?? (/-free$/i.test(model) ? 'free' : COST_NOT_REPORTED_SHORT)
}

/**
 * The same cell on a list read at a glance (the conversation list, 0.723): what a person PAYS -- money or
 * Copilot's premium requests -- or "free" for a free model, else nothing. "9.0k in · 870 out" on every row was
 * an engineer's unit, as it was on the header (headerCostTail); the counts stay in Details.
 */
export function costGlanceCell(cost: RunCost | undefined, model: string): string {
  return moneyLine(cost) ?? (/-free$/i.test(model) ? 'free' : '')
}

/**
 * The cost part of the mission header line.
 *
 * Every other part of that line is a fact about ONE mission -- its id, its
 * model, its phase, its sandbox -- and the cost used to be the whole
 * conversation's. Grok measured what that does (2026-09-14, finding 2): a
 * mission cancelled before anything started, whose own ledger holds a
 * create, a cancel and no usage at all, wore "1.8k in · 189 out" -- the
 * previous completed turn's exact counts. A run that died on a provider
 * error wore the turn before IT. A stranger reads a number sitting beside a
 * mission id as that mission's, and it was not.
 *
 * So: this run's own cost, unlabelled because the line it joins is already
 * about this run; nothing at all when this run reported no number, which is
 * the honest answer for a run that never reached a model.
 *
 * The conversation's total is NOT here. It was, briefly, with the word
 * `conversation` on it -- and that is what pushed a seven-fact line past its
 * width and truncated on Colin's screen. Design ruling, 2026-09-14: it
 * belongs in the context ring, which is already the conversation-scoped
 * object drawing a conversation-scoped budget, because cost and context are
 * the same KIND of fact -- how much of a finite thing this conversation has
 * spent. See `conversationCostLine`.
 */
/**
 * What the conversation's header says a run cost: MONEY, or nothing.
 *
 * The header is the first line a person reads, and it said "9.0k in . 240
 * out" and "in your plan" under every conversation -- token counts are an
 * engineer's unit, and a subscription run cost nothing extra (Colin,
 * 2026-09-23: a conversation cost "is silly for a subscription plan").
 * Claude's header says neither. Dollars and Copilot's premium requests are
 * what a person pays, so those stay; the rest is one hover away on the
 * context ring and in Details (first-impressions pass, after 0.349).
 */
export function headerCostTail(input: {
  readonly events: readonly NormalizedRuntimeEvent[]
  readonly running: boolean
}): string {
  const line = moneyLine(runCostOf(input.events))
  return line === undefined ? '' : ' · ' + (input.running ? 'so far ' : '') + line
}

/**
 * A cost for the surfaces read AT A GLANCE -- the conversation's header, the
 * relay's strip, a teammate's card: money, or nothing. Token counts and "in
 * your plan" stay where a person goes for detail (Details, the Activity
 * panel, the context ring's hover), not on every surface they pass
 * (first-impressions pass, after 0.349).
 */
export function moneyLine(cost: RunCost | undefined): string | undefined {
  return costUnit(cost) === 'money' ? costLine(cost) : undefined
}

/** Why some runs carry no price, said where they are counted. */
export const UNPRICED_NOTE = 'A model of your own reports no price, so its runs do not count toward the limit. Check its provider for what they cost.'

/**
 * A teammate's month, for their card and their dialog: what they spent, and
 * -- when the person set a limit -- against what. Money, or nothing, as
 * every glance surface says it; with a limit there is always a line, because
 * "$0.00 of $5.00" is a fact the person asked to see. Premium requests are
 * said beside the dollars, never counted toward a dollar limit.
 */
export function monthSpendLine(
  spend: Spend | undefined,
  limitUsd: number | undefined
): { readonly text: string; readonly reached: boolean; readonly unpriced?: string; readonly note?: string } | undefined {
  if (limitUsd === undefined) {
    const line = moneyLine(spend)
    return line === undefined ? undefined : { text: line, reached: false }
  }
  const requests = spend?.premiumRequests === undefined ? '' : ` · ${costLine({ premiumRequests: spend.premiumRequests })!}`
  // Reached is said by the row's LABEL, not tacked onto the amount: on a
  // card three across, "$0.02 of $0.01 . limit reached" broke over two lines
  // (the 0.353 drive's Team screen).
  // A model of the person's own reports no price (0.689): its runs are said
  // under the dollars, so "$0.00 of $5.00" is never read as "nothing spent".
  // Their own line, not tacked on: tacked on, it broke the amount over two
  // lines on a card three across (the 0.689 packaged drive's Team screen).
  const unpriced = spend?.unpricedRuns ?? 0
  return {
    text: `${dollars(spend?.usd ?? 0)} of ${dollars(limitUsd)}${requests}`,
    reached: limitReached(spend, limitUsd),
    ...(unpriced === 0 ? {} : { unpriced: `${String(unpriced)} ${unpriced === 1 ? 'run' : 'runs'} with no price`, note: UNPRICED_NOTE })
  }
}

export function missionCostTail(input: {
  readonly events: readonly NormalizedRuntimeEvent[]
  readonly earlierTurns: readonly { readonly events: readonly NormalizedRuntimeEvent[] }[]
  readonly running: boolean
}): string {
  const run = costLine(runCostOf(input.events))
  return run === undefined ? '' : ' · ' + (input.running ? 'so far ' : '') + run
}

/**
 * The conversation's total, for the one surface that is conversation-scoped.
 *
 * The context ring already answers "how much of a finite thing has this
 * conversation spent"; cost is the same question with a different unit, so
 * it is one row inside a hover that already exists rather than a new surface
 * anybody has to find. Design ruling, 2026-09-14.
 */
export function conversationCostLine(
  earlierTurns: readonly { readonly events: readonly NormalizedRuntimeEvent[] }[],
  events: readonly NormalizedRuntimeEvent[]
): string | undefined {
  const whole = conversationCost(earlierTurns, events)
  // A conversation on a subscription has spent nothing a person pays per
  // turn. Colin, 2026-09-23: "it has a conversation cost, which is silly for
  // a subscription plan". The hover says how full the context is, and that
  // is all it has to say.
  if (costUnit(whole) === 'plan') return undefined
  return costLine(whole)
}
