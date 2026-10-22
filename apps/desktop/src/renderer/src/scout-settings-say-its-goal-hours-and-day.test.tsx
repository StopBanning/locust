import { createElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'

import { seedAvatar } from '../../shared/avatar.js'
import { DEFAULT_SCOUT_SETTINGS, briefFact, cardOf, type ScoutState } from '../../shared/scout.js'
import { ScoutCards } from './components/ScoutCards.js'
import { ScoutSetting } from './components/ScoutSetting.js'

/*
 * SCOUT'S SETTINGS SAY ITS GOAL, ITS HOURS AND ITS DAY (0.815). What it keeps watch for, in the person's own words;
 * the folder's check, which needs a check command first; quiet hours as a clock reads them; waiting while the person
 * works; and what today's looks used, in numbers.
 */
const api = { state: vi.fn(), turnOn: vi.fn(), set: vi.fn(), dismiss: vi.fn(), tune: vi.fn(), look: vi.fn(), saveNotes: vi.fn(), onChanged: vi.fn(() => () => undefined) }
const on = (change: Partial<ScoutState['settings']> = {}, more: Partial<ScoutState> = {}): ScoutState =>
  ({ settings: { ...DEFAULT_SCOUT_SETTINGS, on: true, teammateId: 'tm_scout', ...change }, cards: [], lookIds: [], ...more })
const draw = (state: ScoutState, extra: Record<string, unknown> = {}): string =>
  renderToStaticMarkup(createElement(ScoutSetting, { heading: 'Scout keeps watch', state, api, ...extra }))

describe('Scout’s settings', () => {
  it('show what it keeps watch for, in the person’s words, with no Save until it is changed', () => {
    const html = draw(on({ goal: 'Keep main’s tests passing.' }))
    expect(html).toContain('What to keep watch for')
    expect(html).toContain('Keep main’s tests passing.</textarea>')
    expect(html).not.toContain('>Save</button>')
    expect(draw(on())).toContain('placeholder="For example: keep main’s tests passing, and tell me about new issues on the sign-in page."')
  })

  it('show Scout’s notes as lines the person can read and change, and how many', () => {
    const html = draw(on({}, { notes: ['The person merges on Fridays', '2026-10-22: Wren owns the Mac build.'] }))
    expect(html).toContain('Scout’s notes · 2')
    expect(html).toContain('The person merges on Fridays\n2026-10-22: Wren owns the Mac build.</textarea>')
    expect(draw(on())).toContain('placeholder="Nothing yet. A look that learns something worth keeping adds a line here."')
  })

  it('offer the folder’s check only once the folder has a check command, and say which', () => {
    const without = draw(on())
    expect(without).toContain('Set a check in Settings &gt; Project folder first, such as npm test.')
    expect(without).toMatch(/aria-label="This folder’s check" aria-checked="false" disabled=""/)
    const withCheck = draw(on({ watches: { github: true, teammates: true, check: true } }), { checkCommand: 'npm test' })
    expect(withCheck).toContain('When the folder has changed, Scout runs <code class="lc-mono">npm test</code>, never while a turn is working here.')
    expect(withCheck).toMatch(/aria-label="This folder’s check" aria-checked="true"(?! disabled)/)
  })

  it('say quiet hours as a clock reads them, and offer waiting while the person works', () => {
    const html = draw(on({ quiet: { from: 22, to: 7 } }))
    expect(html).toContain('Quiet hours')
    expect(html).toContain('10 PM to 7 AM')
    expect(html).toContain('Wait while you’re working')
    expect(html).toMatch(/aria-label="Wait while you’re working" aria-checked="true"/)
  })

  it('offer a morning brief at an hour of the morning', () => {
    const off = draw(on())
    expect(off).toContain('Morning brief')
    expect(off).toContain('One card on Home each morning: who waits on you, what finished or failed since yesterday, and how the check stands.')
    expect(draw(on({ briefAt: 7 }))).toContain('At 7 AM')
  })

  it('draw a morning brief as one card, its lines under its sentence', () => {
    const scout = { teammateId: 'tm_scout', name: 'Scout', hue: 'teal', role: 'Custom', roleTitle: 'Keeps watch', avatar: seedAvatar('tm_scout'), createdAt: '2026-10-12' } as const
    const fact = briefFact({ waiting: [{ name: 'Ada', minutes: 30 }], finished: 4, failed: 1, cards: { total: 0, now: 0 } }, '2026-10-22')
    const html = renderToStaticMarkup(createElement(ScoutCards, {
      state: on({}, { cards: [cardOf(fact, 'card_brief', new Date('2026-10-22T12:00:00.000Z'), false)] }),
      scout, onOpen: vi.fn(), onSend: vi.fn(), onDraftIntoBox: vi.fn(), onDismiss: vi.fn(), now: new Date('2026-10-22T12:05:00.000Z')
    }))
    expect(html).toContain('>Morning brief<')
    expect(html).toContain('Good morning. Here is where things stand.')
    expect(html).toContain('<ul class="lc-scoutcard__lines"><li>Waiting on you: Ada, 30 min.</li><li>Since yesterday: 4 turns finished, 1 failed.</li></ul>')
  })

  it('say what today’s looks used', () => {
    expect(draw(on({}, { today: { day: '2026-10-22', looks: 14, asked: 2 } }), { model: 'Deepseek V4.1 Flash Free' }))
      .toContain('Today: 14 looks, 2 of them read by Deepseek V4.1 Flash Free.')
    expect(draw(on({}, { today: { day: '2026-10-22', looks: 1, asked: 0 } }))).toContain('Today: 1 look, none of them needing its model.')
  })
})
