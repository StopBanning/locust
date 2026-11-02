import { mkdtempSync, readFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { readLook, writeAppearance } from './appearance.js'
import { createMacGlass, LIQUID_LIGHTS, readGlassPanel } from './mac-glass.js'
import type { GlassWindow, MacGlassAddon } from './mac-glass.js'
import { STANDARD_LIGHTS } from './window-chrome.js'
import { drawsLiquid, liquidFromArguments, LOOK_ARGUMENT } from '../shared/appearance.js'

/**
 * THE WHOLE WINDOW CAN BE LIQUID GLASS (0.855). Colin, 2026-11-02: "a way for whole app to be liquid glass ... like an
 * appearance toggle". On a Mac with macOS 26's glass, Settings > Appearance > Look puts the window on two of macOS's
 * own glass views (native/mac-glass.mm): the window's ground and a floating panel under the sidebar. Pinned here: the
 * choice is kept beside the theme and never drops it; the window changes in place and goes back exactly; the glass is
 * offered only where it can be drawn; the panel is placed where the page says, inside sane bounds; and the build,
 * the sheet and the window agree on where everything sits.
 */
const read = (relative: string): string => readFileSync(join(__dirname, relative), 'utf8')

const addon = (overrides: Partial<MacGlassAddon> = {}): MacGlassAddon & { calls: string[] } => {
  const calls: string[] = []
  return {
    calls,
    supported: () => true,
    attach: () => {
      calls.push('attach')
      return true
    },
    panel: (_handle, panel) => {
      calls.push(`panel ${panel === null ? 'none' : `${String(panel.width)}x${String(panel.height)}`}`)
    },
    detach: () => {
      calls.push('detach')
    },
    backdrop: (_handle, path) => {
      calls.push(`backdrop ${path}`)
    },
    ...overrides
  }
}

const glassWindow = (calls: string[]): GlassWindow => ({
  getNativeWindowHandle: () => Buffer.alloc(8),
  setVibrancy: (type) => {
    calls.push(`vibrancy ${String(type)}`)
  },
  setWindowButtonPosition: (position) => {
    calls.push(`lights ${position === null ? 'default' : `${String(position.x)},${String(position.y)}`}`)
  },
  isDestroyed: () => false
})

describe('the whole window can be Liquid glass', () => {
  it('the look is kept beside the theme, Standard when it is missing or unreadable, and saving one keeps the other', () => {
    expect(readLook('/nowhere', () => { throw new Error('ENOENT') })).toBe('standard')
    expect(readLook('/f', () => 'not json')).toBe('standard')
    expect(readLook('/f', () => JSON.stringify({ theme: 'light' }))).toBe('standard')
    expect(readLook('/f', () => JSON.stringify({ look: 'frosted' }))).toBe('standard')
    expect(readLook('/f', () => JSON.stringify({ theme: 'dark', look: 'liquid' }))).toBe('liquid')
    const file = join(mkdtempSync(join(tmpdir(), 'locust-look-')), 'appearance.json')
    writeAppearance(file, 'light', 'liquid')
    expect(JSON.parse(readFileSync(file, 'utf8'))).toEqual({ theme: 'light', look: 'liquid' })
    expect(readLook(file)).toBe('liquid')
  })

  it('a window is drawn in it only when it is chosen and offered, and starts marked so from its own command line', () => {
    expect(drawsLiquid({ look: 'liquid', liquidOffered: true })).toBe(true)
    expect(drawsLiquid({ look: 'liquid', liquidOffered: false })).toBe(false)
    expect(drawsLiquid({ look: 'standard', liquidOffered: true })).toBe(false)
    expect(liquidFromArguments(['--x', `${LOOK_ARGUMENT}liquid`])).toBe(true)
    expect(liquidFromArguments([`${LOOK_ARGUMENT}standard`])).toBe(false)
    expect(liquidFromArguments([])).toBe(false)
  })

  it('is offered only on a Mac whose glass the addon can reach', () => {
    expect(createMacGlass('win32', () => addon()).offered).toBe(false)
    expect(createMacGlass('linux', () => addon()).offered).toBe(false)
    expect(createMacGlass('darwin', () => { throw new Error('no addon in this build') }).offered).toBe(false)
    expect(createMacGlass('darwin', () => addon({ supported: () => false })).offered).toBe(false)
    expect(createMacGlass('darwin', () => addon()).offered).toBe(true)
  })

  it('changes the window in place: its material off before the glass goes on, and the lights into the panel', () => {
    const native = addon()
    const glass = createMacGlass('darwin', () => native)
    const window = glassWindow(native.calls)
    expect(glass.apply(window, true)).toBe(true)
    expect(native.calls).toEqual(['vibrancy null', 'attach', `lights ${String(LIQUID_LIGHTS.x)},${String(LIQUID_LIGHTS.y)}`])
    // Asked again, nothing is laid twice.
    expect(glass.apply(window, true)).toBe(true)
    expect(native.calls).toHaveLength(3)
  })

  it('goes back exactly: the glass off, the material and the lights where the window was made with them', () => {
    const native = addon()
    const glass = createMacGlass('darwin', () => native)
    const window = glassWindow(native.calls)
    glass.apply(window, true)
    native.calls.length = 0
    expect(glass.apply(window, false)).toBe(false)
    expect(native.calls).toEqual(['detach', 'vibrancy sidebar', `lights ${String(STANDARD_LIGHTS.x)},${String(STANDARD_LIGHTS.y)}`])
    // A window that was never drawn in it is left alone.
    native.calls.length = 0
    expect(glass.apply(glassWindow(native.calls), false)).toBe(false)
    expect(native.calls).toEqual([])
  })

  it('a glass that will not go on leaves the window as it was made', () => {
    const refused = addon({ attach: () => false })
    const window = glassWindow(refused.calls)
    expect(createMacGlass('darwin', () => refused).apply(window, true)).toBe(false)
    expect(refused.calls).toEqual(['vibrancy null', 'vibrancy sidebar'])
    const threw = addon({ attach: () => { throw new Error('gone') } })
    expect(createMacGlass('darwin', () => threw).apply(glassWindow(threw.calls), true)).toBe(false)
    expect(threw.calls).toEqual(['vibrancy null', 'vibrancy sidebar'])
    // Nothing at all off a Mac.
    const elsewhere = addon()
    expect(createMacGlass('win32', () => elsewhere).apply(glassWindow(elsewhere.calls), true)).toBe(false)
    expect(elsewhere.calls).toEqual([])
  })

  it("places the sidebar's panel only on a window drawn in it, and a drive's picture only when one is given", () => {
    const native = addon()
    const glass = createMacGlass('darwin', () => native, '/pictures/sonoma.jpg')
    const window = glassWindow([])
    glass.panel(window, { x: 8, y: 8, width: 268, height: 784, radius: 16 })
    expect(native.calls).toEqual([])
    glass.apply(window, true)
    glass.panel(window, { x: 8, y: 8, width: 268, height: 784, radius: 16 })
    glass.panel(window, null)
    expect(native.calls).toEqual(['attach', 'backdrop /pictures/sonoma.jpg', 'panel 268x784', 'panel none'])
    const plain = addon()
    createMacGlass('darwin', () => plain).apply(glassWindow([]), true)
    expect(plain.calls).toEqual(['attach'])
  })

  it('takes a panel from the page only when every number is finite, positive and sane', () => {
    expect(readGlassPanel({ x: 8, y: 8, width: 268, height: 784, radius: 16 })).toEqual({ x: 8, y: 8, width: 268, height: 784, radius: 16 })
    for (const junk of [null, 'panel', {}, { x: 8, y: 8, width: 0, height: 784, radius: 16 }, { x: -1, y: 8, width: 268, height: 784, radius: 16 },
      { x: 8, y: 8, width: Number.NaN, height: 784, radius: 16 }, { x: 8, y: 8, width: 268, height: 1e9, radius: 16 }, { x: 8, y: 8, width: 268, height: 784, radius: 999 },
      { x: '8', y: 8, width: 268, height: 784, radius: 16 }]) {
      expect(readGlassPanel(junk)).toBeNull()
    }
  })

  it('the sheet and the window agree on where the lights sit', () => {
    const shell = read('../renderer/src/shell.css')
    const liquid = shell.slice(shell.indexOf(":root[data-look='liquid'] {"))
    const px = (name: string): number => Number(new RegExp(`${name}: (\\d+)px;`).exec(liquid)?.[1])
        // The lights, 14px tall, centred on the taller title bar, and inside the panel's corner -- the rail's too.
    expect(LIQUID_LIGHTS.y * 2 + 14).toBe(px('--lc-titlebar-height'))
    expect(LIQUID_LIGHTS.x).toBeGreaterThan(px('--lc-liquid-inset'))
    expect(LIQUID_LIGHTS.x + 52).toBeLessThan(px('--lc-sidebar-rail-width'))
    // The sidebar and the title bar paint nothing; what floats is frosted; the composer is a glass pill.
    expect(liquid).toMatch(/:root\[data-look='liquid'\] \.lc-titlebar,\s*:root\[data-look='liquid'\] \.lc-sidebar \{\s*background: transparent;/)
    expect(liquid).toMatch(/\.lc-dialog, \.lc-palette, \.lc-menu[^{]*\{\s*background: var\(--lc-liquid-float\);\s*-webkit-backdrop-filter: var\(--lc-liquid-blur\);/)
    expect(liquid).toMatch(/:root\[data-look='liquid'\] \.lc-composer__box \{\s*-webkit-backdrop-filter: var\(--lc-liquid-blur\);/)
    // Light has its own washes.
    expect(liquid).toContain(":root[data-look='liquid'][data-theme='light'] {")
  })

  it('the addon is built after the bundle on a Mac, unpacked from the archive, loaded from beside the main bundle', () => {
    const pkg = JSON.parse(read('../../package.json')) as { scripts: Record<string, string> }
    for (const script of ['build', 'package', 'package:dir']) expect(pkg.scripts[script]).toMatch(/^electron-vite build && node scripts\/build-mac-glass\.cjs/)
    expect(read('../../electron-builder.yml')).toMatch(/asarUnpack:[\s\S]*- out\/native\/\*\*\/\*/)
    const main = read('index.ts')
    expect(main).toContain("createRequire(import.meta.url)(join(__dirname, '../native/mac-glass.node'))")
    // A drive's picture under the glass never reaches a build a person installs.
    expect(main).toContain('app.isPackaged ? undefined : process.env.LOCUST_GLASS_BACKDROP')
    const build = read('../../scripts/build-mac-glass.cjs')
    expect(build).toContain("if (process.platform !== 'darwin') process.exit(0)")
    expect(build).toContain("'-arch', 'arm64', '-arch', 'x86_64'")
    expect(build).toContain('if (process.env.CI) process.exit(1)')
  })
})
