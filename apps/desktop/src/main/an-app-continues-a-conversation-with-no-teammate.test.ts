import { describe, expect, it, vi } from 'vitest'
import { createLocustMcpTools } from './locust-mcp-tools.js'
import type { PublicRecoveredMission, CodexMissionStartResponse } from '../shared/ipc.js'

/**
 * AN APP CONTINUES A CONVERSATION WITH NO TEAMMATE, AND ANSWERS ITS QUESTION (0.878).
 *
 * Colin, 17 Nov: "Can we make it so you can interact with conversations?" A conversation he had started without a
 * teammate (a hand-off to Claude Code) was refused to the app he asked to look after it (NO_TEAMMATE, 0.877.2), and
 * the question card it ended on -- "A decision is needed to continue", two options -- could only be pressed in the
 * window. Now, while the person lets apps see their conversations, an app continues it as the window would (its own
 * agent and model, belonging to nobody, in Ask unless "each teammate's own mode" is on), read_reply gives the question
 * and its numbered options, and answer_question sends what the card's press sends. Approvals stay the person's.
 */
const QUESTION = [
  'Batch 8 is done.',
  '',
  '<locust-ask>',
  'When Rust\'s command guard refuses a command, what should the record keep?',
  '- Keep it as Electron :: guard refusals leave no line, and every file stays readable by Electron',
  '- Allow "command-guard" in Rust :: the record shows guard refusals again',
  '</locust-ask>'
].join('\n')

function tools(options: { owner?: string; seeAll?: boolean; ownMode?: boolean; final?: string; startedBy?: 'mcp'; live?: boolean; phase?: string; noMode?: boolean } = {}) {
  const events = options.final === undefined ? [] : [{ sequence: 1, type: 'message.delta', occurredAt: '2026-11-17T20:00:00Z', payload: { itemId: 'm', operation: 'replace', text: options.final, final: true } }]
  const mission = {
    missionId: 'tip', runtime: 'claude', model: 'opus', ...(options.noMode === true ? {} : { mode: 'auto' }), phase: options.phase ?? 'completed', events,
    ...(options.startedBy === undefined ? {} : { startedBy: { kind: options.startedBy } })
  } as unknown as PublicRecoveredMission
  const start = vi.fn(async (_request: unknown) => ({ ok: true, data: { missionId: 'next', runtime: 'claude', model: 'opus', mode: 'ask' } }) as CodexMissionStartResponse)
  const call = createLocustMcpTools({
    teammates: async () => ({ ok: true, data: { teammates: [], missionOwners: {}, missionTitles: {} } }),
    start, busy: () => false, read: async () => ({ ok: true, data: { mission } }), newest: async () => 'tip',
    owner: async () => options.owner, live: () => options.live === true, waiting: () => false, background: async () => [],
    seeAll: () => options.seeAll ?? true, ownMode: () => options.ownMode === true,
    folder: () => '/work', turnFolder: async () => '/work', followUpRoute: async () => ({ runtime: 'claude', model: 'opus' })
  } as never)
  return { call, start, request: () => start.mock.calls[0]![0] as Record<string, unknown> }
}

describe('a conversation with no teammate', () => {
  it('is continued on its own agent and model, belonging to nobody, in Ask', async () => {
    const f = tools()
    const result = await f.call('send_message', { conversation_id: 'tip', message: 'Carry on.' })
    expect(result.isError).toBeUndefined()
    expect(f.start).toHaveBeenCalledOnce()
    const request = f.request()
    expect(request).toMatchObject({ prompt: 'Carry on.', followUpOf: 'tip', runtime: 'claude', model: 'opus', mode: 'ask', keepSavedRoute: true })
    expect(request).not.toHaveProperty('teammateId')
    expect(request).not.toHaveProperty('routeOverrideFor')
  })

  it('keeps the conversation\'s own mode when the person lets apps use each teammate\'s own', async () => {
    const f = tools({ ownMode: true })
    await f.call('send_message', { conversation_id: 'tip', message: 'Carry on.' })
    expect(f.request()).toMatchObject({ mode: 'auto' })
  })

  it('runs in Ask when the conversation never recorded a mode, the switch on or not', async () => {
    const f = tools({ ownMode: true, noMode: true })
    await f.call('send_message', { conversation_id: 'tip', message: 'Carry on.' })
    expect(f.request()).toMatchObject({ mode: 'ask' })
  })

  it('stays out of reach while the person does not let apps see their conversations', async () => {
    const f = tools({ seeAll: false })
    const result = await f.call('send_message', { conversation_id: 'tip', message: 'Carry on.' })
    expect(result.isError).toBe(true)
    expect(f.start).not.toHaveBeenCalled()
  })

  it('still continues one that has a teammate, as that teammate', async () => {
    const f = tools({ owner: 'mate' })
    await f.call('send_message', { conversation_id: 'tip', message: 'Carry on.' })
    expect(f.request()).toMatchObject({ teammateId: 'mate', routeOverrideFor: 'mate' })
  })
})

