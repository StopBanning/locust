import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'
import { describe, expect, it } from 'vitest'

import { NEEDS_YOU_TEXT, needsYou, needsYouChip, needsYouLabel, needsYouPlace, openQuestion } from './needsYou.js'

/**
 * WHAT WAITS ON YOU IS IN ONE PLACE (0.373).
 *
 * A paused run, a question a teammate stopped on, memory suggestions: each
 * was found only by opening the conversation it was in. The title bar now
 * counts them and lists them, most urgent first.
 */
const final = (text: string, itemId = 'm1'): NormalizedRuntimeEvent =>
  ({ type: 'message.delta', payload: { itemId, operation: 'replace', text, final: true } }) as unknown as NormalizedRuntimeEvent
const completed = { type: 'run.completed', payload: {} } as unknown as NormalizedRuntimeEvent
const ASK = '<locust-ask>\nWhich port should the API use?\n- 3000 :: as documented\n- 3001 :: as deployed\n</locust-ask>'

const names: Record<string, string> = { tm_wren: 'Wren', tm_pip: 'Pip' }
const base = {
  approvals: [] as { approvalId: string; runId: string; missionId: string; kind: string; summary: string; detail: string }[],
  ownerOfRun: (runId: string) => (runId === 'run_w' ? 'tm_wren' : undefined),
  conversations: [] as { missionId: string; phase: string }[],
  ownerOfMission: (missionId: string) => (missionId === 'mission_p' ? 'tm_pip' : undefined),
  eventsOf: (() => undefined) as (missionId: string) => readonly NormalizedRuntimeEvent[] | undefined,
  nameOf: (teammateId: string) => names[teammateId],
  memoryWaiting: 0
}

describe('the question a finished turn stopped on', () => {
  it('is its last final message’s question, once the run has completed', () => {
    expect(openQuestion([final(`Two ways to go.\n${ASK}`), completed])).toBe('Which port should the API use?')
    // Still running: the block may not be finished.
    expect(openQuestion([final(`Two ways to go.\n${ASK}`)])).toBeUndefined()
    // An earlier message asked; the last one did not.
    expect(openQuestion([final(ASK, 'm1'), final('Done, I went with 3001.', 'm2'), completed])).toBeUndefined()
  })

  it('is read the same the second time', () => {
    const events = [final(ASK), completed]
    expect(openQuestion(events)).toBe(openQuestion(events))
  })
})

describe('what waits on you', () => {
  it('is nothing when nothing does', () => {
    expect(needsYou(base)).toEqual([])
  })

  it('is a paused run first, then a question, then memory -- each named by its teammate', () => {
    const items = needsYou({
      ...base,
      approvals: [
        { approvalId: 'a1', runId: 'run_w', missionId: 'mission_w', kind: 'command', summary: 'Run a command', detail: 'npm test' },
        { approvalId: 'a2', runId: 'run_x', missionId: 'mission_x', kind: 'question', summary: 'Which branch?', detail: '' },
        { approvalId: 'a3', runId: 'run_w', missionId: 'mission_w', kind: 'file-change', summary: 'Change src/app.ts', detail: '@@ -1 +1 @@' }
      ],
      conversations: [
        { missionId: 'mission_p', phase: 'completed' },
        { missionId: 'mission_running', phase: 'running' },
        { missionId: 'mission_failed', phase: 'failed' },
        { missionId: 'mission_unread', phase: 'completed' }
      ],
      eventsOf: (missionId) => (missionId === 'mission_p' || missionId === 'mission_running' || missionId === 'mission_failed' ? [final(ASK), completed] : undefined),
      memoryWaiting: 3
    })
    expect(items.map((item) => item.key)).toEqual(['approval:a1', 'approval:a2', 'approval:a3', 'decision:mission_p', 'memory'])
    expect(items.map(needsYouLabel)).toEqual([
      'Wren wants to run: npm test',
      'A teammate is asking: Which branch?',
      'Wren needs your approval: Change src/app.ts',
      'Pip asked: Which port should the API use?',
      '3 memory suggestions waiting'
    ])
  })

  it('says each thing in one line, short enough to be a menu row', () => {
    const [item] = needsYou({ ...base, approvals: [{ approvalId: 'a1', runId: 'run_w', missionId: 'm', kind: 'command', summary: 'Run a command', detail: `echo\n${'x'.repeat(300)}` }] })
    const what = item?.kind === 'approval' ? item.what : ''
    expect(what.length).toBeLessThanOrEqual(NEEDS_YOU_TEXT)
    expect(what).not.toContain('\n')
    expect(what.endsWith('…')).toBe(true)
  })
})

describe('a Scout card and a routine waiting on you', () => {
  it('counts a card that needs you now, and a copy waiting for Keep or Discard, after a question and before memory', () => {
    const items = needsYou({
      ...base,
      approvals: [{ approvalId: 'a1', runId: 'run_w', missionId: 'mission_w', kind: 'command', summary: 'Run a command', detail: 'npm test' }],
      conversations: [{ missionId: 'mission_p', phase: 'completed' }],
      eventsOf: () => [final(ASK), completed],
      scoutName: 'Scout',
      scoutCards: [
        { cardId: 'card_soon', urgency: 'soon', said: 'A review can wait.' },
        { cardId: 'card_now', urgency: 'now', said: 'Atlas has waited 25 minutes for your OK.' },
        { cardId: 'card_later', urgency: 'later', said: 'This can wait.' },
        { cardId: '  ', urgency: 'now', said: 'No card.' }
      ],
      routines: [
        { routineId: 'rt_idle', name: 'Idle sweep' },
        { routineId: 'rt_inbox', name: 'Morning digest', files: 2 },
        { routineId: 'rt_empty', name: 'Quiet pass', files: 0 }
      ],
      memoryWaiting: 1
    })
    expect(items.map((item) => item.key)).toEqual(['approval:a1', 'decision:mission_p', 'scout:card_now', 'routine:rt_inbox', 'routine:rt_empty', 'memory'])
    expect(items.map(needsYouLabel)).toEqual([
      'Wren wants to run: npm test',
      'Pip asked: Which port should the API use?',
      'Scout noticed: Atlas has waited 25 minutes for your OK.',
      'Morning digest: Waiting for you · 2 files in its copy',
      'Quiet pass: Waiting for you',
      '1 memory suggestion waiting'
    ])
    expect(items.map(needsYouPlace)).toEqual([
      { to: 'conversation', missionId: 'mission_w' },
      { to: 'conversation', missionId: 'mission_p' },
      { to: 'home', cardId: 'card_now' },
      { to: 'routines', routineId: 'rt_inbox' },
      { to: 'routines', routineId: 'rt_empty' },
      { to: 'memory' }
    ])
  })

  it('says Scout when the card has no name of its own, and leaves soon and later off the list', () => {
    const items = needsYou({ ...base, scoutCards: [{ cardId: 'card_soon', urgency: 'soon', said: 'Later is fine.' }, { cardId: 'card_now', urgency: 'now', said: 'Checks failed.' }] })
    expect(items.map(needsYouLabel)).toEqual(['Scout noticed: Checks failed.'])
  })
})

describe('the chip', () => {
  it('reads as a sentence', () => {
    expect(needsYouChip(1)).toBe('1 needs you')
    expect(needsYouChip(3)).toBe('3 need you')
    expect(needsYouLabel({ kind: 'memory', key: 'memory', count: 1 })).toBe('1 memory suggestion waiting')
  })
})
