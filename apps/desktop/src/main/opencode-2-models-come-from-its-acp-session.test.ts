import { existsSync } from 'node:fs'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it, vi } from 'vitest'
import type { AppServerRunProcess, RuntimeDiscovery } from '@teammate/runtime-adapters'

import { createModelCatalog, opencodeModelsFrom } from './model-catalog.js'
import { readOpenCodeAcpModels } from './opencode-acp-models.js'

const legacy = { id: 'old/stale-free', displayName: 'Stale', efforts: ['max'] }
const runtime = (major = 2): RuntimeDiscovery => ({
  id: 'opencode', kind: 'agent-runtime', displayName: 'OpenCode', optional: true,
  availability: 'available', readiness: 'ready', diagnostics: [], supportedFeatures: [], requiredFeatures: [],
  version: { raw: `opencode ${major}.1.0`, version: `${major}.1.0`, major, minor: 1, patch: 0 },
  executable: { commandName: 'opencode', discoveredPath: '/bin/opencode', executablePath: '/bin/opencode', prefixArgs: ['shim'], kind: 'native', env: { ONLY: 'inherited' } },
  modelHints: { aliases: [], efforts: [], models: [legacy] }
})
const session = {
  sessionId: 'catalog-only',
  configOptions: [
    { id: 'model', category: 'model', type: 'select', currentValue: 'opencode/zen-free', options: [
      { group: 'OpenCode', options: [{ value: 'opencode/zen-free', name: 'Zen Free' }] },
      { value: 'provider/model', name: 'A paid model' },
      { value: 'opencode/zen-free', name: 'Duplicate' }
    ] },
    { id: 'effort', category: 'thought_level', options: [{ value: 'high', name: 'High' }] }
  ]
}
type Request = { id: number | string; method?: string; params?: Record<string, unknown>; error?: unknown }
function server(answer: unknown = session, onRequest?: (request: Request, server: ReturnType<typeof endpoints>) => boolean) {
  const endpoint = endpoints()
  const messages: Request[] = []
  const child: AppServerRunProcess = {
    write: (line) => {
      const request = JSON.parse(line) as Request
      messages.push(request)
      if (request.method === undefined) return
      if (onRequest?.(request, endpoint) === true) return
      queueMicrotask(() => endpoint.data(`${JSON.stringify({ jsonrpc: '2.0', id: request.id, result: request.method === 'initialize' ? { protocolVersion: 1 } : answer })}\n`))
    },
    kill: vi.fn(),
    onData: (listener) => { endpoint.data = listener },
    onExit: (listener) => { endpoint.exit = listener },
    onStdoutClose: (listener) => { endpoint.close = listener }
  }
  return { child, endpoint, messages }
}
function endpoints() {
  return { data: (_chunk: string): void => undefined, exit: (): void => undefined, close: (_error?: Error): void => undefined }
}
const scratch = async () => mkdtemp(join(tmpdir(), 'locust-catalog-unit-'))

afterEach(() => { vi.useRealTimers() })

