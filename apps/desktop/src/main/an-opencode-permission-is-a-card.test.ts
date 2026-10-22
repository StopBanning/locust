import { describe, expect, it } from 'vitest'

import { createApprovalChannel, openCodePermissionRequest, openCodeReplyFor } from './approval-channel.js'
import type { MissionApprovalRequest } from '../shared/ipc.js'

/**
 * A6.7: what OpenCode's server asks becomes the card Codex's requests do, and
 * the person's answer goes back as the server's own reply.
 */
describe("an OpenCode server's permission request", () => {
  it('is a command card naming the command, from OpenCode, and the answer comes back as its reply', async () => {
    const raised: MissionApprovalRequest[] = []
    const channel = createApprovalChannel({ emitApproval: (request) => raised.push(request), createId: () => '1' })
    const handler = channel.requestHandlerFor({ runId: 'run_1', missionId: 'm_1', cwd: 'C:/work', changesByItem: new Map(), runtime: 'opencode' })
    const reply = handler(openCodePermissionRequest({ permission: 'bash', patterns: ['echo hi'], metadata: { command: 'echo hi' } }, 'C:/work'))
    expect(raised[0]).toMatchObject({ runtime: 'opencode', kind: 'command', detail: 'echo hi', summary: 'Run a command' })
    channel.decide({ approvalId: raised[0]!.approvalId, decision: 'approve-once' })
    expect(openCodeReplyFor(await reply)).toBe('once')
  })

  it('shows the edit it proposes, by its file relative to the folder (the 0.345 beta report)', () => {
    // OpenCode's own request shape, measured: metadata { filepath, diff }.
    const raised: MissionApprovalRequest[] = []
    const channel = createApprovalChannel({ emitApproval: (request) => raised.push(request), createId: () => '2' })
    const handler = channel.requestHandlerFor({ runId: 'run_2', missionId: 'm_2', cwd: 'C:/work', changesByItem: new Map(), runtime: 'opencode' })
    const diff = [
      'Index: C:/work/notes.txt',
      '='.repeat(67),
      '--- C:/work/notes.txt',
      '+++ C:/work/notes.txt',
      '@@ -1,1 +1,1 @@',
      '-Status: draft',
      '+Status: ready',
      ''
    ].join('\n')
    void handler(openCodePermissionRequest({ permission: 'edit', patterns: ['notes.txt'], metadata: { filepath: 'C:/work/notes.txt', diff } }, 'C:/work'))
    expect(raised[0]).toMatchObject({ kind: 'file-change', summary: 'Change 1 file', detail: 'notes.txt' })
    expect(raised[0]?.patch?.text).toContain('+Status: ready')
    expect(raised[0]?.patch?.text).toContain('-Status: draft')
    expect(raised[0]?.patch?.text).not.toContain('Index:')
    expect(raised[0]?.patch).toMatchObject({ added: 1, removed: 1 })
  })

  it('names an edit by its file, and anything else by what it is', () => {
    expect(openCodePermissionRequest({ permission: 'edit', patterns: ['a.txt'], metadata: { filepath: 'C:/work/a.txt' } }, 'C:/work'))
      .toMatchObject({ method: 'item/fileChange/requestApproval', params: { summary: 'a.txt' } })
    expect(openCodePermissionRequest({ permission: 'external_directory', patterns: ['C:/other/*'], metadata: {} }, 'C:/work'))
      .toMatchObject({ method: 'item/commandExecution/requestApproval', params: { command: 'C:/other/*', locustCard: { summary: 'Reach files outside your project folder' } } })
  })

  // QA-2026-09-29 round 2, R37 and R35: a fetch is not a command, and Always says what it allows.
  it('says a fetch is a fetch, a write outside the folder is outside it, and what Always allows', () => {
    expect(openCodePermissionRequest({ permission: 'webfetch', patterns: ['https://example.com/'], metadata: { url: 'https://example.com/' } }, 'C:/work'))
      .toMatchObject({ params: { command: 'https://example.com/', locustCard: { summary: 'Fetch a web page', reversibleSays: 'Nothing on this machine is changed by fetching.' } } })
    expect(openCodePermissionRequest({ permission: 'edit', patterns: ['C:/Windows/hosts'], metadata: { filepath: 'C:/Windows/hosts' } }, 'C:/work'))
      .toMatchObject({ params: { locustCard: { summary: 'Change 1 file outside your project folder' } } })
    expect(openCodePermissionRequest({ permission: 'bash', patterns: ['echo SERVED'], always: ['echo *'], metadata: { command: 'echo SERVED' } }, 'C:/work'))
      .toMatchObject({ params: { command: 'echo SERVED', locustCard: { alwaysCovers: 'anything matching “echo *”' } } })
  })

  // 0.822: a person's own MCP tool asks with the pattern `*` alone, and the card said "Exact *". It names the tool,
  // shows no exact line, and its Always still covers that tool.
  it("names a person's own MCP tool, with no exact line where OpenCode gives no details", () => {
    const raised: MissionApprovalRequest[] = []
    const channel = createApprovalChannel({ emitApproval: (request) => raised.push(request), createId: () => '3' })
    const handler = channel.requestHandlerFor({ runId: 'run_3', missionId: 'm_3', cwd: 'C:/work', changesByItem: new Map(), runtime: 'opencode' })
    const asked = { permission: 'notes_save_note', patterns: ['*'], always: ['*'], metadata: {} }
    void handler(openCodePermissionRequest(asked, 'C:/work'))
    expect(raised[0]).toMatchObject({ kind: 'command', summary: 'Use notes_save_note', detail: '' })
    expect(openCodePermissionRequest(asked, 'C:/work')).toMatchObject({ params: { command: '', locustAlwaysKey: 'opencode:notes_save_note:*', locustCard: { alwaysCovers: 'every “notes_save_note” it asks for' } } })
    // A tool that does give a pattern keeps it.
    expect(openCodePermissionRequest({ permission: 'notes_save_note', patterns: ['note.txt'], metadata: {} }, 'C:/work')).toMatchObject({ params: { command: 'note.txt' } })
  })

  // 0.616: "for the rest of the run" is remembered by Locust, after the saved
  // rules (shared/who-decides.ts), so OpenCode is told "once" and keeps asking.
  it('reads "for the rest of the run" as once, Locust keeping the Always, and anything unclear as a refusal', () => {
    expect(openCodeReplyFor({ decision: 'acceptForSession' })).toBe('once')
    expect(openCodeReplyFor({ decision: 'reject' })).toBe('reject')
    expect(openCodeReplyFor(null)).toBe('reject')
    expect(openCodeReplyFor({ decision: 'accept', extra: 1 })).toBe('once')
  })
})
