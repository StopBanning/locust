import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

/**
 * THE THREAD BESIDE THE BROWSER KEEPS ITS PARTS WHOLE (0.867).
 *
 * Colin's screenshots of 0.866 (a 1205px window, the browser open beside a Codex teammate's long turn, the
 * conversation column ~418px), each one measured again by `_tools/drive-browser-beside-a-long-turn.mjs` at
 * 1120 to 1728px wide before and after. These are the rules that made the difference; markup cannot show them.
 */
const shell = readFileSync(fileURLToPath(new URL('../renderer/src/shell.css', import.meta.url)), 'utf8')
const rule = (selector: string): string => {
  const start = shell.indexOf(`\n${selector} {`)
  return start < 0 ? '' : shell.slice(start, shell.indexOf('}', start))
}
/** Every rule for the selector, in order: a later one can add to or override the first. */
const rules = (selector: string): string => shell.split(`\n${selector} {`).slice(1).map((part) => part.slice(0, part.indexOf('}'))).join('\n')
const query = (name: string): string => {
  const start = shell.indexOf(`@container ${name}`)
  return start < 0 ? '' : shell.slice(start, shell.indexOf('\n}\n', start))
}

describe('the thread beside the browser', () => {
  it('keeps the plan pill of the running line whole, so the thread never scrolls sideways under it', () => {
    // "step 2 of" cut at the edge and a sideways scrollbar under the row: the pill shrank with the words before it.
    expect(rule('.lc-livestep__peek')).toContain('flex: none')
    // The words give way instead, to their ellipsis.
    expect(rule('.lc-livestep__label')).toContain('min-width: 0')
    expect(rule('.lc-livestep__label')).toContain('overflow: hidden')
  })

  it("wraps a turn's closing line by whole parts instead of cutting it at the Copy button", () => {
    // "3 exited non-zero · 4 o", no ellipsis: the Copy button after the last part kept the last part's ellipsis off.
    expect(rules('.lc-turnfoot__trace')).toContain('display: block')
    const part = rule('.lc-turnfoot__trace .lc-trace__seg:not(:first-child)')
    expect(part).toContain('display: inline-block')
    expect(part).toContain('max-width: 100%')
  })

  it("fits a handed file's pill to the reply, the note giving way before the path", () => {
    // "Blocker eviden": the pill was wider than the column and the column cut it mid-word.
    const pill = rule('.lc-handedfile')
    expect(pill).toContain('max-width: 100%')
    expect(pill).toContain('min-width: 0')
    expect(rule('.lc-handedfile__open')).toContain('min-width: 0')
    expect(rule('.lc-handedfile__save')).toContain('flex: none')
    expect(rule('.lc-handedfile__note')).toMatch(/flex: 0 1000 auto/)
    // In a narrow reply the note goes altogether; it is the pill's title.
    expect(rule('.lc-handedfiles')).toContain('container: lc-handedfiles / inline-size')
    expect(query('lc-handedfiles')).toMatch(/\.lc-handedfile__note \{\s*display: none;/)
  })

  it("lets a file row give way to its open and copy buttons, never push them past the card", () => {
    expect(rule('.lc-filerow__line > .lc-filerow')).toContain('min-width: 0')
  })

  it("gives the header's name the room beside the face, and in a narrow header the name before the role", () => {
    // "handoff-0501-polish..." with half the header empty: the name's 60% was of a column as wide as its own words.
    expect(rule('.lc-workroom__who')).toContain('flex: 1 1 auto')
    expect(rule('.lc-workroom__header')).toContain('container: lc-workhead / inline-size')
    const narrow = query('lc-workhead')
    expect(narrow).toMatch(/\.lc-workroom__role \{\s*display: none;/)
    expect(narrow).toMatch(/\.lc-workroom__name \{\s*max-width: 100%;/)
    expect(narrow).toMatch(/\.lc-commitwrap__word \{\s*display: none;/)
    // After the name's own rule, or its 60% wins at the same specificity (the first try of this, measured 35 of 58px).
    expect(shell.indexOf('@container lc-workhead')).toBeGreaterThan(shell.indexOf('\n.lc-workroom__name {'))
  })

  it("keeps Home's account marks in a row, the row wrapping before the marks stack", () => {
    // Seven marks in a column down Home at 1120x720 with the browser open: the usage chip took the room.
    expect(rule('.lc-agenthead__marks')).toContain('flex: 1 1 auto')
    expect(rule('.lc-agenthead.is-folded')).toContain('flex-wrap: wrap')
  })
})
