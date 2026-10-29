import { describe, expect, it, vi } from 'vitest'
import type { MissionReadResponse, PublicRecoveredMission, TeammateListResponse } from '../shared/ipc.js'
import { createLocustMcpTools, MCP_WAIT_LIMIT_S } from './locust-mcp-tools.js'

/**
 * AN APP WAITS FOR THE ANSWER (0.842). Claude on the web reads a reply again and again while a turn works, and every
 * read is one more Allow for the person. read_reply may wait for the turn to end, up to MCP_WAIT_LIMIT_S, and stops
 * waiting the moment a card comes up in Locust: the app cannot answer it.
 */
const teammates: TeammateListResponse = { ok: true, data: { teammates: [], missionOwners: {}, missionTitles: {} } }
const working = { missionId: 'mission_1', startedBy: { kind: 'mcp' }, phase: 'running', events: [] } as unknown as PublicRecoveredMission
const done = { missionId: 'mission_1', startedBy: { kind: 'mcp' }, phase: 'completed', events: [{ type: 'message.delta', payload: { text: 'All set.', final: true } }] } as unknown as PublicRecoveredMission
function fixture(endsAfter: number, card = false) {
  let looks = 0
  let ended = false
  const pause = vi.fn(async () => { looks += 1; if (looks >= endsAfter) ended = true })
  const read = vi.fn(async (): Promise<MissionReadResponse> => ({ ok: true, data: { mission: ended ? done : working } }))
  const call = createLocustMcpTools({ teammates: async () => teammates, busy: () => false, start: vi.fn(), read, newest: async (id) => id, owner: async () => undefined, live: () => !ended, waiting: () => card, background: async () => [], pause })
  return { call, pause }
}
const said = (result: { content: readonly { text: string }[] }): string => result.content[0]!.text

describe('an app waits for the answer', () => {
  it('waits for the turn to finish and gives its answer in the same call', async () => {
    const f = fixture(3)
    expect(said(await f.call('read_reply', { conversation_id: 'mission_1', wait_seconds: 10 }))).toBe('All set.')
    expect(f.pause).toHaveBeenCalledTimes(3)
  })

  it('says it is still working once the wait runs out, never longer than the limit', async () => {
    const f = fixture(Infinity)
    expect(said(await f.call('read_reply', { conversation_id: 'mission_1', wait_seconds: 600 }))).toMatch(/^Still working/)
    expect(f.pause).toHaveBeenCalledTimes(MCP_WAIT_LIMIT_S * 2)
  })

  it('does not wait at all unless asked, or while a card waits for the person', async () => {
    const asked = fixture(Infinity)
    expect(said(await asked.call('read_reply', { conversation_id: 'mission_1' }))).toMatch(/^Still working/)
    expect(said(await asked.call('read_reply', { conversation_id: 'mission_1', wait_seconds: -5 }))).toMatch(/^Still working/)
    expect(asked.pause).not.toHaveBeenCalled()
    const card = fixture(Infinity, true)
    expect(said(await card.call('read_reply', { conversation_id: 'mission_1', wait_seconds: 20 }))).toMatch(/^Waiting for the person/)
    expect(card.pause).not.toHaveBeenCalled()
  })
})
