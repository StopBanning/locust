import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createFileMissionLedger } from '@teammate/mission-store'
import type { MissionStarter } from '@teammate/mission-store'
import type { CodexMissionStartRequest, CodexMissionStartResponse, MissionReadResponse, TeammateListResponse } from '../shared/ipc.js'
import { createLocustMcpTools } from './locust-mcp-tools.js'
import { publicRecoveredMission } from './mission-history.js'

/**
 * AN APP'S TURN AFTER A CARRY-ON STAYS IN ITS FOLDER (0.882).
 *
 * Found 22 Nov: three lanes an app had sent to their own folders were carried on to Claude when Codex ran out, and the
 * app's next message to each ran in ~/Documents/Locust, the folder Locust had open. The carried turn recorded the
 * folder (0.872), but the window's read of a turn copied a carry-on's `startedBy` with `from` alone, so the app's
 * follow-up never saw it. Here: a real ledger record, read back the way the window reads it, then send_message.
 */
const NOW = '2026-11-22T12:00:00.000Z'
const roots: string[] = []
afterEach(async () => { await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true }))) })
const teammates: TeammateListResponse = { ok: true, data: { teammates: [], missionOwners: {}, missionTitles: {} } }

async function carriedTurn(startedBy: MissionStarter) {
  const root = await mkdtemp(join(tmpdir(), 'locust-carried-folder-'))
  roots.push(root)
  const ledger = createFileMissionLedger({ rootDirectory: root })
  await ledger.createMission({
    missionId: 'mission_carried', runId: 'run_carried', prompt: 'Carry on the brief.', runtime: 'claude', model: 'opus',
    requestedRouteId: 'claude', resolvedRouteId: 'claude:cli', cliVersion: '1.0.0', workspaceId: 'ws_test', sandbox: 'workspace-write',
    mode: 'auto', executionPolicyVersion: 1, createdAt: NOW, startedBy
  })
  const recorded = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_carried')
  return publicRecoveredMission(recorded!)
}

describe("an app's turn after a carry-on", () => {
  it('reads the folder a carried turn recorded, as the window reads the turn', async () => {
    const turn = await carriedTurn({ kind: 'carry-on', from: 'codex', folder: '/work/lane' })
    expect(turn.startedBy).toEqual({ kind: 'carry-on', from: 'codex', folder: '/work/lane' })
    // A carry-on with no folder named stays without one.
    expect((await carriedTurn({ kind: 'carry-on', from: 'codex' })).startedBy).toEqual({ kind: 'carry-on', from: 'codex' })
  })

  it('sends the next message to the same folder, not the one Locust has open', async () => {
    const turn = { ...(await carriedTurn({ kind: 'carry-on', from: 'codex', folder: '/work/lane' })), phase: 'completed' as const }
    const start = vi.fn(async (_request: CodexMissionStartRequest, _origin: MissionStarter): Promise<CodexMissionStartResponse> =>
      ({ ok: true, data: { runId: 'run_next', missionId: 'mission_next', mode: 'auto' } } as unknown as CodexMissionStartResponse))
    const read = vi.fn(async (): Promise<MissionReadResponse> => ({ ok: true, data: { mission: turn } }))
    const call = createLocustMcpTools({
      teammates: async () => teammates, busy: () => false, start, read, newest: async (id) => id, owner: async () => undefined,
      live: () => false, background: async () => [], seeAll: () => true, ownMode: () => true, appChooses: true,
      checkFolder: async (path) => ({ path }), folder: () => '/Users/someone/Documents/Locust'
    })
    await call('send_message', { conversation_id: 'mission_carried', message: 'Keep going.' })
    expect(start).toHaveBeenCalledTimes(1)
    expect(start.mock.calls[0]![1]).toMatchObject({ kind: 'mcp', folder: '/work/lane' })
  })
})
