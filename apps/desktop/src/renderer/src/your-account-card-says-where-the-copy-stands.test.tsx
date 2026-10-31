import { renderToStaticMarkup } from 'react-dom/server'
import { expect, it } from 'vitest'

import { YourAccountSettings, agoOf, sizeOf } from './components/YourAccountSettings.js'

/** Settings › Your account (accounts slice 3): each state the card can be in, drawn from what main says. */

it('signed out, the card says what an account does and where to sign in, with no switch', () => {
  const markup = renderToStaticMarkup(<YourAccountSettings initial={{ sync: 'off', hasKey: false, syncing: false }} />)
  expect(markup).toContain('follow you to any computer where you sign in')
  expect(markup).toContain('Turn on From anywhere under Your phone, then press Sign in with GitHub there.')
  expect(markup).not.toContain('role="switch"')
  expect(markup).not.toContain('Delete my copy')
})

it('signed in and off, it says the relay cannot read the copy before anything is sent', () => {
  const markup = renderToStaticMarkup(<YourAccountSettings initial={{ signedIn: { login: 'colin' }, sync: 'off', hasKey: false, syncing: false }} />)
  expect(markup).toContain('Signed in with GitHub as colin.')
  expect(markup).toContain('so the relay cannot read it. Agent sign-ins and keys are never sent.')
  expect(markup).toContain('aria-checked="false"')
  expect(markup).not.toContain('Delete my copy')
})

it('on, it says when it last synced, how much, and the storage used, with Sync now and Delete my copy', () => {
  const at = new Date(Date.now() - 4 * 60_000).toISOString()
  const markup = renderToStaticMarkup(<YourAccountSettings initial={{ signedIn: { login: 'colin' }, sync: 'on', hasKey: true, syncing: false, lastSynced: { at, files: 214, bytes: 31 * 1024 * 1024 }, storage: { used: 33 * 1024 * 1024, quota: 1024 * 1024 * 1024 } }} />)
  expect(markup).toContain('Last synced 4 min ago: 214 files, 31.0 MB. Using 33.0 MB of 1.00 GB.')
  expect(markup).toContain('aria-checked="true"')
  expect(markup).toContain('Sync now')
  expect(markup).toContain('Delete my copy')
})

it('the recovery code is shown with what losing it costs, until it is put away', () => {
  const markup = renderToStaticMarkup(<YourAccountSettings initial={{ signedIn: { login: 'colin' }, sync: 'on', hasKey: true, syncing: true }} initialCode="ABCDE-FGHJK" />)
  expect(markup).toContain('Write down your recovery code. It is shown only now.')
  expect(markup).toContain('ABCDE-FGHJK')
  expect(markup).toContain('cannot be opened by anyone, Locust included')
  expect(markup).toContain('I wrote it down')
  expect(markup).toContain('Syncing now.')
})

it('sizes and times read as a person says them', () => {
  expect(sizeOf(300)).toBe('1 KB')
  expect(sizeOf(5 * 1024 * 1024)).toBe('5.0 MB')
  const now = Date.parse('2026-10-31T18:00:00Z')
  expect(agoOf('2026-10-31T17:59:30Z', now)).toBe('just now')
  expect(agoOf('2026-10-31T15:00:00Z', now)).toBe('3 h ago')
})
