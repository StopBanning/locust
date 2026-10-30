import { renderToStaticMarkup } from 'react-dom/server'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { TitleBar } from './components/TitleBar.js'

/**
 * THE TITLE BAR IS A MAC'S ON A MAC (the first Mac run, 2026-10-30). It drew Windows' minimize, maximize and close
 * at the right on a Mac. There macOS draws its own traffic lights in the bar's left column (window-chrome.ts), so the
 * bar draws none; on Windows and Linux it still draws its own.
 */
afterEach(() => { vi.unstubAllGlobals() })
const bar = (): string => renderToStaticMarkup(<TitleBar workspaceName="tides" runningCount={0} swarm={false} />)

describe('the title bar on a Mac', () => {
  it('draws no window controls of its own, and nothing in its left column', () => {
    vi.stubGlobal('window', { desktop: { platform: 'darwin' } })
    const html = bar()
    expect(html).not.toContain('lc-windowcontrols')
    expect(html).not.toContain('aria-label="Close"')
    expect(html).toMatch(/^<header class="lc-titlebar"[^>]*><span><\/span><span class="lc-titlebar__title">/)
  })

  it('still draws its own on Windows and Linux (control)', () => {
    for (const platform of ['win32', 'linux']) {
      vi.stubGlobal('window', { desktop: { platform } })
      expect(bar()).toContain('aria-label="Minimize"')
    }
  })
})