describe('OpenCode 2 models come from its ACP session', () => {
  it('opens an empty no-prompt session and lists only its advertised models, including free grouped rows', async () => {
    const fixture = server()
    const spawn = vi.fn(() => fixture.child)
    let directory = ''
    const models = await readOpenCodeAcpModels(runtime(), { spawn, scratch: async () => { directory = await scratch(); return directory } })
    expect(spawn).toHaveBeenCalledWith('/bin/opencode', ['shim', 'acp'], { ONLY: 'inherited' }, directory)
    expect(fixture.messages.map((message) => message.method)).toEqual(['initialize', 'session/new'])
    expect(fixture.messages[0]?.params).toEqual({ protocolVersion: 1, clientCapabilities: { fs: { readTextFile: false, writeTextFile: false }, terminal: false } })
    expect(fixture.messages[1]?.params).toEqual({ cwd: directory, mcpServers: [] })
    expect(models.map((model) => model.id)).toEqual(['opencode/zen-free', 'provider/model'])
    expect(models[0]).toMatchObject({ runtime: 'opencode', displayName: 'Zen Free', description: 'Free · no sign-in' })
    expect(fixture.child.kill).toHaveBeenCalledOnce()
    expect(existsSync(directory)).toBe(false)
  })

  it('does not turn the current session effort menu into a promise about every model', async () => {
    const fixture = server()
    const models = await readOpenCodeAcpModels(runtime(), { spawn: () => fixture.child, scratch })
    expect(models.map((model) => model.supportedEfforts)).toEqual([[], []])
  })

  it('rejects unexpected client operations without reading files or approving permission', async () => {
    const fixture = server(session, (request, endpoint) => {
      if (request.method !== 'session/new') return false
      queueMicrotask(() => {
        endpoint.data(`${JSON.stringify({ jsonrpc: '2.0', id: 'permission', method: 'session/request_permission', params: { options: [{ kind: 'allow_always' }] } })}\n`)
        endpoint.data(`${JSON.stringify({ jsonrpc: '2.0', id: request.id, result: session })}\n`)
      })
      return true
    })
    expect(await readOpenCodeAcpModels(runtime(), { spawn: () => fixture.child, scratch })).toHaveLength(2)
    expect(fixture.messages.find((message) => message.id === 'permission')).toMatchObject({ error: { code: -32601 } })
  })

  it('keeps the V1 CLI list unchanged and never offers its stale hints for V2', () => {
    expect(opencodeModelsFrom([runtime(1)])).toEqual([{ id: legacy.id, runtime: 'opencode', displayName: 'Stale', description: 'Free · no sign-in', supportedEfforts: ['max'] }])
    expect(opencodeModelsFrom([runtime(2)])).toEqual([])
  })

  it('makes the catalog select ACP for V2 and caches a complete advertisement', async () => {
    const fixture = server()
    const spawn = vi.fn(() => fixture.child)
    const discover = vi.fn(async () => [runtime()])
    const catalog = createModelCatalog({ discover, spawn, opencodeScratch: scratch })
    const result = await catalog.read()
    expect(result.ok && result.data.models.map((model) => model.id)).toEqual(['account-default', 'opencode/zen-free', 'provider/model'])
    expect(await catalog.read()).toBe(result)
    expect(discover).toHaveBeenCalledOnce()
    expect(spawn).toHaveBeenCalledOnce()
  })

  it('starts independent catalog probes together instead of serialising their deadlines', async () => {
    let release: (() => void) | undefined
    const acp = server(session, (request, endpoint) => {
      if (request.method !== 'session/new') return false
      release = () => endpoint.data(`${JSON.stringify({ jsonrpc: '2.0', id: request.id, result: session })}\n`)
      return true
    })
    const codex = server({ data: [{ id: 'codex-model', displayName: 'Codex model' }] })
    const spawn = vi.fn((_path: string, args: readonly string[]) => args.includes('acp') ? acp.child : codex.child)
    const catalog = createModelCatalog({ discover: async () => [runtime(), { ...runtime(1), id: 'codex' }], spawn, opencodeScratch: scratch })
    const reading = catalog.read()
    try {
      await vi.waitFor(() => expect(spawn.mock.calls.some(([, args]) => args.includes('app-server'))).toBe(true))
      await vi.waitFor(() => expect(release).toBeTypeOf('function'))
    } finally {
      // A broken serial implementation must not leave a process/promise alive
      // just because the concurrency assertion failed.
      await vi.waitFor(() => expect(release).toBeTypeOf('function'))
      release!()
    }
    const result = await reading
    expect(result.ok && result.data.models.map((model) => model.id)).toContain('codex-model')
    expect(result.ok && result.data.models.map((model) => model.id)).toContain('opencode/zen-free')
  })

  it('keeps V1 discovery-only without opening any ACP process', async () => {
    const spawn = vi.fn(() => { throw new Error('V1 must not open ACP') })
    const catalog = createModelCatalog({ discover: async () => [runtime(1)], spawn })
    const result = await catalog.read()
    expect(result.ok && result.data.models.map((model) => model.id)).toEqual(['account-default', legacy.id])
    expect(spawn).not.toHaveBeenCalled()
  })

  it('retries a failed ACP catalog rather than caching a partial list or falling back to stale hints', async () => {
    let attempts = 0
    const catalog = createModelCatalog({ discover: async () => [runtime()], opencodeScratch: scratch,
      spawn: () => server(undefined, (request, endpoint) => {
        if (request.method !== 'session/new') return false
        attempts += 1
        queueMicrotask(() => endpoint.data(`${JSON.stringify({ jsonrpc: '2.0', id: request.id, ...(attempts === 1 ? { error: { code: -32000 } } : { result: session }) })}\n`))
        return true
      }).child
    })
    expect((await catalog.read()).ok).toBe(false)
    const recovered = await catalog.read()
    expect(recovered.ok && recovered.data.models.some((model) => model.id === legacy.id)).toBe(false)
    expect(recovered.ok && recovered.data.models.some((model) => model.id === 'opencode/zen-free')).toBe(true)
    expect(attempts).toBe(2)
  })

  it.each(['exit', 'close'] as const)('stops immediately when %s arrives while a request is pending', async (event) => {
    const directory = await scratch()
    vi.useFakeTimers()
    const fixture = server(undefined, (_request, endpoint) => { queueMicrotask(() => endpoint[event]()); return true })
    const reading = readOpenCodeAcpModels(runtime(), { spawn: () => fixture.child, scratch: async () => directory })
    await vi.advanceTimersByTimeAsync(0)
    expect(fixture.messages).toHaveLength(1)
    expect(fixture.child.kill).toHaveBeenCalledTimes(event === 'close' ? 1 : 0)
    expect(await reading).toEqual([])
  })

  it('accepts a complete session reply in the final output chunk before stdout closes', async () => {
    const fixture = server(session, (request, endpoint) => {
      if (request.method !== 'session/new') return false
      queueMicrotask(() => {
        endpoint.data(`${JSON.stringify({ jsonrpc: '2.0', id: request.id, result: session })}\n`)
        endpoint.close()
      })
      return true
    })
    expect(await readOpenCodeAcpModels(runtime(), { spawn: () => fixture.child, scratch })).toHaveLength(2)
  })

  it('bounds the entire handshake and session read with one deadline', async () => {
    const directory = await scratch()
    vi.useFakeTimers()
    const fixture = server(session, (request, endpoint) => {
      if (request.method === 'initialize') {
        setTimeout(() => endpoint.data(`${JSON.stringify({ jsonrpc: '2.0', id: request.id, result: { protocolVersion: 1 } })}\n`), 15_000)
      }
      return true
    })
    const reading = readOpenCodeAcpModels(runtime(), { spawn: () => fixture.child, scratch: async () => directory })
    await vi.advanceTimersByTimeAsync(19_999)
    expect(fixture.messages.map((message) => message.method)).toEqual(['initialize', 'session/new'])
    expect(fixture.child.kill).not.toHaveBeenCalled()
    await vi.advanceTimersByTimeAsync(1)
    expect(await reading).toEqual([])
    expect(fixture.child.kill).toHaveBeenCalledOnce()
    expect(vi.getTimerCount()).toBe(0)
  })

  it('refuses an unbounded output stream and stops its process', async () => {
    const fixture = server(undefined, (_request, endpoint) => { queueMicrotask(() => endpoint.data('x'.repeat(4 * 1024 * 1024 + 1))); return true })
    expect(await readOpenCodeAcpModels(runtime(), { spawn: () => fixture.child, scratch })).toEqual([])
    expect(fixture.child.kill).toHaveBeenCalledOnce()
  })

  it('ignores malformed JSON and accepts split frames without corrupting the catalog', async () => {
    const fixture = server(session, (request, endpoint) => {
      if (request.method !== 'session/new') return false
      const line = `${JSON.stringify({ jsonrpc: '2.0', id: request.id, result: session })}\n`
      queueMicrotask(() => { endpoint.data('not-json\n'); endpoint.data(line.slice(0, 17)); endpoint.data(line.slice(17)) })
      return true
    })
    expect(await readOpenCodeAcpModels(runtime(), { spawn: () => fixture.child, scratch })).toHaveLength(2)
  })

  it('cleans up its scratch directory when process launch fails', async () => {
    let directory = ''
    expect(await readOpenCodeAcpModels(runtime(), { spawn: () => { throw new Error('missing binary') }, scratch: async () => { directory = await scratch(); return directory } })).toEqual([])
    expect(existsSync(directory)).toBe(false)
  })

  it('does not open a session on a different ACP protocol version', async () => {
    const fixture = server(undefined, (request, endpoint) => { queueMicrotask(() => endpoint.data(`${JSON.stringify({ jsonrpc: '2.0', id: request.id, result: { protocolVersion: 99 } })}\n`)); return true })
    expect(await readOpenCodeAcpModels(runtime(), { spawn: () => fixture.child, scratch })).toEqual([])
    expect(fixture.messages.map((message) => message.method)).toEqual(['initialize'])
    expect(fixture.child.kill).toHaveBeenCalledOnce()
  })

  it('contains failed writes without leaving a pending catalog read', async () => {
    const fixture = server()
    fixture.child.write = () => { throw new Error('pipe closed') }
    expect(await readOpenCodeAcpModels(runtime(), { spawn: () => fixture.child, scratch })).toEqual([])
    expect(fixture.child.kill).toHaveBeenCalledOnce()
  })

  it('does not invent models when a session has no model option', async () => {
    const fixture = server({ sessionId: 'no-models', configOptions: [{ id: 'mode', options: [{ value: 'build', name: 'Build' }] }] })
    expect(await readOpenCodeAcpModels(runtime(), { spawn: () => fixture.child, scratch })).toEqual([])
    expect(fixture.child.kill).toHaveBeenCalledOnce()
  })

  it('cannot launch late when scratch creation outlives the total deadline', async () => {
    const directory = await scratch()
    let ready!: (directory: string) => void
    vi.useFakeTimers()
    const spawn = vi.fn(() => server().child)
    const reading = readOpenCodeAcpModels(runtime(), { spawn, scratch: () => new Promise((resolve) => { ready = resolve }) })
    await vi.advanceTimersByTimeAsync(20_000)
    expect(await reading).toEqual([])
    ready(directory)
    vi.useRealTimers()
    await vi.waitFor(() => expect(existsSync(directory)).toBe(false))
    expect(spawn).not.toHaveBeenCalled()
  })

  it('contains a failed permission-refusal write and stops the process', async () => {
    const fixture = server(session, (request, endpoint) => {
      if (request.method !== 'session/new') return false
      queueMicrotask(() => endpoint.data(`${JSON.stringify({ jsonrpc: '2.0', id: 'ask', method: 'fs/read_text_file' })}\n`))
      return true
    })
    const write = fixture.child.write
    fixture.child.write = (line) => {
      if ((JSON.parse(line) as Request).id === 'ask') throw new Error('refusal pipe closed')
      write(line)
    }
    expect(await readOpenCodeAcpModels(runtime(), { spawn: () => fixture.child, scratch })).toEqual([])
    expect(fixture.child.kill).toHaveBeenCalledOnce()
  })

  it('keeps a valid advertisement even when terminating its process throws', async () => {
    const fixture = server()
    fixture.child.kill = vi.fn(() => { throw new Error('process already gone') })
    expect(await readOpenCodeAcpModels(runtime(), { spawn: () => fixture.child, scratch })).toHaveLength(2)
    expect(fixture.child.kill).toHaveBeenCalledOnce()
  })
})
