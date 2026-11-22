import { describe, expect, it } from 'vitest'

import { withAttachments } from '../../shared/attachments.js'
import type { PublicRecoveredMission } from '../../shared/ipc.js'
import { MAX_ROUTINE_STEPS, draftName, routineDraft, routineRunSummary, routineStepLabel, routineScheduleSummary, routineWaitsForYou } from './routines.js'

function turn(
  missionId: string,
  prompt: string,
  options: {
    readonly follows?: string
    readonly startedBy?: PublicRecoveredMission['startedBy']
    readonly phase?: PublicRecoveredMission['phase']
  } = {}
): PublicRecoveredMission {
  return {
    missionId,
    runId: `run_${missionId}`,
    workspaceId: 'ws_test',
    prompt,
    runtime: 'cursor',
    model: 'composer-2.5',
    requestedRouteId: 'cursor',
    resolvedRouteId: 'cursor-account:default',
    cliVersion: null,
    createdAt: '2026-09-05T10:00:00.000Z',
    lastUpdatedAt: '2026-09-05T10:00:00.000Z',
    phase: options.phase ?? 'completed',
    events: [],
    eventCount: 0,
    eventsTruncated: false,
    integrityIssueCount: 0,
    sandbox: 'read-only',
    checkpoints: [],
    peerMessages: [],
    ...(options.startedBy === undefined ? {} : { startedBy: options.startedBy }),
    ...(options.follows === undefined
      ? {}
      : { continuesFrom: { missionId: options.follows, reason: 'follow-up' as const, checkpointEpoch: 1 } })
  } as PublicRecoveredMission
}

const index = (missions: readonly PublicRecoveredMission[]): ReadonlyMap<string, PublicRecoveredMission> =>
  new Map(missions.map((mission) => [mission.missionId, mission]))

describe('saving a conversation as a routine', () => {
  // L17 (the code review): a first turn with a file attached named the
  // routine "Read this file in the workspace before you answer: - notes.md".
  it('is named after what the person typed, not the attachment line', () => {
    const missions = [turn('m1', withAttachments('Summarise the notes.', ['notes.md']))]
    const draft = routineDraft(missions[0]!, index(missions))
    expect(draft?.name).toBe('Summarise the notes.')
    // The step keeps its file, so a replay reads it again.
    expect(draft?.steps[0]).toContain('notes.md')
  })

  it('takes the words a person typed on each turn, in order, with where each came from', () => {
    const missions = [
      turn('m1', 'Read status.ts and summarise it.'),
      turn('m2', 'Now list every file, one per line.', { follows: 'm1' }),
      turn('m3', 'Say done.', { follows: 'm2' })
    ]
    const draft = routineDraft(missions[2]!, index(missions))
    expect(draft?.steps).toEqual(['Read status.ts and summarise it.', 'Now list every file, one per line.', 'Say done.'])
    expect(draft?.learnedFrom).toEqual(['m1', 'm2', 'm3'])
    expect(draft?.name).toBe('Read status.ts and summarise it.')
    expect(draft?.truncated).toBe(false)
  })

  it('drops turns the HOST wrote, whatever started them', () => {
    // A relayed reply, a resume and a routine step each carry a briefing
    // written for a runtime. Replaying those would replay the app's own
    // words back at itself.
    const missions = [
      turn('m1', 'Read status.ts.'),
      turn('m2', 'Booty (Custom) replied to you; it is quoted below...', {
        follows: 'm1',
        startedBy: { kind: 'relay', hop: 1 }
      }),
      turn('m3', 'Pick this back up from its last checkpoint...', {
        follows: 'm2',
        startedBy: { kind: 'resume', epoch: 2 }
      }),
      turn('m4', 'List every file.', { follows: 'm3' })
    ]
    const draft = routineDraft(missions[3]!, index(missions))
    expect(draft?.steps).toEqual(['Read status.ts.', 'List every file.'])
    expect(draft?.learnedFrom).toEqual(['m1', 'm4'])
  })

  it('leaves out a turn the person stopped, and keeps the ones that worked', () => {
    // The 0.271 recheck: "Save as routine" proposed a CANCELLED prompt as a
    // step. A routine replays what a conversation did.
    const missions = [
      turn('m1', 'Read status.ts.'),
      turn('m2', 'Delete everything in dist.', { follows: 'm1', phase: 'cancelled' }),
      turn('m3', 'List every file.', { follows: 'm2' })
    ]
    expect(routineDraft(missions[2]!, index(missions))?.steps).toEqual(['Read status.ts.', 'List every file.'])
  })

  it('still offers a conversation whose every turn failed, less the ones stopped', () => {
    const missions = [
      turn('m1', 'Summarise the report.', { phase: 'failed' }),
      turn('m2', 'Not this one.', { follows: 'm1', phase: 'cancelled' })
    ]
    expect(routineDraft(missions[1]!, index(missions))?.steps).toEqual(['Summarise the report.'])
  })

  it('has nothing to save when a person typed nothing in it', () => {
    const relayed = turn('m1', 'Wren asked you to...', { startedBy: { kind: 'relay', hop: 1 } })
    expect(routineDraft(relayed, index([relayed]))).toBeUndefined()
    const empty = turn('m1', '   ')
    expect(routineDraft(empty, index([empty]))).toBeUndefined()
  })

  it('cuts a long conversation at the limit and says it was cut', () => {
    const missions: PublicRecoveredMission[] = []
    for (let index_ = 1; index_ <= MAX_ROUTINE_STEPS + 3; index_ += 1) {
      missions.push(
        turn(`m${String(index_)}`, `step ${String(index_)}`, index_ === 1 ? {} : { follows: `m${String(index_ - 1)}` })
      )
    }
    const draft = routineDraft(missions.at(-1)!, index(missions))
    expect(draft?.steps).toHaveLength(MAX_ROUTINE_STEPS)
    expect(draft?.steps.at(-1)).toBe(`step ${String(MAX_ROUTINE_STEPS)}`)
    expect(draft?.learnedFrom).toHaveLength(MAX_ROUTINE_STEPS)
    expect(draft?.truncated).toBe(true)
  })
})

