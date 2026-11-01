import { KILO_AUTO_FREE_MODEL as ADAPTER_AUTO_FREE } from '@teammate/runtime-adapters'
import { describe, expect, it } from 'vitest'

import { KILO_AUTO_FREE_MODEL, kiloModelDescription, kiloModelName, kiloStartModel } from './kilo-models.js'

describe('Kilo model rows', () => {
  it('uses the same Auto Free id the run starts on', () => {
    expect(KILO_AUTO_FREE_MODEL).toBe(ADAPTER_AUTO_FREE)
    expect(KILO_AUTO_FREE_MODEL).toBe('kilo/kilo-auto/free')
  })

  it('says the privacy cost on Auto Free only', () => {
    const auto = kiloModelDescription(KILO_AUTO_FREE_MODEL)
    expect(auto).toContain('Do not submit personal or confidential data')
    expect(auto).toContain('log prompts and outputs')
    expect(kiloModelDescription('kilo/other/model:free')).toBe('Free · no sign-in')
    expect(kiloModelDescription('vendor/model-free')).toBe('Free · no sign-in')
    expect(kiloModelDescription('kilo/google/gemini-3-pro-image')).toBe('Through Kilo. A paid model needs a sign-in.')
    expect(kiloModelDescription('kilo/free-tier-preview')).not.toContain('confidential')
  })

  it('moves a new conversation off the account default once Auto Free is listed', () => {
    const listed = [{ runtime: 'kilo', id: KILO_AUTO_FREE_MODEL }]
    expect(kiloStartModel('kilo', 'account-default', listed)).toBe(KILO_AUTO_FREE_MODEL)
    expect(kiloStartModel('kilo', 'account-default', [])).toBeUndefined()
    expect(kiloStartModel('kilo', 'kilo/google/gemini-3-pro-image', listed)).toBeUndefined()
    expect(kiloStartModel('opencode', 'account-default', listed)).toBeUndefined()
  })

  it('names a model without the gateway every Kilo row would repeat', () => {
    // The picker's Kilo group, 2026-11-01: five rows each read "Kilo Gateway/NVIDIA: Nemotr..." at its width.
    expect(kiloModelName('kilo/nvidia/nemotron-3.5-lightning:free', 'Kilo Gateway/NVIDIA: Nemotron 3.5 Lightning (free)')).toBe('NVIDIA: Nemotron 3.5 Lightning (free)')
    // A provider of the person's own, through Kilo, is whose model it is, and stays.
    expect(kiloModelName('anthropic/claude-sonnet-4.5', 'Anthropic/Claude Sonnet 4.5')).toBe('Anthropic/Claude Sonnet 4.5')
  })

  it("keeps Kilo's word on its own router, so the chip never says a bare Auto", () => {
    expect(kiloModelName(KILO_AUTO_FREE_MODEL, 'Kilo Gateway/Auto Free')).toBe('Kilo Auto Free')
    expect(kiloModelName('kilo/kilo-auto/frontier', 'Kilo Gateway/Auto Frontier')).toBe('Kilo Auto Frontier')
    expect(kiloModelName('kilo/kilo-auto/small', 'Kilo Gateway/Kilo Auto Small')).toBe('Kilo Auto Small')
  })
})