describe('a question a turn ends on', () => {
  it('reads back with its numbered options', async () => {
    const f = tools({ final: QUESTION })
    const result = await f.call('read_reply', { conversation_id: 'tip' })
    expect(result.structuredContent).toMatchObject({
      status: 'done',
      question: {
        text: 'When Rust\'s command guard refuses a command, what should the record keep?',
        options: [
          { number: 1, label: 'Keep it as Electron', note: 'guard refusals leave no line, and every file stays readable by Electron' },
          { number: 2, label: 'Allow "command-guard" in Rust', note: 'the record shows guard refusals again' }
        ]
      }
    })
  })

  it('is answered by number with exactly what the card\'s press sends', async () => {
    const f = tools({ final: QUESTION })
    const result = await f.call('answer_question', { conversation_id: 'tip', choice: 1 })
    expect(result.isError).toBeUndefined()
    expect(f.request()).toMatchObject({
      prompt: 'Keep it as Electron (guard refusals leave no line, and every file stays readable by Electron). Continue with that.',
      followUpOf: 'tip', mode: 'ask'
    })
  })

  it('is answered by label, whatever its case', async () => {
    const f = tools({ final: QUESTION, owner: 'mate' })
    await f.call('answer_question', { conversation_id: 'tip', choice: 'allow "command-guard" in rust' })
    expect(f.request()).toMatchObject({ prompt: 'Allow "command-guard" in Rust (the record shows guard refusals again). Continue with that.', teammateId: 'mate' })
  })

  it('refuses a choice it does not offer, naming the ones it does, and sends nothing', async () => {
    const f = tools({ final: QUESTION })
    const result = await f.call('answer_question', { conversation_id: 'tip', choice: 3 })
    expect(result.isError).toBe(true)
    expect(result.content[0]!.text).toBe('Choose one of: 1 Keep it as Electron; 2 Allow "command-guard" in Rust. Nothing was sent.')
    expect(f.start).not.toHaveBeenCalled()
  })

  it('is not there when the answer asks nothing, or while the turn still works', async () => {
    const plain = tools({ final: 'All done.' })
    expect((await plain.call('read_reply', { conversation_id: 'tip' })).structuredContent).not.toHaveProperty('question')
    const asked = await plain.call('answer_question', { conversation_id: 'tip', choice: 1 })
    expect(asked.content[0]!.text).toBe('Its newest answer asks no question with choices. Use send_message to reply in words. Nothing was sent.')
    const working = tools({ final: QUESTION, live: true })
    const early = await working.call('answer_question', { conversation_id: 'tip', choice: 1 })
    expect(early.isError).toBe(true)
    expect(working.start).not.toHaveBeenCalled()
  })

  it('is not offered from a turn that failed or is still working, even if its last words asked one', async () => {
    const failed = tools({ final: QUESTION, phase: 'failed' })
    expect((await failed.call('answer_question', { conversation_id: 'tip', choice: 1 })).isError).toBe(true)
    expect(failed.start).not.toHaveBeenCalled()
    const working = tools({ final: QUESTION, live: true })
    expect((await working.call('read_reply', { conversation_id: 'tip' })).structuredContent).not.toHaveProperty('question')
  })

  it('is out of reach in the person\'s own conversation while they do not let apps see it', async () => {
    const f = tools({ final: QUESTION, seeAll: false })
    expect((await f.call('answer_question', { conversation_id: 'tip', choice: 1 })).isError).toBe(true)
    expect(f.start).not.toHaveBeenCalled()
  })
})
