import { createElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'

import type { PhoneAnywhereSettings, PhoneApi, PhoneSettings as PhoneState } from '../../shared/phone.js'
import { PhoneSettings } from './components/PhoneSettings.js'

/*
 * SETTINGS OPENS LOCUST IN A BROWSER WITHOUT A CODE (0.838). Under Your phone: Open in your browser, one click, even
 * with phones off; and Sign in with GitHub, which says what it checks and what it asks GitHub for, then whom it lets in.
 */
const api: PhoneApi = {
  settings: vi.fn(), setEnabled: vi.fn(), forget: vi.fn(), anywhere: vi.fn(), setMode: vi.fn(), newCode: vi.fn(), remove: vi.fn(), setNightApprovals: vi.fn(),
  setDesk: vi.fn(), openInBrowser: vi.fn(), githubSignIn: vi.fn(), githubSignOut: vi.fn(),
  onAnywhereChanged: vi.fn(() => () => undefined)
}
const OFF_WIFI: PhoneState = { enabled: false, addresses: [] }
const URL = 'https://app.locust.lol/#p=AAAAAAAAAAAAAAAAAAAAAA.BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB.CCCCCCCCCCCCCCCCCCCCCC'
const draw = (anywhere: PhoneAnywhereSettings, wifi = OFF_WIFI): string =>
  renderToStaticMarkup(createElement(PhoneSettings, { heading: 'Your phone', api, initial: { wifi, anywhere } }))

describe('Settings opens Locust in a browser without a code', () => {
  it('offers Open in your browser with phones off, saying it turns From anywhere on', () => {
    const html = draw({ enabled: false, connected: false, devices: [] })
    expect(html).toContain('Or open Locust’s whole window in this computer’s browser. It turns this on, from anywhere.')
    expect(html).toContain('>Open in your browser</button>')
    // Not while phones reach Locust only on this Wi-Fi: that way would be turned off.
    expect(draw({ enabled: false, connected: false, devices: [] }, { enabled: true, addresses: [], url: 'http://192.168.1.2:1234/#k' })).not.toContain('Open in your browser')
  })

  it('shows Open in your browser’s code as a link, never as a code for a phone to scan', () => {
    const html = draw({ enabled: true, connected: true, devices: [], code: { url: `${URL}.d`, until: '2099-01-01T00:00:00.000Z', desk: true } })
    expect(html).toContain('Opening Locust in this computer’s browser.')
    expect(html).toContain('>Copy the link</button>')
    expect(html).not.toContain('lc-phone__qr')
    const plain = draw({ enabled: true, connected: true, devices: [], code: { url: URL, until: '2099-01-01T00:00:00.000Z' } })
    expect(plain).toContain('lc-phone__qr')
    expect(plain).toContain('Or open the whole window in this computer’s browser, paired at once.')
  })

  it('offers Sign in with GitHub, saying what is checked and that GitHub is asked for nothing else', () => {
    const html = draw({ enabled: true, connected: true, devices: [] })
    expect(html).toContain('Sign in with GitHub, and any browser where you sign in to GitHub opens Locust at app.locust.lol/desk, with no code.')
    expect(html).toContain('then this computer checks again with GitHub itself. Locust asks GitHub for no permissions, only who you are.')
    expect(html).toMatch(/<button type="button" class="lc-control lc-control--boxed">Sign in with GitHub<\/button>/)
    // Not before the relay answers: there is nothing to sign in through.
    expect(draw({ enabled: true, connected: false, devices: [] })).toMatch(/disabled="">Sign in with GitHub<\/button>/)
  })

  it('once signed in, says whom it lets in, and marks the browsers that came in that way', () => {
    const html = draw({
      enabled: true, connected: true, github: { login: 'octo' },
      devices: [{ id: 'd1', name: 'Chrome on Windows', added: '2026-10-28T00:00:00.000Z', online: true, desk: true, github: true }]
    })
    expect(html).toContain('Any browser where you sign in to GitHub as octo opens Locust at app.locust.lol/desk, with no code.')
    expect(html).toContain('Signing out stops new sign-ins. Browsers already in stay paired until you remove them above.')
    expect(html).toContain('>Sign out of GitHub</button>')
    expect(html).toContain('<span class="lc-phone__state">Open now · signed in with GitHub</span>')
  })
})
