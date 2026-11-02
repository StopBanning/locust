/**
 * THE MAIN WINDOW'S FRAME, PER PLATFORM.
 *
 * On Windows and Linux the window is frameless and the title bar (TitleBar.tsx)
 * draws its own minimize, maximize and close at the right. On a Mac it is
 * macOS's own traffic lights: the first Mac run (2026-10-30) found the frameless
 * window had none, and Windows' three buttons drawn at the right instead. So a
 * Mac keeps its frame with the title bar hidden and inset, and the lights are
 * placed in the bar's empty left column, centered on its 38 px height.
 *
 * A MAC'S WINDOW IS GLASS BEHIND THE PAGE (polish pass, 2026-11-01): the sidebar material, as Finder, Notes and Mail
 * are made, following the window's theme (nativeTheme.themeSource is the person's choice). The page lets it through
 * the sidebar and the title bar and paints every other pane its own ground (shell.css `[data-glass='mac']`, set in
 * main.tsx), and the window's own ground is clear so the glass is what shows (MAC_GLASS_GROUND, index.ts).
 */
export type WindowChrome =
  | { readonly frame: false; readonly titleBarStyle: 'hidden'; readonly titleBarOverlay: false }
  | {
      readonly titleBarStyle: 'hiddenInset'
      readonly trafficLightPosition: { readonly x: number; readonly y: number }
      readonly vibrancy: 'sidebar'
      readonly visualEffectState: 'followWindow'
    }

/** A Mac window's own ground: clear, so the glass behind the page shows where the page lets it through. */
export const MAC_GLASS_GROUND = '#00000000'

/** Where a Mac's traffic lights sit: centred on the 38px title bar's empty left column. */
export const STANDARD_LIGHTS = { x: 14, y: 13 }

export function windowChrome(platform: NodeJS.Platform): WindowChrome {
  return platform === 'darwin'
    ? { titleBarStyle: 'hiddenInset', trafficLightPosition: { ...STANDARD_LIGHTS }, vibrancy: 'sidebar', visualEffectState: 'followWindow' }
    : { frame: false, titleBarStyle: 'hidden', titleBarOverlay: false }
}
