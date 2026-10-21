import { describe, expect, it } from 'vitest'

import { publicStatus } from './runtime-discovery.js'

/* A future OpenCode protocol is refused in discovery and remains unavailable in the window. */
const NOTE = 'OpenCode 3.0.0 is installed; this Locust supports OpenCode 1 and OpenCode 2.'

describe('an OpenCode version Locust does not run yet', () => {
  it('reaches the window as unsupported, with why and what to do', () => {
    const shown = publicStatus({
      id: 'opencode',
      displayName: 'OpenCode',
      availability: 'available',
      readiness: 'unsupported',
      version: { raw: 'opencode v3.0.0', version: '3.0.0', major: 3, minor: 0, patch: 0 },
      diagnostics: [{ code: 'version-unsupported', severity: 'error', message: NOTE, resolution: 'Use a supported OpenCode 1 or OpenCode 2 release, or update Locust for this version.' }]
    } as never)
    expect(shown).toMatchObject({ status: 'unsupported', ready: false })
    expect(shown.unsupportedNote).toContain(NOTE)
    expect(shown.unsupportedNote).toContain('OpenCode 1 or OpenCode 2')
  })

  it('leaves every other runtime as it was (control)', () => {
    const shown = publicStatus({ id: 'opencode', displayName: 'OpenCode', availability: 'available', readiness: 'ready', diagnostics: [] } as never)
    expect(shown.status).toBe('ready')
    expect(shown).not.toHaveProperty('unsupportedNote')
  })
})
