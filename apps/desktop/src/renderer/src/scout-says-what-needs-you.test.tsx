import { createElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'

import { seedAvatar } from '../../shared/avatar.js'
import { DEFAULT_SCOUT_SETTINGS, type ScoutCard, type ScoutState } from '../../shared/scout.js'
import { ScoutCards, ScoutInvite, ScoutWatching } from './components/ScoutCards.js'
import { HomeTeam } from './components/HomeTeam.js'
import { ScoutSetting } from './components/ScoutSetting.js'

/*
 * SCOUT SAYS WHAT NEEDS YOU (0.756). On Home, each card leads with Locust's own sentence, what needs the person now
 * comes first and raised, Scout's take and draft sit under it as Scout's, and every action is a press. Before Scout is
 * on, one invitation says what it reads, with which model, and that it never acts by itself.
 */
const scout = { teammateId: 'tm_scout', name: 'Scout', hue: 'teal', role: 'Custom', roleTitle: 'Keeps watch', avatar: seedAvatar('tm_scout'), createdAt: '2026-10-12' } as const
const card = (change: Partial<ScoutCard>): ScoutCard => ({
  key: 'k', urgency: 'soon', said: 'Someone asked you to review pull request #214, Rename the call log.', cardId: 'card_1', at: '2026-10-12T06:00:00.000Z',
  open: { kind: 'url', url: 'https://github.com/acme/app/pull/214', label: 'Open the pull request' }, ...change
})
const state = (cards: readonly ScoutCard[], change: Partial<ScoutState> = {}): ScoutState => ({ settings: { ...DEFAULT_SCOUT_SETTINGS, on: true, teammateId: 'tm_scout' }, cards, lookIds: [], ...change })
const now = new Date('2026-10-12T06:06:00.000Z')
const draw = (shown: ScoutState): string => renderToStaticMarkup(createElement(ScoutCards, { state: shown, scout, onOpen: vi.fn(), onSend: vi.fn(), onDraftIntoBox: vi.fn(), onDismiss: vi.fn(), now }))

describe('Scout’s cards on Home', () => {
  it('lead with Locust’s sentence, say who noticed it and when, and open what it is about', () => {
    const html = draw(state([card({})]))
    expect(html).toContain('Scout noticed · 1')
    expect(html).toContain('<p class="lc-scoutcard__said">Someone asked you to review pull request #214, Rename the call log.</p>')
    expect(html).toContain('Scout · 6 min ago')
    expect(html).toContain('>Open the pull request<')
    expect(html).toContain('>Dismiss<')
    expect(html).not.toContain('Send to')
  })

  it('raise what needs the person now, first, whenever it was found', () => {
    const html = draw(state([card({ cardId: 'card_old', said: 'Older news.' }), card({ cardId: 'card_now', urgency: 'now', said: 'Atlas has waited 25 minutes for your OK.' })]))
    expect(html.indexOf('Atlas has waited')).toBeLessThan(html.indexOf('Older news.'))
    expect(html).toMatch(/<article class="lc-card lc-scoutcard is-pending is-amber" data-scout-card="card_now">/)
    expect(draw(state([card({ urgency: 'now', said: 'Atlas has waited 25 minutes for your OK.', quote: 'pnpm test --filter desktop' })]))).toContain('<p class="lc-scoutcard__quote lc-mono">pnpm test --filter desktop</p>')
    expect(html).toContain('Needs you')
  })

  it('show Scout’s take and draft as Scout’s, with Send and Change it first, and say while it is still reading', () => {
    const drafted = draw(state([card({ take: 'Ada renamed the file the call log reads.', draft: { teammateId: 'tm_atlas', name: 'Atlas', message: 'Review #214 before I do.' } })]))
    expect(drafted).toContain('<span class="lc-scoutcard__who">Scout:</span> Ada renamed the file the call log reads.')
    expect(drafted).toContain('Drafted for Atlas')
    expect(drafted).toContain('Review #214 before I do.')
    expect(drafted).toContain('>Send to Atlas<')
    expect(drafted).toContain('>Change it first<')
    expect(drafted).toContain('Noticed, and drafted')
    expect(draw(state([card({ reading: true })]))).toContain('Scout is reading it…')
  })

  it('say why its model added nothing, and show three until asked for all', () => {
    const many = draw(state([1, 2, 3, 4].map((index) => card({ cardId: `card_${String(index)}`, said: `News ${String(index)}.` })), { modelSaid: 'Scout’s model did not answer.' }))
    expect(many).toContain('Scout’s model did not answer. The cards below are in Locust’s words.')
    expect(many.match(/data-scout-card=/g)).toHaveLength(3)
    expect(many).toContain('Show all 4')
  })

  it('keeps a card brought into view even when it is past the first three', () => {
    const cards = [1, 2, 3, 4].map((index) => card({ cardId: `card_${String(index)}`, urgency: 'now', said: `News ${String(index)}.` }))
    const html = renderToStaticMarkup(createElement(ScoutCards, { state: state(cards), scout, onOpen: vi.fn(), onSend: vi.fn(), onDraftIntoBox: vi.fn(), onDismiss: vi.fn(), now, focusCardId: 'card_4' }))
    expect(html).toContain('data-scout-card="card_4"')
    expect(html).toContain('News 4.')
  })

  it('are not there while Scout is off, or has nothing to say', () => {
    expect(draw(state([card({})], { settings: DEFAULT_SCOUT_SETTINGS }))).toBe('')
    expect(draw(state([]))).toBe('')
  })
})

describe('meeting Scout', () => {
  it('says what it reads, with which model, and that it never acts by itself', () => {
    const html = renderToStaticMarkup(createElement(ScoutInvite, { model: 'Space Bunny Free through OpenCode', onTurnOn: vi.fn(), onNotNow: vi.fn(), onClose: vi.fn() }))
    expect(html).toContain('Meet Scout, a teammate that keeps watch')
    // A dialog over Home (0.808), not a row on it.
    expect(html).toContain('role="dialog" aria-modal="true" aria-label="Meet Scout"')
    expect(html).toContain('It never sends, merges or changes anything: you press.')
    expect(html).toContain('That model is Space Bunny Free through OpenCode.')
    expect(html).toContain('>Turn on Scout<')
    expect(html).toContain('>Not now<')
  })

  /*
   * OFFERED IN SCOUT'S PLACE ON THE TEAM (0.808): as a card four lines tall above the team, the offer made Home scroll
   * at 1205x803, 1366x768 and 1440x900. Now it is a card in the slot Scout takes once it is on, and costs no row.
   */
  it('is offered as a card on the team, in the slot Scout will take, with Not now as its x', () => {
    const team = [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime' as const, avatar: seedAvatar('tm_wren'), role: 'Code', working: false }]
    const html = renderToStaticMarkup(createElement(HomeTeam, { team, onMessage: vi.fn(), invite: { onOpen: vi.fn(), onNotNow: vi.fn() } }))
    expect(html).toContain('class="lc-hometeam__card is-invite" aria-haspopup="dialog"')
    expect(html).toContain('>Meet Scout<')
    expect(html).toContain('aria-label="Not now: do not offer Scout again"')
    // After the teammates, inside the same grid.
    expect(html.indexOf('is-invite')).toBeGreaterThan(html.indexOf('Message Wren'))
    expect(renderToStaticMarkup(createElement(HomeTeam, { team, onMessage: vi.fn() }))).not.toContain('is-invite')
  })
})

describe('Scout in Settings', () => {
  const api = { state: vi.fn(), turnOn: vi.fn(), set: vi.fn(), dismiss: vi.fn(), look: vi.fn(), saveNotes: vi.fn(), onChanged: vi.fn(() => () => undefined) }
  it('off, says what turning it on does', () => {
    const html = renderToStaticMarkup(createElement(ScoutSetting, { heading: 'Scout keeps watch', state: state([], { settings: DEFAULT_SCOUT_SETTINGS }), api }))
    expect(html).toContain('Off. Turn it on and Scout joins your team')
    expect(html).not.toContain('Change its model')
  })

  it('on, says what it reads with, offers to change its model, and lists what it watches', () => {
    const html = renderToStaticMarkup(createElement(ScoutSetting, { heading: 'Scout keeps watch', state: state([]), model: 'Space Bunny Free through OpenCode', onChangeModel: vi.fn(), api }))
    expect(html).toContain('It reads with Space Bunny Free through OpenCode.')
    expect(html).toContain('>Change its model<')
    expect(html).toContain('Pull requests and their checks')
    expect(html).toContain('Your teammates’ turns')
    expect(html).toContain('Every 10 minutes')
    expect(html).toContain('Tell me in Windows')
  })
})

/*
 * KEEPING WATCH, WITH NOTHING TO SAY (0.768). Colin, 2026-10-13: "you tell me they are working properly but i have not
 * seen anything". A Scout with no cards drew nothing, so it read as one that was off.
 */
describe('Scout with nothing to say', () => {
  const quiet = (shown: ScoutState, looking = false): string =>
    renderToStaticMarkup(createElement(ScoutWatching, { state: shown, scout, onLookNow: vi.fn(), looking, now }))

  it('says it keeps watch, when it last looked, and that nothing needs the person', () => {
    const html = quiet(state([], { lastLookAt: '2026-10-12T06:02:00.000Z' }))
    expect(html).toContain('aria-label="Scout keeps watch"')
    expect(html).toContain(' Scout keeps watch')
    expect(html).toContain('looked 4 min ago · nothing needs you')
    expect(html).toContain('>Look now</button>')
    expect(quiet(state([], { lastLookAt: '2026-10-12T06:02:00.000Z' }), true)).toContain('>Looking…</button>')
  })

  it('before its first look, says one is on its way', () => {
    expect(quiet(state([]))).toContain('its first look is on its way')
  })

  it('in Settings, says when it last looked and offers a look now', () => {
    const api = { state: vi.fn(), turnOn: vi.fn(), set: vi.fn(), dismiss: vi.fn(), look: vi.fn(), saveNotes: vi.fn(), onChanged: vi.fn() }
    const html = renderToStaticMarkup(createElement(ScoutSetting, { heading: createElement('h2', null, 'Scout'), state: state([], { lastLookAt: new Date(Date.now() - 3 * 60_000).toISOString() }), api }))
    expect(html).toContain('Last looked 3 min ago, and nothing needed you.')
    expect(html).toContain('>Look now</button>')
  })
})
