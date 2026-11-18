import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, it, vi } from 'vitest'
import type { CodexMissionStartResponse, MissionReadResponse, PublicModel, PublicRecoveredMission, TeammateListResponse } from '../shared/ipc.js'
import { createLocustMcpTools } from './locust-mcp-tools.js'
import { mcpText } from './locust-mcp-host.js'
import { seedAvatar } from '../shared/avatar.js'

/**
 * AN APP STARTS A CONVERSATION WITH NO TEAMMATE (0.879). Colin: "I don't understand how you still haven't developed
 * the ability to start conversations without teammates." The window could; the door made every app pick a teammate.
 * Now an app names an AI agent (and, if it likes, a model and effort) as list_models with no teammate lists them:
 * the same Ask, the same switch for mode and folder, nothing saved on any teammate.
 */
const teammates: TeammateListResponse = { ok: true, data: { teammates: [
  { teammateId: 'tm_wren', name: 'Wren', role: 'Custom', roleTitle: 'Reviewer', hue: 'lime', avatar: seedAvatar('tm_wren'), createdAt: '2026-10-07', route: { runtime: 'codex', model: 'gpt-6.1-sol', mode: 'auto', effort: 'medium', account: 'work' } }
], missionOwners: {}, missionTitles: {} } }
const model = (over: Partial<PublicModel>): PublicModel => ({ id: 'x', runtime: 'codex', displayName: 'X', description: '', supportedEfforts: [], ...over }) as PublicModel
const MODELS: readonly PublicModel[] = [
  model({ id: 'gpt-6.1-sol', displayName: 'GPT-6.1 Sol', supportedEfforts: ['low', 'medium', 'high', 'xhigh'] }),
  model({ id: 'claude-opus-5-5', runtime: 'claude', displayName: 'Opus 5.5', supportedEfforts: ['high', 'max'] }),
  model({ id: 'claude-haiku-5-5', runtime: 'claude', displayName: 'Haiku 5.5' }),
  model({ id: 'own_local', runtime: 'opencode', displayName: 'My local model', own: true })
]
const mission = { missionId: 'mission_1', startedBy: { kind: 'mcp' }, phase: 'completed', events: [] } as unknown as PublicRecoveredMission
function fixture(extra: { models?: (() => Promise<readonly PublicModel[]>) | null; ownMode?: () => boolean; appChooses?: boolean } = {}) {
  const start = vi.fn(async (): Promise<CodexMissionStartResponse> => ({ ok: true, data: { missionId: 'mission_1' } } as CodexMissionStartResponse))
  const read = vi.fn(async (): Promise<MissionReadResponse> => ({ ok: true, data: { mission } }))
  const { models = async () => MODELS, ...rest } = extra
  const call = createLocustMcpTools({ teammates: async () => teammates, busy: () => false, start, read, newest: async (id) => id, owner: async () => undefined, live: () => false, background: async () => [], ...(models === null ? {} : { models }), ...rest })
  return { call, start }
}
const said = (result: { content: readonly { text: string }[] }): string => result.content[0]!.text
const read = (path: string): string => readFileSync(fileURLToPath(new URL(path, import.meta.url)), 'utf8')

