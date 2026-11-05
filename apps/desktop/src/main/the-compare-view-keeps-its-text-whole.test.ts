import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

/**
 * The compare view is DRAWN to keep its text whole (2026-10-05) -- the half
 * of `a-compare-column-reads-whole.test.tsx` that markup cannot show. Each
 * rule here was one of the cuts in Colin's three-column frames
 * (docs/colin-compare-overflow-2026-10-05.png) or in the frames of
 * docs/compare-layout-frames-2026-10-05.
 */
const shell = readFileSync(fileURLToPath(new URL('../renderer/src/shell.css', import.meta.url)), 'utf8')
// From the start of a line: `.lc-compare__name {` also ends `.lc-compare__head.is-rail .lc-compare__name {`.
const rule = (selector: string): string => {
  const start = shell.indexOf(`\n${selector} {`)
  return start < 0 ? '' : shell.slice(start, shell.indexOf('}', start))
}
/** Every rule for the selector, in order: a later one can add to or override the first. */
const rules = (selector: string): string => shell.split(`\n${selector} {`).slice(1).map((part) => part.slice(0, part.indexOf('}'))).join('\n')

describe('the compare view', () => {
  it("wraps a foot's numbers between their parts instead of cutting the line short", () => {
    const numbers = rule('.lc-compare__numbers')
    expect(numbers).toContain('flex-wrap: wrap')
    expect(numbers).not.toContain('text-overflow')
    expect(numbers).not.toContain('overflow: hidden')
    expect(rule('.lc-compare__part')).toContain('white-space: nowrap')
  })

  it('keeps Keep its own size beside numbers that wrap', () => {
    expect(rule('.lc-compare__foot > .lc-button,\n.lc-compare__foot > .lc-primarybutton')).toContain('flex: none')
  })

  it("holds a column's body in view while a longer column scrolls on", () => {
    const body = rule('.lc-compare__cellbody')
    expect(body).toContain('position: sticky')
    expect(body).toContain('top: var(--lc-compare-stick')
  })

  it("lets a turn's closing line wrap in a column, not lose its last parts", () => {
    // Since 0.867 the thread's own closing line wraps the same way (beside the browser it lost "4 other tool calls"),
    // so the rule is no longer a Compare cell's alone; a column is one of the places it applies.
    expect(rules('.lc-turnfoot__trace')).toContain('display: block')
    expect(rule('.lc-turnfoot__trace .lc-trace__seg:not(:first-child)')).toContain('white-space: normal')
  })

  it('gives up the runtime word before the name in a narrow head, except where it says the column is in a copy', () => {
    expect(rule('.lc-compare__head:not(.is-rail)')).toContain('container-type: inline-size')
    const query = shell.slice(shell.indexOf('@container (max-width: 380px)'))
    expect(query.slice(0, query.indexOf('}\n}'))).toMatch(/\.lc-compare__runtime:not\(\.is-copy\) \{\s*display: none;/)
    expect(rule('.lc-compare__name')).toContain('text-overflow: ellipsis')
  })

  it("starts a column's live line at the column's edge, as its words do", () => {
    const gutters = shell.slice(shell.indexOf('.lc-compare__cell .lc-agentline__gutter,'))
    expect(gutters.slice(0, gutters.indexOf('}'))).toContain('.lc-compare__cell .lc-livestep__gutter')
    expect(gutters.slice(0, gutters.indexOf('}'))).toContain('display: none')
  })
})
