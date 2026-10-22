import { describe, expect, it } from 'vitest'

import type { PublicModel } from '../../shared/ipc.js'
import { defaultComparePicks } from './compareDefaults.js'
import { retiredFreeModels } from './missionView.js'
import { FREE_MODELS_BEST_FIRST, freeModelsBestFirst, freeStartModel, nextFreeModel } from './status.js'

/*
 * 0.712. OpenCode lists its free models alphabetically, and a new person's
 * first message went to the first of them: Exo (retired, 0.710), then Fledge
 * Alpha (gone from OpenCode's server, the first message on a bare Mac), then
 * Ling 3.0 -- down since 2026-10-05, and OpenCode retries a model that is down
 * for about two and a half minutes before it gives up. The free models are
 * now taken steadiest first; the alphabet only orders the ones not named.
 */
const model = (id: string): PublicModel =>
  ({ id, runtime: 'opencode', displayName: id, description: '', supportedEfforts: [] }) as unknown as PublicModel
// What OpenCode listed as free on 2026-10-22, as the picker showed it (shared/free-models.ts).
const FRESH = [
  'opencode/beam-free', 'opencode/deepseek-v4.1-flash-free', 'opencode/flint-40b-free',
  'opencode/glm-5.3-flash-free', 'opencode/mimo-v2.6-flash-free', 'opencode/minimax-m3.1-flash-free',
  'opencode/nemotron-3-ultra-free', 'opencode/nemotron-3.5-lightning-free', 'opencode/nemotron-3.5-super-vl-free'
].map(model)
const retired = retiredFreeModels([])
const shown = FRESH.filter((entry) => !retired.has(entry.id))

describe('a new person starts on the steadiest free model', () => {
  it('not on the first the alphabet lists', () => {
    expect(freeStartModel('opencode', shown, [])).toBe('opencode/deepseek-v4.1-flash-free')
    expect(freeStartModel('opencode', shown, [])).not.toBe('opencode/beam-free')
  })

  it('and a failure there offers the next steadiest, with the unnamed after all of them', () => {
    expect(nextFreeModel('opencode', 'opencode/deepseek-v4.1-flash-free', shown)?.id).toBe('opencode/nemotron-3.5-lightning-free')
    const order = freeModelsBestFirst(shown).map((entry) => entry.id)
    expect(order.slice(0, FREE_MODELS_BEST_FIRST.length)).toEqual(FREE_MODELS_BEST_FIRST)
    // Not yet asked, so not named: after the named ones, in the order OpenCode lists them.
    expect(order.slice(FREE_MODELS_BEST_FIRST.length)).toEqual(['opencode/flint-40b-free', 'opencode/minimax-m3.1-flash-free', 'opencode/nemotron-3-ultra-free'])
  })

  it('and Compare puts the next steadiest beside it', () => {
    const picks = defaultComparePicks({
      current: { runtime: 'opencode', model: 'opencode/nemotron-3.5-lightning-free' },
      recent: [],
      models: shown,
      ready: () => true,
      refusal: () => undefined,
      label: (choice) => choice.model
    }).map((pick) => pick.model)
    expect(picks).toEqual(['opencode/nemotron-3.5-lightning-free', 'opencode/deepseek-v4.1-flash-free'])
  })

  it('the order only orders: a name OpenCode no longer lists adds nothing', () => {
    expect(freeModelsBestFirst([model('opencode/zz-new-free')]).map((entry) => entry.id)).toEqual(['opencode/zz-new-free'])
    expect(freeStartModel('opencode', [model('opencode/paid-thing')])).toBeUndefined()
  })
})
