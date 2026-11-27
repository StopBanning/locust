import { describe, expect, it, vi } from 'vitest'

import type { AppServerRequest, JsonValue } from '@teammate/runtime-adapters'

import {
  approvalKindFor,
  createApprovalChannel,
  describeApproval,
  protocolDecisionFor,
  questionsOf,
  withFileChanges
} from './approval-channel.js'
import type { FileChangeRecord } from './approval-patch.js'
import type { CodexMissionUpdate, MissionApprovalRequest } from '../shared/ipc.js'

/**
 * The approval channel, tested without a mission.
 *
 * These used to run through a whole second mission service -- Approve-each
 * had one of its own, because it was the only mode whose transport could stop
 * and ask. Every Codex mode runs on that transport now, so the service is
 * gone and what is left is this: describe what was asked, wait for a person,
 * and match the answer to the question. None of that needs a process, a
 * ledger, or a run.
 *
 * What a real Approve-each MISSION does with the channel -- that a card is
 * raised mid-run, that a stop refuses what was waiting -- is in
 * `codex-mission.test.ts`, beside every other mode.
 */

const NOW = '2026-09-01T10:00:00.000Z'

function channel() {
  const approvals: MissionApprovalRequest[] = []
  const updates: CodexMissionUpdate[] = []
  let nextId = 0
  const instance = createApprovalChannel({
    emitApproval: (request) => approvals.push(request),
    emitUpdate: (update) => updates.push(update),
    createId: () => String(++nextId),
    now: () => new Date(NOW)
  })
  const run = {
    runId: 'run_1',
    missionId: 'mission_1',
    cwd: 'C:\\work',
    changesByItem: new Map<string, readonly FileChangeRecord[]>()
  }
  return { instance, approvals, updates, run, handler: instance.requestHandlerFor(run) }
}

/** Ask, and hand back the promise the runtime is waiting on. */
function ask(
  handler: (request: AppServerRequest) => Promise<JsonValue>,
  method: string,
  params: Record<string, unknown> = {}
): Promise<JsonValue> {
  return handler({ id: 1, method, params } as AppServerRequest)
}

