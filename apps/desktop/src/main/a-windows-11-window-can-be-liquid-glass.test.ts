import { existsSync, readFileSync } from 'node:fs'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { createWindowsGlass, windowsGlassAt } from './windows-glass.js'
import type { WindowsGlassWindow } from './windows-glass.js'

/**
 * A WINDOWS 11 WINDOW CAN BE LIQUID GLASS (0.864). Colin, 2026-11-03: "Is there a way to make locust glass work on other
 * operating systems ... Maybe just give it a shot?" On Windows 11 22H2+ the Look choice puts the window on Acrylic,
 * Windows' own blur of what is behind the window, and the page draws the sidebar's panel and the Rainbow itself.
 * Pinned here: offered only where Windows draws Acrylic; the window's ground is cleared before the material goes on
 * and put back exactly when it comes off; a failure leaves the window as it was made; and the main process, the page
 * and the sheet agree.
 */
const read = (relative: string): string => readFileSync(join(__dirname, relative), 'utf8')

const glassWindow = (calls: string[], overrides: Partial<WindowsGlassWindow> = {}): WindowsGlassWindow => ({
  setBackgroundMaterial: (material) => {
    calls.push(`material ${material}`)
  },
  setBackgroundColor: (color) => {
    calls.push(`ground ${color}`)
  },
  isDestroyed: () => false,
  ...overrides
})

describe('a Windows 11 window can be Liquid glass', () => {
  it('is offered only on Windows 11 22H2 (build 22621) and later', () => {
    expect(windowsGlassAt('win32', '10.0.22621')).toBe(true)
    expect(windowsGlassAt('win32', '10.0.26100')).toBe(true)
    expect(windowsGlassAt('win32', '10.0.22000')).toBe(false)
    expect(windowsGlassAt('win32', '10.0.19045')).toBe(false)
    expect(windowsGlassAt('win32', 'unknown')).toBe(false)
    expect(windowsGlassAt('darwin', '25.0.0')).toBe(false)
    expect(windowsGlassAt('linux', '10.0.26100')).toBe(false)
  })

  it('goes on Acrylic in place: the ground cleared first, and only once', () => {
    const calls: string[] = []
    const glass = createWindowsGlass('win32', () => '#141516', '10.0.26100')
    const window = glassWindow(calls)
    expect(glass.offered).toBe(true)
    expect(glass.apply(window, true)).toBe(true)
    expect(glass.apply(window, true)).toBe(true)
    expect(calls).toEqual(['ground #00000000', 'material acrylic'])
    expect(glass.drawn(window)).toBe(true)
  })

  it('comes off exactly: no material, and the ground of the theme now', () => {
    const calls: string[] = []
    let ground = '#141516'
    const glass = createWindowsGlass('win32', () => ground, '10.0.22621')
    const window = glassWindow(calls)
    glass.apply(window, true)
    ground = '#f6f5f2'
    expect(glass.apply(window, false)).toBe(false)
    expect(calls.slice(2)).toEqual(['material none', 'ground #f6f5f2'])
    expect(glass.drawn(window)).toBe(false)
    // Standard on a window that was never on the glass touches nothing.
    const untouched: string[] = []
    glass.apply(glassWindow(untouched), false)
    expect(untouched).toEqual([])
  })

  it('a failure leaves the window as it was made, and a computer without the glass is never touched', () => {
    const calls: string[] = []
    const glass = createWindowsGlass('win32', () => '#141516', '10.0.26100')
    const window = glassWindow(calls, { setBackgroundMaterial: () => { throw new Error('not here') } })
    expect(glass.apply(window, true)).toBe(false)
    expect(calls).toEqual(['ground #00000000', 'ground #141516'])
    expect(glass.drawn(window)).toBe(false)
    for (const [platform, osRelease] of [['win32', '10.0.19045'], ['darwin', '25.0.0'], ['linux', '6.8.0']] as const) {
      const none: string[] = []
      const off = createWindowsGlass(platform, () => '#141516', osRelease)
      expect(off.offered).toBe(false)
      expect(off.apply(glassWindow(none), true)).toBe(false)
      expect(none).toEqual([])
    }
  })

  it('the main process offers it, draws it on the main window, and keeps a drawn window clear through a theme change', () => {
    const main = read('index.ts')
    expect(main).toContain('liquidOffered: macGlass().offered || windowsGlass().offered,')
    expect(main).toContain("if (process.platform === 'win32') {\n    windowsGlass().apply(window, drawsLiquid(appearanceNow()))")
    expect(main).toContain('if (!glassWindows.has(target) && !windowsGlass().drawn(target)) target.setBackgroundColor(windowGround(state.resolved))')
    expect(main).toContain("(process.platform === 'darwin' || process.platform === 'win32') && drawsLiquid(appearanceNow()) ? [`${LOOK_ARGUMENT}liquid`] : []")
    expect(main.match(/if \(liquidWindows\.has\(target\)\) drawGlass\(target\)/g)).toHaveLength(2)
    expect(main).not.toMatch(/if \(glassWindows\.has\(target\)\) drawGlass\(target\)/)
  })

  it('the page marks only the Windows window, and the sheet clears its grounds and paints the Rainbow', () => {
    const page = read('../renderer/src/main.tsx')
    expect(page).toContain("window.desktop?.platform === 'win32' && window.desktop.remote === undefined")
    expect(page).toContain("document.documentElement.dataset.glass = 'windows'")
    const sheet = read('../renderer/src/shell.css')
    const windows = sheet.slice(sheet.indexOf('LIQUID GLASS ON WINDOWS'))
    expect(windows).toMatch(/:root\[data-glass='windows'\]\[data-look='liquid'\] body,\s*:root\[data-glass='windows'\]\[data-look='liquid'\] \.lc-body \{\s*background: transparent;/)
    expect(windows).toContain("[data-glass-ground='rainbow'] body {\n  background: #1b1233 url('./assets/glass-rainbow.jpg') center / cover no-repeat;")
    expect(existsSync(join(__dirname, '../renderer/src/assets/glass-rainbow.jpg'))).toBe(true)
    // No pane of glass darkens the sidebar on Windows, so its wash is stronger; and over the page's own Rainbow the
    // composer is not frosted, which would saturate the picture under its thin fill.
    expect(windows).toMatch(/:root\[data-glass='windows'\]\[data-look='liquid'\] \{\s*--lc-liquid-panel: rgba\(13, 14, 15, 0\.58\);/)
    expect(windows).toMatch(/\[data-glass-ground='rainbow'\] \.lc-composer__box \{\s*-webkit-backdrop-filter: none;\s*backdrop-filter: none;/)
    // The panel's wash lies under the sidebar's words (0.864): positioned over them, it dimmed the footer.
    const wash = sheet.slice(sheet.indexOf(":root[data-look='liquid'] .lc-shell::before {"))
    expect(wash.slice(0, wash.indexOf('}'))).toContain('z-index: -1;')
  })
})
