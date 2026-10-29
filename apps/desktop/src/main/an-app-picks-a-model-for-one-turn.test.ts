import { describe, expect, it, vi } from 'vitest'
import type { CodexMissionStartResponse, MissionReadResponse, PublicModel, PublicRecoveredMission, TeammateListResponse } from '../shared/ipc.js'
import { createLocustMcpTools } from './locust-mcp-tools.js'
import { seedAvatar } from '../shared/avatar.js'

/**
 * AN APP PICKS A MODEL FOR ONE TURN (0.842). Colin's Claude Duet asked a teammate to add it: "run this on model X at
 * high effort". Another app may name a model and effort for a turn -- only one the teammate's own AI agent offers in
 * Locust, only an effort that model takes, on the teammate's own account -- and never the mode. The saved route is
 * not changed, and left out, the teammate's own is used.
 */
const teammates: TeammateListResponse = { ok: true, data: { teammates: [
  { teammateId: 'tm_wren', name: 'Wren', role: 'Custom', roleTitle: 'Reviewer', hue: 'lime', avatar: seedAvatar('tm_wren'), createdAt: '2026-10-07', route: { runtime: 'codex', model: 'gpt-6.1-sol', mode: 'auto', effort: 'medium', account: 'work' } },
  { teammateId: 'tm_new', name: 'Newt', role: 'Custom', roleTitle: 'Helper', hue: 'lime', avatar: seedAvatar('tm_new'), createdAt: '2026-10-07' }
], missionOwners: {}, missionTitles: {} } }
const model = (over: Partial<PublicModel>): PublicModel => ({ id: 'x', runtime: 'codex', displayName: 'X', description: '', supportedEfforts: [], ...over }) as PublicModel
const MODELS: readonly PublicModel[] = [
  model({ id: 'gpt-6.1-sol', displayName: 'GPT-6.1 Sol', supportedEfforts: ['low', 'medium', 'high', 'xhigh'] }),
  model({ id: 'gpt-6.1-mini', displayName: 'GPT-6.1 Mini', supportedEfforts: ['low', 'medium'] }),
  model({ id: 'gpt-5.6', displayName: 'GPT-5.6', supportedEfforts: ['high'], older: true }),
  model({ id: 'claude-opus-5-5', runtime: 'claude', displayName: 'Opus 5.5', supportedEfforts: ['high'] }),
  model({ id: 'own_local', displayName: 'My local model', own: true })
]
const mission = { missionId: 'mission_1', startedBy: { kind: 'mcp' }, phase: 'completed', events: [] } as unknown as PublicRecoveredMission
function fixture(models: (() => Promise<readonly PublicModel[]>) | null = async () => MODELS) {
  const start = vi.fn(async (): Promise<CodexMissionStartResponse> => ({ ok: true, data: { missionId: 'mission_1' } } as CodexMissionStartResponse))
  const read = vi.fn(async (): Promise<MissionReadResponse> => ({ ok: true, data: { mission } }))
  const call = createLocustMcpTools({ teammates: async () => teammates, busy: () => false, start, read, newest: async (id) => id, owner: async () => 'tm_wren', live: () => false, background: async () => [], ...(models === null ? {} : { models }) })
  return { call, start }
}
const said = (result: { content: readonly { text: string }[] }): string => result.content[0]!.text

