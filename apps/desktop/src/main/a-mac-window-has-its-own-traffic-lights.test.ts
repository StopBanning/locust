import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { windowChrome } from './window-chrome.js'

/**
 * A MAC WINDOW HAS ITS OWN TRAFFIC LIGHTS (the first Mac run, 2026-10-30).
 *
 * The main window was frameless everywhere, so on a Mac it had no red, yellow and green buttons, and the title bar
 * drew Windows' minimize, maximize and close at the right instead. The window's other half is TitleBar.tsx, which
 * draws its own three buttons everywhere but a Mac and a browser (the-title-bar-is-a-macs-on-a-mac.test.tsx).
 */
describe("the main window's frame", () => {
  it('on a Mac keeps its frame, with the title bar hidden and inset and the lights in its left column', () => {
    const mac = windowChrome('darwin')
    expect(mac).toEqual({ titleBarStyle: 'hiddenInset', trafficLightPosition: { x: 14, y: 13 } })
    expect('frame' in mac).toBe(false)
  })

  it('on Windows and Linux stays frameless, its own controls drawn by the title bar (control)', () => {
    for (const platform of ['win32', 'linux'] as const) {
      expect(windowChrome(platform)).toEqual({ frame: false, titleBarStyle: 'hidden', titleBarOverlay: false })
    }
  })

  it('is what the main window is made with', () => {
    const main = readFileSync(join(__dirname, 'index.ts'), 'utf8')
    const window = main.slice(main.indexOf('const window = new BrowserWindow({'), main.indexOf('backgroundColor: windowGround(appearanceNow().resolved)'))
    expect(window).toContain('...windowChrome(process.platform)')
    expect(window).not.toContain('frame: false')
  })
})
