import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, it, vi } from 'vitest'
import type { MissionReadResponse, PublicRecoveredMission, TeammateListResponse } from '../shared/ipc.js'
import { createLocustMcpTools } from './locust-mcp-tools.js'

/**
 * AN APP STOPS A TURN IT STARTED (0.842). Colin's Claude Duet started a turn in Auto that went the wrong way, and only
 * he could stop it, in Locust. stop_turn is the window's own Stop for a turn an app started, and nothing more: a turn
 * the person started -- even in a conversation the app began -- is theirs alone to stop.
 */
const teammates: TeammateListResponse = { ok: true, data: { teammates: [], missionOwners: {}, missionTitles: {} } }
const turn = (missionId: string, kind: string) => ({ missionId, runId: `run_${missionId}`, startedBy: { kind }, phase: 'completed', events: [] }) as unknown as PublicRecoveredMission
const MISSIONS: Record<string, PublicRecoveredMission> = { mission_app: turn('mission_app', 'mcp'), mission_person: turn('mission_person', 'window') }
function fixture(options: { newest?: string, live?: boolean, stops?: boolean, offered?: boolean } = {}) {
  const cancel = vi.fn((_runId: string) => options.stops ?? true)
  const read = vi.fn(async (id: string): Promise<MissionReadResponse> => ({ ok: true, data: { mission: MISSIONS[id]! } }))
  const call = createLocustMcpTools({ teammates: async () => teammates, busy: () => false, start: vi.fn(), read, newest: async () => options.newest ?? 'mission_app', owner: async () => undefined, live: () => options.live ?? true, background: async () => [], ...(options.offered === false ? {} : { cancel }) })
  return { call, cancel }
}
const said = (result: { content: readonly { text: string }[] }): string => result.content[0]!.text

describe('an app stops a turn it started', () => {
  it('stops the running turn by its own run id, and says what is kept', async () => {
    const f = fixture()
    const result = await f.call('stop_turn', { conversation_id: 'mission_app' })
    expect(f.cancel).toHaveBeenCalledWith('run_mission_app')
    expect(result.isError).toBeFalsy()
    expect(said(result)).toMatch(/^Stopping it\..*undo that turn in Locust/)
  })

  it('never stops a turn the person started, even in a conversation the app began', async () => {
    const f = fixture({ newest: 'mission_person' })
    const result = await f.call('stop_turn', { conversation_id: 'mission_app' })
    expect(f.cancel).not.toHaveBeenCalled()
    expect(result.isError).toBe(true)
    // Nor a conversation the app did not start at all.
    expect((await f.call('stop_turn', { conversation_id: 'mission_person' })).isError).toBe(true)
    expect(f.cancel).not.toHaveBeenCalled()
  })

  it('says so when nothing is running, or when Locust could not stop it', async () => {
    const idle = fixture({ live: false })
    expect(said(await idle.call('stop_turn', { conversation_id: 'mission_app' }))).toMatch(/^Nothing is running/)
    expect(idle.cancel).not.toHaveBeenCalled()
    const stuck = fixture({ stops: false })
    const result = await stuck.call('stop_turn', { conversation_id: 'mission_app' })
    expect(result.isError).toBe(true)
    expect(said(result)).toMatch(/can stop it in Locust/)
  })

  it('is not offered by a server without the window’s Stop, and the window’s Stop is what Locust passes', async () => {
    const f = fixture({ offered: false })
    expect(await f.call('stop_turn', { conversation_id: 'mission_app' })).toMatchObject({ isError: true })
    const main = readFileSync(fileURLToPath(new URL('./index.ts', import.meta.url)), 'utf8')
    expect(main).toContain('cancel: runId => codexMissions.cancel(runId).ok || antigravityMissions.cancel(runId)')
  })
})
