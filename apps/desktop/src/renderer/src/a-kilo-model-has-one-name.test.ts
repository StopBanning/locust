import { afterEach, describe, expect, it } from 'vitest'

import { modelDisplayName, rememberOwnModels, routeChrome } from './routeName.js'

/**
 * A KILO MODEL HAS ONE NAME (0.848). The picker drew Kilo's own names and the chip spelled the ids: Kit's free
 * model was "Kilo Gateway/Auto Free" in one and "Kilo Auto/Free" in the other, and a free Nemotron read "Nvidia/Nemotron
 * 3.5 Lightning:free" on the chip. The names arrive without "Kilo Gateway/" (kiloModelName, as the catalog reads them).
 */
describe('a Kilo model on the chip', () => {
  afterEach(() => rememberOwnModels([]))

  it('says the name Kilo lists it by, as the picker does', () => {
    rememberOwnModels([
      { id: 'kilo/kilo-auto/free', displayName: 'Kilo Auto Free', runtime: 'kilo' },
      { id: 'kilo/nvidia/nemotron-3.5-lightning:free', displayName: 'NVIDIA: Nemotron 3.5 Lightning (free)', runtime: 'kilo' },
      { id: 'kilo/~anthropic/claude-sonnet-latest', displayName: 'Anthropic: Claude Sonnet Latest', runtime: 'kilo' }
    ])
    expect(modelDisplayName('kilo', 'kilo/kilo-auto/free')).toBe('Kilo Auto Free')
    // A row of the team or a room says the runtime once.
    expect(routeChrome('kilo', 'kilo/kilo-auto/free', modelDisplayName('kilo', 'kilo/kilo-auto/free'))).toBe('Kilo Auto Free')
    expect(routeChrome('kilo', 'kilo/nvidia/nemotron-3.5-lightning:free', 'NVIDIA: Nemotron 3.5 Lightning (free)')).toBe('Kilo / NVIDIA: Nemotron 3.5 Lightning (free)')
    expect(modelDisplayName('kilo', 'kilo/nvidia/nemotron-3.5-lightning:free')).toBe('NVIDIA: Nemotron 3.5 Lightning (free)')
    expect(modelDisplayName('kilo', 'kilo/~anthropic/claude-sonnet-latest')).toBe('Anthropic: Claude Sonnet Latest')
  })

  it('leaves every other runtime to spell its ids', () => {
    // OpenCode's catalogue says a model's id; a name that is only a description never replaces a spelling.
    rememberOwnModels([{ id: 'gpt-6-luna', displayName: 'Fast and affordable', runtime: 'codex' }])
    expect(modelDisplayName('codex', 'gpt-6-luna')).toBe('GPT-6 Luna')
  })

  it('spells the id while Kilo has not listed its models yet', () => {
    expect(modelDisplayName('kilo', 'kilo/nvidia/nemotron-3-nano-30b-a3b')).toBe('Nvidia/Nemotron 3 Nano 30b A3b')
  })
})