describe('an app picks a model for one turn', () => {
  it('runs that model and effort on the teammate’s own agent and account, in Ask, keeping the saved route', async () => {
    const f = fixture()
    const result = await f.call('start_conversation', { teammate: 'Wren', message: 'Review this.', model: 'GPT-6.1 Mini', effort: 'LOW', mode: 'auto', runtime: 'claude' })
    expect(f.start).toHaveBeenCalledWith({ teammateId: 'tm_wren', prompt: 'Review this.', runtime: 'codex', model: 'gpt-6.1-mini', effort: 'low', account: 'work', routeOverrideFor: 'tm_wren', mode: 'ask', keepSavedRoute: true }, { kind: 'mcp' })
    expect(JSON.parse(said(result))).toEqual({ conversation_id: 'mission_1', mode: 'ask', model: 'gpt-6.1-mini', effort: 'low' })
  })

  it('uses the teammate’s own when the app names neither', async () => {
    const f = fixture()
    await f.call('start_conversation', { teammate: 'Wren', message: 'Review this.' })
    expect(f.start).toHaveBeenCalledWith({ teammateId: 'tm_wren', prompt: 'Review this.', mode: 'ask', keepSavedRoute: true }, { kind: 'mcp' })
  })

  it('takes an effort alone on the saved model, and keeps the saved effort with a model named alone when it fits', async () => {
    const f = fixture()
    await f.call('start_conversation', { teammate: 'Wren', message: 'Think hard.', effort: 'xhigh' })
    expect(f.start).toHaveBeenLastCalledWith(expect.objectContaining({ model: 'gpt-6.1-sol', effort: 'xhigh' }), { kind: 'mcp' })
    await f.call('start_conversation', { teammate: 'Wren', message: 'Quick one.', model: 'gpt-6.1-mini' })
    expect(f.start).toHaveBeenLastCalledWith(expect.objectContaining({ model: 'gpt-6.1-mini', effort: 'medium' }), { kind: 'mcp' })
  })

  it('starts nothing for a model the agent does not offer here, or an effort the model does not take', async () => {
    const f = fixture()
    for (const [args, words] of [
      [{ model: 'claude-opus-5-5' }, 'not one codex offers in this Locust'],
      [{ model: 'gpt-7-imaginary' }, 'not one codex offers in this Locust'],
      [{ model: 'My local model' }, 'not one codex offers in this Locust'],
      [{ model: 'gpt-6.1-mini', effort: 'xhigh' }, 'GPT-6.1 Mini takes these efforts here: low, medium.'],
      [{ model: 'x'.repeat(500) }, 'short text']
    ] as const) {
      const result = await f.call('start_conversation', { teammate: 'Wren', message: 'Go.', ...args })
      expect(result.isError).toBe(true)
      expect(said(result)).toContain(words)
      expect(said(result)).toContain('Nothing was started.')
    }
    expect((await f.call('start_conversation', { teammate: 'Newt', message: 'Go.', model: 'gpt-6.1-mini' })).isError).toBe(true)
    expect(f.start).not.toHaveBeenCalled()
  })

  it('lists the choices for a teammate, current ones first, without retired or own models', async () => {
    const f = fixture()
    expect(JSON.parse(said(await f.call('list_models', { teammate: 'wren' })))).toEqual({
      teammate: 'Wren', runtime: 'codex', model: 'gpt-6.1-sol', effort: 'medium',
      models: [{ id: 'gpt-6.1-sol', name: 'GPT-6.1 Sol', efforts: ['low', 'medium', 'high', 'xhigh'] }, { id: 'gpt-6.1-mini', name: 'GPT-6.1 Mini', efforts: ['low', 'medium'] }]
    })
    expect(f.start).not.toHaveBeenCalled()
  })

  it('is refused by a server that does not offer the choice, such as a voice call’s', async () => {
    const f = fixture(null)
    const result = await f.call('start_conversation', { teammate: 'Wren', message: 'Go.', model: 'gpt-6.1-mini' })
    expect(said(result)).toContain('does not take a model or effort')
    expect(f.start).not.toHaveBeenCalled()
  })

  it('takes the same choice when a conversation goes on', async () => {
    const f = fixture()
    await f.call('send_message', { conversation_id: 'mission_1', message: 'Again, harder.', effort: 'high' })
    expect(f.start).toHaveBeenCalledWith(expect.objectContaining({ followUpOf: 'mission_1', model: 'gpt-6.1-sol', effort: 'high', routeOverrideFor: 'tm_wren', mode: 'ask' }), { kind: 'mcp' })
  })
})
