import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

/**
 * A TEXT BOX SHOWS THE LINES IT ASKS FOR (0.820). `.lc-input` is a one-line field, 34px tall with no padding above
 * the text, and every textarea wearing it was held to that: a routine's "Choices, one per line" showed "Brief" and
 * half of "Detailed" with rows={4}, and Tune's "Ask to run again" set its words against the top edge (the 0.820
 * drive's frame). CSS is read here, in main: a renderer test's CSS import is empty.
 */
const renderer = join(__dirname, '..', 'renderer', 'src')
const shell = readFileSync(join(renderer, 'shell.css'), 'utf8')

const block = (selector: string): string => {
  const at = shell.indexOf(`\n${selector} {`)
  expect(at, selector).toBeGreaterThanOrEqual(0)
  return shell.slice(at, shell.indexOf('}', at))
}

describe('a textarea in the input style', () => {
  it('grows to its rows, with room above and below the words', () => {
    const rule = block('.lc-input:where(textarea)')
    expect(rule).toMatch(/height:\s*auto;/)
    expect(rule).toMatch(/padding:\s*7px 11px;/)
    expect(rule).toMatch(/line-height:\s*18px;/)
  })

  it('comes after the one-line field it corrects, and stays under a textarea\'s own rule', () => {
    expect(shell.indexOf('\n.lc-input:where(textarea) {')).toBeGreaterThan(shell.indexOf('\n.lc-input {'))
    // `:where` adds nothing, so Scout's goal box (later, one class) still sets its own height and padding.
    expect(shell.indexOf('\n.lc-scout__goal {')).toBeGreaterThan(shell.indexOf('\n.lc-input:where(textarea) {'))
  })
})
