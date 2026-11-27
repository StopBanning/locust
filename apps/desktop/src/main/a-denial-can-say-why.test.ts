import { describe, expect, it } from 'vitest'

import type { AppServerRequest, JsonValue } from '@teammate/runtime-adapters'

import type { MissionApprovalRequest } from '../shared/ipc.js'
import { MAX_DENY_REASON, approvalAnswerFrom, createApprovalChannel, deniedSaying, openCodeReplyFor } from './approval-channel.js'
import type { FileChangeRecord } from './approval-patch.js'

/**
 * A DENIAL CAN SAY WHY (0.374).
 *
 * A bare "denied" left the teammate to guess, and the guess was usually the
 * same action by another route. The person may now say why, and it reaches
 * the teammate by whatever that runtime can carry: in OpenCode's reject
 * message, in Claude's denial (permission-host.test.ts), and as the next
 * input to a Codex run, whose reply has no room for it.
 */
function channel(runtime?: 'opencode' | 'codex') {
  const approvals: MissionApprovalRequest[] = []
  const steered: { runId: string; missionId: string; reason: string }[] = []
  let nextId = 0
  const instance = createApprovalChannel({
    emitApproval: (request) => approvals.push(request),
    createId: () => String(++nextId),
    onDeniedSaying: (denied) => steered.push(denied)
  })
  const handler = instance.requestHandlerFor({
    runId: 'run_1',
    missionId: 'mission_1',
    cwd: 'C:\\work',
    changesByItem: new Map<string, readonly FileChangeRecord[]>(),
    ...(runtime === undefined ? {} : { runtime })
  })
  const ask = (): Promise<JsonValue> =>
    handler({ id: 1, method: 'item/commandExecution/requestApproval', params: { command: 'rm -rf build', cwd: 'C:\\work' } } as AppServerRequest)
  return { instance, approvals, steered, ask }
}

describe('the window’s answer', () => {
  it('keeps a denial’s reason, in one line and bounded, and nothing when there is none', () => {
    expect(approvalAnswerFrom({ approvalId: 'a', decision: 'deny', reason: '  use the\nbuild script instead  ' })).toEqual({ approvalId: 'a', decision: 'deny', reason: 'use the build script instead' })
    expect(approvalAnswerFrom({ approvalId: 'a', decision: 'deny', reason: '   ' })).toEqual({ approvalId: 'a', decision: 'deny' })
    expect(approvalAnswerFrom({ approvalId: 'a', decision: 'deny' })).toEqual({ approvalId: 'a', decision: 'deny' })
    expect((approvalAnswerFrom({ approvalId: 'a', decision: 'deny', reason: 'x'.repeat(2_000) }) as { reason?: string }).reason).toHaveLength(MAX_DENY_REASON)
  })

  it('never lets a reason ride an approval, or turn a malformed answer into anything but a denial', () => {
    expect(approvalAnswerFrom({ approvalId: 'a', decision: 'approve-once', reason: 'sure' })).toEqual({ approvalId: 'a', decision: 'approve-once' })
    expect(approvalAnswerFrom({ approvalId: 'a', decision: 'yes please', reason: 'no' })).toEqual({ approvalId: 'a', decision: 'deny', reason: 'no' })
  })
})

describe('a denial with a reason', () => {
  it('on Codex: the reply is the plain protocol reject, and the reason goes to the run as its next input', async () => {
    const { instance, approvals, steered, ask } = channel()
    const waiting = ask()
    await Promise.resolve()
    expect(instance.decide({ approvalId: approvals[0]!.approvalId, decision: 'deny', reason: 'use the build script instead' })).toBe(true)
    // Exactly what Codex's app-server expects: no field it does not know.
    expect(await waiting).toEqual({ decision: 'decline' })
    expect(steered).toEqual([{ runId: 'run_1', missionId: 'mission_1', reason: 'use the build script instead' }])
  })

  it('on OpenCode: the reason rides the reject itself, and nothing is steered', async () => {
    const { instance, approvals, steered, ask } = channel('opencode')
    const waiting = ask()
    await Promise.resolve()
    instance.decide({ approvalId: approvals[0]!.approvalId, decision: 'deny', reason: 'use the build script instead' })
    expect(openCodeReplyFor(await waiting)).toEqual({ reply: 'reject', message: deniedSaying('use the build script instead') })
    expect(steered).toEqual([])
  })

  it('without a reason: a plain reject everywhere, and nothing steered', async () => {
    const codex = channel()
    const onCodex = codex.ask()
    await Promise.resolve()
    codex.instance.decide({ approvalId: codex.approvals[0]!.approvalId, decision: 'deny' })
    expect(await onCodex).toEqual({ decision: 'decline' })
    const open = channel('opencode')
    const onOpen = open.ask()
    await Promise.resolve()
    open.instance.decide({ approvalId: open.approvals[0]!.approvalId, decision: 'deny' })
    expect(openCodeReplyFor(await onOpen)).toBe('reject')
    expect([...codex.steered, ...open.steered]).toEqual([])
  })

  it('reads, to the runtime, as the person’s words', () => {
    expect(deniedSaying('use the build script instead')).toBe('The person declined this, and said: use the build script instead')
  })
})
