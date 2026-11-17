import { describe, expect, it } from 'vitest'

import { RUNTIME_MARKS } from './runtimeMarks.js'
import { modeRunsOn } from './status.js'

/** Apple Intelligence (0.878) as the window offers it: Ask and Plan only, and a mark of Locust's own. */
describe('Apple Intelligence in the window', () => {
  it('runs in Ask and Plan only, because it has no tools', () => {
    expect(modeRunsOn('ask', 'apple')).toBe(true)
    expect(modeRunsOn('plan', 'apple')).toBe(true)
    for (const mode of ['accept-edits', 'approve-each', 'auto'] as const) expect(modeRunsOn(mode, 'apple'), mode).toBe(false)
  })

  it('wears a chip with a spark, never Apple’s logo', () => {
    expect(RUNTIME_MARKS.apple.viewBox).toBe('0 0 28 28')
    expect(RUNTIME_MARKS.apple.source).toMatch(/^Locust: a chip with a spark/)
  })
})
