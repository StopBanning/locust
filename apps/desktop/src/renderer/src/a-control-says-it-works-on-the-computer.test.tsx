import { renderToStaticMarkup } from 'react-dom/server'
import { afterEach, describe, expect, it } from 'vitest'

import { APPEARANCE_SET_CHANNEL } from '../../shared/appearance.js'
import { WINDOW_ONLY_SAYS } from '../../shared/desk-calls.js'
import { DISCORD_SETTINGS_CHANNEL } from '../../shared/discord.js'
import { OnTheComputer } from './components/OnTheComputer.js'
import { failureSaid, inABrowser, onTheComputer } from './onTheComputer.js'

/**
 * A CONTROL SAYS IT WORKS ON THE COMPUTER (0.849). Colin, in Safari on app.locust.lol (2026-11-01): Discord, Slack and
 * Telegram said "This setting could not be read", voice typing "could not be saved", Plans sprang back, and "Allow it
 * in GitHub" did nothing. Each call stays on the computer by design; the window never said so.
 */
const asBrowser = (): void => {
  ;(globalThis as { window?: unknown }).window = { desktop: { remote: { computer: 'your computer' } } }
}
afterEach(() => {
  delete (globalThis as { window?: unknown }).window
})

describe('a control whose call stays on the computer', () => {
  it('is said to stay there only in a browser, and only for a call a browser is refused', () => {
    expect(inABrowser()).toBe(false)
    expect(onTheComputer(DISCORD_SETTINGS_CHANNEL)).toBeUndefined()
    asBrowser()
    expect(inABrowser()).toBe(true)
    expect(onTheComputer(DISCORD_SETTINGS_CHANNEL)).toBe(WINDOW_ONLY_SAYS)
    // The theme changes from a browser: nothing to say.
    expect(onTheComputer(APPEARANCE_SET_CHANNEL)).toBeUndefined()
  })

  it('is drawn disabled under the words in a browser, and untouched on the computer', () => {
    const line = <OnTheComputer channel={DISCORD_SETTINGS_CHANNEL}><button type="button">Set up</button></OnTheComputer>
    expect(renderToStaticMarkup(line)).toBe('<button type="button">Set up</button>')
    asBrowser()
    expect(renderToStaticMarkup(line)).toBe(`<fieldset class="lc-oncomputer" disabled=""><button type="button">Set up</button><p class="lc-settings__note lc-oncomputer__said">${WINDOW_ONLY_SAYS}</p></fieldset>`)
    expect(renderToStaticMarkup(<OnTheComputer channel={[APPEARANCE_SET_CHANNEL, DISCORD_SETTINGS_CHANNEL]}><span /></OnTheComputer>)).toContain('disabled=""')
    expect(renderToStaticMarkup(<OnTheComputer channel={APPEARANCE_SET_CHANNEL}><span /></OnTheComputer>)).toBe('<span></span>')
  })

  it('says the refusal, not its own failure, when a browser was refused anyway', () => {
    expect(failureSaid(new Error(WINDOW_ONLY_SAYS), 'This setting could not be read.')).toBe(WINDOW_ONLY_SAYS)
    expect(failureSaid(new Error('disk full'), 'This setting could not be read.')).toBe('This setting could not be read.')
    expect(failureSaid('not an error', 'This setting could not be read.')).toBe('This setting could not be read.')
  })
})
