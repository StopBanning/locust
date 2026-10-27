import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

/**
 * THE MESSAGE BOX IS ONE ROW, AS CLAUDE CODE'S (0.837). Colin, 2026-10-27, with Claude Code's box beside ours: "we can
 * condense this like claude code now that we have this new setup". The + left of the field, voice typing, Talk and
 * send right of it; measured on screen by _tools/drive-one-row-box.mjs (56px for a line, where it was 108).
 */
const shell = readFileSync(fileURLToPath(new URL('../renderer/src/shell.css', import.meta.url)), 'utf8')
/** The LAST rule for a selector: the one that decides. */
const rule = (selector: string): string => {
  const start = shell.lastIndexOf(`\n${selector} {`)
  return start < 0 ? '' : shell.slice(start, shell.indexOf('}', start))
}
const px = (text: string, property: string): number => Number(new RegExp(`${property}:\\s*(\\d+)px`).exec(text)?.[1] ?? NaN)

describe('the message box', () => {
  it('lays the +, the field and the buttons out on one row', () => {
    const box = rule('.lc-composer__box')
    expect(box).toContain('display: grid')
    expect(box).toContain('grid-template-columns: auto minmax(0, 1fr) auto')
    expect(rule('.lc-composer__box > .lc-composer__controls')).toContain('display: contents')
    expect(rule('.lc-composer__box > .lc-composer__controls > .lc-composer__group:first-child')).toContain('grid-column: 1')
    expect(rule('.lc-composer__box > textarea')).toContain('grid-column: 2')
    expect(rule('.lc-composer__box > .lc-composer__controls > .lc-composer__group:last-child')).toContain('grid-column: 3')
  })

  it('keeps the buttons on the last line as the field grows', () => {
    expect(rule('.lc-composer__box')).toContain('align-items: end')
  })

  it('makes one line of text as tall as the send, so they share a centre', () => {
    const field = rule('.lc-composer__box > textarea')
    const base = shell.slice(shell.indexOf('\n.lc-composer textarea {'), shell.indexOf('}', shell.indexOf('\n.lc-composer textarea {')))
    const send = rule('.lc-composer__controls .lc-send')
    expect(px(base, 'line-height') + 2 * px(field, 'padding')).toBe(px(send, 'height'))
  })
})
