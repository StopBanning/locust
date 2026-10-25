import { createElement } from 'react'
import type { ReactElement } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { afterEach, describe, expect, it } from 'vitest'

import { isSavedCall, MAX_CALL_TURNS, type SavedCall, type SavedCallHandOff } from '../../shared/calls.js'
import { seedAvatar } from '../../shared/avatar.js'
import { CallTodosContext, callTodoState, type CallTodoState } from './callTodos.js'
import { CallTodoList, CallWords, type CallPieceShown } from './components/CallWords.js'
import { addCallHandOff, callHandOffs, keptCall, liveCall, setLiveCall, startCallHandOffs, type LiveCall } from './liveCall.js'

/*
 * A CALL SUMS UP WHAT IT HANDED OUT (0.755). Colin passed it on 2026-10-12, from Grok Bot's voice mode, where a summary
 * arrives after the call: every to-do a Talk call started, to whom, how each is going -- read from the same run and
 * record the sidebar reads -- and one press to open each one that is not already on screen. A call with a teammate
 * keeps them, so its summary stays where the call ends in their conversation.
 */
const ada = { teammateId: 'tm_ada', name: 'Ada', hue: 'violet', role: 'Code & Migrations', avatar: seedAvatar('tm_ada'), createdAt: '2026-10-11' } as const
const fix: SavedCallHandOff = { missionId: 'mis_fix', teammateId: 'tm_ada', name: 'Ada', task: 'Fix the flaky login test', at: 2_000 }
const note: SavedCallHandOff = { missionId: 'mis_note', teammateId: 'tm_wren', name: 'Wren', task: 'Draft the release note', at: 3_000 }
const call: SavedCall = {
  callId: 'call_9', teammateId: 'tm_ada', name: 'Ada', startedAt: 1_000, endedAt: 9_000,
  lines: [{ role: 'you', text: 'Fix the login test, and have Wren draft the note.', at: 1_500 }],
  turns: [{ missionId: 'mis_fix', at: 2_000 }],
  handed: [fix, note]
}
const states = (pairs: Record<string, CallTodoState>) => new Map(Object.entries(pairs))
const within = (element: ReactElement, pairs: Record<string, CallTodoState> = {}): string =>
  renderToStaticMarkup(createElement(CallTodosContext.Provider, { value: { states: states(pairs), open: () => undefined, receivedIn: new Map() } }, element))
const piece = (overrides: Partial<CallPieceShown> = {}): CallPieceShown => ({ call, lines: call.lines, first: false, last: true, live: false, ...overrides })

describe('a call’s to-dos', () => {
  afterEach(() => { setLiveCall(undefined); startCallHandOffs() })

  it('each say who has it, what was asked and how it is going, with Open for one not on screen', () => {
    const html = within(createElement(CallTodoList, { handed: [fix, note], label: 'What the call handed out', here: new Set(['mis_fix']) }), { mis_fix: 'done', mis_note: 'needs-you' })
    expect(html).toContain('What the call handed out')
    expect(html).toMatch(/Ada<\/span><span class="lc-calltodo__task" title="Fix the flaky login test">Fix the flaky login test<\/span><span class="lc-status lc-status--green">Done<\/span><\/li>/)
    expect(html).toContain('<span class="lc-status lc-status--amber">Needs you</span>')
    // Ada's turn is in the conversation on screen; Wren's is elsewhere, and opens.
    expect(html.match(/>Open</g)).toHaveLength(1)
    expect(html).toContain('aria-label="Open Wren’s conversation: Draft the release note"')
    expect(html).toContain('data-call-todo="mis_note"')
  })

  it('open nothing while the call is on: opening a conversation would leave the one the call lives in', () => {
    const html = within(createElement(CallTodoList, { handed: [note], label: 'Handed out on this call', openable: false }), { mis_note: 'working' })
    expect(html).toContain('<span class="lc-status lc-status--blue">Working</span>')
    expect(html).not.toContain('>Open<')
  })

  it('read as starting until the window has seen them run', () => {
    expect(within(createElement(CallTodoList, { handed: [note], label: 'x' }))).toContain('<span class="lc-status lc-status--quiet">Starting</span>')
  })
})

