import { describe, expect, it, vi } from 'vitest'
import type { CodexMissionStartResponse, MissionReadResponse, PublicRecoveredMission, PublicRoutine, RoutineRunResponse, TeammateListResponse } from '../shared/ipc.js'
import { createLocustMcpTools, type McpRoutineStep } from './locust-mcp-tools.js'
import { mcpText } from './locust-mcp-host.js'
import { seedAvatar } from '../shared/avatar.js'

const teammates: TeammateListResponse = { ok: true, data: { teammates: [{ teammateId: 'tm_wren', name: 'Wren', role: 'Custom', roleTitle: 'Reviewer', hue: 'lime', avatar: seedAvatar('tm_wren'), createdAt: '2026-10-07', route: { runtime: 'codex', model: 'model-test', mode: 'auto' } }], missionOwners: {}, missionTitles: {} } }
const mission = (overrides: Partial<PublicRecoveredMission> = {}): PublicRecoveredMission => ({ missionId: 'mission_1', startedBy: { kind: 'mcp' }, phase: 'completed', events: [], ...overrides }) as PublicRecoveredMission
function fixture(extra: { ownMode?: () => boolean; waiting?: (id: string) => boolean } = {}) {
  const start = vi.fn(async (): Promise<CodexMissionStartResponse> => ({ ok: true, data: { missionId: 'mission_1' } } as CodexMissionStartResponse))
  const read = vi.fn(async (): Promise<MissionReadResponse> => ({ ok: true, data: { mission: mission() } }))
  const background = vi.fn(async () => [])
  const deps = { teammates: vi.fn(async () => teammates), busy: vi.fn(() => true), start, read, newest: vi.fn(async id => id), owner: vi.fn(async () => 'tm_wren'), live: vi.fn(() => false), background }
  return { ...deps, call: createLocustMcpTools({ ...deps, ...extra }) }
}
describe('other apps can only ask Locust teammates', () => {
  it('starts a writing teammate in Ask, ignores caller mode/route, and keeps the saved route', async () => {
    const f = fixture()
    const result = await f.call('start_conversation', { teammate: 'wReN', message: 'Review this.', mode: 'auto', runtime: 'claude' })
    expect(f.start).toHaveBeenCalledWith({ teammateId: 'tm_wren', prompt: 'Review this.', mode: 'ask', keepSavedRoute: true }, { kind: 'mcp' })
    expect(result).toEqual(mcpText(JSON.stringify({ conversation_id: 'mission_1', mode: 'ask' })))
    expect(teammates.ok && teammates.data.teammates[0]?.route?.mode).toBe('auto')
  })
  it('uses an exact teammate id and reports the host monthly-limit sentence verbatim', async () => {
    const f = fixture()
    const sentence = 'Wren has reached the $10 monthly limit you set. Nothing was started.'
    f.start.mockResolvedValue({ ok: false, error: { code: 'SPEND_LIMIT_REACHED', message: sentence } })
    expect(await f.call('start_conversation', { teammate: 'tm_wren', message: 'Review this.' })).toEqual(mcpText(sentence, true))
  })
  it('lists public teammate identity, route and busy state, but never the stores or keys', async () => {
    const f = fixture()
    expect(await f.call('list_teammates', {})).toEqual(mcpText(JSON.stringify([{ id: 'tm_wren', name: 'Wren', role: 'Reviewer', runtime: 'codex', model: 'model-test', effort: null, busy: true, runs_in: 'ask' }])))
    expect(f.start).not.toHaveBeenCalled()
  })
  it('continues the newest turn in Ask and returns the stable conversation id', async () => {
    const f = fixture()
    f.newest.mockResolvedValue('mission_2')
    f.read.mockResolvedValueOnce({ ok: true, data: { mission: mission() } }).mockResolvedValueOnce({ ok: true, data: { mission: mission({ missionId: 'mission_2' }) } })
    expect(await f.call('send_message', { conversation_id: 'mission_1', message: 'And this?', mode: 'auto' })).toEqual(mcpText(JSON.stringify({ conversation_id: 'mission_1', mode: 'ask' })))
    expect(f.start).toHaveBeenCalledWith({ teammateId: 'tm_wren', prompt: 'And this?', followUpOf: 'mission_2', mode: 'ask', keepSavedRoute: true }, { kind: 'mcp' })
  })
  it('reports activity and does not queue a message while the turn is running', async () => {
    const f = fixture()
    f.live.mockReturnValue(true)
    f.read.mockResolvedValue({ ok: true, data: { mission: mission({ events: [{ type: 'tool.started', payload: { name: 'Read', title: 'Reading the source' } }] as unknown as PublicRecoveredMission['events'] }) } })
    expect(await f.call('read_reply', { conversation_id: 'mission_1' })).toEqual(mcpText('Still working: Reading the source'))
    expect(await f.call('send_message', { conversation_id: 'mission_1', message: 'next' })).toEqual(mcpText('Still working: Reading the source'))
    expect(f.start).not.toHaveBeenCalled()
  })
  it('returns the newest complete reply rather than an earlier answer or partial text', async () => {
    const f = fixture()
    f.read.mockResolvedValue({ ok: true, data: { mission: mission({ events: [
      { type: 'message.delta', payload: { itemId: 'a', operation: 'append', text: 'earlier', final: true } },
      { type: 'message.delta', payload: { itemId: 'b', operation: 'append', text: 'new ', final: false } },
      { type: 'message.delta', payload: { itemId: 'b', operation: 'append', text: 'answer', final: true } }
    ] as unknown as PublicRecoveredMission['events'] }) } })
    expect(await f.call('read_reply', { conversation_id: 'mission_1' })).toEqual(mcpText('new answer'))
  })
  it('returns the host failure in Locust words, not a previous successful answer', async () => {
    const f = fixture()
    f.read.mockResolvedValue({ ok: true, data: { mission: mission({ phase: 'failed', hostFailureMessage: 'The run could not save its record. Nothing was retried.' }) } })
    expect(await f.call('read_reply', { conversation_id: 'mission_1' })).toEqual(mcpText('The run could not save its record. Nothing was retried.', true))
  })
  it('lists background turns without starting or stopping anything', async () => {
    const f = fixture()
    expect(await f.call('list_background_runs', {})).toEqual(mcpText('[]'))
    expect(f.background).toHaveBeenCalledOnce()
    expect(f.start).not.toHaveBeenCalled()
  })
  it('refuses invalid messages, unknown teammates, unknown tools and non-server conversations', async () => {
    const f = fixture()
    for (const args of [{ teammate: 'Wren', message: '' }, { teammate: 'Wren', message: 'x'.repeat(8001) }, { teammate: 'missing', message: 'hi' }]) expect((await f.call('start_conversation', args)).isError).toBe(true)
    expect((await f.call('delete_files', {})).isError).toBe(true)
    f.read.mockResolvedValue({ ok: true, data: { mission: mission({ startedBy: undefined }) } })
    expect((await f.call('send_message', { conversation_id: 'person_1', message: 'hi' })).isError).toBe(true)
    expect(f.start).not.toHaveBeenCalled()
  })
  it('claims a send before the first asynchronous ledger read, refusing simultaneous sends', async () => {
    const f = fixture()
    let release!: (value: MissionReadResponse) => void
    f.read.mockImplementationOnce(() => new Promise(resolve => { release = resolve }))
    const first = f.call('send_message', { conversation_id: 'mission_1', message: 'first' })
    expect(await f.call('send_message', { conversation_id: 'mission_1', message: 'second' })).toEqual(mcpText('Still working: starting the next turn.'))
    release({ ok: true, data: { mission: mission() } })
    await first
    expect(f.start).toHaveBeenCalledOnce()
  })

  it('keeps a conversation claimed until its next turn has started', async () => {
    const f = fixture()
    let release!: (value: CodexMissionStartResponse) => void
    f.start.mockImplementationOnce(() => new Promise(resolve => { release = resolve }))
    const first = f.call('send_message', { conversation_id: 'mission_1', message: 'first' })
    await vi.waitFor(() => expect(f.start).toHaveBeenCalledOnce())
    const second = await f.call('send_message', { conversation_id: 'mission_1', message: 'second' })
    const poll = await f.call('read_reply', { conversation_id: 'mission_1' })
    release({ ok: true, data: { missionId: 'mission_2' } } as CodexMissionStartResponse)
    expect(await first).toEqual(mcpText(JSON.stringify({ conversation_id: 'mission_1', mode: 'ask' })))
    expect(second).toEqual(mcpText('Still working: starting the next turn.'))
    expect(poll).toEqual(mcpText('Still working: starting the next turn.'))
    expect(f.start).toHaveBeenCalledOnce()
    await f.call('send_message', { conversation_id: 'mission_1', message: 'after startup' })
    expect(f.start).toHaveBeenCalledTimes(2)
  })
  it('keeps a conversation claimed while looking up its next turn mode', async () => {
    const f = fixture({ ownMode: () => true })
    let release!: () => void
    f.teammates.mockImplementationOnce(() => new Promise(resolve => { release = () => resolve(teammates) }))
    const first = f.call('send_message', { conversation_id: 'mission_1', message: 'first' })
    await vi.waitFor(() => expect(f.teammates).toHaveBeenCalledOnce())
    const second = await f.call('send_message', { conversation_id: 'mission_1', message: 'second' })
    release()
    expect(await first).toEqual(mcpText(JSON.stringify({ conversation_id: 'mission_1', mode: 'auto' })))
    expect(second).toEqual(mcpText('Still working: starting the next turn.'))
    expect(f.teammates).toHaveBeenCalledOnce()
    expect(f.start).toHaveBeenCalledOnce()
  })
  it('releases a conversation claim when starting its next turn rejects', async () => {
    const f = fixture()
    let reject!: (reason: Error) => void
    f.start.mockImplementationOnce(() => new Promise((_, fail) => { reject = fail }))
    const first = f.call('send_message', { conversation_id: 'mission_1', message: 'first' })
    const failed = expect(first).rejects.toThrow('start unavailable')
    await vi.waitFor(() => expect(f.start).toHaveBeenCalledOnce())
    const second = await f.call('send_message', { conversation_id: 'mission_1', message: 'second' })
    reject(new Error('start unavailable'))
    await failed
    expect(second).toEqual(mcpText('Still working: starting the next turn.'))
    expect(f.start).toHaveBeenCalledOnce()
    expect(await f.call('send_message', { conversation_id: 'mission_1', message: 'after failure' })).toEqual(mcpText(JSON.stringify({ conversation_id: 'mission_1', mode: 'ask' })))
    expect(f.start).toHaveBeenCalledTimes(2)
  })
})

