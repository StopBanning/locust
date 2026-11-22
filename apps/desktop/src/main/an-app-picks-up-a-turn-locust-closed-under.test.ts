import { describe, expect, it, vi } from 'vitest'
import type { MissionReadResponse, PublicModel, PublicRecoveredMission, TeammateListResponse } from '../shared/ipc.js'
import { cutOffMidTurn, cutOffPrompt } from '../shared/carry-on.js'
import { createCarryOn } from './carry-on.js'
import { createLocustMcpTools } from './locust-mcp-tools.js'

/**
 * AN APP PICKS UP A TURN LOCUST CLOSED UNDER, AND MOVES A FINISHED ONE (0.882).
 *
 * Found 22 Nov: Locust quit at 00:14 with five turns working. Each read "interrupted" afterwards, carry_on refused them
 * (none stopped on a plan or a provider), and the app could only find out by reading the ledger files. A turn that
 * started, recorded no ending and is not running now is said to have been cut off, and carry_on picks it up on its own
 * agent or another, told to reconcile what it cannot confirm. A finished turn moves to another agent with a message for
 * its next turn. The window's own card is unchanged: it never asks for either.
 */
const teammates: TeammateListResponse = { ok: true, data: { teammates: [], missionOwners: {}, missionTitles: {} } } as unknown as TeammateListResponse
const started = { type: 'run.started', payload: {} }
const model = (id: string, runtime: string) => ({ id, runtime, displayName: id, description: '', supportedEfforts: [] }) as unknown as PublicModel
const turn = (phase: string, events: readonly unknown[]) => ({ missionId: 'mission_lane', runId: 'run_lane', startedBy: { kind: 'mcp', folder: '/work/lane' }, prompt: 'Port the rows', runtime: 'codex', model: 'gpt-6.2-sol', mode: 'ask', phase, events, lastUpdatedAt: '2026-11-22T05:14:00.000Z' }) as unknown as PublicRecoveredMission
function door(mission: PublicRecoveredMission, live = false) {
  const carryOn = vi.fn(async () => ({ ok: true as const, missionId: 'mission_next' }))
  const read = vi.fn(async (): Promise<MissionReadResponse> => ({ ok: true, data: { mission } }))
  const call = createLocustMcpTools({ teammates: async () => teammates, busy: () => false, start: vi.fn(), read, newest: async (id) => id, owner: async () => undefined, live: () => live, background: async () => [],
    models: async () => [model('gpt-6.2-sol', 'codex'), model('opus', 'claude')], seeAll: () => true, conversations: async () => [mission], carryOn })
  return { call, carryOn }
}
const said = (result: { content: readonly { text: string }[] }): string => result.content[0]!.text
const host = (phase: string, events: readonly unknown[]) => {
  const start = vi.fn(async () => ({ ok: true as const, missionId: 'mission_next' }))
  const { carry } = createCarryOn({ ledger: { getMission: async () => ({ phase, events, metadata: { runtime: 'codex', mode: 'ask', resolvedRouteId: 'codex-account:default' } }) as never }, start, setting: async () => undefined, sentByPerson: () => false, note: () => undefined })
  return { carry, start }
}

