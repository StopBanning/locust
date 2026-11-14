/*
 * WHICH HAND MOVED LAST (0.875). The focus ring is for the keyboard: "Never a sticky ring after a mouse click"
 * (tokens.css). Chromium's own `:focus-visible` mostly keeps that, but Colin's frame of 2026-11-13 showed a ring on a
 * step line he had clicked: the click that brought a background window forward counted as keyboard focus (measured on
 * the window itself: `matches(':focus-visible')` was true for the clicked button, false for the same press sent again
 * once the window was in front). So the page keeps its own word for which hand moved last, and a button's ring is
 * hidden after the pointer whatever Chromium decided. A key that moves focus or presses gives the keyboard back its
 * rings; typing a letter does not change anything.
 */
export type InputModality = 'pointer' | 'keyboard'

const NAVIGATION_KEYS = new Set(['Tab', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Enter', ' ', 'Escape', 'Home', 'End', 'PageUp', 'PageDown'])

/** What an input event says about which hand moved last; undefined when it says nothing. */
export function modalityAfter(event: { readonly type: string; readonly key?: string; readonly metaKey?: boolean; readonly ctrlKey?: boolean; readonly altKey?: boolean }): InputModality | undefined {
  if (event.type === 'pointerdown' || event.type === 'mousedown') return 'pointer'
  if (event.type !== 'keydown' || event.key === undefined) return undefined
  if (event.metaKey === true || event.ctrlKey === true || event.altKey === true) return undefined
  return NAVIGATION_KEYS.has(event.key) ? 'keyboard' : undefined
}

/** Keep `data-input` on the root current. Capture phase, so a handler that stops the event cannot hide it. */
export function watchInputModality(target: Pick<Window, 'addEventListener'>, root: { dataset: DOMStringMap }): void {
  const note = (event: Event): void => {
    const next = modalityAfter(event as KeyboardEvent)
    if (next !== undefined && root.dataset.input !== next) root.dataset.input = next
  }
  for (const type of ['pointerdown', 'mousedown', 'keydown']) target.addEventListener(type, note, { capture: true, passive: true })
}
