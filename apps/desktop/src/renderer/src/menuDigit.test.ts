import { describe, expect, it } from 'vitest'

import { menuRowFromDigit } from './menuDigit.js'

const bare = { ctrl: false, meta: false, alt: false, shift: false }

describe('a digit picks a row in an open menu', () => {
  it('counts from the top, 1 through 9', () => {
    expect(menuRowFromDigit('1', bare, false)).toBe(0)
    expect(menuRowFromDigit('9', bare, false)).toBe(8)
    expect(menuRowFromDigit('0', bare, false)).toBeUndefined()
  })

  it('leaves a digit alone in a search box, and leaves chords alone', () => {
    expect(menuRowFromDigit('1', bare, true)).toBeUndefined()
    expect(menuRowFromDigit('1', { ...bare, ctrl: true }, false)).toBeUndefined()
    expect(menuRowFromDigit('1', { ...bare, shift: true }, false)).toBeUndefined()
    expect(menuRowFromDigit('a', bare, false)).toBeUndefined()
  })
})
