import { describe, expect, it, vi } from 'vitest'
import type { MissionReadResponse, PublicRecoveredMission, TeammateListResponse } from '../shared/ipc.js'
import { createLocustMcpTools } from './locust-mcp-tools.js'

/**
 * AN APP READS THE ANSWER IN BOTH PARTS OF THE REPLY (0.882).
 *
 * A tool result has its text and, from Locust, a structured part with the turn's status. A client that reads the
 * structured part shows it in place of the text: Claude Code read every finished answer as {"status":"done",...} and
 * nothing else (found 22 Nov, a whole night of lane answers read from the ledger files instead). The structured part
 * now carries the same words as `message`: the answer, the activity, or Locust's own words for why there is none.
 */
const teammates: TeammateListResponse = { ok: true, data: { teammates: [], missionOwners: {}, missionTitles: {} } }
const answer = (text: string) => ({ id: 'run_1:codex:1', runId: 'run_1', missionId: 'mission_1', sequence: 1, occurredAt: '2026-11-22T12:00:00.000Z', sourceAdapter: 'codex', type: 'message.delta', payload: { itemId: 'item_1', operation: 'replace', text, final: true } })

function tools(mission: Partial<PublicRecoveredMission>, live = false, waiting = false) {
  const turn = { missionId: 'mission_1', runId: 'run_1', startedBy: { kind: 'mcp' }, events: [], ...mission } as unknown as PublicRecoveredMission
  const read = vi.fn(async (): Promise<MissionReadResponse> => ({ ok: true, data: { mission: turn } }))
  return createLocustMcpTools({ teammates: async () => teammates, busy: () => false, start: vi.fn(), read, newest: async (id) => id, owner: async () => undefined, live: () => live, waiting: () => waiting, background: async () => [] })
}
async function reply(call: ReturnType<typeof tools>) {
  const result = await call('read_reply', { conversation_id: 'mission_1' }) as { content: readonly { text: string }[]; structuredContent?: Record<string, unknown> }
  return { text: result.content[0]!.text, structured: result.structuredContent! }
}

describe('an app reads the answer in both parts of the reply', () => {
  it('gives a finished answer in the structured part too, beside its status', async () => {
    const { text, structured } = await reply(tools({ phase: 'completed', events: [answer('The four rows are carried.')] as never }))
    expect(text).toBe('The four rows are carried.')
    expect(structured).toMatchObject({ status: 'done', message: 'The four rows are carried.' })
  })

  it('gives the failure, the activity and the wait for the person in both parts too', async () => {
    const failed = await reply(tools({ phase: 'failed', hostFailureMessage: 'Codex ran out of usage.' }))
    expect(failed.structured).toMatchObject({ status: 'failed', message: failed.text })
    expect(failed.text).toBe('Codex ran out of usage.')
    const working = await reply(tools({ phase: 'interrupted' }, true))
    expect(working.structured).toMatchObject({ status: 'working', message: working.text })
    expect(working.text).toMatch(/^Still working/)
    const waiting = await reply(tools({ phase: 'interrupted' }, true, true))
    expect(waiting.structured).toMatchObject({ status: 'waiting_for_person', message: waiting.text })
    expect(waiting.text).toMatch(/approve or deny/)
  })
})
