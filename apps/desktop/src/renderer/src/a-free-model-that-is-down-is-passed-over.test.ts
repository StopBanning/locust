import { describe, expect, it } from 'vitest'

import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'
import type { PublicModel } from '../../shared/ipc.js'
import { defaultComparePicks } from './compareDefaults.js'
import { DOWN_FOR_MS, downFreeModels } from './missionView.js'
import { freeStartModel, nextFreeModel } from './status.js'

/*
 * 0.711. Ling 3.0 answered "Upstream request failed: Model is unavailable."
 * for more than twelve hours over 2026-10-08/09. First of the free models
 * listed, it was still what "Use a free model" started on, what Compare put
 * beside a free model (the 0.710 and 0.711 drives of compare-keep-carries-on
 * both failed on a B column that never answered), and what a card offered
 * after another free model failed. A free model that just said so is passed
 * over wherever Locust suggests one, for six hours or until it answers.
 */
const model = (id: string): PublicModel =>
  ({ id, runtime: 'opencode', displayName: id, description: '', supportedEfforts: [] }) as unknown as PublicModel
const LING = 'opencode/ling-3.0-flash-fin-free'
const BEAM = 'opencode/beam-free'
const MIMO = 'opencode/mimo-v2.6-flash-free'
const catalog = [LING, MIMO, BEAM].map(model)
const failedWith = (message: string): NormalizedRuntimeEvent =>
  ({ type: 'run.failed', payload: { message } }) as unknown as NormalizedRuntimeEvent
const run = (id: string, phase: 'completed' | 'failed', at: string, message?: string) => ({
  runtime: 'opencode' as const,
  model: id,
  phase,
  lastUpdatedAt: at,
  events: message === undefined ? [] : [failedWith(message)]
})
const NOW = Date.parse('2026-10-09T17:00:00Z')
const lingDown = run(LING, 'failed', '2026-10-09T16:00:00Z', 'OpenCode stopped: Upstream request failed: Model is unavailable.')

describe('a free model that is down is passed over', () => {
  it('is down after its latest run said so, for six hours, and not once it answers again', () => {
    expect([...downFreeModels([lingDown], NOW)]).toEqual([LING])
    expect(downFreeModels([lingDown], Date.parse('2026-10-09T16:00:00Z') + DOWN_FOR_MS).size).toBe(0)
    expect(downFreeModels([lingDown, run(LING, 'completed', '2026-10-09T16:30:00Z')], NOW).size).toBe(0)
    // Any other failure is not down: a limit, a retirement, a plain error.
    expect(downFreeModels([run(LING, 'failed', '2026-10-09T16:00:00Z', 'OpenCode stopped: Model ling has been deprecated.')], NOW).size).toBe(0)
    expect(downFreeModels([run(LING, 'failed', '2026-10-09T16:00:00Z', 'Rate limit exceeded')], NOW).size).toBe(0)
  })

  it('"Use a free model" starts on one that is up, even one that answered before', () => {
    const down = downFreeModels([run(LING, 'completed', '2026-10-08T10:00:00Z'), lingDown], NOW)
    expect(freeStartModel('opencode', catalog, [LING])).toBe(LING)
    expect(freeStartModel('opencode', catalog, [LING], down)).toBe(MIMO)
    // Every free model down: still the first of them, best first since 0.712 (Mimo), never nothing.
    expect(freeStartModel('opencode', catalog, [], new Set([LING, MIMO, BEAM]))).toBe(MIMO)
  })

  it('the card after a failure offers the next one that is up', () => {
    const down = new Set([MIMO])
    expect(nextFreeModel('opencode', LING, catalog)?.id).toBe(MIMO)
    expect(nextFreeModel('opencode', LING, catalog, down)?.id).toBe(BEAM)
  })

  it("Compare's second column is not one that is down", () => {
    const picks = (down?: ReadonlySet<string>) =>
      defaultComparePicks({
        current: { runtime: 'opencode', model: BEAM },
        recent: [],
        models: catalog,
        ...(down === undefined ? {} : { down }),
        ready: () => true,
        refusal: () => undefined,
        label: (choice) => choice.model
      }).map((pick) => pick.model)
    // Best first since 0.712: Beam beside Mimo, and Ling 3.0, which the order does not name, only when Mimo is down.
    expect(picks()).toEqual([BEAM, MIMO])
    expect(picks(new Set([MIMO]))).toEqual([BEAM, LING])
  })
})
