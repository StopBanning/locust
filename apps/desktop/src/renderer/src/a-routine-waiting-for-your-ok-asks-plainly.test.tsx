import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import type { PublicRoutine } from '../../shared/ipc.js'
import { RoutineRecovery } from './components/RoutineRecovery.js'
import { routineRunSummary, routineScheduleSummary } from './routines.js'

/**
 * A ROUTINE WAITING FOR YOUR OK ASKS PLAINLY (0.840). A step set to Wait for my OK first holds the run before it. That
 * is a question the person chose to be asked, not a recovery: the card says what the next step will do and offers
 * Go ahead and Stop here, without the recovery's "I reviewed..." box.
 */
const read = (path: string): string => readFileSync(fileURLToPath(new URL(path, import.meta.url)), 'utf8')

const waiting: PublicRoutine = {
  routineId: 'rt_ship', name: 'Ship the fix', teammateId: 'tm_wren', route: { runtime: 'cursor', model: 'composer-2.5', mode: 'ask' },
  steps: ['Fix the failing test.', 'Push the branch and open a pull request.'], learnedFrom: [], createdAt: '2026-10-28T00:00:00.000Z', runs: 3,
  handOffs: [{}, { askFirst: true }],
  execution: { attemptId: 'attempt_one', status: 'held', step: 1, of: 2, workspaceId: 'ws_test',
    startedAt: '2026-10-28T09:00:00.000Z', updatedAt: '2026-10-28T09:05:00.000Z',
    steps: ['Fix the failing test.', 'Push the branch and open a pull request.'], route: { runtime: 'cursor', model: 'composer-2.5', mode: 'ask' },
    missionId: 'mission_one', runId: 'run_one', reason: 'Step 2 waits for your OK. Read what step 1 did, then go ahead or stop here.',
    canContinue: true, settledAtDispatch: true, waitsForOk: true }
}

describe('a routine waiting for your OK', () => {
  it('asks with the next step in its own words and two answers, ready to press', () => {
    const html = renderToStaticMarkup(<RoutineRecovery routine={waiting} recover={async () => ({ ok: true })} onOpenMission={() => undefined} />)
    expect(html).toContain('Waiting for your OK')
    expect(html).toContain('Step 2 of 2')
    expect(html).toContain('Step 2: Push the branch and open a pull request.')
    expect(html).toContain('Open the conversation')
    expect(html).toMatch(/<button[^>]*class="lc-primarybutton"[^>]*>Go ahead with step 2<\/button>/)
    expect(html).toMatch(/<button[^>]*>Stop here<\/button>/)
    expect(html).not.toContain('disabled')
    expect(html).not.toContain('I reviewed')
    expect(html).not.toContain('Waiting for your review')
    expect(html).toContain('the routine runs again from step 1 when you press Run')
  })

  it('a held routine that did not ask is still the recovery card', () => {
    const { waitsForOk: _asked, ...execution } = waiting.execution!
    const html = renderToStaticMarkup(<RoutineRecovery routine={{ ...waiting, execution }} recover={async () => ({ ok: true })} />)
    expect(html).toContain('Waiting for your review')
    expect(html).toContain('I reviewed')
  })

  it('says waiting for your OK on the routine row', () => {
    expect(routineRunSummary(waiting)).toBe('2 steps · waiting for your OK · 3 completed runs')
    expect(routineScheduleSummary({ ...waiting, schedule: { kind: 'daily', at: '09:00' } }, new Date('2026-10-28T10:00:00'))).toBe('daily at 09:00 · waiting for your OK')
  })

  it('is set per step from step 2 on in the routine editor, for one teammate too', () => {
    const dialog = read('./components/RoutineDialog.tsx')
    expect(dialog).toContain('Wait for my OK first')
    expect(dialog).toContain('{(canHandOff || index > 0) && (')
    expect(dialog).toContain('{index > 0 && (')
    expect(dialog).toContain('...(canHandOff || asksFirst')
  })
})