describe('approval descriptions', () => {
  it('recognizes the three protocol methods and nothing else', () => {
    expect(approvalKindFor('item/commandExecution/requestApproval')).toBe('command')
    expect(approvalKindFor('item/fileChange/requestApproval')).toBe('file-change')
    expect(approvalKindFor('item/tool/requestUserInput')).toBe('question')
    expect(approvalKindFor('item/somethingNew/requestApproval')).toBeUndefined()
  })

  it('carries the exact command so the card can say what would happen', () => {
    const described = describeApproval({
      id: 1,
      method: 'item/commandExecution/requestApproval',
      params: { command: 'rm -rf build', cwd: 'C:\\work' }
    })
    expect(described).toMatchObject({ kind: 'command', detail: 'rm -rf build', cwd: 'C:\\work' })
  })

  it('says so plainly when the runtime described nothing', () => {
    // A card that cannot say what would happen is not an approval, it is a
    // dare -- so the absence is stated rather than hidden behind a generic
    // "Run a command".
    const described = describeApproval({ id: 1, method: 'item/commandExecution/requestApproval', params: {} })
    expect(described?.summary).toMatch(/did not describe/)
  })

  it('counts file changes rather than pasting a diff', () => {
    const described = describeApproval({
      id: 1,
      method: 'item/fileChange/requestApproval',
      params: { changes: [1, 2], summary: 'update config' }
    })
    expect(described?.summary).toBe('Change 2 files')
    expect(described?.detail).toBe('update config')
  })

  // Whole up to a size no real command reaches, then the rest COUNTED (R14):
  // a 9,000-character command was cut at 4,000 with nothing saying so.
  it('shows a long command whole, and counts what is past the cap', () => {
    const long = describeApproval({ id: 1, method: 'item/commandExecution/requestApproval', params: { command: 'x'.repeat(9_000) } })
    expect(long?.detail).toBe('x'.repeat(9_000))
    const huge = describeApproval({ id: 2, method: 'item/commandExecution/requestApproval', params: { command: 'x'.repeat(25_000) } })
    expect(huge?.detail).toMatch(/\[5,000 more characters not shown here/)
  })

  it('drops a question with no id, because an answer would have nowhere to go', () => {
    const questions = questionsOf({
      questions: [
        { id: 'q1', question: 'Which branch?', options: [{ label: 'main' }] },
        { question: 'Nameless' }
      ]
    })
    expect(questions.map((entry) => entry.id)).toEqual(['q1'])
    expect(questions[0]?.options).toEqual([{ label: 'main', description: null }])
  })
})

describe('decisions', () => {
  it('maps the product answers onto the protocol', () => {
    expect(protocolDecisionFor('approve-once')).toBe('accept')
    // Session-scoped, never durable: a forever-grant is a Settings decision.
    expect(protocolDecisionFor('approve-always')).toBe('acceptForSession')
    expect(protocolDecisionFor('deny')).toBe('decline')
  })
})

describe('the approval round trip', () => {
  it('surfaces a request to the person and answers the runtime when decided', async () => {
    const { instance, approvals, handler } = channel()
    const waiting = ask(handler, 'item/commandExecution/requestApproval', { command: 'pnpm build', cwd: 'C:\\work' })
    await vi.waitFor(() => {
      expect(approvals).toHaveLength(1)
    })
    expect(approvals[0]).toMatchObject({ runId: 'run_1', missionId: 'mission_1', runtime: 'codex', kind: 'command' })
    expect(instance.pendingCount).toBe(1)

    expect(instance.decide({ approvalId: approvals[0]!.approvalId, decision: 'approve-once' })).toBe(true)
    await expect(waiting).resolves.toEqual({ decision: 'accept' })
    expect(instance.pendingCount).toBe(0)
  })

  it('ignores a decision for an unknown or already-answered approval', async () => {
    const { instance, approvals, handler } = channel()
    void ask(handler, 'item/fileChange/requestApproval', { changes: [1] })
    await vi.waitFor(() => {
      expect(approvals).toHaveLength(1)
    })
    const id = approvals[0]!.approvalId
    expect(instance.decide({ approvalId: id, decision: 'deny' })).toBe(true)
    // A double click on the card must not take the run down.
    expect(instance.decide({ approvalId: id, decision: 'approve-once' })).toBe(false)
    expect(instance.decide({ approvalId: 'nope', decision: 'approve-once' })).toBe(false)
  })

  it('refuses a request it does not understand rather than guessing', async () => {
    const { approvals, updates, handler } = channel()
    await expect(ask(handler, 'item/futureThing/requestApproval')).resolves.toEqual({ decision: 'decline' })
    // And it never reaches the person, because the card could not describe it.
    expect(approvals).toHaveLength(0)
    // Said out loud: an unattributed denial is the one outcome that must not
    // be quiet in a mode whose whole point is that a person decides.
    expect(JSON.stringify(updates)).toMatch(/does not recognise/)
  })

  it('refuses rather than queueing without limit when too many are waiting', async () => {
    const { instance, approvals, updates, handler } = channel()
    for (let i = 0; i < 16; i += 1) {
      void ask(handler, 'item/commandExecution/requestApproval', { command: `step ${String(i)}` })
    }
    await vi.waitFor(() => {
      expect(approvals).toHaveLength(16)
    })
    await expect(ask(handler, 'item/commandExecution/requestApproval', { command: 'one too many' }))
      .resolves.toEqual({ decision: 'decline' })
    expect(instance.pendingCount).toBe(16)
    expect(JSON.stringify(updates)).toMatch(/already waiting on you/)
  })

  it('will not let a question be answered with a decision, or the reverse', async () => {
    const { instance, approvals, updates, handler } = channel()
    const waiting = ask(handler, 'item/tool/requestUserInput', {
      questions: [{ id: 'q1', question: 'Which branch?', options: [{ label: 'main' }] }]
    })
    await vi.waitFor(() => {
      expect(approvals).toHaveLength(1)
    })
    // A decision sent to a question: refused, not forwarded. The server does
    // not validate the shape -- it deserializes, fails quietly, and
    // substitutes an empty answer -- so this is the only place it is caught.
    expect(instance.decide({ approvalId: approvals[0]!.approvalId, decision: 'approve-once' })).toBe(false)
    await expect(waiting).resolves.toEqual({ decision: 'decline' })
    expect(JSON.stringify(updates)).toMatch(/answered rather than approved/)
  })

  it('carries a question and its answers through in the shape the protocol wants', async () => {
    const { instance, approvals, handler } = channel()
    const waiting = ask(handler, 'item/tool/requestUserInput', {
      questions: [{ id: 'q1', question: 'Which branch?', options: [{ label: 'main' }] }]
    })
    await vi.waitFor(() => {
      expect(approvals).toHaveLength(1)
    })
    expect(approvals[0]?.questions).toMatchObject([{ id: 'q1', question: 'Which branch?' }])
    expect(instance.decide({ approvalId: approvals[0]!.approvalId, answers: { q1: ['main'] } })).toBe(true)
    await expect(waiting).resolves.toEqual({ answers: { q1: { answers: ['main'] } } })
  })

  it('releases what a finished run was still asking, so no card waits forever', async () => {
    const { instance, approvals, handler } = channel()
    const waiting = ask(handler, 'item/commandExecution/requestApproval', { command: 'x' })
    await vi.waitFor(() => {
      expect(approvals).toHaveLength(1)
    })
    instance.release('run_1')
    await expect(waiting).resolves.toEqual({ decision: 'decline' })
    expect(instance.pendingCount).toBe(0)
    // And the card is dead: answering it now changes nothing.
    expect(instance.decide({ approvalId: approvals[0]!.approvalId, decision: 'approve-once' })).toBe(false)
  })

  it('releases only the run that ended, and leaves another run its waiting card', async () => {
    const { instance, approvals } = channel()
    const other = instance.requestHandlerFor({
      runId: 'run_2',
      missionId: 'mission_2',
      cwd: 'C:\\work',
      changesByItem: new Map()
    })
    const first = instance.requestHandlerFor({
      runId: 'run_1',
      missionId: 'mission_1',
      cwd: 'C:\\work',
      changesByItem: new Map()
    })
    const ending = ask(first, 'item/commandExecution/requestApproval', { command: 'x' })
    void ask(other, 'item/commandExecution/requestApproval', { command: 'y' })
    await vi.waitFor(() => {
      expect(approvals).toHaveLength(2)
    })
    instance.release('run_1')
    await expect(ending).resolves.toEqual({ decision: 'decline' })
    // Wren's is still waiting on the person, and only the person may answer.
    expect(instance.pendingCount).toBe(1)
  })

  it('shows the diff for the item the card is about', async () => {
    const changes: readonly FileChangeRecord[] = [
      { path: 'C:\\work\\HELLO.txt', kind: 'add', movePath: undefined, diff: 'hello\n' }
    ]
    const raised: MissionApprovalRequest[] = []
    const withCard = createApprovalChannel({
      emitApproval: (request) => raised.push(request),
      createId: () => '1'
    })
    const handler = withCard.requestHandlerFor({
      runId: 'run_1',
      missionId: 'mission_1',
      cwd: 'C:\\work',
      changesByItem: new Map([['item_fc', changes]])
    })
    void ask(handler, 'item/fileChange/requestApproval', { itemId: 'item_fc', changes: [1] })
    await vi.waitFor(() => {
      expect(raised).toHaveLength(1)
    })
    expect(raised[0]?.summary).toBe('Change 1 file')
    expect(raised[0]?.patch?.text).toContain('+hello')
  })
})

describe('a fileChange item carries its change into the activity row', () => {
  const at = '2026-09-05T05:00:00.000Z'
  const base = { id: 'e1', runId: 'run_1', sequence: 1, occurredAt: at, sourceAdapter: 'codex' as const }
  const changes: readonly FileChangeRecord[] = [{ path: 'C:\\work\\pebble\\HELLO.txt', kind: 'add', movePath: undefined, diff: 'hello from wren\n' }]
  const byItem: ReadonlyMap<string, readonly FileChangeRecord[]> = new Map([['item_fc', changes]])

  it("gives a tool row the file's path and a patch, where the normaliser wrote only a count", () => {
    const [out] = withFileChanges(
      [{ ...base, type: 'tool.completed', payload: { itemId: 'item_fc', toolKind: 'fileChange', name: 'apply_patch', command: '1 file change(s)', phase: 'completed', evidence: { runtimeEventType: 'item/completed', redacted: true } } } as never],
      byItem,
      'C:\\work\\pebble'
    )
    const payload = out!.payload as { command?: string; patch?: { text: string; added: number } }
    expect(payload.command).toBe('HELLO.txt')
    expect(payload.patch?.added).toBe(1)
    expect(payload.patch?.text).toContain('+hello from wren')
  })

  // M2 (the code review): a change the person declined was drawn with its
  // diff, as though it had been written. It names its files; it has no patch.
  it('gives a declined fileChange its paths but no patch', () => {
    const [out] = withFileChanges(
      [{ ...base, type: 'tool.failed', payload: { itemId: 'item_fc', toolKind: 'fileChange', name: 'apply_patch', status: 'declined', phase: 'completed', evidence: { runtimeEventType: 'item/completed', redacted: true } } } as never],
      byItem,
      'C:' + String.fromCharCode(92) + 'work' + String.fromCharCode(92) + 'pebble'
    )
    const payload = out!.payload as { command?: string; patch?: unknown }
    expect(payload.command).toBe('HELLO.txt')
    expect(payload.patch).toBeUndefined()
  })

  it('leaves every other event, and a fileChange it never saw, exactly as it was', () => {
    const shell = { ...base, type: 'tool.completed', payload: { itemId: 'item_sh', toolKind: 'commandExecution', name: 'shell', command: 'ls', phase: 'completed', evidence: { runtimeEventType: 'item/completed', redacted: true } } } as never
    const unknown = { ...base, type: 'tool.started', payload: { itemId: 'item_other', toolKind: 'fileChange', name: 'apply_patch', phase: 'started', evidence: { runtimeEventType: 'item/started', redacted: true } } } as never
    const message = { ...base, type: 'message.delta', payload: { itemId: 'm', operation: 'append', text: 'hi', final: false, evidence: { runtimeEventType: 'x', redacted: true } } } as never
    const out = withFileChanges([shell, unknown, message], byItem, 'C:\\work\\pebble')
    expect(out[0]).toBe(shell)
    expect(out[1]).toBe(unknown)
    expect(out[2]).toBe(message)
  })
})
