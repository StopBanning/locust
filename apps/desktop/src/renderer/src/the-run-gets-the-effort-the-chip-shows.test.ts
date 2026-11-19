import { describe, expect, it } from 'vitest'

import { runEffortFor } from './App.js'
import type { PublicModel } from '../../shared/ipc.js'

// Was swarm.test.ts. Swarm (every run at its model's maximum) was cut in 0.880; what is left is the effort a run is
// started with, which is the one the chip shows.
const MODELS: readonly PublicModel[] = [
  {
    id: 'sol',
    runtime: 'codex',
    displayName: 'Sol',
    description: '',
    supportedEfforts: ['low', 'medium', 'high', 'xhigh', 'max', 'ultra']
  },
  { id: 'fable', runtime: 'claude', displayName: 'Fable (latest)', description: '', supportedEfforts: ['low', 'medium', 'high', 'xhigh', 'max'], defaultEffort: 'high' }
]

describe('the effort a run is started with', () => {
  it('uses the chosen effort', () => {
    expect(runEffortFor(MODELS, 'sol', 'medium')).toBe('medium')
  })

  it('falls back to the level the chip shows, not to nothing', () => {
    // This used to assert `undefined`, and that assertion was the bug: the
    // composer rendered `effort ?? defaultEffort(supported)` while this sent
    // only `effort`, so from every launch the chip stated "medium" and the
    // run was started with no effort argument at all. Both now read the same
    // expression, so the level on screen is the level the run is given.
    expect(runEffortFor(MODELS, 'sol', undefined)).toBe('medium')
  })

  it('still sends nothing for a model that reports no levels', () => {
    expect(runEffortFor([{ ...MODELS[0]!, id: 'bare', supportedEfforts: [] }], 'bare', undefined)).toBeUndefined()
  })

  it("reads a Claude alias's default from the Claude entry, never a Codex one", () => {
    expect(runEffortFor(MODELS, 'fable', undefined, 'claude')).toBe('high')
    expect(runEffortFor(MODELS, 'fable', undefined, 'codex')).toBeUndefined()
  })
})
