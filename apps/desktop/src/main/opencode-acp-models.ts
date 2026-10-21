import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import type { AppServerRunProcess, RuntimeDiscovery } from '@teammate/runtime-adapters'

import { modelOptionOf } from '../shared/acp-agents.js'
import type { PublicModel } from '../shared/ipc.js'

const CATALOG_DEADLINE_MS = 20_000
const MAX_OUTPUT = 4 * 1024 * 1024

export interface OpenCodeAcpModelsOptions {
  readonly spawn: (executablePath: string, args: readonly string[], env?: Readonly<Record<string, string>>, cwd?: string) => AppServerRunProcess
  /** A new, disposable empty folder. Never a project or the person's profile. */
  readonly scratch?: () => Promise<string>
}

const object = (value: unknown): Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value) ? value as Record<string, unknown> : {}

/**
 * OpenCode 2 advertises its catalog in session/new, not the V1 models command.
 * The probe offers no files, terminal or MCP servers and sends no prompt.
 * A failed advertisement supplies no rows; V1 hints are never its fallback.
 */
export async function readOpenCodeAcpModels(runtime: RuntimeDiscovery, options: OpenCodeAcpModelsOptions): Promise<readonly PublicModel[]> {
  if (runtime.readiness !== 'ready' || runtime.executable === undefined) return []
  let active = true
  let exited = false
  let child: AppServerRunProcess | undefined
  let cwd: string | undefined
  let buffer = ''
  let received = 0
  let next = 0
  let sessionRequest = 0
  let sessionAnswered = false
  const waiting = new Map<number, { resolve: (result: unknown) => void; reject: (error: Error) => void }>()
  let rejectDeadline!: (error: Error) => void
  const deadline = new Promise<never>((_resolve, reject) => { rejectDeadline = reject })
  const fail = (): void => {
    active = false
    for (const request of waiting.values()) request.reject(new Error('OpenCode model probe stopped'))
    waiting.clear()
    rejectDeadline(new Error('OpenCode model probe stopped'))
  }
  const timer = setTimeout(fail, CATALOG_DEADLINE_MS)
  const cleanup = async (directory: string): Promise<void> => { await rm(directory, { recursive: true, force: true }).catch(() => undefined) }
  const call = (method: string, params: unknown): Promise<unknown> => new Promise((resolve, reject) => {
    if (!active || child === undefined) { reject(new Error('OpenCode model probe stopped')); return }
    const id = ++next
    if (method === 'session/new') sessionRequest = id
    waiting.set(id, { resolve, reject })
    try { child.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`) } catch { fail() }
  })
  const probe = async (): Promise<readonly PublicModel[]> => {
    const directory = await (options.scratch ?? (() => mkdtemp(join(tmpdir(), 'locust-opencode-models-'))))()
    // Scratch creation can finish after the overall deadline. It cannot then
    // start a process, and it still owns this empty folder's cleanup.
    if (!active) { await cleanup(directory); return [] }
    cwd = directory
    const launch = runtime.executable!
    child = options.spawn(launch.executablePath, [...launch.prefixArgs, 'acp'], launch.env, cwd)
    child.onExit(() => { exited = true; if (active && !sessionAnswered) fail() })
    child.onStdoutClose?.(() => { if (active && !sessionAnswered) fail() })
    child.onData((chunk) => {
      if (!active) return
      received += chunk.length
      if (received > MAX_OUTPUT) { fail(); return }
      buffer += chunk
      for (let at = buffer.indexOf('\n'); at >= 0 && active; at = buffer.indexOf('\n')) {
        const line = buffer.slice(0, at).trim()
        buffer = buffer.slice(at + 1)
        if (line.length === 0) continue
        let message: Record<string, unknown>
        try { message = object(JSON.parse(line)) } catch { continue }
        if (typeof message.id === 'number' && waiting.has(message.id) && ('result' in message || 'error' in message)) {
          const request = waiting.get(message.id)!
          waiting.delete(message.id)
          if ('error' in message) request.reject(new Error('OpenCode did not advertise its models'))
          else { if (message.id === sessionRequest) sessionAnswered = true; request.resolve(message.result) }
        } else if ((typeof message.id === 'string' || typeof message.id === 'number') && typeof message.method === 'string') {
          // Even an unexpected permission/filesystem request is refused. This
          // session is a catalog read and cannot authorise work or read files.
          try { child?.write(`${JSON.stringify({ jsonrpc: '2.0', id: message.id, error: { code: -32601, message: 'No client operations are available during model discovery.' } })}\n`) } catch { fail() }
        }
      }
    })
    const initialized = object(await call('initialize', {
      protocolVersion: 1,
      clientCapabilities: { fs: { readTextFile: false, writeTextFile: false }, terminal: false }
    }))
    if (initialized.protocolVersion !== 1) return []
    const session = await call('session/new', { cwd, mcpServers: [] })
    const advertised = modelOptionOf(session)
    const seen = new Set<string>()
    return (advertised?.models ?? []).flatMap((model) => {
      if (seen.has(model.value)) return []
      seen.add(model.value)
      return [{
        id: model.value,
        runtime: 'opencode' as const,
        displayName: model.name,
        description: model.value.endsWith('-free') ? 'Free · no sign-in' : 'Through OpenCode',
        // An ACP effort option applies to the session's current model. It is
        // not a catalog-wide promise and cannot be assigned to every row.
        supportedEfforts: []
      }]
    })
  }
  try {
    return await Promise.race([probe(), deadline])
  } catch {
    return []
  } finally {
    active = false
    clearTimeout(timer)
    waiting.clear()
    if (!exited) {
      try { child?.kill() } catch { /* The result never depends on shutdown succeeding. */ }
    }
    if (cwd !== undefined) await cleanup(cwd)
  }
}