describe('the summary where a call with a teammate ends', () => {
  const draw = (shown: CallPieceShown, pairs: Record<string, CallTodoState> = {}): string =>
    within(createElement(CallWords, { piece: shown, teammate: ada, open: false, onToggle: () => undefined, here: new Set(['mis_fix']) }), pairs)

  it('is under the call once it has ended, folded or open', () => {
    const folded = draw(piece(), { mis_fix: 'working', mis_note: 'done' })
    expect(folded).toContain('From the call · 2 to-dos')
    expect(folded).toContain('Fix the flaky login test')
    expect(folded).toContain('<span class="lc-status lc-status--green">Done</span>')
    const open = within(createElement(CallWords, { piece: piece(), teammate: ada, open: true, onToggle: () => undefined, here: new Set(['mis_fix']) }))
    expect(open.indexOf('Call ended')).toBeLessThan(open.indexOf('From the call'))
  })

  it('is not there while the call is on, at a piece that is not its last, or for a call kept before it', () => {
    expect(draw(piece({ live: true }))).not.toContain('From the call')
    expect(draw(piece({ last: false }))).not.toContain('From the call')
    const { handed: _gone, ...before } = call
    expect(draw(piece({ call: before }))).not.toContain('From the call')
  })

  it('sums up: one to-do in another conversation is said, one whose turn is right above is not repeated', () => {
    expect(draw(piece({ call: { ...call, handed: [note] } }))).toContain('From the call · 1 to-do')
    expect(draw(piece({ call: { ...call, handed: [fix] } }))).not.toContain('From the call')
  })
})

describe('how a to-do is going', () => {
  it('follows its run while the window holds it, else the record; waiting on you outranks both', () => {
    expect(callTodoState({ needsYou: false })).toBe('starting')
    expect(callTodoState({ runPhase: 'starting', needsYou: false })).toBe('starting')
    expect(callTodoState({ runPhase: 'running', needsYou: false })).toBe('working')
    expect(callTodoState({ runPhase: 'cancelling', needsYou: false })).toBe('working')
    expect(callTodoState({ runPhase: 'running', needsYou: true })).toBe('needs-you')
    // A finished turn that stopped on a question still needs the person.
    expect(callTodoState({ runPhase: 'completed', needsYou: true })).toBe('needs-you')
    expect(callTodoState({ runPhase: 'completed', needsYou: false })).toBe('done')
    expect(callTodoState({ recordPhase: 'completed', needsYou: false })).toBe('done')
    expect(callTodoState({ runPhase: 'running', recordPhase: 'completed', needsYou: false })).toBe('working')
    expect(callTodoState({ recordPhase: 'failed', needsYou: false })).toBe('failed')
    expect(callTodoState({ recordPhase: 'cancelled', needsYou: false })).toBe('stopped')
    expect(callTodoState({ runPhase: 'interrupted', needsYou: false })).toBe('stopped')
  })
})

describe('the call on now, handing work out', () => {
  afterEach(() => { setLiveCall(undefined); startCallHandOffs() })

  it('lists each turn it starts once, and a call with a teammate keeps them', () => {
    const live: LiveCall = { callId: 'call_live', teammateId: 'tm_ada', name: 'Ada', startedAt: 1_000, lines: [], turns: [], phase: 'live', partial: { you: '', voice: '' } }
    setLiveCall(live)
    addCallHandOff(fix)
    addCallHandOff(fix)
    addCallHandOff(note)
    expect(callHandOffs().map((one) => one.missionId)).toEqual(['mis_fix', 'mis_note'])
    expect(liveCall()?.handed?.map((one) => one.missionId)).toEqual(['mis_fix', 'mis_note'])
    const kept = keptCall(liveCall()!, {}, 9_000)
    expect(kept.handed).toEqual([fix, note])
    expect(isSavedCall(kept)).toBe(true)
  })

  it('leaves out what a kept call could not hold, rather than have the whole call refused when it is saved', () => {
    addCallHandOff({ ...note, missionId: 'not an id!' })
    addCallHandOff({ ...note, missionId: 'mis_2', teammateId: '' })
    addCallHandOff({ ...note, missionId: 'mis_3', task: '   ' })
    addCallHandOff({ ...note, missionId: 'mis_4', name: '' })
    addCallHandOff({ ...note, missionId: 'mis_5', task: 'x'.repeat(900) })
    expect(callHandOffs().map((one) => [one.missionId, one.name, one.task.length])).toEqual([['mis_4', 'Teammate', 'Draft the release note'.length], ['mis_5', 'Wren', 400]])
    for (let index = 0; index < MAX_CALL_TURNS + 5; index += 1) addCallHandOff({ ...note, missionId: `mis_many_${String(index)}` })
    expect(callHandOffs()).toHaveLength(MAX_CALL_TURNS)
  })

  it('begins again with each call, and a call from Home keeps nothing', () => {
    addCallHandOff(note)
    expect(liveCall()).toBeUndefined()
    startCallHandOffs()
    expect(callHandOffs()).toEqual([])
  })

  it('is refused when saved only for a to-do no call could hold', () => {
    expect(isSavedCall({ ...call, handed: [{ ...note, task: '' }] })).toBe(false)
    expect(isSavedCall({ ...call, handed: [{ ...note, name: 'x'.repeat(81) }] })).toBe(false)
    expect(isSavedCall({ ...call, handed: 'all of them' })).toBe(false)
    expect(isSavedCall({ ...call, handed: Array.from({ length: MAX_CALL_TURNS + 1 }, (_, index) => ({ ...note, missionId: `mis_${String(index)}` })) })).toBe(false)
    const { handed: _gone, ...before } = call
    expect(isSavedCall(before)).toBe(true)
  })
})
