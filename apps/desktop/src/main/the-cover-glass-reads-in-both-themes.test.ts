import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

/**
 * THE COVER'S GLASS READS IN BOTH THEMES (0.843, F20). The first Mac run (2026-10-30) shot Home in the light theme:
 * the glass stays a dark tube by design, but the LOCUST wordmark and the claim took the light theme's dark text and
 * all but vanished into it. What is written on the glass now takes the glass's own ink, the same in both themes.
 */
const read = (path: string): string => readFileSync(fileURLToPath(new URL(path, import.meta.url)), 'utf8')
const css = read('../renderer/src/shell.css')
const tokens = read('../renderer/src/tokens.css')
const rule = (selector: string): string => {
  const at = css.indexOf(`${selector} {`)
  return at < 0 ? '' : css.slice(at, css.indexOf('}', at))
}

describe("the cover's glass", () => {
  it('writes the wordmark and the claim in its own ink', () => {
    expect(rule('.lc-cover__glass .lc-lockup')).toContain('color: var(--lc-boot-ink)')
    expect(rule('.lc-cover .lc-cover__claim')).toContain('color: var(--lc-boot-ink-muted)')
  })

  it('keeps that ink the same in both themes, as the glass is', () => {
    // Defined once, with the glass's own colours, and never redefined by a theme.
    expect(tokens.match(/--lc-boot-ink:/g)).toHaveLength(1)
    expect(tokens.match(/--lc-boot-ink-muted:/g)).toHaveLength(1)
    expect(tokens.match(/--lc-boot-screen-hot:/g)).toHaveLength(1)
  })
})
