import { renderToStaticMarkup } from 'react-dom/server'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

import { LookSetting, lookLede } from './components/LookSetting.js'
import { glassPanelFor, GLASS_INSET, GLASS_RADIUS } from './glassPanel.js'

/**
 * Liquid glass (0.855) is a choice only in the window of a Mac that has macOS's glass. The row draws nothing until the
 * window says the glass is offered, and nothing in a browser showing Locust; what it says names what changes.
 */
describe('the look is offered only where it can be drawn', () => {
  it('draws nothing before the window has said the glass is offered', () => {
    expect(renderToStaticMarkup(<LookSetting />)).toBe('')
  })

  it('says what each look is, and that Liquid glass shows the desktop through it', () => {
    expect(lookLede('liquid')).toMatch(/glass/)
    expect(lookLede('liquid')).toMatch(/desktop/)
    expect(lookLede('standard')).toMatch(/everywhere/)
  })

  it('is hidden in a browser, and sits under the theme on the Appearance page', () => {
    const row = readFileSync(join(__dirname, 'components/LookSetting.tsx'), 'utf8')
    expect(row).toContain("if (state?.liquidOffered !== true || window.desktop?.remote !== undefined) return null")
    const screens = readFileSync(join(__dirname, 'components/Screens.tsx'), 'utf8')
    expect(screens.indexOf("{shownPage === 'appearance' && <LookSetting />}")).toBeGreaterThan(screens.indexOf('<ThemeSetting />'))
  })

  it("the panel runs from the window's top inset to its bottom inset, as wide as the sidebar, or is none", () => {
    expect(glassPanelFor({ right: 276, width: 276 }, 800)).toEqual({ x: GLASS_INSET, y: GLASS_INSET, width: 276 - GLASS_INSET, height: 800 - 2 * GLASS_INSET, radius: GLASS_RADIUS })
    expect(glassPanelFor({ right: 82, width: 82 }, 660)?.width).toBe(82 - GLASS_INSET)
    expect(glassPanelFor(undefined, 800)).toBeNull()
    expect(glassPanelFor({ right: 0, width: 0 }, 800)).toBeNull()
    expect(glassPanelFor({ right: 276, width: 276 }, 20)).toBeNull()
  })

  it('the panel the page asks for is the shape the sheet washes', () => {
    const shell = readFileSync(join(__dirname, 'shell.css'), 'utf8')
    const liquid = shell.slice(shell.indexOf(":root[data-look='liquid'] {"))
    expect(liquid).toContain(`--lc-liquid-inset: ${String(GLASS_INSET)}px;`)
    expect(liquid).toContain(`--lc-liquid-radius: ${String(GLASS_RADIUS)}px;`)
  })
})
