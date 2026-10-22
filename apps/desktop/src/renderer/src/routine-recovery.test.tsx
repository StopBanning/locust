import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import type { PublicRoutine, PublicTeammate } from '../../shared/ipc.js'
import { RoutineRecovery } from './components/RoutineRecovery.js'
import { AutomationsScreen } from './components/AutomationsScreen.js'
import { TeammatesScreen } from './components/Screens.js'
import { routineRunSummary, routineScheduleSummary } from './routines.js'
import { seedAvatar } from '../../shared/avatar.js'

const routine: PublicRoutine = {
  routineId: 'rt_one', name: 'Release', teammateId: 'tm_one', route: { runtime: 'codex', model: 'account-default', mode: 'ask' },
  steps: ['Open PR', 'Summarise', 'Report'], learnedFrom: [], createdAt: '2026-09-01T00:00:00.000Z', runs: 0,
  schedule: { kind: 'every', hours: 6 },
  execution: { attemptId: 'attempt_one', status: 'held', step: 2, of: 3, workspaceId: 'ws_test',
    startedAt: '2026-09-04T00:00:00.000Z', updatedAt: '2026-09-04T00:00:00.000Z',
    steps: ['Open PR', 'Summarise', 'Report'], route: { runtime: 'codex', model: 'account-default', mode: 'ask' },
    missionId: 'mission_two', runId: 'run_two', reason: 'The dispatch outcome is uncertain.', canContinue: false }
}
const teammate: PublicTeammate = { teammateId: 'tm_one', name: 'One', role: 'Custom', hue: 'lime', avatar: seedAvatar('tm_one'), route: routine.route, createdAt: routine.createdAt }
const nothing = (): void => undefined

