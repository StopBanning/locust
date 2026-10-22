import { describe, expect, it } from 'vitest'

import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'
import type { PublicModel } from '../../shared/ipc.js'
import { defaultComparePicks } from './compareDefaults.js'
import { KNOWN_RETIRED_MODELS, modelRetired, retiredFreeModels } from './missionView.js'
import { freeStartModel } from './status.js'

/*
 * 0.710. OpenCode retired Exo Free on 2026-10-08 -- every run on it ended
 * "OpenCode stopped: Model exo-free has been deprecated." -- while its model
 * list still said active. First of the free models alphabetically, it was
 * what "Use a free model" gave a new person and what Compare put beside a free
 * model. The window's list now leaves out a model OpenCode retired.
 */
const model = (id: string): PublicModel =>
  ({ id, runtime: 'opencode', displayName: id, description: '', supportedEfforts: [] }) as unknown as PublicModel
const catalog = ['opencode/exo-free', 'opencode/ling-3.0-flash-fin-free', 'opencode/glm-5.3-flash-free', 'opencode/zz-later-free'].map(model)
const failedWith = (message: string): NormalizedRuntimeEvent =>
  ({ type: 'run.failed', payload: { message } }) as unknown as NormalizedRuntimeEvent
const run = (id: string, phase: 'completed' | 'failed', at: string, message?: string) => ({
  runtime: 'opencode' as const,
  model: id,
  phase,
  lastUpdatedAt: at,
  events: message === undefined ? [] : [failedWith(message)]
})
const shown = (retired: ReadonlySet<string>) => catalog.filter((entry) => !retired.has(entry.id))

describe('a free model OpenCode retired is not offered', () => {
  it('reads OpenCode\'s own words, and nothing else, as retired', () => {
    expect(modelRetired({ message: 'OpenCode stopped: Model exo-free has been deprecated.' })).toBe(true)
    expect(modelRetired({ message: 'OpenCode stopped: Endpoint is unavailable.' })).toBe(false)
    expect(modelRetired({ message: 'This API has been deprecated in favour of v2' })).toBe(false)
    expect(modelRetired({})).toBe(false)
  })

  it('Exo Free is left out before anyone here has run it: a new person starts on a free model that can answer', () => {
    const retired = retiredFreeModels([])
    expect([...retired]).toEqual(KNOWN_RETIRED_MODELS)
    expect(shown(retired).map((entry) => entry.id)).not.toContain('opencode/exo-free')
    // Best first since 0.712 (status.FREE_MODELS_BEST_FIRST): GLM 5.3 Flash is named; Ling 3.0 is not.
    expect(freeStartModel('opencode', shown(retired), [])).toBe('opencode/glm-5.3-flash-free')
  })

  it('a model whose run ended "has been deprecated" is left out from then on', () => {
    const retired = retiredFreeModels([
      run('opencode/ling-3.0-flash-fin-free', 'completed', '2026-10-01T10:00:00Z'),
      run('opencode/ling-3.0-flash-fin-free', 'failed', '2026-10-09T01:00:00Z', 'OpenCode stopped: Model ling-3.0-flash-fin-free has been deprecated.')
    ])
    expect(retired.has('opencode/ling-3.0-flash-fin-free')).toBe(true)
    expect(freeStartModel('opencode', shown(retired), [])).toBe('opencode/glm-5.3-flash-free')
  })

  it('one that answered again afterwards is back; another failure is not a retirement', () => {
    const retired = retiredFreeModels([
      run('opencode/exo-free', 'completed', '2026-10-20T10:00:00Z'),
      run('opencode/glm-5.3-flash-free', 'failed', '2026-10-09T01:00:00Z', 'OpenCode stopped: Endpoint is unavailable.')
    ])
    expect(retired.has('opencode/exo-free')).toBe(false)
    expect(retired.has('opencode/glm-5.3-flash-free')).toBe(false)
  })

  it('Compare on a free model puts another WORKING free model beside it', () => {
    const picks = (models: readonly PublicModel[]) =>
      defaultComparePicks({
        current: { runtime: 'opencode', model: 'opencode/glm-5.3-flash-free' },
        recent: [],
        models,
        ready: () => true,
        refusal: () => undefined,
        label: (choice) => choice.model
      }).map((pick) => pick.model)
    // What 0.709 did: Exo, the first other free model.
    expect(picks(catalog)).toEqual(['opencode/glm-5.3-flash-free', 'opencode/exo-free'])
    expect(picks(shown(retiredFreeModels([])))).toEqual(['opencode/glm-5.3-flash-free', 'opencode/ling-3.0-flash-fin-free'])
  })
})
