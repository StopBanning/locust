import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { seedAvatar } from '../../shared/avatar.js'
import type { PublicRecoveredMission, PublicTeammate } from '../../shared/ipc.js'
import { NewTeammateDialog, limitHint, limitSaid, parsedLimit } from './components/NewTeammateDialog.js'
import { TeammatesScreen } from './components/Screens.js'
import { missionCost, monthSpendLine, UNPRICED_NOTE } from './cost.js'

/**
 * WHAT A TEAMMATE SPENT THIS MONTH, AND THE LIMIT THE PERSON SET (0.353).
 */

const nothing = (): void => undefined
const wren: PublicTeammate = {
  teammateId: 'tm_wren',
  name: 'Wren',
  hue: 'lime',
  role: 'Code & Migrations',
  avatar: seedAvatar('tm_wren'),
  createdAt: '2026-09-01T00:00:00.000Z'
}

const card = (teammate: PublicTeammate, spend: Record<string, { usd?: number; premiumRequests?: number; unpricedRuns?: number }>): string =>
  renderToStaticMarkup(
    <TeammatesScreen teammates={[teammate]} missions={[]} missionOwners={{}} spendByTeammate={spend} viewByTeammate={{}}
      titleOf={() => ''} onOpenMission={nothing} onNewTeammate={nothing} onEdit={nothing} onRemove={nothing} onMessage={nothing}
      routines={[]} routineStepByTeammate={{}} onRunRoutine={nothing} onEditRoutine={nothing} onRemoveRoutine={nothing} />
  )

describe("a teammate's month, on their card", () => {
  it('is money, or no row -- and "This month", not a total of whatever the window held', () => {
    expect(card(wren, { tm_wren: { usd: 3.2 } })).toContain('This month')
    expect(card(wren, { tm_wren: { usd: 3.2 } })).toContain('$3.20')
    expect(card(wren, {})).not.toContain('This month')
  })

  it('says it against the limit, whenever there is one -- even at nothing spent', () => {
    const limited = { ...wren, monthlyLimitUsd: 5 }
    expect(card(limited, {})).toContain('$0.00 of $5.00')
    // Reached is the row's label, so the amount stays one short line.
    expect(card(limited, { tm_wren: { usd: 5.02 } })).toContain('<dt>Limit reached</dt><dd class="lc-mono">$5.02 of $5.00</dd>')
    expect(card(limited, { tm_wren: { usd: 5.02 } })).toContain('lc-rostercard__cost is-reached')
    expect(card(limited, { tm_wren: { usd: 1 } })).toContain('<dt>This month</dt><dd class="lc-mono">$1.00 of $5.00</dd>')
    expect(card(limited, { tm_wren: { usd: 1 } })).not.toContain('is-reached')
  })

  it('says runs on a model of your own beside the dollars, with why: unpriced, not $0.00 (0.689)', () => {
    expect(monthSpendLine({ usd: 0.4, unpricedRuns: 2 }, 5)).toEqual({ text: '$0.40 of $5.00', reached: false, unpriced: '2 runs with no price', note: UNPRICED_NOTE })
    expect(monthSpendLine({ unpricedRuns: 1 }, 5)).toEqual({ text: '$0.00 of $5.00', reached: false, unpriced: '1 run with no price', note: UNPRICED_NOTE })
    // Its own line, under the amount, so the amount stays one short line.
    expect(card({ ...wren, monthlyLimitUsd: 5 }, { tm_wren: { unpricedRuns: 1 } }))
      .toContain(`<dd class="lc-mono">$0.00 of $5.00</dd><dd class="lc-rostercard__unpriced" title="${UNPRICED_NOTE}">1 run with no price</dd>`)
    // Without a limit there is nothing to read it against: the card stays as it was.
    expect(monthSpendLine({ unpricedRuns: 3 }, undefined)).toBeUndefined()
  })

  it('says premium requests beside the dollars, never toward them', () => {
    expect(monthSpendLine({ usd: 1, premiumRequests: 3 }, 5)).toEqual({ text: '$1.00 of $5.00 · 3 premium requests', reached: false })
    expect(monthSpendLine({ premiumRequests: 3 }, undefined)).toEqual({ text: '3 premium requests', reached: false })
  })
})

describe('the limit field', () => {
  it('reads amounts as a person types them, and nothing as no limit', () => {
    expect(parsedLimit('')).toBeUndefined()
    expect(parsedLimit('  ')).toBeUndefined()
    expect(parsedLimit('5')).toBe(5)
    expect(parsedLimit('$12.50')).toBe(12.5)
    expect(parsedLimit('12.')).toBe(12)
    expect(parsedLimit('1,000')).toBe(1000)
    expect(parsedLimit('.5')).toBe(0.5)
    expect(parsedLimit('12.345')).toBe(12.35)
  })

  it('says a limit on an edit only when it changed, so an edit that leaves it can be saved from a browser (0.834)', () => {
    expect(limitSaid(5, true, 5)).toEqual({})
    expect(limitSaid(undefined, true, undefined)).toEqual({})
    expect(limitSaid(8, true, 5)).toEqual({ monthlyLimitUsd: 8 })
    expect(limitSaid(undefined, true, 5)).toEqual({ monthlyLimitUsd: null })
    expect(limitSaid(5, false, undefined)).toEqual({ monthlyLimitUsd: 5 })
    expect(limitSaid(undefined, false, undefined)).toEqual({})
  })

  it('refuses what is not an amount, rather than saving a limit nobody meant', () => {
    for (const bad of ['0', '-5', 'five', '1e3', '5$', '100001']) expect(parsedLimit(bad)).toBe('invalid')
  })

  it('says how the limit works, and what has been spent when editing', () => {
    expect(limitHint('Wren', undefined, false)).toContain('Checked before each run')
    expect(limitHint('Wren', undefined, false)).toContain('Plans and free models are not priced')
    expect(limitHint('Wren', { usd: 3.2 }, true).startsWith('$3.20 spent this month so far.')).toBe(true)
  })

  it('is in the dialog, filled with the limit already set', () => {
    const html = renderToStaticMarkup(
      <NewTeammateDialog onCancel={nothing} onCreate={nothing} error={undefined} initial={{ ...wren, monthlyLimitUsd: 5 }} mode="accept-edits" spentThisMonth={{ usd: 3.2 }} />
    )
    expect(html).toContain('Monthly limit')
    expect(html).toContain('value="5.00"')
    expect(html).toContain('$3.20 spent this month so far.')
  })
})

describe('a row sent without its events', () => {
  it('still says what it cost, from the money the host read', () => {
    const row = { events: [], money: { usd: 0.75 } } as unknown as PublicRecoveredMission
    expect(missionCost(row)).toEqual({ usd: 0.75 })
    expect(missionCost({ events: [] })).toBeUndefined()
  })
})
