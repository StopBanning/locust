import { describe, expect, it, vi } from 'vitest'
import type { MissionReadResponse, PublicRecoveredMission, TeammateListResponse } from '../shared/ipc.js'
import { createLocustMcpTools } from './locust-mcp-tools.js'

/**
 * AN APP HEARS WHY A TURN STOPPED SHORT (0.871).
 *
 * 8 Nov: a review handed to Argon (Antigravity, Ask mode) over Locust's MCP server came back as "I will read the three
 * files to begin the review." and nothing more. Antigravity had tried to read them with a command, Ask refused it,
 * and the turn ended. Locust's window showed the warning under that sentence; the app read only the sentence. Warnings
 * recorded after the answer now ride along with it.
 */
const teammates: TeammateListResponse = { ok: true, data: { teammates: [], missionOwners: {}, missionTitles: {} } }
const said = (events: readonly unknown[]): Promise<string> => {
  const mission = { missionId: 'mission_1', startedBy: { kind: 'mcp' }, phase: 'completed', events } as unknown as PublicRecoveredMission
  const read = vi.fn(async (): Promise<MissionReadResponse> => ({ ok: true, data: { mission } }))
  const call = createLocustMcpTools({ teammates: async () => teammates, busy: () => false, start: vi.fn(), read, newest: async (id) => id, owner: async () => undefined, live: () => false, background: async () => [] })
  return call('read_reply', { conversation_id: 'mission_1' }).then(r => r.content[0]!.text)
}
const answer = (text: string, itemId = 'm1') => ({ type: 'message.delta', payload: { itemId, operation: 'append', text, final: true } })
const note = (level: string, message: string) => ({ type: 'adapter.diagnostic', payload: { level, code: 'x', message, terminal: false } })

describe('an app reading a turn that stopped short', () => {
  it('gets the warning Locust recorded after the last words', async () => {
    const refused = 'Antigravity was not allowed to run a command in this mode. Edit lets it change files but not run commands; Auto lets it do both.'
    expect(await said([answer('I will read the three files to begin the review.'), note('warning', refused), note('info', 'Started in 2.2 s.')]))
      .toBe(`I will read the three files to begin the review.\n\nLocust: ${refused}`)
  })

  it('gets the answer alone when nothing was refused, and no warning from before the answer', async () => {
    expect(await said([note('warning', 'An earlier hiccup.'), answer('All set.'), note('info', 'Usage 50%.')])).toBe('All set.')
  })

  it('finds the end of an answer that arrived in pieces', async () => {
    const pieces = [{ type: 'message.delta', payload: { itemId: 'm1', operation: 'append', text: 'Half ', final: false } }, note('warning', 'Too early to count.'),
      { type: 'message.delta', payload: { itemId: 'm1', operation: 'append', text: 'and whole.', final: true } }, note('error', 'Then it stopped.')]
    expect(await said(pieces)).toBe('Half and whole.\n\nLocust: Then it stopped.')
  })
})