describe('an app starts a conversation with no teammate', () => {
  it('starts it on the agent it names, in Ask, on the account default model, with no teammate', async () => {
    const f = fixture()
    const result = await f.call('start_conversation', { message: 'Look at the login page.', runtime: 'claude' })
    expect(f.start).toHaveBeenCalledWith({ prompt: 'Look at the login page.', runtime: 'claude', mode: 'ask', keepSavedRoute: true }, { kind: 'mcp' })
    expect(JSON.parse(said(result))).toEqual({ conversation_id: 'mission_1', mode: 'ask' })
  })

  it('takes a model by id or name and an effort that model takes, and says them back', async () => {
    const f = fixture()
    const result = await f.call('start_conversation', { message: 'Think hard.', runtime: 'claude', model: 'Opus 5.5', effort: 'MAX' })
    expect(f.start).toHaveBeenCalledWith({ prompt: 'Think hard.', runtime: 'claude', model: 'claude-opus-5-5', effort: 'max', mode: 'ask', keepSavedRoute: true }, { kind: 'mcp' })
    expect(JSON.parse(said(result))).toEqual({ conversation_id: 'mission_1', mode: 'ask', model: 'claude-opus-5-5', effort: 'max' })
    await f.call('start_conversation', { message: 'Quick.', runtime: 'claude', model: 'claude-haiku-5-5' })
    expect(f.start).toHaveBeenLastCalledWith({ prompt: 'Quick.', runtime: 'claude', model: 'claude-haiku-5-5', mode: 'ask', keepSavedRoute: true }, { kind: 'mcp' })
  })

  it('refuses, in words, an agent this Locust cannot run, a model it does not offer, and an effort it does not take', async () => {
    const f = fixture()
    const agents = 'Name a teammate, or an AI agent this Locust can run, as list_models with no teammate lists them: codex, claude. Nothing was started.'
    expect(await f.call('start_conversation', { message: 'Go.' })).toEqual(mcpText(agents, true))
    expect(await f.call('start_conversation', { message: 'Go.', runtime: 'cursor' })).toEqual(mcpText(agents, true))
    // An agent only the person's own models run on is not one an app may name.
    expect(await f.call('start_conversation', { message: 'Go.', runtime: 'opencode' })).toEqual(mcpText(agents, true))
    expect(await f.call('start_conversation', { message: 'Go.', runtime: 'claude', model: 'gpt-6.1-sol' })).toEqual(mcpText('That model is not one claude offers in this Locust. Ask list_models with no teammate for its choices. Nothing was started.', true))
    expect(await f.call('start_conversation', { message: 'Go.', runtime: 'claude', model: 'claude-opus-5-5', effort: 'low' })).toEqual(mcpText('Opus 5.5 takes these efforts here: high, max. Nothing was started.', true))
    expect(await f.call('start_conversation', { message: 'Go.', runtime: 'claude', effort: 'high' })).toEqual(mcpText('With no teammate, claude runs on its account’s default model, whose efforts Locust cannot name: give a model too, from list_models. Nothing was started.', true))
    expect(await f.call('start_conversation', { message: '', runtime: 'claude' })).toEqual(mcpText('Give a message of up to 8,000 characters. Nothing was started.', true))
    expect(await f.call('start_conversation', { message: 'x'.repeat(8001), runtime: 'claude' })).toEqual(mcpText('Give a message of up to 8,000 characters. Nothing was started.', true))
    expect(f.start).not.toHaveBeenCalled()
  })

  it('needs a teammate on a server that offers no models (a voice call)', async () => {
    const f = fixture({ models: null })
    expect(await f.call('start_conversation', { message: 'Go.', runtime: 'claude' })).toEqual(mcpText('This server starts a conversation only with a teammate. Name one from list_teammates. Nothing was started.', true))
    expect(f.start).not.toHaveBeenCalled()
  })

  it('runs in Ask with the switch on unless the app names a mode, and read_only still steps it down', async () => {
    const f = fixture({ ownMode: () => true, appChooses: true })
    await f.call('start_conversation', { message: 'Go.', runtime: 'claude' })
    expect(f.start).toHaveBeenLastCalledWith({ prompt: 'Go.', runtime: 'claude', mode: 'ask', keepSavedRoute: true }, { kind: 'mcp' })
    expect(JSON.parse(said(await f.call('start_conversation', { message: 'Build it.', runtime: 'claude', mode: 'accept-edits' })))).toEqual({ conversation_id: 'mission_1', mode: 'accept-edits' })
    await f.call('start_conversation', { message: 'Look.', runtime: 'claude', mode: 'auto', read_only: true })
    expect(f.start).toHaveBeenLastCalledWith({ prompt: 'Look.', runtime: 'claude', mode: 'ask', keepSavedRoute: true }, { kind: 'mcp' })
    // With the switch off, a mode is refused in words as for a teammate.
    const off = fixture({ appChooses: true })
    expect((await off.call('start_conversation', { message: 'Go.', runtime: 'claude', mode: 'auto' })).content[0]!.text).toMatch(/Use each teammate's own mode.*Nothing was started\./)
    expect(off.start).not.toHaveBeenCalled()
  })

  it('still asks a named teammate when one is given, ignoring a runtime beside it', async () => {
    const f = fixture()
    await f.call('start_conversation', { teammate: 'Wren', message: 'Review this.', runtime: 'claude' })
    expect(f.start).toHaveBeenCalledWith({ teammateId: 'tm_wren', prompt: 'Review this.', mode: 'ask', keepSavedRoute: true }, { kind: 'mcp' })
  })

  it('both tool lists take runtime, need only a message, and say so in the same words', () => {
    const bridge = read('../../resources/locust-mcp-bridge.mjs')
    const remote = read('./remote-mcp.ts')
    for (const source of [bridge, remote]) {
      const line = source.split('\n').find((one) => one.includes("name: 'start_conversation'"))!
      expect(line).toContain('or start a conversation with no teammate on an AI agent you name as runtime, from list_models with no teammate')
      expect(line).toContain("or with no teammate the account\\'s default model")
      expect(line).toMatch(/runtime: effort, message, .*\['message'\]\) \}/)
    }
  })
})
