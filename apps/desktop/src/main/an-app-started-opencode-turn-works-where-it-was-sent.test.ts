import { describe, expect, it, vi } from 'vitest'
import type { CodexMissionStartRequest, CodexMissionStartResponse, MissionReadResponse, PublicRecoveredMission, TeammateListResponse } from '../shared/ipc.js'
import type { MissionStarter } from '@teammate/mission-store'
import { createLocustMcpTools } from './locust-mcp-tools.js'
import { asksScout } from './asking-scout.js'
import { createCarryOn, type CarryOnStart } from './carry-on.js'
import { carryOnPrompt } from '../shared/carry-on.js'
import { conversationFolder } from './conversation-folder.js'
import { seedAvatar } from '../shared/avatar.js'

/*
 * Found 9 Nov 2026: the Mac chat handed a review to Scout -- an OpenCode teammate that is also the one Scout's looks
 * run on -- through the MCP door with `mode: "auto"` and a folder. The turn ran sealed in Ask, every tool refused, and
 * the door still replied `{"mode":"auto"}`. Codex and Antigravity teammates honoured the same call.
 */
const SCOUT = 'tm_scout'
const teammates: TeammateListResponse = { ok: true, data: { teammates: [{ teammateId: SCOUT, name: 'Scout', role: 'Custom', hue: 'coral', avatar: seedAvatar(SCOUT), createdAt: '2026-11-01', route: { runtime: 'opencode', model: 'opencode/deepseek-v4.1-flash-free', mode: 'ask' } }], missionOwners: {}, missionTitles: {} } }

function door(ran: (request: CodexMissionStartRequest, origin: MissionStarter) => CodexMissionStartRequest['mode']) {
  const start = vi.fn(async (request: CodexMissionStartRequest, origin: MissionStarter): Promise<CodexMissionStartResponse> => ({ ok: true, data: { missionId: 'mission_1', mode: ran(request, origin) } } as CodexMissionStartResponse))
  const read = vi.fn(async (): Promise<MissionReadResponse> => ({ ok: true, data: { mission: { missionId: 'mission_1', startedBy: { kind: 'mcp' }, phase: 'completed', events: [] } as unknown as PublicRecoveredMission } }))
  const call = createLocustMcpTools({
    teammates: vi.fn(async () => teammates), busy: vi.fn(() => false), start, read, newest: vi.fn(async (id: string) => id), owner: vi.fn(async () => SCOUT), live: vi.fn(() => false), background: vi.fn(async () => []),
    ownMode: () => true, appChooses: true, checkFolder: async (path: string) => ({ path })
  })
  return { call, start }
}

