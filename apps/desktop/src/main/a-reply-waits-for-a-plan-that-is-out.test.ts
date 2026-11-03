import type { RecoveredMission } from '@teammate/mission-store'
import { describe, expect, it } from 'vitest'

import { planOutSentence } from './mission-history.js'

/**
 * A REPLY WAITS FOR A PLAN THAT IS OUT (0.862). Colin, 2026-11-03: Codec handed a finished job to Argon, whose
 * Antigravity plan had run out the night before, and the relay started Argon anyway -- it failed twelve seconds in.
 * The relay now asks this before it starts a reply. Out is what the agent itself said (`route.limit_detected`,
 * `quota-exhausted`), until a run on that plan finishes or the reset it named has passed.
 */
const run = (events: readonly { type: string; at: string; message?: string; adapter?: string }[], route = 'antigravity-account:default'): RecoveredMission =>
  ({
    missionId: `mission_${events[0]?.at ?? 'x'}`,
    metadata: { resolvedRouteId: route },
    events: events.map((event, index) => ({
      id: `e${String(index)}`,
      type: event.type,
      occurredAt: event.at,
      sourceAdapter: event.adapter ?? 'antigravity',
      payload: event.type === 'route.limit_detected' ? { kind: 'quota-exhausted', message: event.message ?? '' } : {}
    }))
  }) as unknown as RecoveredMission

const NOW = new Date('2026-11-03T05:39:30Z')
const OUT = "Antigravity's quota is used up · resets 2026-11-09T23:52:20Z"

describe('a reply waits for a plan that is out', () => {
  it('is out from the limit the agent named until the reset it named, said with that day and time', () => {
    const said = planOutSentence([run([{ type: 'route.limit_detected', at: '2026-11-03T02:27:31Z', message: OUT }])], 'antigravity', 'Antigravity', NOW)
    expect(said).toMatch(/^their Antigravity plan is used up until Mon, Nov 9, /)
    expect(said?.endsWith('.')).toBe(true)
  })

  it('is not out once the reset has passed, so the next run finds out', () => {
    const later = new Date('2026-11-10T00:00:00Z')
    expect(planOutSentence([run([{ type: 'route.limit_detected', at: '2026-11-03T02:27:31Z', message: OUT }])], 'antigravity', 'Antigravity', later)).toBeUndefined()
  })

  it('is not out once a later run on that plan finished', () => {
    const missions = [
      run([{ type: 'route.limit_detected', at: '2026-11-03T02:27:31Z', message: OUT }]),
      run([{ type: 'run.completed', at: '2026-11-03T04:00:00Z' }])
    ]
    expect(planOutSentence(missions, 'antigravity', 'Antigravity', NOW)).toBeUndefined()
  })

  it('reads only the plan asked about, and says plainly when the agent named no reset', () => {
    const codexOut = run([{ type: 'route.limit_detected', at: '2026-11-03T02:27:31Z', message: 'usage limit reached', adapter: 'codex' }], 'codex-account:default')
    expect(planOutSentence([codexOut], 'antigravity', 'Antigravity', NOW)).toBeUndefined()
    expect(planOutSentence([codexOut], 'codex', 'Codex CLI', NOW)).toBe('their Codex CLI plan is used up.')
  })

  it('answers nothing for a plan with no record at all', () => {
    expect(planOutSentence([], 'claude', 'Claude Code', NOW)).toBeUndefined()
  })
})
