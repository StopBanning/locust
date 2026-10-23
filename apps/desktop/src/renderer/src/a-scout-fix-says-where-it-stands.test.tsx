import { createElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'

import { DEFAULT_SCOUT_SETTINGS, type ScoutFix, type ScoutState } from '../../shared/scout.js'
import { cardOf } from '../../shared/scout.js'
import { FixLine, ScoutCards } from './components/ScoutCards.js'
import { ScoutSetting } from './components/ScoutSetting.js'

/*
 * A FIX SCOUT PREPARED, ON ITS CARD (0.828, main/scout-fix.ts): who is preparing it; ready, with how many files and
 * that the check passes, and Review, Apply and Skip; applied; made on an older copy; or what was missing.
 */
const draw = (fix: ScoutFix): string => renderToStaticMarkup(createElement(FixLine, { fix, cardId: 'card_1', onOpen: () => undefined }))

describe('a fix on Scout’s card', () => {
  it('says each step in words, with only the presses that step has', () => {
    expect(draw({ state: 'preparing', fixId: 'rt_scoutfix0123456789ab', name: 'Wren', since: '2026-10-23T11:00:00Z' })).toContain('Wren is preparing a fix in a copy of the folder…')
    const ready = draw({ state: 'ready', fixId: 'rt_scoutfix0123456789ab', name: 'Wren', files: ['src/answer.ts', 'test/answer.test.ts'], added: 3, removed: 1, missionId: 'mission_fix' })
    expect(ready).toContain('Fix ready: 2 files changed (+3 −1), the check passes.')
    expect(ready).toContain('src/answer.ts · test/answer.test.ts')
    expect(ready).toMatch(/>Review<\/button>.*>Apply<\/button>.*>Skip<\/button>/)
    expect(draw({ state: 'applied', files: ['src/answer.ts'] })).toContain('Applied: 1 file in your folder, beside your own changes. Opening a pull request is yours.')
    const stale = draw({ state: 'stale', fixId: 'rt_scoutfix0123456789ab', name: 'Wren', files: ['src/answer.ts'], missionId: 'mission_fix' })
    expect(stale).toContain('Not applied: the fix was made on an older copy, and you changed that file since.')
    expect(stale).toContain('>Prepare again</button>')
    expect(stale).not.toContain('>Apply</button>')
    const unfixed = draw({ state: 'unfixed', name: 'Wren', why: 'Wren changed nothing, and said: “The test needs a database I cannot reach.”' })
    expect(unfixed).toContain('Wren could not prepare a fix: Wren changed nothing, and said: “The test needs a database I cannot reach.”')
    expect(draw({ state: 'held', why: 'Today’s fix is used, so Scout did not prepare one.' })).toContain('>Prepare again</button>')
  })
})

describe('a card with a fix', () => {
  const card = (fix: ScoutFix) => ({
    ...cardOf({ key: 'check:m1:fails', urgency: 'now', said: 'The check npm test fails: 1 failed.', draft: { teammateId: 'tm_wren', name: 'Wren', message: 'Fix the failing test.' } }, 'card_1', new Date('2026-10-23T11:00:00Z'), false),
    fix
  })
  const cards = (fix: ScoutFix): string => renderToStaticMarkup(createElement(ScoutCards, {
    state: { settings: { ...DEFAULT_SCOUT_SETTINGS, on: true }, cards: [card(fix)], lookIds: [] },
    scout: undefined, onOpen: () => undefined, onSend: () => undefined, onDraftIntoBox: () => undefined, onDismiss: () => undefined, now: new Date('2026-10-23T11:05:00Z')
  }))
  it('drops the handoff drafted for the same check while the fix is the next step, and offers it again when the fix could not be made', () => {
    expect(cards({ state: 'ready', fixId: 'rt_scoutfix0123456789ab', name: 'Wren', files: ['a.ts'], missionId: 'm' })).not.toContain('Send to Wren')
    expect(cards({ state: 'preparing', fixId: 'rt_scoutfix0123456789ab', name: 'Wren', since: '2026-10-23T11:00:00Z' })).not.toContain('Drafted for Wren')
    expect(cards({ state: 'applied', files: ['a.ts'] })).not.toContain('Send to Wren')
    expect(cards({ state: 'unfixed', name: 'Wren', why: 'It ran past 30 minutes, so Locust stopped it.' })).toContain('Send to Wren')
  })
})

describe('Prepare fixes before I look, in Settings', () => {
  const api = { state: vi.fn(), turnOn: vi.fn(), set: vi.fn(), post: vi.fn(), editPost: vi.fn(), dismiss: vi.fn(), tune: vi.fn(), look: vi.fn(), saveNotes: vi.fn(), onChanged: vi.fn(() => () => undefined) }
  const state = (change: Partial<ScoutState['settings']> = {}): ScoutState => ({ settings: { ...DEFAULT_SCOUT_SETTINGS, on: true, teammateId: 'tm_scout', ...change }, cards: [], lookIds: [], today: { day: '2026-10-23', looks: 4, asked: 1, fixes: 1 } })
  const html = (change: Partial<ScoutState['settings']>, checkCommand?: string): string => renderToStaticMarkup(createElement(ScoutSetting, {
    heading: 'Scout keeps watch', state: state(change), api, team: [{ teammateId: 'tm_wren', name: 'Wren' }], ...(checkCommand === undefined ? {} : { checkCommand })
  }))

  it('is off until turned on, needs the folder’s check first, and once on names who prepares them and how many a day', () => {
    const off = html({})
    expect(off).toContain('Prepare fixes before I look')
    expect(off).toContain('Nothing reaches your folder until you press Apply, and it never pushes or opens a pull request. It runs on that teammate’s plan.')
    expect(off).toContain('Turn on This folder’s check first.')
    expect(off).toMatch(/aria-label="Prepare fixes before I look" aria-checked="false" disabled=""/)
    const on = html({ watches: { github: true, teammates: true, check: true }, fixes: { on: true, teammateId: 'tm_wren', perDay: 2 } }, 'npm test')
    expect(on).toMatch(/aria-label="Prepare fixes before I look" aria-checked="true"/)
    expect(on).toContain('aria-label="Who prepares Scout’s fixes"')
    expect(on).toContain('Wren')
    expect(on).toContain('At most 2 a day')
    expect(on).toContain('1 of 2 fixes prepared.')
  })
})
