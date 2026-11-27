import { describe, expect, it, vi } from 'vitest'

import { createCodexAppServerCommand } from './index.js'
import { startCodexAppServerRun, type AppServerRunProcess } from './codex-app-server-run.js'

const CODEX = { commandName: 'codex', discoveredPath: '/usr/bin/codex', executablePath: '/usr/bin/codex', prefixArgs: [], kind: 'native' as const }

// Codex 0.172.0's generated result shapes. These replies come from the run's default handler,
// with no approval channel. The fake app-server records the entire JSON-RPC envelope.
describe('a Codex run with no handler refuses each request in its own shape', () => {
  const turn = { threadId: 'thread_1', turnId: 'turn_1' }
  it.each([
    ['item/tool/requestUserInput', { ...turn, itemId: 'item_1', isBlocking: true, questions: [
      { id: 'q', header: 'Continue', question: 'Continue?', options: null }
    ] }, { result: { answers: {} } }],
    ['mcpServer/elicitation/request', {
      ...turn, serverName: 'example', mode: 'url', message: 'Review this link.', elicitationId: 'elicitation_1', url: 'https://example.com'
    }, { result: { action: 'decline' } }],
    ['item/permissions/requestApproval', {
      ...turn, itemId: 'item_1', startedAtMs: 1, cwd: '/work', permissions: { network: { enabled: true } }
    }, { result: { permissions: {}, scope: 'turn' } }],
    ['item/tool/call', { ...turn, callId: 'call_1', tool: 'lookup', arguments: {} }, { result: { success: false, contentItems: [] } }],
    ['attestation/generate', {}, { error: { code: -32601 } }],
    ['account/chatgptAuthTokens/refresh', { reason: 'unauthorized' }, { error: { code: -32601 } }],
    ['unknown/request', {}, { error: { code: -32601 } }]
  ])('answers %s', async (method, params, expected) => {
    const written: Record<string, unknown>[] = []
    let emit: (chunk: string) => void = () => undefined
    const process: AppServerRunProcess = {
      write: (line) => { for (const part of line.trim().split('\n')) written.push(JSON.parse(part) as Record<string, unknown>) },
      kill: () => undefined,
      onData: (listener) => { emit = listener },
      onExit: () => undefined
    }
    const run = startCodexAppServerRun({
      spawn: () => process,
      command: createCodexAppServerCommand(CODEX, { workspacePath: '/work', sandbox: 'workspace-write' }),
      prompt: 'count the files',
      sandbox: 'workspace-write',
      approvalPolicy: 'never'
    })
    const id = 'server-request'
    emit(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`)
    try {
      await vi.waitFor(() => {
        const response = written.find((message) => message.id === id)
        expect(response).toMatchObject({ jsonrpc: '2.0', id, ...expected })
        expect(Object.hasOwn(response!, 'result')).toBe(Object.hasOwn(expected, 'result'))
      })
    } finally {
      emit(`${JSON.stringify({ jsonrpc: '2.0', method: 'turn/completed', params: {} })}\n`)
      await run.completion
    }
  })
})
