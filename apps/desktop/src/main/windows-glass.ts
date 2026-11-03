import { release } from 'node:os'

/**
 * LIQUID GLASS ON WINDOWS (0.864). Colin, 2026-11-03: "Is there a way to make locust glass work on other operating
 * systems ... Maybe just give it a shot?"
 *
 * Apple's glass is the window server blurring what is behind the window; on Windows 11 the window manager does the same
 * (Acrylic), and Electron reaches it (setBackgroundMaterial). Windows has no second pane of glass inside a window, so
 * the sidebar's floating panel is the page's own wash and rim over the Acrylic (shell.css `.lc-shell::before`), which
 * is how the Mac's panel reads anyway. Rainbow is the page's too: the picture under everything, where a Mac puts it
 * under the glass (shell.css `[data-glass='windows'][data-glass-ground='rainbow']`). Proved on Windows 11 24H2 by
 * locust-glass's screens workflow (StopBanning/locust-glass).
 *
 * Acrylic is tinted by the app's theme, which already follows the person's choice (nativeTheme.themeSource). With no
 * graphics card, or Transparency effects off, Windows paints Acrylic's plain stand-in colour instead of the blur, and
 * the window reads as a quiet frosted one. Anything that fails leaves the window as it was made.
 */

/** The part of a BrowserWindow this needs. */
export interface WindowsGlassWindow {
  setBackgroundMaterial(material: 'acrylic' | 'none'): void
  setBackgroundColor(color: string): void
  isDestroyed(): boolean
}

export interface WindowsGlass {
  /** Windows 11's glass is here. */
  readonly offered: boolean
  /** Draws the window on Acrylic, or as it was made; true when it is on Acrylic now. */
  apply(window: WindowsGlassWindow, liquid: boolean): boolean
  /** True while the window is on Acrylic: its own ground stays clear through a theme change. */
  drawn(window: WindowsGlassWindow): boolean
}

/** Windows 11 22H2 (build 22621) and later: where the window manager draws Acrylic behind a window. */
export function windowsGlassAt(platform: NodeJS.Platform, osRelease: string): boolean {
  if (platform !== 'win32') return false
  const [major, , build] = osRelease.split('.').map(Number)
  return major === 10 && Number.isFinite(build) && (build ?? 0) >= 22621
}

/** `ground`: the window's own ground in the theme now (appearance.ts windowGround), put back when the glass comes off. */
export function createWindowsGlass(platform: NodeJS.Platform, ground: () => string, osRelease: string = release()): WindowsGlass {
  const offered = windowsGlassAt(platform, osRelease)
  const onGlass = new WeakSet<WindowsGlassWindow>()
  return {
    offered,
    apply(window, liquid) {
      if (!offered || window.isDestroyed()) return false
      if (liquid) {
        if (onGlass.has(window)) return true
        try {
          // Clear first: Acrylic is drawn under the window's ground, so an opaque one would hide it.
          window.setBackgroundColor('#00000000')
          window.setBackgroundMaterial('acrylic')
          onGlass.add(window)
          return true
        } catch {
          // Put back below.
        }
        window.setBackgroundColor(ground())
        return false
      }
      if (onGlass.has(window)) {
        try {
          window.setBackgroundMaterial('none')
        } catch {
          // The ground goes back on regardless, over whatever is left.
        }
        onGlass.delete(window)
        window.setBackgroundColor(ground())
      }
      return false
    },
    drawn: (window) => onGlass.has(window)
  }
}
