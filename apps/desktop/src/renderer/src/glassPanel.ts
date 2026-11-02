import type { GlassPanel } from '../../shared/appearance.js'

/**
 * WHERE THE SIDEBAR'S GLASS PANEL GOES (Liquid glass, 0.855; main/mac-glass.ts).
 *
 * In Liquid glass the sidebar sits on a floating panel of macOS's glass, set in from the window's edges as Finder's
 * and Settings' are on macOS 26, with the traffic lights inside its top corner. The glass is behind the page, so the
 * page says where the panel is: from the window's top inset to its bottom inset, as wide as the sidebar. Sent again
 * only when that changes (the sidebar's width: the compact rail, or the sidebar hidden); the window's own height is
 * followed by the glass itself as the window is resized.
 */
export const GLASS_INSET = 8
export const GLASS_RADIUS = 16

/** The panel for a sidebar of this width, or none when there is no sidebar to sit on one. */
export function glassPanelFor(sidebar: { readonly right: number; readonly width: number } | undefined, windowHeight: number): GlassPanel | null {
  if (sidebar === undefined || sidebar.width < 2 * GLASS_INSET || windowHeight < 4 * GLASS_INSET) return null
  return {
    x: GLASS_INSET,
    y: GLASS_INSET,
    width: Math.round(sidebar.right - GLASS_INSET),
    height: Math.round(windowHeight - 2 * GLASS_INSET),
    radius: GLASS_RADIUS
  }
}

const same = (a: GlassPanel | null, b: GlassPanel | null): boolean =>
  a === b || (a !== null && b !== null && a.x === b.x && a.y === b.y && a.width === b.width && a.height === b.height && a.radius === b.radius)

/**
 * Watches the sidebar while the page is drawn in Liquid glass and tells the window where its panel goes. Returns the
 * stop, which takes the panel away.
 */
export function watchGlassPanel(place: (panel: GlassPanel | null) => void): () => void {
  let sent: GlassPanel | null | undefined
  let frame = 0
  let watched: Element | null = null
  const resize = new ResizeObserver(() => schedule())
  const measure = (): void => {
    frame = 0
    const sidebar = document.querySelector('.lc-sidebar')
    if (sidebar !== watched) {
      if (watched !== null) resize.unobserve(watched)
      watched = sidebar
      if (sidebar !== null) resize.observe(sidebar)
    }
    const box = sidebar?.getBoundingClientRect()
    const visible = box !== undefined && box.width > 0 && getComputedStyle(sidebar as Element).display !== 'none'
    // The height goes with the window's on the glass's own side; it is sent when the window's height is first known.
    const next = glassPanelFor(visible ? { right: box.right, width: box.width } : undefined, window.innerHeight)
    if (sent !== undefined && same(sent, next)) return
    sent = next
    place(next)
  }
  function schedule(): void {
    if (frame === 0) frame = requestAnimationFrame(measure)
  }
  // The sidebar comes and goes with the screens, so the page's own changes are watched for it -- cheaply: while the
  // sidebar is on the page, a change elsewhere (a reply streaming in) costs one check that it still is.
  const tree = new MutationObserver(() => {
    if (watched === null || !watched.isConnected) schedule()
  })
  tree.observe(document.body, { childList: true, subtree: true })
  window.addEventListener('resize', schedule)
  schedule()
  return () => {
    tree.disconnect()
    resize.disconnect()
    window.removeEventListener('resize', schedule)
    if (frame !== 0) cancelAnimationFrame(frame)
    place(null)
  }
}
