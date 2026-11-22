import { describe, expect, it, vi } from 'vitest'
import type { MissionReadResponse, PublicModel, PublicRecoveredMission, TeammateListResponse, TeammateRoute } from '../shared/ipc.js'
import { createLocustMcpTools } from './locust-mcp-tools.js'

/**
 * AN APP READS USAGE, SETS A TEAMMATE'S MODEL, AND NAMES A CONVERSATION BY A SHORT ID (0.882; Colin, 22 Nov, of the
 * MCP list: "I don't see why we can't do all of those").
 *
 * Usage: the window's own readings, so an app driving several turns moves work before an account runs out (22 Nov:
 * five lanes hit Codex's limit at once, found by reading the ledger). A teammate's model: only with both of the
 * person's switches on, only a model its agent offers, its agent, account and mode kept. A short id: eight or more
 * characters of one list_conversations gave, matching exactly one conversation.
 */
const route: TeammateRoute = { runtime: 'codex', model: 'gpt-6.1-sol', mode: 'auto', effort: 'high', account: 'work' } as TeammateRoute
const teammates = { ok: true, data: { teammates: [{ teammateId: 'tm_codec', name: 'Codec', role: 'Custom', route }], missionOwners: {}, missionTitles: {} } } as unknown as TeammateListResponse
const model = (id: string, efforts: string[] = []) => ({ id, runtime: 'codex', displayName: id.toUpperCase(), description: '', supportedEfforts: efforts }) as unknown as PublicModel
const turn = (missionId: string) => ({ missionId, runId: `run_${missionId}`, startedBy: { kind: 'mcp' }, runtime: 'codex', phase: 'completed', events: [], lastUpdatedAt: '2026-11-22T12:00:00.000Z' }) as unknown as PublicRecoveredMission
const TURNS: Record<string, PublicRecoveredMission> = { 'mission_7a173829-9d97-48a1-92ba-708f65d0421d': turn('mission_7a173829-9d97-48a1-92ba-708f65d0421d'), 'mission_7a17ffff-0000-4000-8000-000000000000': turn('mission_7a17ffff-0000-4000-8000-000000000000') }
function door(o: { seeAll?: boolean; ownMode?: boolean; refuse?: string } = {}) {
  const saveModel = vi.fn(async (_id: string, _route: TeammateRoute) => o.refuse)
  const usage = vi.fn(async () => ({ windows: { codex: '5-hour window 78% used · resets 09:56 · from a run at 2026-11-22T05:13:36.175Z' }, atLimit: { 'codex:work': 'Codex is at its limit until 09:56.' } }))
  const read = vi.fn(async (id: string): Promise<MissionReadResponse> => TURNS[id] === undefined ? { ok: false, error: { code: 'MISSION_UNAVAILABLE', message: 'no' } } as MissionReadResponse : { ok: true, data: { mission: TURNS[id]! } })
  const call = createLocustMcpTools({ teammates: async () => teammates, busy: () => false, start: vi.fn(), read, newest: async (id) => id, owner: async () => undefined, live: () => false, background: async () => [],
    models: async () => [model('gpt-6.1-sol', ['high', 'max']), model('gpt-6.2-sol', ['high', 'ultra']), model('gpt-6-luna', ['low'])], seeAll: () => o.seeAll ?? true, ownMode: () => o.ownMode ?? true,
    conversations: async () => Object.values(TURNS), usage, saveModel })
  return { call, saveModel }
}
const said = (result: { content: readonly { text: string }[] }): string => result.content[0]!.text

describe('an app reads usage', () => {
  it('gives the window’s own readings, dated by their run, and the agents at their limit', async () => {
    expect(JSON.parse(said(await door().call('read_usage', {})))).toEqual({
      usage: { codex: '5-hour window 78% used · resets 09:56 · from a run at 2026-11-22T05:13:36.175Z' }, at_limit: { 'codex:work': 'Codex is at its limit until 09:56.' }
    })
  })
})

describe('an app sets a teammate’s model', () => {
  it('saves a model its agent offers, keeping its agent, account, mode and an effort the new model takes', async () => {
    const f = door()
    expect(JSON.parse(said(await f.call('set_teammate_model', { teammate: 'codec', model: 'GPT-6.2-SOL' })))).toEqual({ teammate: 'Codec', runtime: 'codex', model: 'gpt-6.2-sol', effort: 'high' })
    expect(f.saveModel).toHaveBeenCalledWith('tm_codec', { runtime: 'codex', model: 'gpt-6.2-sol', mode: 'auto', effort: 'high', account: 'work' })
    // An effort the new model does not take is dropped, and one asked for must be offered.
    await f.call('set_teammate_model', { teammate: 'tm_codec', model: 'gpt-6-luna' })
    expect(f.saveModel).toHaveBeenLastCalledWith('tm_codec', { runtime: 'codex', model: 'gpt-6-luna', mode: 'auto', account: 'work' })
    expect(said(await f.call('set_teammate_model', { teammate: 'Codec', model: 'gpt-6.2-sol', effort: 'low' }))).toMatch(/takes these efforts here: high, ultra\. Nothing was changed\.$/)
    expect(f.saveModel).toHaveBeenCalledTimes(2)
  })

  it('changes nothing without both of the person’s switches, a known teammate and an offered model', async () => {
    for (const off of [{ seeAll: false }, { ownMode: false }]) {
      const f = door(off)
      const result = await f.call('set_teammate_model', { teammate: 'Codec', model: 'gpt-6.2-sol' })
      expect(result.isError).toBe(true)
      expect(said(result)).toMatch(/^Off: .*Let apps see your conversations.*Use each teammate's own mode/)
      expect(f.saveModel).not.toHaveBeenCalled()
    }
    const f = door()
    expect(said(await f.call('set_teammate_model', { teammate: 'Nobody', model: 'gpt-6.2-sol' }))).toMatch(/^No teammate/)
    expect(said(await f.call('set_teammate_model', { teammate: 'Codec', model: 'claude-opus-5-5' }))).toMatch(/not one codex offers/)
    expect(f.saveModel).not.toHaveBeenCalled()
  })

  it('says the host’s refusal, from the organization’s policy, as it is', async () => {
    const result = await door({ refuse: 'Your organization does not allow gpt-6.2-sol.' }).call('set_teammate_model', { teammate: 'Codec', model: 'gpt-6.2-sol' })
    expect(result.isError).toBe(true)
    expect(said(result)).toBe('Your organization does not allow gpt-6.2-sol. Nothing was changed.')
  })
})

describe('an app names a conversation by a short id', () => {
  it('reads one by eight or more of its characters, with or without mission_', async () => {
    const f = door()
    expect((await f.call('read_reply', { conversation_id: '7a173829' })).isError).not.toBe(true)
    expect((await f.call('read_reply', { conversation_id: 'mission_7a173829-9d97' })).isError).not.toBe(true)
  })

  it('reads none when the start matches two, is shorter than eight, or is not an id', async () => {
    const f = door()
    for (const asked of ['7a17', '7a17ffff-00'.slice(0, 7), 'mission_7a17', 'zzzzzzzzzz']) expect((await f.call('read_reply', { conversation_id: asked })).isError).toBe(true)
    // Two share "7a17"; eight characters that only one starts with name that one.
    expect((await f.call('read_reply', { conversation_id: '7a17' + 'f'.repeat(4) })).isError).not.toBe(true)
  })
})
