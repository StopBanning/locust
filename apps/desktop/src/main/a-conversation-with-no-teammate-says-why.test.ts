import { describe, expect, it, vi } from 'vitest'
import { createLocustMcpTools, NO_TEAMMATE } from './locust-mcp-tools.js'
import type { PublicRecoveredMission, CodexMissionStartResponse } from '../shared/ipc.js'

/**
 * A CONVERSATION WITH NO TEAMMATE SAYS WHY AN APP CANNOT CONTINUE IT (0.877.2).
 *
 * An app works as a teammate, in the mode the person set for it, so it cannot continue a conversation that has none.
 * It said "That conversation no longer has a teammate", and Colin, 16 Nov, went looking for the teammate that had gone:
 * the conversation was one he had started without a teammate, which never had one.
 */
function tools(owner: string | undefined) {
  const mission = { missionId: 'tip', runtime: 'claude', model: 'opus', phase: 'completed', events: [] } as unknown as PublicRecoveredMission
  const start = vi.fn(async () => ({ ok: true, data: { missionId: 'next', runtime: 'claude', model: 'opus', mode: 'ask' } }) as CodexMissionStartResponse)
  const call = createLocustMcpTools({
    teammates: async () => ({ ok: true, data: { teammates: [], missionOwners: {}, missionTitles: {} } }),
    start, busy: () => false, read: async () => ({ ok: true, data: { mission } }), newest: async () => 'tip',
    owner: async () => owner, live: () => false, waiting: () => false, background: async () => [], seeAll: () => true,
    folder: () => '/work', turnFolder: async () => '/work', followUpRoute: async () => undefined
  } as never)
  return { call, start }
}

describe('a conversation with no teammate', () => {
  it('says it was started without one or lost it, and to continue it in Locust, and sends nothing', async () => {
    const f = tools(undefined)
    const result = await f.call('send_message', { conversation_id: 'tip', message: 'Carry on.' })
    expect(result.isError).toBe(true)
    expect(result.content[0]!.text).toBe(NO_TEAMMATE)
    expect(NO_TEAMMATE).toContain('started without one, or its teammate was removed')
    expect(NO_TEAMMATE).toContain("continue this one in Locust's window")
    expect(NO_TEAMMATE).not.toContain('no longer')
    expect(f.start).not.toHaveBeenCalled()
  })

  it('still continues one that has a teammate', async () => {
    const f = tools('mate')
    await f.call('send_message', { conversation_id: 'tip', message: 'Carry on.' })
    expect(f.start).toHaveBeenCalled()
  })
})
