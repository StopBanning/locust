import { describe, expect, it } from 'vitest'

import { routeChipReading } from './routeChipReading.js'

describe('the model and its effort read as one control', () => {
  it('joins them the way the reference does', () => {
    expect(routeChipReading('Haiku 5.5', 'Medium')).toBe('Haiku 5.5 · Medium')
    expect(routeChipReading('Opus 5.5', 'High · Fast')).toBe('Opus 5.5 · High · Fast')
  })

  it('leaves the name alone when there is no effort to state', () => {
    expect(routeChipReading('Haiku 5.5', undefined)).toBe('Haiku 5.5')
    expect(routeChipReading('Haiku 5.5', '  ')).toBe('Haiku 5.5')
  })
})
