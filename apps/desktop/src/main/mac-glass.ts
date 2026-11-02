import type { GlassPanel } from '../shared/appearance.js'
import { STANDARD_LIGHTS } from './window-chrome.js'

/**
 * LIQUID GLASS (0.855). Colin, 2026-11-02: "a way for whole app to be liquid glass ... like an appearance toggle".
 *
 * The Mac window was already made over the sidebar material (window-chrome.ts), and that glass only shows what is
 * behind the window: over a dark desktop, or behind another window, it is a flat grey. macOS 26 drew its own apps
 * another way -- the window on glass, the sidebar a floating panel of it, the controls glass pills -- and those
 * shapes read as glass even over black. native/mac-glass.mm puts two of macOS's own glass views behind the page: the
 * window's ground and the sidebar's panel. The page lets them through (shell.css `[data-look='liquid']`).
 *
 * The window's own material is taken off while Liquid glass is on and put back when it is off, so the look changes
 * in place, with no new window. Anything that fails leaves the window as it was made.
 */
export interface MacGlassAddon {
  supported(): boolean
  attach(handle: Buffer): boolean
  panel(handle: Buffer, panel: GlassPanel | null): void
  detach(handle: Buffer): void
  backdrop(handle: Buffer, path: string): void
}

/** The part of a BrowserWindow this needs. */
export interface GlassWindow {
  getNativeWindowHandle(): Buffer
  setVibrancy(type: 'sidebar' | null): void
  setWindowButtonPosition(position: { x: number; y: number } | null): void
  isDestroyed(): boolean
}

/** The traffic lights inside the sidebar panel's corner, centred on Liquid glass's taller title bar (shell.css). */
export const LIQUID_LIGHTS = { x: 20, y: 18 }

export interface MacGlass {
  /** macOS's glass is here and this build can reach it. */
  readonly offered: boolean
  /** Draws the window in Liquid glass, or as it was made; true when it is drawn in Liquid glass now. */
  apply(window: GlassWindow, liquid: boolean): boolean
  /** Places the sidebar's panel, or takes it away. */
  panel(window: GlassWindow, panel: GlassPanel | null): void
}

/** A panel worth placing: finite, positive, and inside a sane window. */
export function readGlassPanel(value: unknown): GlassPanel | null {
  if (typeof value !== 'object' || value === null) return null
  const { x, y, width, height, radius } = value as Record<string, unknown>
  const sane = (n: unknown, max: number): n is number => typeof n === 'number' && Number.isFinite(n) && n >= 0 && n <= max
  if (!sane(x, 20_000) || !sane(y, 20_000) || !sane(width, 20_000) || !sane(height, 20_000) || !sane(radius, 200)) return null
  if (width === 0 || height === 0) return null
  return { x, y, width, height, radius }
}

/**
 * `backdrop`: a picture put under the glass inside the window, for a drive (LOCUST_GLASS_BACKDROP, development builds
 * only; drive-liquid-glass.mjs). A path to a picture, or `dark`.
 */
export function createMacGlass(platform: NodeJS.Platform, load: () => MacGlassAddon, backdrop?: string): MacGlass {
  let addon: MacGlassAddon | undefined
  if (platform === 'darwin') {
    try {
      const loaded = load()
      if (loaded.supported()) addon = loaded
    } catch {
      addon = undefined
    }
  }
  const drawn = new WeakSet<GlassWindow>()
  return {
    offered: addon !== undefined,
    apply(window, liquid) {
      if (addon === undefined || window.isDestroyed()) return false
      const handle = window.getNativeWindowHandle()
      if (liquid) {
        if (drawn.has(window)) return true
        try {
          // The material first: left on, it draws over the glass and blurs it again.
          window.setVibrancy(null)
          if (addon.attach(handle)) {
            drawn.add(window)
            window.setWindowButtonPosition(LIQUID_LIGHTS)
            if (backdrop !== undefined && backdrop !== '') addon.backdrop(handle, backdrop)
            return true
          }
        } catch {
          // Put back below.
        }
        window.setVibrancy('sidebar')
        return false
      }
      if (drawn.has(window)) {
        try {
          addon.detach(handle)
        } catch {
          // The material goes back on regardless, over whatever is left.
        }
        drawn.delete(window)
        window.setVibrancy('sidebar')
        window.setWindowButtonPosition(STANDARD_LIGHTS)
      }
      return false
    },
    panel(window, panel) {
      if (addon === undefined || window.isDestroyed() || !drawn.has(window)) return
      try {
        addon.panel(window.getNativeWindowHandle(), panel)
      } catch {
        // No panel: the sidebar sits on the window's glass.
      }
    }
  }
}
