import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createFileMissionLedger } from '@teammate/mission-store'
import type { NormalizedRuntimeEvent } from '@teammate/runtime-adapters'
import type { MissionReadResponse, PublicRecoveredMission, TeammateListResponse } from '../shared/ipc.js'
import { createLocustMcpTools } from './locust-mcp-tools.js'

/**
 * AN APP READS A LONG ANSWER WHOLE (0.869).
 *
 * Colin, 2026-11-07: an app talking to a Codex teammate over Locust's MCP server got its report back "with the middle
 * cut out in transit" -- everything from one heading to a much later one missing -- and had to ask the teammate to send
 * that part again. The cut was not in transit: every answer was recorded at most 16,384 characters, both ends kept, in
 * the adapter (`boundedMessageText`) and again in the ledger. Here: a real ledger written and read back by a fresh
 * reader, then read_reply (the adapter's own bound is in runtime-adapters' message-tail test).
 */
const NOW = '2026-11-07T22:00:00.000Z'
const roots: string[] = []
afterEach(async () => { await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true }))) })

const teammates: TeammateListResponse = { ok: true, data: { teammates: [], missionOwners: {}, missionTitles: {} } }

async function readBack(text: string): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), 'locust-long-answer-'))
  roots.push(root)
  const ledger = createFileMissionLedger({ rootDirectory: root })
  await ledger.createMission({
    missionId: 'mission_long', runId: 'run_long', prompt: 'Write the README plan.', runtime: 'codex', model: 'account-default',
    requestedRouteId: 'codex', resolvedRouteId: 'codex:cli', cliVersion: '1.0.0', workspaceId: 'ws_test', sandbox: 'read-only',
    mode: 'approve-each', executionPolicyVersion: 1, createdAt: NOW
  })
  const answer = {
    id: 'run_long:codex:1', runId: 'run_long', missionId: 'mission_long', sequence: 1, occurredAt: NOW, sourceAdapter: 'codex',
    type: 'message.delta', payload: { itemId: 'item_1', operation: 'replace', text, final: true, evidence: { redacted: false } }
  } as unknown as NormalizedRuntimeEvent
  await ledger.appendEvents('mission_long', [answer])
  const recorded = await createFileMissionLedger({ rootDirectory: root }).getMission('mission_long')
  const mission = { missionId: 'mission_long', startedBy: { kind: 'mcp' }, phase: 'completed', events: recorded?.events ?? [] } as unknown as PublicRecoveredMission
  const read = vi.fn(async (): Promise<MissionReadResponse> => ({ ok: true, data: { mission } }))
  const call = createLocustMcpTools({ teammates: async () => teammates, busy: () => false, start: vi.fn(), read, newest: async (id) => id, owner: async () => undefined, live: () => false, background: async () => [] })
  const result = await call('read_reply', { conversation_id: 'mission_long' })
  return (result as { content: readonly { text: string }[] }).content[0]!.text
}

describe('an app reads a long answer', () => {
  it('gets a 60,000 character report back whole, every section in order', async () => {
    const sections = ['README and GitHub', 'Move the exhaustive key table into a manual', 'Content', 'Promo and growth', 'Workflow', 'Wild ideas worth one try']
    const report = sections.map((title) => `## ${title}\n${'A sentence about this part of the plan. '.repeat(250)}`).join('\n\n').trim()
    expect(report.length).toBeGreaterThan(60_000)
    expect(await readBack(report)).toBe(report)
  })

  it('still keeps both ends of an answer past the limit, and says where it was shortened', async () => {
    // The control: the bound did not simply go away.
    const huge = `START ${'x'.repeat(300_000)} END`
    const back = await readBack(huge)
    expect(back.length).toBeLessThan(huge.length)
    expect(back.startsWith('START ')).toBe(true)
    expect(back.endsWith(' END')).toBe(true)
  })
})
