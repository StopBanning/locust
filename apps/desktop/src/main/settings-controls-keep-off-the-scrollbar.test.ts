import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'

/*
 * F21 (the Mac's first run, 2026-10-30): Settings' controls sat against the
 * pane's scrollbar. Measured 2026-11-01 at 1280x800, a laptop's size:
 * General's "Add to the Claude app" ended 1px from it. The sections are 720px
 * at most, and at that width they filled the pane; the pane now keeps a
 * gutter of its own.
 */
const shell = readFileSync(fileURLToPath(new URL('../renderer/src/shell.css', import.meta.url)), 'utf8')
const rule = (selector: string): string => {
  const start = shell.indexOf(`${selector} {`)
  return start < 0 ? '' : shell.slice(start, shell.indexOf('}', start))
}

describe("Settings' pane", () => {
  it('keeps its controls off its scrollbar', () => {
    expect(rule('.lc-settings__pane')).toContain('padding-right: var(--lc-space-6)')
    expect(rule('.lc-settings__pane')).toContain('overflow-y: auto')
  })
})
