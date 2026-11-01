import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

/**
 * THE PALETTE'S INPUT WEARS NO RING OF ITS OWN (0.414).
 *
 * Fresh-eyes area 8: Ctrl K opened with a square lime box inside the rounded
 * panel. tokens.css rings every `input:focus-visible`, which outranks the
 * input's own `outline: none`; the head's lit edge is the focus it was meant
 * to have. drive-sidebar-and-search captures the palette open.
 */
const shell = readFileSync(fileURLToPath(new URL('../renderer/src/shell.css', import.meta.url)), 'utf8')

describe('the Ctrl K palette input', () => {
  it('turns the global focus ring off at a specificity that beats it', () => {
    expect(shell).toMatch(/\.lc-palette__input:focus-visible \{\s*outline: none;\s*\}/)
  })

  it('keeps the head’s lit edge as the focus it shows', () => {
    // A step brighter than the hairline, not ivory (polish pass, 2026-11-01): the box is focused whenever the palette
    // is open, so an ivory rule read as a stray line across the sheet; it still dims when focus leaves for the list.
    expect(shell).toMatch(/\.lc-palette__head:focus-within \{\s*border-bottom-color: var\(--lc-border-hover\);\s*\}/)
  })
})
