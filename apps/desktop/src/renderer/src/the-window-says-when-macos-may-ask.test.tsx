import { readFileSync } from 'node:fs'

import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { KEYCHAIN_NOTE, KeychainNote } from './components/KeychainNote.js'

/*
 * THE WINDOW SAYS WHEN MACOS MAY ASK (0.854, main/keychain.ts). After an update a Mac asks for its password before
 * Locust may open the keys it keeps, and the window does not move until it is answered -- often with the box behind
 * it. The host says so first; the window shows it, and asks what the keychain is doing once it listens.
 */

describe('the keychain note', () => {
  it('is drawn only while macOS may be asking, and names the box and the answer', () => {
    expect(renderToStaticMarkup(<KeychainNote asking={false} />)).toBe('')
    const html = renderToStaticMarkup(<KeychainNote asking />)
    expect(html).toContain('role="alert"')
    expect(html).toContain('data-keychain-note=""')
    expect(KEYCHAIN_NOTE).toContain('“@teammate/desktop Safe Storage”')
    expect(KEYCHAIN_NOTE).toContain('choose Always Allow')
    expect(KEYCHAIN_NOTE).toContain('behind this window')
  })

  it('the window takes the host’s word, and asks for it only once its listener is in place', () => {
    const app = readFileSync(new URL('./App.tsx', import.meta.url), 'utf8')
    const listening = app.indexOf('const removeMissionListener = bridge.onCodexMissionUpdate(')
    const asked = app.indexOf('void bridge.keychainAsking?.()')
    expect(listening).toBeGreaterThan(-1)
    expect(asked).toBeGreaterThan(listening)
    expect(app).toContain("if (update.kind === 'keychain') {\n        setKeychainAsking(update.asking)")
    expect(app).toContain('<KeychainNote asking={keychainAsking} />')
  })
})
