import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, it, vi } from 'vitest'
import type { MissionReadResponse, PublicModel, PublicRecoveredMission, TeammateListResponse } from '../shared/ipc.js'
import { createLocustMcpTools } from './locust-mcp-tools.js'

/**
 * AN APP CARRIES ON A CONVERSATION THAT RAN OUT (0.842, Colin: "can duet start or look at active chats as well? like if
 * a chat hits limit can she start it up with a new model?"). An app sees the person's own conversations only while
 * "Let apps see your conversations" is on. A turn that stopped on its plan is carried on to another AI agent through
 * Locust's own carry-on -- the card's press -- in the stopped turn's mode, and never above what an app may run.
 */
const teammates: TeammateListResponse = { ok: true, data: { teammates: [
  { teammateId: 'tm_wren', name: 'Wren', role: 'Custom', roleTitle: 'Reviewer', hue: 'lime', avatar: 'x', createdAt: '2026-10-07' }
], missionOwners: {}, missionTitles: { mission_person: 'Fix the drums' } } } as unknown as TeammateListResponse
const ranOut = [{ type: 'route.limit_detected', payload: { kind: 'quota-exhausted' } }]
const turn = (missionId: string, kind: string, over: Record<string, unknown> = {}) => ({ missionId, runId: `run_${missionId}`, startedBy: { kind }, prompt: 'Fix the drums in the mix', runtime: 'claude', model: 'claude-opus-5-5', phase: 'failed', events: ranOut, mode: 'ask', lastUpdatedAt: '2026-10-29T15:00:00.000Z', ...over }) as unknown as PublicRecoveredMission
const model = (id: string, runtime: string, own = false) => ({ id, runtime, displayName: id.toUpperCase(), description: '', supportedEfforts: [], ...(own ? { own } : {}) }) as unknown as PublicModel
const MODELS = [model('claude-opus-5-5', 'claude'), model('gpt-6.1-sol', 'codex'), model('my-local', 'codex', true)]
function fixture(o: { seeAll?: boolean, ownMode?: boolean, turns?: Record<string, PublicRecoveredMission> } = {}) {
  const turns = o.turns ?? { mission_person: turn('mission_person', 'window'), mission_app: turn('mission_app', 'mcp') }
  const carryOn = vi.fn(async () => ({ ok: true as const, missionId: 'mission_next' }))
  const read = vi.fn(async (id: string): Promise<MissionReadResponse> => turns[id] === undefined ? { ok: false, error: { code: 'MISSION_UNAVAILABLE', message: 'no' } } as MissionReadResponse : { ok: true, data: { mission: turns[id]! } })
  const call = createLocustMcpTools({ teammates: async () => teammates, busy: () => false, start: vi.fn(), read, newest: async (id) => id, owner: async () => 'tm_wren', live: () => false, background: async () => [],
    models: async () => MODELS, seeAll: () => o.seeAll ?? false, ownMode: () => o.ownMode ?? false, conversations: async () => Object.values(turns), carryOn })
  return { call, carryOn }
}
const said = (result: { content: readonly { text: string }[] }): string => result.content[0]!.text

describe('an app carries on a conversation that ran out', () => {
  it('lists the person’s conversations only while they let it, saying which ran out', async () => {
    expect(said(await fixture().call('list_conversations', {}))).toMatch(/^Off: .*Let apps see your conversations/)
    const rows = JSON.parse(said(await fixture({ seeAll: true }).call('list_conversations', {})))
    expect(rows[0]).toEqual({ conversation_id: 'mission_person', teammate: 'Wren', title: 'Fix the drums', started_by: 'the person', runtime: 'claude', model: 'claude-opus-5-5',
      state: 'stopped: its plan ran out', can_carry_on: true, updated: '2026-10-29T15:00:00.000Z' })
  })

  it('carries one on to another agent and model through Locust’s own carry-on', async () => {
    const f = fixture({ seeAll: true })
    const result = await f.call('carry_on', { conversation_id: 'mission_person', runtime: 'codex', model: 'GPT-6.1-SOL' })
    expect(f.carryOn).toHaveBeenCalledWith('mission_person', { runtime: 'codex', model: 'gpt-6.1-sol' })
    expect(JSON.parse(said(result))).toEqual({ conversation_id: 'mission_next', runtime: 'codex', model: 'gpt-6.1-sol', mode: 'ask' })
  })

  it('carries on its own conversations with the switch off, and never the person’s', async () => {
    const f = fixture()
    await f.call('carry_on', { conversation_id: 'mission_app', runtime: 'codex' })
    expect(f.carryOn).toHaveBeenCalledWith('mission_app', { runtime: 'codex' })
    expect((await f.call('carry_on', { conversation_id: 'mission_person', runtime: 'codex' })).isError).toBe(true)
    expect((await f.call('read_reply', { conversation_id: 'mission_person' })).isError).toBe(true)
    expect(f.carryOn).toHaveBeenCalledTimes(1)
  })

  it('never carries a turn above Ask unless each teammate’s own mode is on', async () => {
    const turns = { mission_auto: turn('mission_auto', 'window', { mode: 'auto' }) }
    const off = fixture({ seeAll: true, turns })
    expect(said(await off.call('carry_on', { conversation_id: 'mission_auto', runtime: 'codex' }))).toMatch(/ran in auto, not Ask/)
    expect(off.carryOn).not.toHaveBeenCalled()
    const own = fixture({ seeAll: true, ownMode: true, turns })
    await own.call('carry_on', { conversation_id: 'mission_auto', runtime: 'codex' })
    expect(own.carryOn).toHaveBeenCalledWith('mission_auto', { runtime: 'codex' })
  })

  it('starts nothing for an agent or model this Locust cannot run, or an own model', async () => {
    const f = fixture({ seeAll: true })
    expect(said(await f.call('carry_on', { conversation_id: 'mission_person', runtime: 'gemini' }))).toMatch(/claude, codex/)
    expect((await f.call('carry_on', { conversation_id: 'mission_person', runtime: 'codex', model: 'my-local' })).isError).toBe(true)
    expect(f.carryOn).not.toHaveBeenCalled()
    expect(JSON.parse(said(await f.call('list_models', {})))).toEqual([{ runtime: 'claude', models: [{ id: 'claude-opus-5-5', name: 'CLAUDE-OPUS-5-5' }] }, { runtime: 'codex', models: [{ id: 'gpt-6.1-sol', name: 'GPT-6.1-SOL' }] }])
  })

  it('is wired to the person’s switch and Locust’s own carry-on', () => {
    const main = readFileSync(fileURLToPath(new URL('./index.ts', import.meta.url)), 'utf8')
    expect(main).toContain('seeAll: () => mcpHost.conversations(),')
    expect(main).toContain('carryOn: (missionId, route) => carryOn?.carry(missionId, route)')
  })
})
