import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

import { openCodeReadOnlyRefusal } from './codex-mission.js'

/**
 * A REFUSAL IN ASK SAYS THE MODE, NOT "YOU DECLINED" (0.822).
 *
 * In Ask and Plan, OpenCode's server asks about anything that acts and Locust answers no with nobody shown a card.
 * The answer read "The person declined this in Locust.", and on the packaged 0.821 drive the model told a person who
 * had seen no card "It didn't work -- you declined the permission request". The refusal names the mode now, and the
 * activity line counts it as refused, not declined (opencode-events reads anything but the person's words as refused).
 */
describe('what OpenCode is told when Ask or Plan refuses', () => {
  it('names the mode and says nobody was asked', () => {
    expect(openCodeReadOnlyRefusal('ask')).toBe('Locust refused this without asking anyone: this teammate is in Ask mode, which reads and changes nothing, so nothing was run.')
    expect(openCodeReadOnlyRefusal('plan')).toContain('Plan mode, which reads and plans')
    for (const mode of ['ask', 'plan'] as const) expect(openCodeReadOnlyRefusal(mode)).not.toMatch(/declined/i)
  })

  it('is what a served Ask or Plan run is given, and only one nobody answers for', () => {
    const source = readFileSync(fileURLToPath(new URL('./codex-mission.ts', import.meta.url)), 'utf8')
    expect(source).toContain("...(handler === undefined && effectiveSandbox === 'read-only' ? { refusal: openCodeReadOnlyRefusal(mode) } : {}),")
  })
})
