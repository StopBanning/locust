import { createElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { afterEach, describe, expect, it } from 'vitest'

import { WINDOW_ONLY_SAYS } from '../../shared/desk-calls.js'
import { SettingsScreen } from './components/Screens.js'
import { SETTINGS_PAGES } from './settingsPages.js'

/**
 * SETTINGS IN A BROWSER SAY WHAT STAYS ON THE COMPUTER (0.849). Colin, in Safari on app.locust.lol (2026-11-01), met
 * four controls that failed after a press, or did nothing: the chat apps' "could not be read", voice typing's "could
 * not be saved", Plans springing back, "Allow it in GitHub" silent. Every page is drawn here as a browser draws it and
 * as the window on the computer does.
 */
const browser = (): void => {
  ;(globalThis as { window?: unknown }).window = { desktop: { remote: { computer: 'your computer' }, platform: 'darwin' } }
}
afterEach(() => {
  delete (globalThis as { window?: unknown }).window
})
const page = (id: string, extra: Record<string, unknown> = {}): string =>
  renderToStaticMarkup(createElement(SettingsScreen as never, { initialPage: id, runtimes: [], ...extra } as never))
const BROKEN = /could not be (read|saved|changed)/i

describe('Settings in a browser', () => {
  it('draws every page, and none of them starts by saying a setting could not be read or saved', () => {
    browser()
    for (const each of SETTINGS_PAGES) {
      const html = page(each.id)
      expect(html.length, each.id).toBeGreaterThan(0)
      expect(html, each.id).not.toMatch(BROKEN)
    }
  })

  it('says, in place of the sections whose every setting stays on the computer, that they work there', () => {
    browser()
    const html = page('app')
    for (const heading of ['Your phone', 'Your account', 'Chat apps', 'Let your other AI apps use Locust']) {
      expect(html).toMatch(new RegExp(`<h2 class="lc-settings__heading">${heading}</h2><p class="lc-settings__lede">[^<]+</p><p class="lc-settings__note lc-oncomputer__said">${WINDOW_ONLY_SAYS}</p>`))
    }
    // The chat apps' rows, which read settings a browser is refused, are not drawn at all.
    expect(html).not.toContain('data-chat-app=')
  })

  it("keeps the teammates' switches that grant something on the computer, and lets Plans change from here", () => {
    browser()
    const html = page('teammates', { swarm: false, autoMode: false, keepATodoList: true, claudeOwnSkills: false })
    for (const label of ['Swarm', 'Your skills']) {
      expect(html).toMatch(new RegExp(`<fieldset class="lc-oncomputer" disabled=""><div class="lc-settingline"><div class="lc-settingline__text"><h2 class="lc-settings__heading">${label}</h2>`))
    }
    expect(html).toContain('<fieldset class="lc-oncomputer" disabled=""><div class="lc-settingline"><div class="lc-settingline__text"><h2 class="lc-settings__heading">Auto mode</h2>')
    expect(html).not.toMatch(/<fieldset class="lc-oncomputer" disabled=""><div class="lc-settingline"><div class="lc-settingline__text"><h2 class="lc-settings__heading">Plans<\/h2>/)
  })

  it('lets the look change from here: no part of Appearance is held on the computer', () => {
    browser()
    expect(page('appearance')).not.toContain('lc-oncomputer')
  })
})

describe('Settings in the window on the computer', () => {
  it('holds nothing back and says nothing about a browser, on any page', () => {
    for (const each of SETTINGS_PAGES) {
      const html = page(each.id)
      expect(html, each.id).not.toContain('lc-oncomputer')
      expect(html, each.id).not.toContain(WINDOW_ONLY_SAYS)
    }
  })
})