describe('recovery where a person looks for routines', () => {
  it('both routine surfaces render a held attempt from persisted data, with no notification event', () => {
    const persisted = JSON.parse(JSON.stringify(routine)) as PublicRoutine
    const automations = renderToStaticMarkup(<AutomationsScreen routines={[persisted]} teammates={[teammate]}
      routineStepByTeammate={{}} onRunRoutine={nothing} onEditRoutine={nothing} onRemoveRoutine={nothing}
      notice={undefined} onDismissNotice={nothing}  />)
    const team = renderToStaticMarkup(<TeammatesScreen teammates={[teammate]} missions={[]} missionOwners={{}} spendByTeammate={{}} viewByTeammate={{}}
      titleOf={() => ''} onOpenMission={nothing} onNewTeammate={nothing} onEdit={nothing} onRemove={nothing} onMessage={nothing}
      routines={[persisted]} routineStepByTeammate={{}} onRunRoutine={nothing} onEditRoutine={nothing} onRemoveRoutine={nothing} />)
    for (const html of [automations, team]) {
      expect(html).toContain('Waiting for your review')
      expect(html).toContain('Step 2 of 3')
      expect(html).toContain('Attempt started')
      expect(html).toContain('mission_two')
      expect(html).toContain('no automatic retry')
      expect(html).not.toContain('not run yet')
      expect(html).not.toContain('due now')
      expect(html).toMatch(/disabled=""[^>]*>Run<\/button>/)
    }
  })

  it('review acknowledgement is unchecked and both decisions start disabled, even for a confirmed step', () => {
    const html = renderToStaticMarkup(<RoutineRecovery routine={{ ...routine, execution: { ...routine.execution!, canContinue: true } }} recover={async () => ({ ok: true })} />)
    expect(html).toContain('I reviewed the saved conversation and external work')
    expect(html).not.toContain('checked=""')
    expect(html).toMatch(/disabled="">Continue remaining steps/)
    expect(html).toMatch(/disabled="">Abandon attempt and remove schedule/)
    expect(html).toContain('Neither stops a runtime or undoes work')
    expect(html).toContain('starts from step 1')
    expect(html).toContain('Open PR')
    expect(html).toContain('codex')
  })

  it('abandonment stays visible after acknowledgement instead of reverting to never ran', () => {
    const abandoned = { ...routine, execution: { ...routine.execution!, status: 'abandoned' as const, reason: 'Abandoned by you; schedule removed.' } }
    const html = renderToStaticMarkup(<RoutineRecovery routine={abandoned} />)
    expect(html).toContain('Attempt abandoned')
    expect(html).toContain('Step 2 of 3')
    expect(html).not.toContain('<button')
    expect(routineRunSummary(abandoned)).toContain('last attempt abandoned')
  })

  it('is a card of its own, never the row meta the Routines screen crushes into one clipped cell (2026-09-27)', () => {
    // Colin's screenshot: every sentence a column a few words wide, and the
    // review box and both decisions cut off, because the card wore this class.
    const html = renderToStaticMarkup(<RoutineRecovery routine={routine} onOpenMission={nothing} />)
    expect(html).toMatch(/^<div class="lc-recovery">/)
    expect(html).not.toContain('lc-routinerow__meta')
    expect(html).toContain('<span class="lc-recovery__actions">')
  })

  it('says the reason once, and opens the saved conversation rather than printing its id', () => {
    const interrupted = { ...routine, execution: { ...routine.execution!, reason: 'Review required: that run was interrupted. Nothing will be replayed.' } }
    const html = renderToStaticMarkup(<RoutineRecovery routine={interrupted} onOpenMission={nothing} />)
    expect(html).toContain('That run was interrupted. Nothing will be replayed.')
    expect(html).not.toContain('Review required')
    expect(html).toContain('title="mission_two">Open the saved conversation</button>')
    expect(html).not.toContain('Saved mission:')
  })

  it('names when Keep runs it next -- six hours from the decision, not the attempt (0.402 beta retest)', () => {
    const now = new Date('2026-09-27T14:00:00.000Z')
    const html = renderToStaticMarkup(<RoutineRecovery routine={routine} recover={async () => ({ ok: true })} now={now} />)
    const sixHoursOn = new Date(now.getTime() + 6 * 3_600_000).toLocaleString(undefined, { weekday: 'short', hour: 'numeric', minute: '2-digit' })
    expect(html).toContain(`its next run starts from step 1 at ${sixHoursOn}, not straight away`)
  })

  it('says why Continue is off, and offers to keep the schedule (0.390 beta pass)', () => {
    const earlier = renderToStaticMarkup(<RoutineRecovery routine={routine} recover={async () => ({ ok: true })} />)
    expect(earlier).toContain('Continue waits for step 2 to be confirmed finished. It was not, so step 3 would build on work that may not be there.')
    expect(earlier).toMatch(/disabled="">Keep the schedule/)
    // 0.404: the next run named, one interval from the decision, never at once.
    expect(earlier).toMatch(/its next run starts from step 1 at [^<]+, not straight away/)
    const last = { ...routine, execution: { ...routine.execution!, step: 3 } }
    const html = renderToStaticMarkup(<RoutineRecovery routine={last} recover={async () => ({ ok: true })} />)
    expect(html).toContain('This was the last step, so there is nothing left to continue.')
    expect(html).not.toContain('Continue remaining steps')
    expect(html).toContain('Keep the schedule')
    const { schedule: _schedule, ...unscheduled } = last
    expect(renderToStaticMarkup(<RoutineRecovery routine={unscheduled} recover={async () => ({ ok: true })} />)).toContain('Keep the routine')
  })

  it('says a refused start would be refused again, not that the step before it never finished (0.818)', () => {
    const tooLong = 'Step 2 is too long to send: 439 characters too long. Shorten it in Edit, then run the routine again. Nothing was started.'
    const refused = { ...routine, execution: { ...routine.execution!, step: 1, reason: tooLong, settledAtDispatch: true } }
    const html = renderToStaticMarkup(<RoutineRecovery routine={refused} recover={async () => ({ ok: true })} />)
    expect(html).toContain(tooLong)
    expect(html).toContain('Continue would be refused the same way, so it is off until that changes.')
    expect(html).not.toContain('to be confirmed finished')
  })

  it('an unresolved dispatch has the same visible protection; no optimistic next-run promise', () => {
    const dispatching = { ...routine, execution: { ...routine.execution!, status: 'dispatching' as const } }
    expect(renderToStaticMarkup(<RoutineRecovery routine={dispatching} />)).toContain('Waiting for your review')
    expect(routineScheduleSummary(dispatching, new Date('2026-09-08'))).toContain('held for review; no automatic retry')
  })
})
