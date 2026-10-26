import { KILO_AUTO_FREE_MODEL as ADAPTER_AUTO_FREE } from '@teammate/runtime-adapters'
import { describe, expect, it } from 'vitest'

import { KILO_AUTO_FREE_MODEL, kiloModelDescription, kiloStartModel } from './kilo-models.js'

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
})
