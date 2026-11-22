import { renderToStaticMarkup } from 'react-dom/server'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { PublicRoutine } from '../../shared/ipc.js'
import { AutomationsScreen } from './components/AutomationsScreen.js'
import { missedLine, routineScheduleSummary } from './routines.js'

/**
 * A routine whose time passed while Locust was closed shows as
 * "Missed, 08:00 today" with Run now. The Routines screen says whether
 * signing in will start Locust, and that word opens the switch.
 */

const now = new Date(2026, 9, 3, 9, 0, 0)
const missedAt = new Date(2026, 9, 3, 8, 0, 0).toISOString()

const routine = {
  routineId: 'rt_1',
  name: 'Morning digest',
  teammateId: 'tm_wren',
  route: { runtime: 'cursor', model: 'composer-2.5', mode: 'ask' },
  steps: ['Say good morning.'],
  learnedFrom: [],
  createdAt: new Date(2026, 9, 2, 8, 0).toISOString(),
  lastRunAt: missedAt,
  runs: 0,
  schedule: { kind: 'daily', at: '08:00' },
  missedAt
} as PublicRoutine

describe('a missed routine', () => {
  // The screen reads the clock itself; pinned to the test's own `now`, or the
  // render says "yesterday" whenever the suite runs before 08:00 (0.583).
  afterEach(() => {
    vi.useRealTimers()
  })

  it('says Missed, 08:00 today, and the row offers Run now', () => {
    expect(missedLine(missedAt, now)).toBe('Missed, 8:00 AM today')
    expect(routineScheduleSummary(routine, now)).toBe('Missed, 8:00 AM today')
    vi.useFakeTimers({ toFake: ['Date'] })
    vi.setSystemTime(now)
    const html = renderToStaticMarkup(
      <AutomationsScreen
        routines={[routine]}
        teammates={[]}
        routineStepByTeammate={{}}
        onRunRoutine={() => undefined}
        onEditRoutine={() => undefined}
        onRemoveRoutine={() => undefined}
        notice={undefined}
        onDismissNotice={() => undefined}
        signInOn={false}
        onOpenSignIn={() => undefined}
      />
    )
    expect(html).toContain('Missed, 8:00 AM today')
    expect(html).toContain('Run now')
    expect(html).toContain('Routines run while Locust is open. Start Locust when you sign in:')
    expect(html).toContain('Off')
  })
})
