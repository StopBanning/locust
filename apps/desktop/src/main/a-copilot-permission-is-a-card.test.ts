import { acpPermissionRequestOf } from '@teammate/runtime-adapters'
import { describe, expect, it } from 'vitest'

import { acpAnswerFor, acpPermissionRequest, createApprovalChannel } from './approval-channel.js'
import type { MissionApprovalRequest } from '../shared/ipc.js'

/**
 * 0.377: what Copilot asks over the Agent Client Protocol becomes the card
 * Codex's and OpenCode's requests do, and the person's answer goes back as
 * the KIND of option to choose.
 *
 * The first request is Copilot 1.0.88's own, as measured on 2026-09-26
 * (runtime-adapters test/fixtures/acp/copilot-1.0.88-load-approved.jsonl),
 * read by the adapter's own reader.
 */
const OPTIONS = [
  { optionId: 'allow_once', kind: 'allow_once', name: 'Allow once' },
  { optionId: 'allow_always', kind: 'allow_always', name: 'Always allow' },
  { optionId: 'reject_once', kind: 'reject_once', name: 'Deny' }
]
const MEASURED = {
  sessionId: '3abe5d9d-b422-4bf5-8c95-97d605d3f819',
  toolCall: {
    toolCallId: 'call_utkWEwr3SiwPun0Iym7cESBP',
    title: 'Print the required value from the shell',
    kind: 'execute',
    status: 'pending',
    rawInput: { command: 'echo ACP-OK-7', commands: ['echo ACP-OK-7'] }
  },
  options: OPTIONS
}

function channelFor(runId: string) {
  const raised: MissionApprovalRequest[] = []
  const denied: Array<{ runId: string; missionId: string; reason: string }> = []
  const channel = createApprovalChannel({ emitApproval: (request) => raised.push(request), onDeniedSaying: (said) => denied.push(said), createId: () => runId })
  const handler = channel.requestHandlerFor({ runId, missionId: `m_${runId}`, cwd: 'C:\\work', changesByItem: new Map(), runtime: 'copilot' })
  return { raised, denied, channel, handler }
}

describe("a Copilot permission request, over ACP", () => {
  it('is a command card naming the command Copilot asked to run, from Copilot, and the answer comes back as the kind of option to choose', async () => {
    const { raised, channel, handler } = channelFor('1')
    const answer = handler(acpPermissionRequest(acpPermissionRequestOf(MEASURED), 'C:\\work'))
    expect(raised[0]).toMatchObject({ runtime: 'copilot', kind: 'command', summary: 'Run a command', detail: 'echo ACP-OK-7' })
    channel.decide({ approvalId: raised[0]!.approvalId, decision: 'approve-once' })
    expect(acpAnswerFor(await answer)).toBe('allow_once')
  })

  it("a shell that only names its folder is a card with no command -- the folder is not what would run", () => {
    const { raised, handler } = channelFor('2b')
    const asked = {
      ...MEASURED,
      toolCall: { toolCallId: 't2b', title: 'shell', kind: 'other', rawInput: {}, locations: [{ path: 'C:\\work\\scratch\\oc2-tool-rows-probe' }] }
    }
    void handler(acpPermissionRequest(acpPermissionRequestOf(asked), 'C:\\work'))
    expect(raised[0]).toMatchObject({ kind: 'command', detail: '' })
    expect(raised[0]?.detail).not.toContain('oc2-tool-rows-probe')
  })

  it("a shell call that did not say its command is a card that says so -- its description is not passed off as the command", () => {
    const { raised, handler } = channelFor('2')
    const asked = { ...MEASURED, toolCall: { toolCallId: 't2', title: 'Tidy the build folder', kind: 'execute' } }
    void handler(acpPermissionRequest(acpPermissionRequestOf(asked), 'C:\\work'))
    expect(raised[0]).toMatchObject({ kind: 'command', summary: 'Run a command it did not describe', detail: '' })
  })

  it('an edit shows its change, by its file relative to the folder', () => {
    const { raised, handler } = channelFor('3')
    const asked = {
      ...MEASURED,
      toolCall: {
        toolCallId: 't3',
        title: 'Edit notes.txt',
        kind: 'edit',
        locations: [{ path: 'C:\\work\\notes.txt' }],
        content: [{ type: 'diff', path: 'C:\\work\\notes.txt', oldText: 'Status: draft\n', newText: 'Status: ready\n' }]
      }
    }
    void handler(acpPermissionRequest(acpPermissionRequestOf(asked), 'C:\\work'))
    expect(raised[0]).toMatchObject({ kind: 'file-change', summary: 'Change 1 file', detail: 'notes.txt' })
    expect(raised[0]?.patch).toMatchObject({ added: 1, removed: 1 })
    expect(raised[0]?.patch?.text).toContain('+Status: ready')
    // Named from the folder, not the drive.
    expect(raised[0]?.patch?.text).not.toContain('C:')
  })

  it("anything else is said in the agent's own words, with what it names", () => {
    const outside = acpPermissionRequest(
      acpPermissionRequestOf({ ...MEASURED, toolCall: { toolCallId: 't4', title: 'Read a file outside the folder', kind: 'read', locations: [{ path: 'C:\\other\\keys.txt' }] } }),
      'C:\\work'
    )
    expect(outside).toMatchObject({ method: 'item/commandExecution/requestApproval', params: { command: 'Read a file outside the folder: C:\\other\\keys.txt', itemId: 't4' } })
    const fetch = acpPermissionRequest(acpPermissionRequestOf({ ...MEASURED, toolCall: { toolCallId: 't5', kind: 'fetch' } }), 'C:\\work')
    expect(fetch).toMatchObject({ params: { command: 'Use fetch' } })
  })

  it('reads "for the rest of the run" as always -- which the run keeps for itself -- and anything unclear as a refusal', () => {
    expect(acpAnswerFor({ decision: 'accept' })).toBe('allow_once')
    expect(acpAnswerFor({ decision: 'acceptForSession' })).toBe('allow_always')
    expect(acpAnswerFor({ decision: 'reject' })).toBe('reject_once')
    expect(acpAnswerFor({ decision: 'approve' })).toBe('reject_once')
    expect(acpAnswerFor(null)).toBe('reject_once')
  })

  it("a denial's reason goes on as the next thing the run is told -- ACP's answer has no room for it", async () => {
    const { raised, denied, channel, handler } = channelFor('6')
    const answer = handler(acpPermissionRequest(acpPermissionRequestOf(MEASURED), 'C:\\work'))
    channel.decide({ approvalId: raised[0]!.approvalId, decision: 'deny', reason: 'Use npm test instead.' })
    expect(acpAnswerFor(await answer)).toBe('reject_once')
    expect(denied).toEqual([{ runId: '6', missionId: 'm_6', reason: 'Use npm test instead.' }])
    expect(channel.declined('6').has('call_utkWEwr3SiwPun0Iym7cESBP')).toBe(true)
  })
})
