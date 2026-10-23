import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import App from './App'
import { SplashApp } from './SplashApp'
import { ElsewhereApp } from './ElsewhereApp'
import { WindowFallback } from './components/WindowFallback'
import { watchWindowPresence } from './windowPresence'
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

// Behind other windows, nothing moves (0.611, windowPresence.ts): watched from the start, so the
// stylesheet's hold works before any face is on screen. The loading window is never held.
watchWindowPresence()

createRoot(document.getElementById('root')!).render(
  <StrictMode>{isSplash ? <SplashApp /> : isElsewhere ? <ElsewhereApp /> : <WindowFallback><App /></WindowFallback>}</StrictMode>
)
