import { describe, expect, it } from 'vitest'

import { clock } from './clock.js'

describe('a time of day on screen', () => {
  // case: Colin, 22 Nov, on a usage card reading "resets 17:34": "I think we should just stop using military time,
  // honestly."
  it('is 12-hour, with no leading zero, and midnight and noon are 12', () => {
    expect(clock(new Date(2026, 10, 22, 17, 34))).toBe('5:34 PM')
    expect(clock(new Date(2026, 10, 22, 9, 5))).toBe('9:05 AM')
    expect(clock(new Date(2026, 10, 22, 0, 0))).toBe('12:00 AM')
    expect(clock(new Date(2026, 10, 22, 12, 0))).toBe('12:00 PM')
    expect(clock(new Date(2026, 10, 22, 23, 59, 7), true)).toBe('11:59:07 PM')
  })
})
