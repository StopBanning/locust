import { describe, expect, it, vi } from 'vitest'

import { PHONE_MAX_SCOUT_CARDS } from '../shared/phone.js'
import type { ScoutCard } from '../shared/scout.js'
import { answerPhone, type PhoneCalls } from './phone-host.js'
import { phonePage } from './phone-page.js'
import { phoneScoutCards } from './phone-scout-cards.js'

/**
 * SCOUT'S CARDS ARE ON YOUR PHONE (0.829). A Scout alert's tap opened a page with no Scout on it. The phone now lists
 * Scout's cards in Locust's own words, newest first, with a link when one is about GitHub and Dismiss; what a card
 * quotes, Scout's model's take and a teammate's draft stay in the window.
 */
const card = (n: number, extra: Partial<ScoutCard> = {}): ScoutCard => ({
  cardId: `card_${String(n).padStart(12, '0')}`,
  key: `issue:${String(n)}`,
  urgency: 'soon',
  said: `Card ${String(n)}.`,
  at: new Date(Date.UTC(2026, 9, 23, n)).toISOString(),
  ...extra
})

describe('Scout’s cards are on your phone', () => {
  it('in Locust’s words, newest first, with a GitHub link only, and nothing the window keeps', () => {
    const shown = phoneScoutCards([
      card(1, { urgency: 'now', quote: 'rm -rf build', take: 'The model thinks…', draft: { teammateId: 'tm_ada', name: 'Ada', message: 'Fix it' }, open: { kind: 'url', url: 'https://github.com/acme/app/pull/7', label: 'Open' } }),
      card(3, { open: { kind: 'url', url: 'https://evil.example/x', label: 'Open' }, lines: ['One line.'] }),
      card(2, { open: { kind: 'conversation', missionId: 'm1', label: 'Open it' } })
    ])
    expect(shown).toEqual([
      { cardId: card(3).cardId, said: 'Card 3.', lines: ['One line.'], now: false, at: card(3).at },
      { cardId: card(2).cardId, said: 'Card 2.', now: false, at: card(2).at },
      { cardId: card(1).cardId, said: 'Card 1.', now: true, at: card(1).at, url: 'https://github.com/acme/app/pull/7' }
    ])
    expect(JSON.stringify(shown)).not.toMatch(/rm -rf|model thinks|Fix it/)
    expect(phoneScoutCards(Array.from({ length: 15 }, (_, n) => card(n)))).toHaveLength(PHONE_MAX_SCOUT_CARDS)
  })

  it('dismisses a card the phone names, and nothing that is not a card’s id', async () => {
    const dismissScout = vi.fn(async () => true)
    const calls: PhoneCalls = { state: async () => ({ waiting: [], conversations: [] }), turns: async () => [], decide: async () => true, send: async () => ({ ok: true }), dismissScout }
    expect(await answerPhone(calls, 'POST', '/api/scout/dismiss', { cardId: card(4).cardId })).toEqual({ status: 200, data: { ok: true } })
    expect(dismissScout).toHaveBeenCalledWith(card(4).cardId)
    expect(await answerPhone(calls, 'POST', '/api/scout/dismiss', { cardId: '../scout.json' })).toBeUndefined()
    expect(dismissScout).toHaveBeenCalledTimes(1)
  })

  it('has the page show them, and what the away looks found with the computer off', () => {
    for (const via of ['lan', 'relay'] as const) expect(phonePage({ via })).toContain("el('h2', { text: 'Scout found' })")
    expect(phonePage({ via: 'relay' })).toContain("el('h2', { text: 'While your computer is off' })")
  })
})
