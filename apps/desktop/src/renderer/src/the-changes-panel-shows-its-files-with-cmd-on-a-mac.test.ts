import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { isShowFilesChord, showFilesChordLabel } from './changesKeys.js'

/*
 * THE CHANGES PANEL'S "SHOW THE FILES" TAKES CMD ON A MAC (F24, the first Mac run, 2026-10-30). It listened for Ctrl
 * alone, so on a Mac it was the one chord that never took Cmd, and its title said Ctrl+Shift+Y there too.
 */
const press = (keys: { ctrl?: boolean; meta?: boolean; alt?: boolean; shift?: boolean; key?: string }) => ({
  ctrlKey: keys.ctrl === true, metaKey: keys.meta === true, altKey: keys.alt === true, shiftKey: keys.shift !== false, key: keys.key ?? 'Y'
})

describe("the Changes panel's show-the-files chord", () => {
  it('on a Mac is Cmd+Shift+Y, and Ctrl+Shift+Y is not it', () => {
    expect(isShowFilesChord(press({ meta: true }), 'darwin')).toBe(true)
    expect(isShowFilesChord(press({ meta: true, key: 'y' }), 'darwin')).toBe(true)
    expect(isShowFilesChord(press({ ctrl: true }), 'darwin')).toBe(false)
  })

  it('on Windows and Linux stays Ctrl+Shift+Y, as it always was', () => {
    for (const platform of ['win32', 'linux', undefined]) {
      expect(isShowFilesChord(press({ ctrl: true }), platform)).toBe(true)
      expect(isShowFilesChord(press({ meta: true }), platform)).toBe(false)
    }
  })

  it('wants Shift and Y, and no Alt, everywhere', () => {
    for (const [platform, mod] of [['darwin', { meta: true }], ['win32', { ctrl: true }]] as const) {
      expect(isShowFilesChord(press({ ...mod, shift: false }), platform)).toBe(false)
      expect(isShowFilesChord(press({ ...mod, alt: true }), platform)).toBe(false)
      expect(isShowFilesChord(press({ ...mod, key: 'T' }), platform)).toBe(false)
    }
  })

  it("the button's title names the chord this machine takes", () => {
    expect(showFilesChordLabel('darwin')).toBe('Cmd+Shift+Y')
    expect(showFilesChordLabel('win32')).toBe('Ctrl+Shift+Y')
  })

  it('is what the panel listens for and what its button says', () => {
    const panel = readFileSync(join(__dirname, 'components', 'ChangesPanel.tsx'), 'utf8')
    expect(panel).toContain('if (isShowFilesChord(event, window.desktop?.platform)) {')
    expect(panel).toContain('title={`Show the files (${showFilesChordLabel(window.desktop?.platform)})`}')
    expect(panel).not.toContain('Ctrl+Shift+Y')
  })
})
