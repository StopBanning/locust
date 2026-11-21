import { describe, expect, it } from 'vitest'

import { integrationOf } from './runtime-integration.js'
import { hostReadsEventsOf } from './runtimes.js'
import { POLICY_RUNTIME_IDS } from './organization-policy.js'

/**
 * A RUNNABLE AGENT IS NEVER PLANNED (0.881). Apple Intelligence shipped in 0.878 with no row in RUNTIME_INTEGRATION,
 * and a runtime missing from it reads as `planned`: the model picker drew its On-device model as PLANNED and would not
 * pick it. Every runtime the host can run a turn on has its own row. Your own agents (`acp`) are drawn from their own
 * list, not by integration.
 */
describe('a runnable agent is never planned', () => {
  it('gives every runtime the host runs a row of its own, never the planned default', () => {
    const runnable = POLICY_RUNTIME_IDS.filter((id) => id !== 'omniroute' && id !== 'acp' && hostReadsEventsOf(id as never))
    expect(runnable).toContain('apple')
    for (const id of runnable) expect(integrationOf(id), id).not.toBe('planned')
  })

  it('lists Apple Intelligence as live', () => {
    expect(integrationOf('apple')).toBe('live')
  })
})
