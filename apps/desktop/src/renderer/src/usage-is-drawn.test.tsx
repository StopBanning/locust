import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { usageSummary } from '../../shared/usage.js'
import type { UsageTurn } from '../../shared/usage.js'
import { Heatmap } from './components/UsageDialog.js'
import { UsageChip, UsageLine } from './components/UsageLine.js'
import { compactCount, heatColumns, heatLevel, paidBy, sparkBars, spentSentence, usageLineFacts } from './usageView.js'

/*
 * USAGE IS DRAWN (0.714): the line under the accounts on Home and the Usage
 * dialog -- Claude Code's /stats and /usage for every agent. Colin: "make
 * sure its very well designed and not sloppy", so the words and the shapes
 * are held here, and look-usage.mjs draws them at four window sizes.
 */

const now = new Date(2026, 9, 9, 18, 0, 0)
const at = (back: number): string => new Date(2026, 9, 9 - back, 12, 0, 0).toISOString()
const turn = (fields: Partial<UsageTurn> & Pick<UsageTurn, 'at'>): UsageTurn => ({ runtime: 'claude', model: 'claude-opus-4-6', conversation: `c${fields.at}`, ...fields })

describe('the line on Home', () => {
  it('says the month in one line: turns, tokens, and money only when something was priced', () => {
    const planned = usageSummary([turn({ at: at(0), plan: true, inputTokens: 100, cacheReadTokens: 1_200_000, outputTokens: 5_000 })], '30d', now)
    expect(usageLineFacts(planned)).toBe('1 turn · 1.2M tokens')
    const priced = usageSummary([turn({ at: at(0), usd: 0.31, inputTokens: 900, outputTokens: 100 }), turn({ at: at(1), usd: 0.2 })], '30d', now)
    expect(usageLineFacts(priced)).toBe('2 turns · 1.0k tokens · $0.51 spent')
  })

  it('a bar a day, the busiest full height, a quiet day flat, today marked', () => {
    const summary = usageSummary([turn({ at: at(0) }), turn({ at: at(0) }), turn({ at: at(2) })], '30d', now)
    const bars = sparkBars(summary.days, summary.today)
    expect(bars).toHaveLength(30)
    expect(bars.at(-1)).toMatchObject({ today: true, height: 1, turns: 2 })
    expect(bars.at(-3)?.height).toBe(0.5)
    expect(bars.at(-2)?.height).toBe(0)
  })

  it('is not drawn before there is anything to count: a new person’s Home is unchanged', () => {
    const empty = usageSummary([], '30d', now)
    expect(renderToStaticMarkup(<UsageLine summary={empty} onOpen={() => undefined} />)).toBe('')
    expect(renderToStaticMarkup(<UsageChip summary={empty} onOpen={() => undefined} />)).toBe('')
    expect(renderToStaticMarkup(<UsageLine summary={undefined} onOpen={() => undefined} />)).toBe('')
  })

  it('draws the label, the facts, thirty bars and the way in', () => {
    const summary = usageSummary([turn({ at: at(0), inputTokens: 5_000, outputTokens: 500 })], '30d', now)
    const html = renderToStaticMarkup(<UsageLine summary={summary} onOpen={() => undefined} />)
    expect(html).toContain('Usage')
    expect(html).toContain('last 30 days')
    expect(html).toContain('1 turn · 5.5k tokens')
    expect(html.match(/lc-usageline__bar/g) ?? []).toHaveLength(30)
    expect(html).toContain('Details')
  })
})

describe('the dialog', () => {
  it('names how each model was paid for, in the receipts’ words', () => {
    const base = { turns: 4, planTurns: 0, freeTurns: 0 }
    expect(paidBy({ ...base, runtime: 'claude', planTurns: 4 })).toBe('your plan')
    expect(paidBy({ ...base, runtime: 'claude', usd: 0.37 })).toBe('$0.37')
    expect(paidBy({ ...base, runtime: 'opencode', freeTurns: 4 })).toBe('free')
    expect(paidBy({ ...base, runtime: 'copilot', premiumRequests: 3 })).toBe('3 premium requests')
    // Codex runs on the account it is signed in to and prices nothing per turn.
    expect(paidBy({ ...base, runtime: 'codex' })).toBe('your account')
    expect(paidBy({ ...base, runtime: 'cursor-unknown' })).toBe('not reported')
    expect(paidBy({ ...base, runtime: 'claude', planTurns: 3, usd: 0.02 })).toBe('$0.02 · 3 in your plan')
  })

  it('says what was spent, or that nothing was beyond the plans', () => {
    expect(spentSentence(usageSummary([turn({ at: at(0), plan: true })], '30d', now))).toBe('Nothing spent beyond your plans and free models.')
    expect(spentSentence(usageSummary([turn({ at: at(0), usd: 1.5 }), turn({ at: at(0), runtime: 'copilot', model: 'auto', premiumRequests: 2 })], '30d', now))).toBe('Spent $1.50 on API keys and paid models and 2 Copilot premium requests.')
  })

  it('a heatmap cell is one of five shades, by steps of the busiest day', () => {
    expect([0, 1, 3, 6, 8, 10].map((turns) => heatLevel(turns, 10))).toEqual([0, 1, 2, 3, 4, 4])
    expect(heatLevel(0, 0)).toBe(0)
  })

  it('names a month over the week it starts in, and never two names run together', () => {
    const summary = usageSummary([turn({ at: at(0) })], 'all', now)
    const columns = heatColumns(summary.heatmap)
    expect(columns).toHaveLength(12)
    const named = columns.flatMap((column, index) => (column.month === undefined ? [] : [{ index, month: column.month }]))
    expect(named.map((entry) => entry.month)).toEqual(expect.arrayContaining(['Aug', 'Sep', 'Oct']))
    for (let index = 1; index < named.length; index += 1) expect(named[index]!.index - named[index - 1]!.index).toBeGreaterThanOrEqual(3)
  })

  it('the grid: twelve weeks of cells, today ringed, the days to come left out', () => {
    const summary = usageSummary([turn({ at: at(0) }), turn({ at: at(1) })], '30d', now)
    const html = renderToStaticMarkup(<Heatmap summary={summary} />)
    expect(html.match(/lc-heat__cell is-[0-4]/g)?.length).toBe(summary.heatmap.length + 5)
    expect(html.match(/is-today/g) ?? []).toHaveLength(1)
    expect(html.match(/is-future/g) ?? []).toHaveLength(7 - (now.getDay() + 1))
  })

  it('writes a token count the way the app does everywhere', () => {
    expect([920, 12_400, 3_100_000, 19_000_000].map(compactCount)).toEqual(['920', '12k', '3.1M', '19M'])
  })

  it('writes a billion and more as B, never as a thousand of the unit below', () => {
    // Colin's all-time Usage, 2026-10-26: 1,133,000,000 tokens read "1133M".
    expect([1_133_000_000, 12_500_000_000, 999_600_000, 999_999, 9_960_000].map(compactCount))
      .toEqual(['1.1B', '13B', '1.0B', '1.0M', '10M'])
  })
})
