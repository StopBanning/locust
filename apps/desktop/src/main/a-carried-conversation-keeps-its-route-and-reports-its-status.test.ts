import { describe, expect, it, vi } from 'vitest'
import { createLocustMcpTools } from './locust-mcp-tools.js'
import { carryOnPrompt } from '../shared/carry-on.js'
import { stoppedFor, createCarryOn } from './carry-on.js'
import type { PublicRecoveredMission, CodexMissionStartResponse } from '../shared/ipc.js'

const crash = { type: 'run.failed', payload: { kind: 'process-failed', message: 'OpenCode ended without a step that reported it had stopped.' } }
function fixture(extra: Partial<Parameters<typeof createLocustMcpTools>[0]> = {}) {
  let live = false
  let waiting = false
  const mission = { missionId: 'tip', runtime: 'cursor', model: 'grok', phase: 'failed', startedBy: { kind: 'carry-on', from: 'antigravity' }, events: [crash] } as unknown as PublicRecoveredMission
  const start = vi.fn(async () => ({ ok: true, data: { missionId: 'next', runtime: 'cursor', model: 'grok', mode: 'ask' } }) as CodexMissionStartResponse)
  const call = createLocustMcpTools({
    teammates: async () => ({ ok: true, data: { teammates: [], missionOwners: {}, missionTitles: {} } }),
    start, busy: () => false, read: async () => ({ ok: true, data: { mission } }), newest: async () => 'tip',
    owner: async () => 'mate', live: () => live, waiting: () => waiting, background: async () => [], seeAll: () => true,
    folder: () => '/wrong', turnFolder: async () => '/work/conversation',
    followUpRoute: async () => ({ runtime: 'cursor', model: 'grok', effort: 'high', account: 'work' }), ...extra
  })
  return { call, start, mission, live: (value: boolean) => { live = value }, waiting: (value: boolean) => { waiting = value } }
}
describe('a carried conversation keeps its route and reports its status', () => {
  it('continues on the carried runtime and model with its account and effort, and reports the actual folder', async () => {
    const f = fixture()
    const result = await f.call('send_message', { conversation_id: 'root', message: 'Continue.' })
    expect(f.start).toHaveBeenCalledWith(expect.objectContaining({ runtime: 'cursor', model: 'grok', effort: 'high', account: 'work', routeOverrideFor: 'mate', followUpOf: 'tip', keepSavedRoute: true }), { kind: 'mcp' })
    expect(JSON.parse(result.content[0]!.text)).toMatchObject({ runtime: 'cursor', model: 'grok', folder: '/work/conversation' })
  })
  it('lists a teammate’s own full folder and the open full folder for one without its own', async () => {
    const f = fixture({ teammates: async () => ({ ok: true, data: { teammates: [{ teammateId: 'own', folder: '/work/teammate' }, { teammateId: 'default' }], missionOwners: {}, missionTitles: {} } }) as never, folder: () => '/work/open' })
    const result = await f.call('list_teammates', {})
    expect(JSON.parse(result.content[0]!.text).map((row: { folder: string }) => row.folder)).toEqual(['/work/teammate', '/work/open'])
  })
  it('keeps readable text beside machine status for working, approval, done, failed and stopped turns', async () => {
    const f = fixture()
    for (const [phase, live, waiting, status, carry] of [
      ['failed', true, false, 'working', false], ['failed', true, true, 'waiting_for_person', false],
      ['completed', false, false, 'done', false], ['failed', false, false, 'failed', true], ['cancelled', false, false, 'stopped', false]
    ] as const) {
      Object.assign(f.mission, { phase }); f.live(live); f.waiting(waiting)
      const result = await f.call('read_reply', { conversation_id: 'root' })
      expect(result.content[0]!.text.length).toBeGreaterThan(0)
      expect(result.structuredContent).toEqual({ status, runtime: 'cursor', model: 'grok', folder: '/work/conversation', can_carry_on: carry })
    }
  })
  it('offers a process crash after tools ran through the same card and door reason, without replaying uncertain actions', async () => {
    const events = [{ type: 'tool.started', payload: {} }, crash] as never
    expect(stoppedFor({ phase: 'failed', events })).toBe('process')
    expect(stoppedFor({ phase: 'failed', events: [{ type: 'run.failed', payload: { kind: 'process-failed', message: 'connection refused' } }] as never })).toBe('provider')
    expect(stoppedFor({ phase: 'failed', events: [{ type: 'run.failed', payload: { kind: 'protocol-mismatch', message: 'Invalid runtime record' } }] as never })).toBe('process')
    const start = vi.fn(async () => ({ ok: true as const, missionId: 'next' }))
    const carry = createCarryOn({ ledger: { getMission: async () => ({ phase: 'failed', events, metadata: { runtime: 'opencode', mode: 'ask' } }) as never }, start, setting: async () => undefined, sentByPerson: () => false, note: () => undefined })
    expect(await carry.carry('tip', { runtime: 'cursor' })).toEqual({ ok: true, missionId: 'next' })
    expect(start).toHaveBeenCalledWith(expect.objectContaining({ prompt: carryOnPrompt('opencode', 'process') }))
    expect(carryOnPrompt('opencode', 'process')).toContain('never retry an irreversible action')
    for (const phase of ['completed', 'cancelled'] as const) expect(stoppedFor({ phase, events })).toBeUndefined()
    for (const [kind, message] of [['safety-blocked', 'refused'], ['process-failed', 'The mode does not allow shell, so OpenCode stopped when it tried to use it.']]) {
      expect(stoppedFor({ phase: 'failed', events: [{ type: 'route.limit_detected', payload: { kind: 'quota-exhausted' } }, { type: 'run.failed', payload: { kind, message } }] as never })).toBeUndefined()
    }
  })
})
