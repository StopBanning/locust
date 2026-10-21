import { describe, expect, it } from 'vitest'

import type { PublicRuntimeStatus } from '../../shared/ipc.js'
import { routeRowStatus } from './status.js'

/* A future OpenCode protocol is refused in discovery and remains unavailable in the window. */
const NOTE = 'OpenCode 3.0.0 is installed; this Locust supports OpenCode 1 and OpenCode 2.'

describe('an OpenCode version Locust does not run yet', () => {
  it('is not offered in the picker, and says why rather than "checking"', () => {
    const runtime = { id: 'opencode', displayName: 'OpenCode', installed: true, version: '3.0.0', auth: 'not-applicable', ready: false, status: 'unsupported', unsupportedNote: NOTE } as PublicRuntimeStatus
    const row = routeRowStatus(runtime, 'supported' as never, false)
    expect(row).toMatchObject({ tag: 'UNAVAILABLE', selectable: false, detail: NOTE })
  })
})
