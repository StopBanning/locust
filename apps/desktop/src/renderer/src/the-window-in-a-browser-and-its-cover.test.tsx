import { renderToStaticMarkup } from 'react-dom/server'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { ElsewhereApp } from './ElsewhereApp.js'
import { TitleBar } from './components/TitleBar.js'

/**
 * THE WINDOW IN A BROWSER, AND ITS COVER HERE (0.830, Locust in a browser). In a browser the window has no window
 * controls of its own -- the browser's window has them; on this computer, while a browser has the window, the window
 * is a cover that says so and takes it back, with no app behind it.
 */
afterEach(() => { vi.unstubAllGlobals() })
const bar = (): string => renderToStaticMarkup(<TitleBar workspaceName="tides" runningCount={0} swarm={false} />)

describe('the window in a browser, and its cover here', () => {
  it('draws its own window controls on this computer, and none in a browser', () => {
    vi.stubGlobal('window', { desktop: {} })
    expect(bar()).toContain('aria-label="Minimize"')
    vi.stubGlobal('window', { desktop: { remote: { computer: 'DESK-PC' } } })
    expect(bar()).not.toContain('lc-windowcontrols')
  })

  it('covers this computer’s window with what has happened and the way back', () => {
    vi.stubGlobal('window', { desktop: {} })
    const cover = renderToStaticMarkup(<ElsewhereApp />)
    expect(cover).toContain('Locust is open in a browser')
    expect(cover).toContain('Your teammates keep working on this computer')
    expect(cover).toContain('Use Locust here')
  })
})