describe('an app picks up a turn Locust closed under', () => {
  it('reads a turn that started and never ended as cut off, and never a finished, failed or unstarted one', () => {
    expect(cutOffMidTurn({ phase: 'interrupted', events: [started, { type: 'message.delta' }] })).toBe(true)
    expect(cutOffMidTurn({ phase: 'interrupted', events: [] })).toBe(false)
    expect(cutOffMidTurn({ phase: 'interrupted', events: [started, { type: 'run.cancelled' }] })).toBe(false)
    expect(cutOffMidTurn({ phase: 'failed', events: [started] })).toBe(false)
    expect(cutOffMidTurn({ phase: 'completed', events: [started, { type: 'run.completed' }] })).toBe(false)
  })

  it('lists it as stopped when Locust closed, offers carry_on, and says so in read_reply', async () => {
    const f = door(turn('interrupted', [started]))
    const [row] = JSON.parse(said(await f.call('list_conversations', {})))
    expect(row).toMatchObject({ state: 'stopped: Locust closed while it was working', can_carry_on: true })
    const reply = await f.call('read_reply', { conversation_id: 'mission_lane' }) as { content: readonly { text: string }[]; structuredContent?: Record<string, unknown> }
    expect(said(reply)).toMatch(/^Locust closed while this turn was working/)
    expect(reply.structuredContent).toMatchObject({ can_carry_on: true })
    // Still running, it is working, not cut off.
    const live = door(turn('interrupted', [started]), true)
    expect(JSON.parse(said(await live.call('list_conversations', {})))[0]).toMatchObject({ state: 'working' })
    expect(JSON.parse(said(await live.call('list_conversations', {})))[0].can_carry_on).toBeUndefined()
  })

  it('carries it on through Locust’s carry-on, saying it was cut off', async () => {
    const f = door(turn('interrupted', [started]))
    await f.call('carry_on', { conversation_id: 'mission_lane', runtime: 'claude', model: 'opus' })
    expect(f.carryOn).toHaveBeenCalledWith('mission_lane', { runtime: 'claude', model: 'opus' }, { cutOff: true })
  })

  it('picks it up on its own agent too, told to reconcile what it cannot confirm', async () => {
    const h = host('interrupted', [started])
    expect(await h.carry('mission_lane', { runtime: 'codex' }, { cutOff: true })).toMatchObject({ ok: true })
    expect(h.start).toHaveBeenCalledWith(expect.objectContaining({ prompt: cutOffPrompt('codex'), runtime: 'codex', followUpOf: 'mission_lane' }))
    expect(cutOffPrompt('codex')).toMatch(/Locust closed while it was working.*never retry an irreversible action/)
  })

  it('leaves the window’s card as it was: without the app’s word a cut-off turn is not carried on', async () => {
    const h = host('interrupted', [started])
    expect(await h.carry('mission_lane', { runtime: 'claude' })).toMatchObject({ ok: false })
    expect(h.start).not.toHaveBeenCalled()
  })
})

describe('an app moves a finished conversation to another agent', () => {
  it('sends the message as the next turn on the other agent, with what was done before it', async () => {
    const h = host('completed', [started, { type: 'run.completed' }])
    expect(await h.carry('mission_lane', { runtime: 'claude' }, { message: 'Review what Codex wrote.' })).toMatchObject({ ok: true })
    expect(h.start).toHaveBeenCalledWith(expect.objectContaining({ prompt: 'Review what Codex wrote.', runtime: 'claude', followUpOf: 'mission_lane', from: 'codex' }))
  })

  it('needs the message, and sends one on its own agent the plain way', async () => {
    const without = host('completed', [started, { type: 'run.completed' }])
    expect(await without.carry('mission_lane', { runtime: 'claude' })).toEqual({ ok: false, message: expect.stringMatching(/finished.*give a message/) })
    const same = host('completed', [started, { type: 'run.completed' }])
    expect(await same.carry('mission_lane', { runtime: 'codex' }, { message: 'More.' })).toEqual({ ok: false, message: expect.stringMatching(/already runs this conversation/) })
    expect(without.start).not.toHaveBeenCalled()
    expect(same.start).not.toHaveBeenCalled()
  })

  it('passes the message through the door, and puts it after the carry-on words of a turn that ran out', async () => {
    const f = door(turn('completed', [started, { type: 'run.completed' }]))
    await f.call('carry_on', { conversation_id: 'mission_lane', runtime: 'claude', message: 'Review it.' })
    expect(f.carryOn).toHaveBeenCalledWith('mission_lane', { runtime: 'claude' }, { message: 'Review it.' })
    const h = host('failed', [started, { type: 'route.limit_detected', payload: { kind: 'quota-exhausted' } }])
    await h.carry('mission_lane', { runtime: 'claude' }, { message: 'Then push it.' })
    expect((h.start.mock.calls[0] as unknown as [{ prompt: string }])[0].prompt).toMatch(/ran out of its plan's usage[\s\S]*\n\nThen push it\.$/)
  })
})
