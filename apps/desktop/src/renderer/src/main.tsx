import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import App from './App'
import { SplashApp } from './SplashApp'
import { ElsewhereApp } from './ElsewhereApp'
import { WindowFallback } from './components/WindowFallback'
import { watchWindowPresence } from './windowPresence'
import { watchGlassPanel } from './glassPanel'
import { drawsLiquid } from '../../shared/appearance.js'
// Tokens first: the shell's palette, type and motion, plus the vendored Geist
// faces. Then the shell itself, which consumes only those tokens.
import './tokens.css'
import './shell.css'

/*
 * Two windows, one bundle.
 *
 * The loading window is a second `BrowserWindow` on the same renderer with
 * `#splash` on the URL. One bundle rather than a second entry point, so
 * there is no way for the boot screen to exist in two builds that drift.
 */
const isSplash = window.location.hash === '#splash'
/** This computer's window while Locust is open in a browser (0.830, shared/desk.ts): a cover, with no app behind it. */
const isElsewhere = new URLSearchParams(window.location.search).get('elsewhere') === '1'

/*
 * THE PAGE ITSELF HAS TO BE TRANSPARENT, not just the window.
 *
 * Making the BrowserWindow transparent was not enough and the black square
 * stayed exactly where it was (Colin, twice). `shell.css` paints `body`
 * with `--lc-bg-app`, and the loading window loads the same stylesheet --
 * so the window was see-through and the page drawn on it was not.
 *
 * Marked on the root element rather than by a wrapper, because `html` and
 * `body` are above anything a component can reach.
 */
if (isSplash) document.documentElement.classList.add('is-splash')

// The theme (0.789, main/appearance.ts): the one this window was started in, at once, then whatever it becomes.
// The loading window stays the dark boot screen it was drawn as.
if (!isSplash) {
  document.documentElement.dataset.theme = window.desktop?.initialTheme ?? 'dark'
  window.desktop?.onAppearance((state) => {
    document.documentElement.dataset.theme = state.resolved
  })
}

/*
 * A MAC'S WINDOW IS GLASS BEHIND THE PAGE (polish pass, 2026-11-01; main/window-chrome.ts): the sidebar and the title
 * bar let it through, as Finder's and Notes' do, and every other pane paints its own ground (shell.css). Only in the
 * Mac window itself: never the loading window, the cover a browser leaves on the computer, or Locust in a browser.
 */
if (!isSplash && !isElsewhere && window.desktop?.platform === 'darwin' && window.desktop.remote === undefined) {
  document.documentElement.dataset.glass = 'mac'
  /*
   * LIQUID GLASS (0.855, main/mac-glass.ts): the whole window on macOS's glass, the sidebar on a floating panel of it.
   * Marked from the first frame when the window starts in it, and changed in place when the person changes the look;
   * while it is on, the page tells the window where the sidebar's panel goes (glassPanel.ts).
   */
  const desktop = window.desktop
  let stopPanel: (() => void) | undefined
  const drawLiquid = (liquid: boolean): void => {
    if (liquid) document.documentElement.dataset.look = 'liquid'
    else delete document.documentElement.dataset.look
    if (liquid && stopPanel === undefined) stopPanel = watchGlassPanel((panel) => desktop.placeGlassPanel(panel))
    if (!liquid && stopPanel !== undefined) {
      stopPanel()
      stopPanel = undefined
    }
  }
  drawLiquid(desktop.initialLiquid)
  desktop.onAppearance((state) => drawLiquid(drawsLiquid(state)))
}

// Behind other windows, nothing moves (0.611, windowPresence.ts): watched from the start, so the
// stylesheet's hold works before any face is on screen. The loading window is never held.
watchWindowPresence()

createRoot(document.getElementById('root')!).render(
  <StrictMode>{isSplash ? <SplashApp /> : isElsewhere ? <ElsewhereApp /> : <WindowFallback><App /></WindowFallback>}</StrictMode>
)
