import { describe, expect, it } from 'vitest'
import { createAttention } from './attention.js'
import { createAttentionReader } from './attention-reader.js'

describe('a waiting notification', () => {
  it('names the teammate and conversation and opens exactly the card that raised it', () => {
    const shown: { title: string; body: string; onClick: () => void }[] = []
    const opened: string[] = []
    let focused = 0
    const attention = createAttention({ focused: () => false, supported: () => true, notify: (input) => { shown.push(input) }, focusWindow: () => { focused++ }, openMission: (id) => { opened.push(id) } })
    attention.decisionAsked('Wren', 'Which database?', 'question', 'Choose storage')
    attention.approvalArrived({ kind: 'command', summary: 'Run a command', missionId: 'approval' }, 'Sage', 'Check storage')
    expect(shown.map((toast) => toast.title)).toEqual(['Wren is asking you something · Choose storage', 'Sage needs your approval · Check storage'])
    shown[0]!.onClick()
    shown[1]!.onClick()
    expect(opened).toEqual(['question', 'approval'])
    expect(focused).toBe(2)
  })

  it('carries a recorded decision destination and the chosen conversation name into attention', async () => {
    const said: unknown[][] = []
    const reader = createAttentionReader({
      ledger: { getMission: async () => ({ phase: 'completed', events: [{ type: 'message.delta', sourceAdapter: 'codex', payload: { itemId: 'one', operation: 'append', final: true, text: '<locust-ask>\nWhich database?\n- SQLite :: local\n- Postgres :: server\n</locust-ask>' } }] }) as never },
      teammates: { list: async () => [], missionOwners: async () => ({}) },
      conversationTitle: async () => 'Storage',
      attention: { decisionAsked: (...args) => { said.push(args); return true }, limitHit: () => false }
    })
    await reader.onRunEnded({ missionId: 'decision-turn' })
    expect(said).toEqual([[undefined, 'Which database?', 'decision-turn', 'Storage']])
  })
})