describe('how a routine reads', () => {
  it('names itself from the first step, on a word boundary, without pretending to be short', () => {
    expect(draftName('  Read   status.ts\nand summarise  ')).toBe('Read status.ts and summarise')
    const long = draftName('Read every file in this workspace and write a summary of what the project is for')
    expect(long.endsWith('...')).toBe(true)
    expect(long.length).toBeLessThanOrEqual(63)
    expect(long).not.toContain('  ')
  })

  it('says how many steps and whether it has ever run', () => {
    expect(routineRunSummary({ steps: ['a'], runs: 0 })).toBe('1 step · not run yet')
    expect(routineRunSummary({ steps: ['a', 'b'], runs: 1 })).toBe('2 steps · run 1 time')
    expect(routineRunSummary({ steps: ['a', 'b'], runs: 4 })).toBe('2 steps · run 4 times')
  })

  it('says which step is running', () => {
    expect(routineStepLabel({ step: 2, of: 3 })).toBe('routine · step 2 of 3')
  })
})

describe('what a card says about a schedule', () => {
  const base = { createdAt: new Date(2026, 8, 4, 9, 0, 0).toISOString() }
  const now = new Date(2026, 8, 5, 8, 0, 0)

  it('says nothing for a routine that only runs when pressed', () => {
    expect(routineScheduleSummary({ ...base, schedule: undefined }, now)).toBeUndefined()
  })

  it('names the rule and the next run in local clock words: today, tomorrow, a weekday, or due now', () => {
    expect(routineScheduleSummary({ ...base, schedule: { kind: 'daily', at: '09:30' } }, now)).toBe('daily at 9:30 AM · next 9:30 AM')
    expect(
      routineScheduleSummary({ ...base, schedule: { kind: 'daily', at: '07:00' }, lastRunAt: new Date(2026, 8, 5, 7, 1).toISOString() }, now)
    ).toBe('daily at 7:00 AM · next tomorrow 7:00 AM')
    expect(
      routineScheduleSummary({ ...base, schedule: { kind: 'every', hours: 4 }, lastRunAt: new Date(2026, 8, 5, 6, 0).toISOString() }, now)
    ).toBe('every 4 hours · next 10:00 AM')
    expect(
      routineScheduleSummary({ ...base, schedule: { kind: 'every', hours: 168 }, lastRunAt: new Date(2026, 8, 5, 6, 0).toISOString() }, now)
    ).toBe('every 168 hours · next Sat 6:00 AM')
    expect(routineScheduleSummary({ ...base, schedule: { kind: 'every', hours: 1 } }, now)).toBe('every hour · due now')
  })
})

describe('a scheduled routine in Approve each (0.591)', () => {
  const route = (mode: 'ask' | 'plan' | 'accept-edits' | 'approve-each' | 'auto') => ({ runtime: 'codex' as const, model: 'account-default', mode })
  it('says it waits for you when it fires unattended', () => {
    expect(routineWaitsForYou({ route: route('approve-each'), schedule: { kind: 'daily', at: '07:00' } })).toBe(
      'Runs in Approve each: fired while you are away, it waits for you on its first card. Ask or Accept edits runs through.'
    )
  })
  it('says nothing for the other modes, or for a routine that only runs when pressed (controls)', () => {
    expect(routineWaitsForYou({ route: route('ask'), schedule: { kind: 'daily', at: '07:00' } })).toBeUndefined()
    expect(routineWaitsForYou({ route: route('accept-edits'), schedule: { kind: 'every', hours: 2 } })).toBeUndefined()
    expect(routineWaitsForYou({ route: route('auto'), schedule: { kind: 'daily', at: '07:00' } })).toBeUndefined()
    expect(routineWaitsForYou({ route: route('approve-each') })).toBeUndefined()
  })
})