describe('an app-started OpenCode turn works where it was sent', () => {
  it("is not taken for a question to Scout, even on Scout's own teammate", () => {
    // The person's own turn and a voice call still ask Scout what it saw, sealed.
    expect(asksScout(SCOUT, SCOUT)).toBe(true)
    expect(asksScout(SCOUT, SCOUT, { kind: 'mcp', via: 'voice' })).toBe(true)
    // An app's own work, with or without a folder named, runs as any teammate's turn does.
    expect(asksScout(SCOUT, SCOUT, { kind: 'mcp' })).toBe(false)
    expect(asksScout(SCOUT, SCOUT, { kind: 'mcp', folder: '/work/review' })).toBe(false)
    // Another teammate, or no Scout set up, is never a question to Scout.
    expect(asksScout('tm_wren', SCOUT)).toBe(false)
    expect(asksScout(SCOUT, undefined)).toBe(false)
  })

  it('hands the host the mode and folder the app named, and replies with them when they ran', async () => {
    const f = door(request => request.mode)
    const reply = await f.call('start_conversation', { teammate: 'Scout', message: 'Review M1.', mode: 'auto', folder: '/work/review' })
    expect(f.start).toHaveBeenCalledWith(expect.objectContaining({ teammateId: SCOUT, mode: 'auto' }), { kind: 'mcp', folder: '/work/review' })
    expect(JSON.parse(reply.content[0]!.text)).toEqual({ conversation_id: 'mission_1', mode: 'auto', folder: '/work/review' })
  })

  it('replies with the mode the host really ran, and names the one asked for, when they differ', async () => {
    const f = door(() => 'ask')
    const reply = await f.call('start_conversation', { teammate: 'Scout', message: 'Review M1.', mode: 'auto', folder: '/work/review' })
    expect(reply.isError).not.toBe(true)
    expect(JSON.parse(reply.content[0]!.text)).toEqual({ conversation_id: 'mission_1', mode: 'ask', mode_asked: 'auto', folder: '/work/review' })
  })

  /*
   * Found the same day: `carry_on` on an app-started conversation in native-review/m1 started the carried turn (Copilot)
   * in the teammate's own folder, now-coding, and the next send_message replied `"folder":"now-coding"`.
   */
  describe('carried on', () => {
    const FOLDER = '/work/review'
    const quota = { type: 'run.failed', sourceAdapter: 'antigravity', payload: { kind: 'quota-exhausted', message: 'Out of quota.' } }
    const stopped = (startedBy: unknown) => ({ phase: 'failed', events: [quota], metadata: { missionId: 'mission_1', runId: 'run_1', runtime: 'antigravity', sandbox: 'workspace-write', mode: 'auto', startedBy } })
    const carry = (startedBy: unknown) => {
      const started: CarryOnStart[] = []
      const carryOn = createCarryOn({ ledger: { getMission: async () => stopped(startedBy) as never }, setting: async () => undefined, sentByPerson: () => false, note: () => undefined,
        start: async (asked) => { started.push(asked); return { ok: true, missionId: 'mission_2', ...(asked.folder === undefined ? {} : { folder: asked.folder }) } } })
      return { carryOn, started }
    }

    it("stays in the folder an app named for the conversation, not the teammate's own", async () => {
      const app = carry({ kind: 'mcp', folder: FOLDER })
      expect(await app.carryOn.carry('mission_1', { runtime: 'copilot', model: 'auto' })).toEqual({ ok: true, missionId: 'mission_2', folder: FOLDER })
      expect(app.started).toEqual([{ prompt: carryOnPrompt('antigravity', 'plan'), runtime: 'copilot', model: 'auto', mode: 'auto', followUpOf: 'mission_1', from: 'antigravity', folder: FOLDER }])
      // A carried turn carried on again keeps it: the carried turn records the folder too.
      const again = carry({ kind: 'carry-on', from: 'codex', folder: FOLDER })
      await again.carryOn.carry('mission_1', { runtime: 'claude' })
      expect(again.started[0]!.folder).toBe(FOLDER)
      // A conversation nobody sent to a folder stands where it always did.
      for (const startedBy of [undefined, { kind: 'mcp' }, { kind: 'routine', routineId: 'r', step: 1 }]) {
        const plain = carry(startedBy)
        await plain.carryOn.carry('mission_1', { runtime: 'claude' })
        expect(plain.started[0]).not.toHaveProperty('folder')
      }
    })

    it('reads the folder from an app turn or a carried turn, and from nothing else', () => {
      expect(conversationFolder({ kind: 'mcp', folder: FOLDER })).toBe(FOLDER)
      expect(conversationFolder({ kind: 'carry-on', folder: FOLDER })).toBe(FOLDER)
      expect(conversationFolder({ kind: 'mcp' })).toBeUndefined()
      expect(conversationFolder({ kind: 'window', folder: FOLDER })).toBeUndefined()
      expect(conversationFolder(undefined)).toBeUndefined()
    })

    it("the door's carry_on names the folder, and the app's next message stays there", async () => {
      const f = door(request => request.mode)
      const carryOn = vi.fn(async () => ({ ok: true as const, missionId: 'mission_2', folder: FOLDER }))
      const ranOut = { missionId: 'mission_1', runId: 'run_1', startedBy: { kind: 'mcp', folder: FOLDER }, runtime: 'antigravity', phase: 'failed', mode: 'auto', events: [{ type: 'route.limit_detected', payload: { kind: 'quota-exhausted' } }] } as unknown as PublicRecoveredMission
      const carried = { ...ranOut, missionId: 'mission_2', runId: 'run_2', runtime: 'copilot', phase: 'completed', events: [], startedBy: { kind: 'carry-on', from: 'antigravity', folder: FOLDER } } as unknown as PublicRecoveredMission
      const turns: Record<string, PublicRecoveredMission> = { mission_1: ranOut, mission_2: carried }
      const checkFolder = vi.fn(async (path: string) => ({ path }))
      const call = createLocustMcpTools({
        teammates: vi.fn(async () => teammates), busy: vi.fn(() => false), start: f.start, owner: vi.fn(async () => SCOUT), live: vi.fn(() => false), background: vi.fn(async () => []),
        read: vi.fn(async (id: string): Promise<MissionReadResponse> => ({ ok: true, data: { mission: turns[id]! } })), newest: vi.fn(async () => 'mission_2'),
        ownMode: () => true, appChooses: true, checkFolder, seeAll: () => true, carryOn,
        models: async () => [{ id: 'auto', runtime: 'copilot', displayName: 'Auto', description: '', supportedEfforts: [] }] as never
      })
      const reply = await call('carry_on', { conversation_id: 'mission_1', runtime: 'copilot', model: 'auto' })
      expect(JSON.parse(reply.content[0]!.text)).toMatchObject({ conversation_id: 'mission_2', folder: FOLDER })
      await call('send_message', { conversation_id: 'mission_2', message: 'Finish the review.' })
      expect(f.start).toHaveBeenLastCalledWith(expect.objectContaining({ followUpOf: 'mission_2' }), { kind: 'mcp', folder: FOLDER })
      expect(checkFolder).toHaveBeenLastCalledWith(FOLDER)
    })
  })
})
