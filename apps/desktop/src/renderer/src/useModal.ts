import { useEffect, useRef } from 'react'
import type { RefObject } from 'react'

/**
 * What a modal dialog owes the keyboard, in one place.
 *
 * The beta review of 0.255.0 (#1) tabbed through the routine editor: after
 * Save changes, focus went to the window's Minimize, Maximize and Close,
 * then Home, Add and the conversation search behind the scrim -- and Escape
 * left the dialog open, even with focus in its own name field. The teammate
 * and group dialogs caught Escape only while focus happened to be inside.
 *
 * So every dialog gets the same four things:
 *   - focus moves INTO it when it opens (unless something inside already has it): to its `[autofocus]`, else its
 *     first control -- or, for a sheet with nothing to type in (`focusSelf`), to the dialog itself, so no control
 *     wears a focus ring before the keyboard asks for one (polish pass, 2026-11-01)
 *   - Tab and Shift+Tab cycle inside it and never reach the page behind
 *   - Escape closes it, wherever focus is
 *   - focus goes back to whatever opened it, once it is gone
 */
const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'

export function focusablesIn(root: HTMLElement): HTMLElement[] {
  return [...root.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
    (element) => !element.hasAttribute('hidden') && element.getAttribute('aria-hidden') !== 'true'
  )
}

export function useModal(ref: RefObject<HTMLElement | null>, onClose: () => void, { focusSelf = false }: { readonly focusSelf?: boolean } = {}): void {
  // The latest close, without re-running the effect (and re-stealing focus)
  // every time a parent hands down a fresh function.
  const close = useRef(onClose)
  close.current = onClose
  /*
   * The opener, noted DURING THE FIRST RENDER. By the time this hook's effect
   * runs, a dialog's own effect may already have moved focus into it -- the
   * teammate dialog focuses its name field -- and the "opener" read then was
   * the name field, gone once the dialog closed, so focus fell to the page
   * (probe-dialog-keyboard, 2026-09-22).
   */
  const opened = useRef<HTMLElement | undefined>(undefined)
  // Guarded: a dialog rendered to markup in a test has no document at all.
  if (opened.current === undefined && typeof document !== 'undefined' && document.activeElement instanceof HTMLElement) {
    opened.current = document.activeElement
  }

  useEffect(() => {
    const opener = opened.current
    const root = ref.current
    if (root !== null && !root.contains(document.activeElement)) {
      const wanted = root.querySelector<HTMLElement>('[autofocus]') ?? (focusSelf ? root : focusablesIn(root)[0])
      wanted?.focus()
    }

    const onKeyDown = (event: KeyboardEvent): void => {
      const box = ref.current
      if (box === null) return
      if (event.key === 'Escape') {
        event.preventDefault()
        event.stopPropagation()
        close.current()
        return
      }
      if (event.key !== 'Tab') return
      const inside = focusablesIn(box)
      if (inside.length === 0) {
        event.preventDefault()
        return
      }
      const first = inside[0]!
      const last = inside[inside.length - 1]!
      const at = document.activeElement
      // Wrap at either end, and pull focus back in if it was ever outside. The dialog itself (focusSelf) is the
      // start: Shift+Tab from it goes to the last control, Tab to the first.
      if (event.shiftKey && (at === first || at === box || !box.contains(at))) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && (at === last || at === box || !box.contains(at))) {
        event.preventDefault()
        first.focus()
      }
    }
    // Captured at the document, so it holds wherever focus has drifted.
    document.addEventListener('keydown', onKeyDown, true)
    return () => {
      document.removeEventListener('keydown', onKeyDown, true)
      if (opener !== undefined && opener.isConnected) opener.focus()
    }
    // Once per opening: `ref` is stable and `close` is read through its ref.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])
}
