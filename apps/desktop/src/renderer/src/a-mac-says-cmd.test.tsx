import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { KeyboardSheet } from './components/KeyboardSheet.js'
import { SHORTCUTS, keysOn } from './keyboardShortcuts.js'
import APP from './App.tsx?raw'
import PROMPT_RAIL from './promptRail.ts?raw'
import TERMINAL_KEYS from './terminalKeys.ts?raw'
import COMPOSER from './components/Composer.tsx?raw'
import SCREENS from './components/Screens.tsx?raw'
import SIDEBAR from './components/Sidebar.tsx?raw'

/**
 * A MAC SAYS CMD (0.848). First impressions on the Mac, 2026-11-01: the command palette listed "Ctrl 1" to "Ctrl 5"
 * and "Ctrl+/", and the shortcuts sheet "Ctrl+K", while every one of those chords takes Cmd on a Mac.
 */
describe('shortcut keys on a Mac', () => {
  it('says Cmd for the chords that take it, and Option for Alt', () => {
    expect(keysOn('Ctrl+K', 'darwin')).toBe('Cmd+K')
    expect(keysOn('Ctrl 1', 'darwin')).toBe('Cmd 1')
    expect(keysOn('Ctrl+Shift+M', 'darwin')).toBe('Cmd+Shift+M')
    expect(keysOn('Alt+Shift+Up / Alt+Shift+Down', 'darwin')).toBe('Option+Shift+Up / Option+Shift+Down')
    expect(keysOn('Ctrl+K', 'win32')).toBe('Ctrl+K')
    expect(keysOn('Ctrl+K', undefined)).toBe('Ctrl+K')
  })

  it('keeps Control where the handler listens for Control alone', () => {
    expect(SHORTCUTS.filter((row) => row.controlOnMac === true).map((row) => row.keys)).toEqual(['Ctrl+`', 'Ctrl+Alt+Up / Ctrl+Alt+Down', 'Ctrl+Alt+Space'])
    expect(keysOn('Ctrl+Alt+Space', 'darwin', true)).toBe('Ctrl+Option+Space')
    // What makes the labels true: the window's chords take Ctrl or Cmd; the terminal's and the rail's, Ctrl alone.
    expect(APP).toContain('const accel = event.ctrlKey || event.metaKey')
    expect(TERMINAL_KEYS).toContain('event.ctrlKey && !event.altKey && !event.metaKey')
    expect(PROMPT_RAIL).toContain('if (!event.ctrlKey || !event.altKey || event.metaKey || event.shiftKey) return undefined')
  })

  it("draws the shortcuts sheet in a Mac's words, and a PC's in its own", () => {
    const mac = renderToStaticMarkup(<KeyboardSheet onClose={() => undefined} platform="darwin" />)
    expect(mac).toContain('<kbd>Cmd+K</kbd>')
    expect(mac).toContain('<kbd>Ctrl+`</kbd>')
    expect(mac).not.toContain('<kbd>Ctrl+K</kbd>')
    expect(renderToStaticMarkup(<KeyboardSheet onClose={() => undefined} platform="win32" />)).toContain('<kbd>Ctrl+K</kbd>')
  })

  it('leaves no Ctrl written into a palette hint or a button title', () => {
    expect(APP).not.toMatch(/hint: 'Ctrl/)
    for (const source of [SIDEBAR, SCREENS]) expect(source).not.toMatch(/title="[^"]*Ctrl/)
    expect(COMPOSER).not.toContain('(Ctrl+Enter while typing)')
  })
})