describe(`with "each teammate's own mode" on (0.703)`, () => {
  it('starts the teammate in the mode saved in Locust, never one the caller names', async () => {
    const f = fixture({ ownMode: () => true })
    const result = await f.call('start_conversation', { teammate: 'Wren', message: 'Fix it.', mode: 'ask' })
    expect(f.start).toHaveBeenCalledWith({ teammateId: 'tm_wren', prompt: 'Fix it.', mode: 'auto', keepSavedRoute: true }, { kind: 'mcp' })
    expect(result).toEqual(mcpText(JSON.stringify({ conversation_id: 'mission_1', mode: 'auto' })))
  })
  it('reads the switch on every turn: turned off, the next turn is Ask', async () => {
    let on = true
    const f = fixture({ ownMode: () => on })
    await f.call('start_conversation', { teammate: 'Wren', message: 'one' })
    on = false
    await f.call('send_message', { conversation_id: 'mission_1', message: 'two' })
    expect(f.start).toHaveBeenLastCalledWith({ teammateId: 'tm_wren', prompt: 'two', followUpOf: 'mission_1', mode: 'ask', keepSavedRoute: true }, { kind: 'mcp' })
  })
  it("continues in the owner's saved mode, and a teammate with no route is Ask", async () => {
    const f = fixture({ ownMode: () => true })
    await f.call('send_message', { conversation_id: 'mission_1', message: 'next', mode: 'ask' })
    expect(f.start).toHaveBeenLastCalledWith({ teammateId: 'tm_wren', prompt: 'next', followUpOf: 'mission_1', mode: 'auto', keepSavedRoute: true }, { kind: 'mcp' })
    f.teammates.mockResolvedValue({ ok: true, data: { ...(teammates.ok ? teammates.data : { missionOwners: {}, missionTitles: {} }), teammates: [{ ...(teammates.ok ? teammates.data.teammates[0]! : ({} as never)), route: undefined }] } } as TeammateListResponse)
    await f.call('start_conversation', { teammate: 'Wren', message: 'hi' })
    expect(f.start).toHaveBeenLastCalledWith({ teammateId: 'tm_wren', prompt: 'hi', mode: 'ask', keepSavedRoute: true }, { kind: 'mcp' })
  })
  it('lists the mode a turn would run in', async () => {
    const f = fixture({ ownMode: () => true })
    expect(JSON.parse((await f.call('list_teammates', {})).content[0]!.text)[0].runs_in).toBe('auto')
  })
  it("passes the host's Auto refusal through as the error, unchanged", async () => {
    const f = fixture({ ownMode: () => true })
    const sentence = 'Auto mode is switched off for this workspace. Turn it on in Settings to let a run work outside the workspace folder. Nothing was recorded.'
    f.start.mockResolvedValue({ ok: false, error: { code: 'RUNTIME_START_FAILED', message: sentence } })
    expect(await f.call('start_conversation', { teammate: 'Wren', message: 'go' })).toEqual(mcpText(sentence, true))
  })
  it("says a card is waiting in Locust's window and offers no way to answer it", async () => {
    const f = fixture({ ownMode: () => true, waiting: id => id === 'mission_1' })
    f.live.mockReturnValue(true)
    const reply = await f.call('read_reply', { conversation_id: 'mission_1' })
    expect(reply.content[0]!.text).toMatch(/^Waiting for the person to approve or deny an action in Locust's window\. This app cannot answer it/)
    for (const tool of ['approve', 'answer_approval', 'decide']) expect((await f.call(tool, { conversation_id: 'mission_1', decision: 'approve-once' })).isError).toBe(true)
    expect(f.start).not.toHaveBeenCalled()
  })
})

describe('other apps run routines through the routine card\'s own path (0.704)', () => {
  const routine = (overrides: Partial<PublicRoutine> = {}): PublicRoutine => ({
    routineId: 'rt_review', name: 'Weekly review', teammateId: 'tm_wren', route: { runtime: 'codex', model: 'model-test', mode: 'ask' },
    steps: ['Read the changes since {{since}}.', 'Write a summary.'], learnedFrom: [], createdAt: '2026-10-08', runs: 0,
    inputs: [{ key: 'since', label: 'Since when', kind: 'text', required: true }], ...overrides
  }) as PublicRoutine
  function routineFixture(saved: PublicRoutine[], own = false) {
    const steps: McpRoutineStep[] = []
    const last = new Map<string, string>()
    const runRoutine = vi.fn(async (): Promise<RoutineRunResponse> => ({ ok: true, data: { missionId: 'mission_r1', runId: 'run_r1' } }))
    const read = vi.fn(async (): Promise<MissionReadResponse> => ({ ok: true, data: { mission: mission({ missionId: 'mission_r2', startedBy: undefined, events: [
      { type: 'message.delta', payload: { itemId: 'a', operation: 'append', text: 'The summary.', final: true } }
    ] as unknown as PublicRecoveredMission['events'] }) } }))
    const call = createLocustMcpTools({ teammates: vi.fn(async () => teammates), busy: () => false, start: vi.fn(), read, newest: vi.fn(async id => id),
      owner: vi.fn(async () => 'tm_wren'), live: () => false, background: vi.fn(async () => []), ownMode: () => own, waiting: id => id === 'mission_wait',
      routines: vi.fn(async () => saved), runRoutine, routineSteps: () => steps, routineLastStep: id => last.get(id) })
    return { call, runRoutine, steps, last, read }
  }
  const json = (result: { content: readonly { text: string }[] }) => JSON.parse(result.content[0]!.text)

  it('lists each step\'s teammate and mode, the inputs, and whether it can run from here', async () => {
    const handed = routine({ routineId: 'rt_fix', name: 'Fix it', route: { runtime: 'codex', model: 'm', mode: 'ask' }, handOffs: [{}, { teammateId: 'tm_wren', check: true }] })
    const writes = routine({ routineId: 'rt_edit', name: 'Edit it', route: { runtime: 'codex', model: 'm', mode: 'accept-edits' } })
    const folder = routine({ routineId: 'rt_folder', name: 'Folder', inputs: [{ key: 'dir', label: 'Folder', kind: 'folder', required: true }] })
    const listed = json(await routineFixture([routine(), handed, writes, folder]).call('list_routines', {}))
    expect(listed[0]).toEqual({ id: 'rt_review', name: 'Weekly review', steps: [{ teammate: 'Wren', mode: 'ask' }, { teammate: 'Wren', mode: 'ask' }],
      inputs: [{ key: 'since', label: 'Since when', kind: 'text', required: true }], changes_files: false, runs_from_here: true })
    expect(listed[1].steps[1]).toEqual({ teammate: 'Wren', mode: 'ask', checks_the_work: true })
    expect(listed[2]).toMatchObject({ changes_files: true, runs_from_here: false })
    expect(listed[2].why_not).toMatch(/Use each teammate's own mode/)
    expect(listed[3]).toMatchObject({ runs_from_here: false })
    expect(listed[3].why_not).toMatch(/folder/)
  })
  it('runs a read-only routine with its values through the run path, by name', async () => {
    const f = routineFixture([routine()])
    expect(json(await f.call('run_routine', { routine: 'weekly REVIEW', values: { since: 'Monday' } }))).toEqual({ routine_id: 'rt_review', steps: 2, first_conversation_id: 'mission_r1' })
    expect(f.runRoutine).toHaveBeenCalledWith('rt_review', { since: 'Monday' })
  })
  it('refuses a routine that changes files while own mode is off -- including one whose HANDED step writes', async () => {
    // Wren's own route is Auto: a step handed to her runs in Auto, whatever the routine was saved with.
    const handed = routine({ handOffs: [{}, { teammateId: 'tm_wren' }], teammateId: 'tm_other' })
    for (const r of [routine({ route: { runtime: 'codex', model: 'm', mode: 'accept-edits' } }), handed]) {
      const f = routineFixture([r])
      const result = await f.call('run_routine', { routine: 'rt_review' })
      expect(result.isError).toBe(true)
      expect(result.content[0]!.text).toMatch(/changes files \(step \d runs in (Edit|Auto)\)\. Turn on "Use each teammate's own mode".*Nothing was started\.$/)
      expect(f.runRoutine).not.toHaveBeenCalled()
    }
    const on = routineFixture([routine({ route: { runtime: 'codex', model: 'm', mode: 'accept-edits' } })], true)
    expect((await on.call('run_routine', { routine: 'rt_review' })).isError).toBeUndefined()
    expect(on.runRoutine).toHaveBeenCalledOnce()
  })
  it('never runs a routine that asks for a folder, and refuses odd values before the run path', async () => {
    const f = routineFixture([routine({ inputs: [{ key: 'dir', label: 'Folder', kind: 'folder', required: true }] }), routine({ routineId: 'rt_two', name: 'Two' })], true)
    expect((await f.call('run_routine', { routine: 'rt_review', values: { dir: 'C:\\' } })).content[0]!.text).toMatch(/chosen in Locust's own picker/)
    for (const values of [['x'], 'since=Monday', { since: 5 }, Object.fromEntries(Array.from({ length: 13 }, (_, i) => [`k${String(i)}`, 'v']))]) {
      expect((await f.call('run_routine', { routine: 'rt_two', values })).isError).toBe(true)
    }
    expect((await f.call('run_routine', { routine: 'nobody' })).isError).toBe(true)
    expect(f.runRoutine).not.toHaveBeenCalled()
  })
  it('never runs a paused routine, and lists it as paused (0.705)', async () => {
    const f = routineFixture([routine({ paused: true })], true)
    const listed = json(await f.call('list_routines', {}))
    expect(listed[0]).toMatchObject({ paused: true, runs_from_here: false, why_not: 'It is paused in Locust.' })
    expect(await f.call('run_routine', { routine: 'rt_review', values: { since: 'Monday' } })).toEqual(mcpText('"Weekly review" is paused in Locust. Resume it there, or run it from Locust. Nothing was started.', true))
    expect(f.runRoutine).not.toHaveBeenCalled()
  })
  it('passes the runner\'s own refusal through, unchanged', async () => {
    const f = routineFixture([routine()])
    f.runRoutine.mockResolvedValue({ ok: false, error: { code: 'ROUTINE_REJECTED', message: 'This routine is already running or waiting for review. Open its routine card before starting more work.' } })
    expect(await f.call('run_routine', { routine: 'rt_review' })).toEqual(mcpText('This routine is already running or waiting for review. Open its routine card before starting more work.', true))
  })
  it('reads status only for routines this server started: the step, a waiting card, then the last step\'s answer', async () => {
    const f = routineFixture([routine()])
    expect((await f.call('routine_status', { routine: 'rt_review' })).isError).toBe(true)
    await f.call('run_routine', { routine: 'rt_review', values: { since: 'Monday' } })
    f.steps.push({ routineId: 'rt_review', teammateId: 'tm_wren', step: 1, of: 2, missionId: 'mission_r1' })
    expect(await f.call('routine_status', { routine: 'rt_review' })).toEqual(mcpText('Still working: step 1 of 2.'))
    f.steps.splice(0, 1, { routineId: 'rt_review', teammateId: 'tm_wren', step: 2, of: 2, missionId: 'mission_wait' })
    expect((await f.call('routine_status', { routine: 'rt_review' })).content[0]!.text).toMatch(/^Waiting for the person to approve or deny an action in Locust's window \(step 2 of 2\)\. This app cannot answer it/)
    f.steps.length = 0
    f.last.set('rt_review', 'mission_r2')
    expect(await f.call('routine_status', { routine: 'rt_review' })).toEqual(mcpText('The summary.'))
    expect(f.read).toHaveBeenCalledWith('mission_r2')
  })
  it('says a held run, a failing check and changes still waiting for Keep or Discard, in Locust\'s words', async () => {
    const held = routine({ execution: { attemptId: 'a', status: 'held', step: 2, of: 2, startedAt: '', updatedAt: '', steps: [], route: { runtime: 'codex', model: 'm', mode: 'ask' }, workspaceId: 'w', reason: 'Review the remaining steps.' } as PublicRoutine['execution'] })
    const f = routineFixture([held])
    await f.call('run_routine', { routine: 'rt_review' })
    expect(await f.call('routine_status', { routine: 'rt_review' })).toEqual(mcpText('Stopped at step 2 of 2 and waiting for the person in Locust: Review the remaining steps.', true))
    const failing = routineFixture([routine({ lastFailed: 'The tests still fail.' })])
    await failing.call('run_routine', { routine: 'rt_review' })
    expect(await failing.call('routine_status', { routine: 'rt_review' })).toEqual(mcpText('The run ended with its check still failing: The tests still fail.', true))
    const staged = routineFixture([routine({ staged: {} as PublicRoutine['staged'] })])
    await staged.call('run_routine', { routine: 'rt_review' })
    staged.last.set('rt_review', 'mission_r2')
    expect((await staged.call('routine_status', { routine: 'rt_review' })).content[0]!.text).toMatch(/^Its changes are waiting in Locust under Routines for the person to Keep or Discard[\s\S]*The summary\.$/)
  })
})
