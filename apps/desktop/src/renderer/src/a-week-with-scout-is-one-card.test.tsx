import { createElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'

import { seedAvatar } from '../../shared/avatar.js'
import { DEFAULT_SCOUT_SETTINGS, cardOf, emptyWeek, weekFact, type ScoutState } from '../../shared/scout.js'
import { ScoutCards } from './components/ScoutCards.js'

/*
 * A WEEK WITH SCOUT IS ONE CARD (0.817). The weekly tune-up wears its own name, says the week in Locust's counts, and
 * offers its one suggestion as the card's button; a week with nothing to change has no button but Dismiss.
 */
const scout = { teammateId: 'tm_scout', name: 'Scout', hue: 'teal', role: 'Custom', roleTitle: 'Keeps watch', avatar: seedAvatar('tm_scout'), createdAt: '2026-10-12' } as const
const draw = (fact: ReturnType<typeof weekFact>, onTune = vi.fn()): string => {
  const state: ScoutState = { settings: { ...DEFAULT_SCOUT_SETTINGS, on: true, teammateId: 'tm_scout' }, cards: [cardOf(fact, 'card_week', new Date('2026-10-29T13:00:00.000Z'), false)], lookIds: [] }
  return renderToStaticMarkup(createElement(ScoutCards, { state, scout, onOpen: vi.fn(), onSend: vi.fn(), onDraftIntoBox: vi.fn(), onDismiss: vi.fn(), onTune, now: new Date('2026-10-29T13:05:00.000Z') }))
}

describe('a week with Scout', () => {
  it('is one card, its lines under its sentence, its one suggestion as its button', () => {
    const week = { ...emptyWeek('2026-10-22'), looks: 410, asked: 6, dismissed: 4, watches: { ...emptyWeek('2026-10-22').watches, github: { raised: 4, used: 0, dismissed: 4 } } }
    const html = draw(weekFact(week, DEFAULT_SCOUT_SETTINGS))
    expect(html).toContain('>Your week<')
    expect(html).toContain('This week: 410 looks, 6 of them read by its model.')
    expect(html).toContain('<ul class="lc-scoutcard__lines"><li>It raised 4 cards. You dismissed 4.</li><li>You dismissed all 4 GitHub cards and sent none on.</li></ul>')
    expect(html).toContain('<button type="button" class="lc-primarybutton">Stop watching GitHub</button>')
  })

  it('has no button but Dismiss when there is nothing to change', () => {
    const week = { ...emptyWeek('2026-10-22'), looks: 40, sent: 2, watches: { ...emptyWeek('2026-10-22').watches, teammates: { raised: 2, used: 2, dismissed: 0 } } }
    const html = draw(weekFact(week, { ...DEFAULT_SCOUT_SETTINGS, everyMinutes: 30 }))
    expect(html).toContain('<li>It raised 2 cards. You sent 2 on.</li>')
    expect(html).not.toContain('lc-primarybutton')
    expect(html).toContain('>Dismiss</button>')
  })
})
