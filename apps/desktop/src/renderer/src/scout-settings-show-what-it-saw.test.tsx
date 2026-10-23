import { createElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'

import { DEFAULT_SCOUT_SETTINGS, type ScoutState } from '../../shared/scout.js'
import { ScoutSetting } from './components/ScoutSetting.js'

/*
 * WHAT SCOUT SAW, IN SETTINGS (0.828, shared/scout-log.ts): a line under Scout's own, read only when the person opens
 * it, that says where a "why didn't you tell me?" is answered from.
 */
const api = { state: vi.fn(), turnOn: vi.fn(), set: vi.fn(), post: vi.fn(), editPost: vi.fn(), dismiss: vi.fn(), tune: vi.fn(), look: vi.fn(), saveNotes: vi.fn(), onChanged: vi.fn(() => () => undefined), log: vi.fn() }
const state = (on: boolean): ScoutState => ({ settings: { ...DEFAULT_SCOUT_SETTINGS, on, teammateId: 'tm_scout' }, cards: [], lookIds: [] })
const draw = (on: boolean): string => renderToStaticMarkup(createElement(ScoutSetting, { heading: 'Scout keeps watch', state: state(on), api }))

describe('What Scout saw, in Settings', () => {
  it('is offered once Scout is on, says what it holds, and reads nothing until it is opened', () => {
    const html = draw(true)
    expect(html).toContain('<h3 class="lc-settings__heading">What Scout saw</h3>')
    expect(html).toContain('Each look of the last 14 days, newest first: what it raised, what it set aside and why, and what it did not read.')
    expect(html).toContain('Ask Scout in its conversation why it did not tell you about something, and it answers from this.')
    expect(html).toContain('>Show what Scout saw</button>')
    expect(api.log).not.toHaveBeenCalled()
    expect(draw(false)).not.toContain('What Scout saw')
  })
})
