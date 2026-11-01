import { describe, expect, it } from 'vitest'

import { pathOnlyTitle } from './pathTitle.js'
import APP from './App.tsx?raw'

describe('a first message that is only a path', () => {
  it('is titled by the file it names', () => {
    expect(pathOnlyTitle('/Users/locust/work/locust-move/handoffs/HANDOFF-menu-bar-icon.md')).toBe('HANDOFF-menu-bar-icon.md')
    expect(pathOnlyTitle('  ~/work/now-coding/handoff-mac-tools.md ')).toBe('handoff-mac-tools.md')
    expect(pathOnlyTitle('C:\\work\\locust\\docs\\PLAN.md')).toBe('PLAN.md')
  })

  it('leaves any other first line as it was typed', () => {
    expect(pathOnlyTitle('Fix /Users/a/b.ts please')).toBeUndefined()
    expect(pathOnlyTitle('continue')).toBeUndefined()
    expect(pathOnlyTitle('/')).toBeUndefined()
    expect(pathOnlyTitle('/README.md')).toBeUndefined()
  })

  it('is how a conversation is titled', () => {
    expect(APP).toContain('return pathOnlyTitle(trimmed) ?? trimmed')
  })
})
