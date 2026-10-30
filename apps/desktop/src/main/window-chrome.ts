/**
 * THE MAIN WINDOW'S FRAME, PER PLATFORM.
 *
 * On Windows and Linux the window is frameless and the title bar (TitleBar.tsx)
 * draws its own minimize, maximize and close at the right. On a Mac it is
 * macOS's own traffic lights: the first Mac run (2026-10-30) found the frameless
 * window had none, and Windows' three buttons drawn at the right instead. So a
 * Mac keeps its frame with the title bar hidden and inset, and the lights are
 * placed in the bar's empty left column, centered on its 38 px height.
 */
export type WindowChrome =
  | { readonly frame: false; readonly titleBarStyle: 'hidden'; readonly titleBarOverlay: false }
  | { readonly titleBarStyle: 'hiddenInset'; readonly trafficLightPosition: { readonly x: number; readonly y: number } }

export function windowChrome(platform: NodeJS.Platform): WindowChrome {
  return platform === 'darwin'
    ? { titleBarStyle: 'hiddenInset', trafficLightPosition: { x: 14, y: 13 } }
    : { frame: false, titleBarStyle: 'hidden', titleBarOverlay: false }
}
