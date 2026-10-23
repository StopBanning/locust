import { createElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'

import type { PhoneAnywhereSettings, PhoneApi, PhoneSettings as PhoneState } from '../../shared/phone.js'
import { PhoneSettings } from './components/PhoneSettings.js'

/*
 * YOUR PHONE REACHES LOCUST FROM ANYWHERE (0.826). One switch, and the way the phone reaches Locust: from anywhere
 * through Locust's relay, the first choice, or only on this Wi-Fi as before. From anywhere shows the code to pair a
 * phone with, whether this computer is connected to the relay, and each paired phone with a Remove.
 */
const api: PhoneApi = {
  settings: vi.fn(), setEnabled: vi.fn(), forget: vi.fn(), anywhere: vi.fn(), setMode: vi.fn(), newCode: vi.fn(), remove: vi.fn(), setNightApprovals: vi.fn(),
  onAnywhereChanged: vi.fn(() => () => undefined)
}
const OFF_WIFI: PhoneState = { enabled: false, addresses: [] }
const OFF_ANYWHERE: PhoneAnywhereSettings = { enabled: false, connected: false, devices: [] }
const CODE = { url: 'https://app.locust.lol/#p=AAAAAAAAAAAAAAAAAAAAAA.BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB.CCCCCCCCCCCCCCCCCCCCCC', until: '2099-01-01T00:00:00.000Z' }
const draw = (wifi: PhoneState, anywhere: PhoneAnywhereSettings): string =>
  renderToStaticMarkup(createElement(PhoneSettings, { heading: 'Your phone', api, initial: { wifi, anywhere } }))

describe('your phone reaches Locust from anywhere', () => {
  it('is off until turned on, with From anywhere chosen for when it is', () => {
    const html = draw(OFF_WIFI, OFF_ANYWHERE)
    expect(html).toMatch(/aria-label="Use Locust from your phone" aria-checked="false"/)
    expect(html).toContain('Off. Turn it on to check on your teammates from your phone')
    expect(html).toMatch(/role="radio" aria-checked="true" class="lc-button is-active">From anywhere</)
    expect(html).toMatch(/role="radio" aria-checked="false" class="lc-button">Only on this Wi-Fi</)
    expect(html).not.toContain('lc-phone__qr')
  })

  it('shows the code to pair a phone with, and says whether this computer is connected to the relay', () => {
    const html = draw(OFF_WIFI, { enabled: true, connected: true, code: CODE, devices: [] })
    expect(html).toMatch(/aria-label="Use Locust from your phone" aria-checked="true"/)
    expect(html).toContain('On, from anywhere.')
    expect(html).toContain('the relay passes along what it cannot read')
    expect(html).toContain('aria-label="A code for your phone’s camera. It pairs your phone with Locust."')
    expect(html).toContain('Connected to Locust’s relay.')
    expect(html).toContain('It pairs one phone, once, within ten minutes')
    // Nothing is paired yet, so there is nothing below to remove.
    expect(html).not.toContain('Lost a phone?')
    expect(draw(OFF_WIFI, { enabled: true, connected: false, code: CODE, devices: [] })).toContain('Connecting to Locust’s relay…')
    // Why it is not connected, in Locust's words, comes before anything it would otherwise say.
    expect(draw(OFF_WIFI, { enabled: true, connected: false, devices: [], message: 'Locust’s relay no longer knows this computer.' })).toContain('Locust’s relay no longer knows this computer.')
  })

  it('lists each paired phone, whether it is open now, and a Remove for it', () => {
    const html = draw(OFF_WIFI, {
      enabled: true, connected: true, devices: [
        { id: 'd1', name: 'iPhone', added: '2026-10-23T00:00:00.000Z', online: true },
        { id: 'd2', name: 'Android phone', added: '2026-10-22T00:00:00.000Z', seen: new Date(Date.now() - 3 * 60_000).toISOString(), online: false }
      ]
    })
    expect(html).toContain('aria-label="Paired phones"')
    expect(html).toContain('<span class="lc-phone__name">iPhone</span><span class="lc-phone__state">Open now</span>')
    expect(html).toContain('<span class="lc-phone__name">Android phone</span><span class="lc-phone__state">Last open 3 minutes ago</span>')
    expect(html.match(/>Remove<\/button>/g)).toHaveLength(2)
    expect(html).toContain('Lost a phone? Remove it below, and it stops at once.')
    // The code is spent once a phone pairs: the next one is made when asked for.
    expect(html).toContain('>Pair a phone</button>')
    expect(html).not.toContain('lc-phone__qr')
  })

  it('says which phones get alerts, and offers approvals in quiet hours only once one does', () => {
    const quietly = draw(OFF_WIFI, { enabled: true, connected: true, devices: [{ id: 'd1', name: 'iPhone', added: '2026-10-23T00:00:00.000Z', online: true }] })
    expect(quietly).not.toContain('Let approvals buzz in quiet hours')
    const html = draw(OFF_WIFI, { enabled: true, connected: true, devices: [{ id: 'd1', name: 'iPhone', added: '2026-10-23T00:00:00.000Z', online: false, alerts: true }] })
    expect(html).toContain('<span class="lc-phone__state">Paired · alerts on</span>')
    expect(html).toMatch(/aria-label="Let approvals buzz in quiet hours" aria-checked="false"/)
    expect(draw(OFF_WIFI, { enabled: true, connected: true, nightApprovals: true, devices: [{ id: 'd1', name: 'iPhone', added: '2026-10-23T00:00:00.000Z', online: false, alerts: true }] }))
      .toMatch(/aria-label="Let approvals buzz in quiet hours" aria-checked="true"/)
  })

  it('keeps the home-network code as it was when Only on this Wi-Fi is on', () => {
    const html = draw({ enabled: true, url: 'http://192.168.1.20:47740/#abcdefghijklmnopqrstuvwxyzABCDEF', addresses: ['http://192.168.1.20:47740'] }, OFF_ANYWHERE)
    expect(html).toMatch(/role="radio" aria-checked="true" class="lc-button is-active">Only on this Wi-Fi</)
    expect(html).toContain('Your phone must be on the same Wi-Fi as this computer.')
    expect(html).toContain('aria-label="A code for your phone’s camera. It opens Locust on your phone."')
    expect(html).toContain('>New code</button>')
    expect(html).not.toContain('Paired phones')
  })
})
