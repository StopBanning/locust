import { describe, expect, it } from 'vitest'

import { RUNTIME_MARKS } from './runtimeMarks.js'
import { modeRunsOn } from './status.js'

/** Apple Intelligence (0.878) as the window offers it: Ask and Plan only, wearing Apple's logo. */
describe('Apple Intelligence in the window', () => {
  it('runs in Ask and Plan only, because it has no tools', () => {
    expect(modeRunsOn('ask', 'apple')).toBe(true)
    expect(modeRunsOn('plan', 'apple')).toBe(true)
    for (const mode of ['accept-edits', 'approve-each', 'auto'] as const) expect(modeRunsOn(mode, 'apple'), mode).toBe(false)
  })

  it('wears Apple’s logo, as every other agent wears its maker’s mark', () => {
    expect(RUNTIME_MARKS.apple.viewBox).toBeUndefined()
    expect(RUNTIME_MARKS.apple.source).toBe('Simple Icons "Apple" (apple.com)')
  })
})
