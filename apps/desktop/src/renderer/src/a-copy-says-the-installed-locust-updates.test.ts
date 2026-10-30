import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { heldBackLine } from './updateLine.js'

/**
 * A COPY SAYS THE INSTALLED LOCUST UPDATES (the first Mac run, 2026-10-30). Settings told any Mac "On a Mac this
 * build cannot update itself yet. Download the newest from the releases page and replace it." -- untrue since 0.516,
 * and shown on a development build and on every copy a script drives. It now says what is true on every system; the
 * service says why (a-held-back-copy-says-why.test.ts).
 */
describe('a copy that does not update itself', () => {
  it('says the installed Locust does, and that a development build never updates', () => {
    expect(heldBackLine('driven')).toBe("This copy of Locust doesn't update itself. The installed Locust does.")
    expect(heldBackLine('copy')).toBe("This copy of Locust doesn't update itself. The installed Locust does.")
    expect(heldBackLine(undefined)).toBe("This copy of Locust doesn't update itself. The installed Locust does.")
    expect(heldBackLine('development')).toBe("This copy of Locust doesn't update itself. The installed Locust does. A development build never updates.")
  })

  it('never says a Mac cannot update itself (the old sentence, pinned away)', () => {
    const screens = readFileSync(join(__dirname, 'components', 'Screens.tsx'), 'utf8')
    expect(screens).not.toContain('cannot update itself yet')
    expect(screens).not.toContain('Download the newest from the releases page and replace it')
    expect(screens).toContain('heldBackLine(update?.heldBack)')
  })
})
