import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { MAC_GLASS_GROUND, windowChrome } from './window-chrome.js'

/**
 * A MAC'S SIDEBAR IS GLASS (polish pass, 2026-11-01). Colin, before the launch: "world class apple quality". On a Mac
 * the window is made with the sidebar material behind the page, as Finder, Notes and Mail are, and the page lets it
 * through the sidebar and the title bar only. Four places have to agree, and each is pinned here: the window is made
 * with the material, its ground is clear (and stays clear through a theme change), the page marks itself only in the
 * Mac window, and the sheet lets the glass through two places while every other pane keeps a ground.
 */
const read = (relative: string): string => readFileSync(join(__dirname, relative), 'utf8')
const MAIN = read('index.ts')
const RENDERER_MAIN = read('../renderer/src/main.tsx')
const SHELL = read('../renderer/src/shell.css')
const TOKENS = read('../renderer/src/tokens.css')

describe("a Mac's sidebar is glass", () => {
  it('a Mac window is made with the sidebar material, following the window; Windows and Linux get none', () => {
    expect(windowChrome('darwin')).toMatchObject({ vibrancy: 'sidebar', visualEffectState: 'followWindow' })
    for (const platform of ['win32', 'linux'] as const) expect('vibrancy' in windowChrome(platform)).toBe(false)
  })

  it("its ground is clear, set after the theme's, and a theme change does not paint over it", () => {
    expect(MAC_GLASS_GROUND).toBe('#00000000')
    const made = MAIN.slice(MAIN.indexOf('const window = new BrowserWindow({'), MAIN.indexOf('webPreferences: {', MAIN.indexOf('const window = new BrowserWindow({')))
    expect(made.indexOf("...(process.platform === 'darwin' ? { backgroundColor: MAC_GLASS_GROUND } : {})")).toBeGreaterThan(made.indexOf('backgroundColor: windowGround(appearanceNow().resolved)'))
    expect(MAIN).toContain("if (process.platform === 'darwin') glassWindows.add(window)")
    // A Windows window on Acrylic stays clear too (windows-glass.ts, 0.864).
    expect(MAIN).toContain('if (!glassWindows.has(target) && !windowsGlass().drawn(target)) target.setBackgroundColor(windowGround(state.resolved))')
  })

  it('the page marks itself glass only in the Mac window, never in a browser, the loading window or the cover', () => {
    expect(RENDERER_MAIN).toContain("if (!isSplash && !isElsewhere && window.desktop?.platform === 'darwin' && window.desktop.remote === undefined) {")
    expect(RENDERER_MAIN).toContain("document.documentElement.dataset.glass = 'mac'")
  })

  it('the sheet lets the glass through the sidebar and the title bar, and every other pane keeps a ground', () => {
    expect(SHELL).toMatch(/:root\[data-glass='mac'\] body,\s*:root\[data-glass='mac'\] \.lc-shell,\s*:root\[data-glass='mac'\] \.lc-body \{\s*background: transparent;/)
    expect(SHELL).toMatch(/:root\[data-glass='mac'\] \.lc-titlebar,\s*:root\[data-glass='mac'\] \.lc-sidebar \{\s*background: var\(--lc-glass-tint\);/)
    expect(SHELL).toMatch(/:where\(:root\[data-glass='mac'\] \.lc-body > :not\(\.lc-sidebar, \.lc-context\)\) \{\s*background: var\(--lc-bg-window\);/)
    // A wash, not a fill: the glass must come through in both themes.
    for (const tint of TOKENS.matchAll(/--lc-glass-tint: rgba\(\d+, \d+, \d+, ([\d.]+)\);/g)) expect(Number(tint[1])).toBeLessThan(0.6)
    expect([...TOKENS.matchAll(/--lc-glass-tint:/g)]).toHaveLength(2)
  })
})
