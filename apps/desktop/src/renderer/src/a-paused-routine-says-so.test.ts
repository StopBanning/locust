import { describe, expect, it } from 'vitest'

import type { PublicRoutine } from '../../shared/ipc.js'
import { routineScheduleSummary } from './routines.js'

/** A paused routine's chip (0.705); the scheduler's side is main/a-paused-routine-waits-to-be-resumed.test.ts. */
const saved = (over: Partial<PublicRoutine> = {}): PublicRoutine => ({
  routineId: 'rt_1', name: 'Morning digest', teammateId: 'tm_wren',
  route: { runtime: 'cursor', model: 'composer-2.5', mode: 'ask' },
  steps: ['Say good morning.'], learnedFrom: [],
  createdAt: new Date(2026, 9, 2, 8, 0).toISOString(), lastRunAt: new Date(2026, 9, 2, 8, 0).toISOString(),
  runs: 0, schedule: { kind: 'every', hours: 1 }, ...over
})

describe('a paused routine says so', () => {
  const now = new Date(2026, 9, 3, 9, 0)
  it('on its schedule chip, in place of its next time', () => {
    expect(routineScheduleSummary(saved({ paused: true }), now)).toBe('every hour · paused')
  })
  it('and counts its next time from a resume', () => {
    const resumedAt = new Date(2026, 9, 3, 8, 30).toISOString()
    expect(routineScheduleSummary(saved({ resumedAt }), now)).toBe('every hour · next 9:30 AM')
  })
})
