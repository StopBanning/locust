import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { LineCounts } from './components/LineCounts.js'

/**
 * A COUNT SAYS ONLY WHAT CHANGED (0.867).
 *
 * Colin's screenshots of 0.866: a long Codex turn beside the browser, every step row and file row ending in a red
 * "-0" for files that were only written. Claude Code's own app says "+52" for a change that removed nothing.
 */
const said = (added: number, removed: number): string => renderToStaticMarkup(<LineCounts added={added} removed={removed} />)

describe('a change\'s line counts', () => {
  it('says only the added lines of a new file, with no red zero', () => {
    expect(said(299, 0)).toBe('<span class="lc-diff__addmark">+299</span>')
  })

  it('says only the removed lines of a deletion', () => {
    expect(said(0, 12)).toBe('<span class="lc-diff__delmark">−12</span>')
  })

  it('says both sides of an edit, a space between them', () => {
    expect(said(5, 3)).toBe('<span class="lc-diff__addmark">+5</span> <span class="lc-diff__delmark">−3</span>')
  })

  it('draws nothing for a change of neither (a rename)', () => {
    expect(said(0, 0)).toBe('')
  })
})
