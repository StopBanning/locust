import type { RecoveredMission } from '@teammate/mission-store'
import { describe, expect, it } from 'vitest'

import { limitHasReset, readingHasQuotaLeft } from '../shared/limit-lifts.js'
import { antigravityUsageText } from './antigravity-usage.js'
import { limitedRuntimesFrom } from './mission-history.js'

/*
 * A LIMIT LIFTS WHEN THE ACCOUNT SAYS SO (0.878). Colin, 17 Nov: he switched Antigravity accounts, Antigravity's own
 * page said 100% left everywhere, and Locust's picker still said AT LIMIT on every Antigravity model. The old account
 * had run out on 16 Nov ("resets 2026-11-22T05:28:22Z"), and only a run finishing in Locust took the tag off. The
 * values below are the ones in his record; the usage answers have the CLI's shape.
 */
const OUT = "Antigravity's quota is used up · resets 2026-11-22T05:28:22Z"
const bucket = (window: string, remaining_fraction: number) => ({ window, remaining_fraction, reset_time: '2026-11-18T06:00:00Z' })
const answer = (gemini: number, claude: number) => ({
  status: 'SUCCESS', num_turns: 0,
  command: { name: 'usage', data: { groups: [
    { name: 'Gemini Models', buckets: [bucket('weekly', gemini), bucket('5h', gemini)] },
    { name: 'Claude and GPT models', buckets: [bucket('weekly', claude), bucket('5h', claude)] }
  ] } }
})
const reading = (gemini: number, claude: number): string => antigravityUsageText(answer(gemini, claude))!
const outOn = (at: string, message = OUT): RecoveredMission =>
  ({
    missionId: 'mission_out', metadata: { resolvedRouteId: 'antigravity-account:default' },
    events: [{ id: 'e0', type: 'route.limit_detected', occurredAt: at, sourceAdapter: 'antigravity', payload: { kind: 'quota-exhausted', message } }]
  }) as unknown as RecoveredMission

describe('a limit lifts when the account says so', () => {
  it("lifts on Antigravity's reading of the account signed in now when every window has some left", () => {
    expect(readingHasQuotaLeft(reading(1, 1))).toBe(true)
    expect(readingHasQuotaLeft(reading(0.4, 1))).toBe(true)
  })

  it('stays while any window the reading names is used up, Claude and GPT too', () => {
    expect(readingHasQuotaLeft(reading(0, 1))).toBe(false)
    expect(readingHasQuotaLeft(reading(1, 0))).toBe(false)
  })

  it('stays when the reading names no window at all', () => {
    expect(readingHasQuotaLeft('')).toBe(false)
    expect(readingHasQuotaLeft('as of 2026-11-18T01:00:00.000Z')).toBe(false)
  })

  it('lifts once the reset it named has passed, and not a moment before', () => {
    expect(limitHasReset(OUT, Date.parse('2026-11-22T05:28:21Z'))).toBe(false)
    expect(limitHasReset(OUT, Date.parse('2026-11-22T05:28:22Z'))).toBe(true)
  })

  it('never lifts on a reset it cannot read as a time', () => {
    expect(limitHasReset('weekly limit reached, resets 01:48', Date.parse('2030-01-01T00:00:00Z'))).toBe(false)
    expect(limitHasReset('You have hit your usage limit.', Date.parse('2030-01-01T00:00:00Z'))).toBe(false)
  })

  it("is read back from the record until its reset, then not, as on Colin's account", () => {
    const missions = [outOn('2026-11-16T04:29:59.050Z')]
    expect(limitedRuntimesFrom(missions, Date.parse('2026-11-17T20:11:00Z'))).toEqual({ antigravity: OUT })
    expect(limitedRuntimesFrom(missions, Date.parse('2026-11-22T06:00:00Z'))).toEqual({})
  })

  it('CONTROL: a limit with no time it can read stays until a run finishes', () => {
    expect(limitedRuntimesFrom([outOn('2026-11-16T04:29:59Z', 'weekly limit reached, resets 01:48')], Date.parse('2030-01-01T00:00:00Z')))
      .toEqual({ antigravity: 'weekly limit reached, resets 01:48' })
  })
})
