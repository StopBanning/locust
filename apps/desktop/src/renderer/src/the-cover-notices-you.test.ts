import { describe, expect, it } from 'vitest'

import {
  ASLEEP,
  DOZE_AFTER_MS,
  WAKE_WITHIN,
  hopperWakes,
  widthsFrom
} from './components/HomeCover.js'
import { replaysOnAsk } from './components/PoweredLockup.js'

/**
 * THE TITLE SCREEN NOTICES YOU.
 *
 * Colin's "have fun" list, 2026-09-23, and his answer, "you can run all
 * those": the sleeping Hopper opens its eyes and watches a pointer that comes
 * near, then dozes off again; a click on the glass powers the lockup on
 * again. (A swarm flight on turning swarm on went with swarm, 0.880.)
 * What each looks like is checked on the built app (probe-cover-notices);
 * these are the rules.
 */

describe('the Hopper', () => {
  it('sleeps through a pointer far away, costing nothing: the same value back', () => {
    expect(hopperWakes(ASLEEP, WAKE_WITHIN + 2, 1_000)).toBe(ASLEEP)
    expect(hopperWakes(ASLEEP, undefined, 1_000)).toBe(ASLEEP)
  })

  it('wakes when a pointer comes near', () => {
    expect(hopperWakes(ASLEEP, WAKE_WITHIN - 0.1, 1_000)).toEqual({ awake: true, awaySince: undefined })
  })

  it('stays awake a while after the pointer goes, then dozes off', () => {
    const awake = hopperWakes(ASLEEP, 0.5, 1_000)
    const left = hopperWakes(awake, WAKE_WITHIN + 1, 2_000)
    expect(left).toEqual({ awake: true, awaySince: 2_000 })
    // Moving about far away does not restart the count.
    expect(hopperWakes(left, WAKE_WITHIN + 3, 3_000)).toBe(left)
    expect(hopperWakes(left, undefined, 2_000 + DOZE_AFTER_MS - 1)).toBe(left)
    expect(hopperWakes(left, undefined, 2_000 + DOZE_AFTER_MS)).toBe(ASLEEP)
  })

  it('stays awake for a pointer that comes back before it dozes', () => {
    const left = { awake: true, awaySince: 2_000 }
    expect(hopperWakes(left, 0.4, 3_000)).toEqual({ awake: true, awaySince: undefined })
  })

  it('measures from the middle of its box, in its own widths', () => {
    expect(widthsFrom({ left: 100, top: 100, width: 80, height: 80 }, 140, 140)).toBe(0)
    expect(widthsFrom({ left: 100, top: 100, width: 80, height: 80 }, 300, 140)).toBe(2)
  })
})

describe('a click on the glass', () => {
  it('lights it again once it has lit, as the tube allows', () => {
    expect(replaysOnAsk('repeat', 1)).toBe(true)
    expect(replaysOnAsk('once', 1)).toBe(true)
  })

  it('never where the tube never lights -- Off, or reduced motion -- nor before its first lighting', () => {
    expect(replaysOnAsk('none', 3)).toBe(false)
    expect(replaysOnAsk('repeat', 0)).toBe(false)
  })
})
