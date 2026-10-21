import { ChildProcess } from 'node:child_process'
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

import { afterEach, describe, expect, it, vi } from 'vitest'

import { stoppedBecause } from '../../resources/locust-command-guard.mjs'
import { openCode2CommandGuardPlugin, openCodeCommandGuardPlugin, writeOpenCode2CommandGuard, writeOpenCodeCommandGuard } from './command-guard.js'

type Hook = (event: { readonly tool?: string; readonly input?: { readonly command?: unknown }; readonly command?: unknown }) => Promise<void>
type GuardOptions = Parameters<typeof openCode2CommandGuardPlugin>[0]
const GUARD = fileURLToPath(new URL('../../resources/locust-command-guard.mjs', import.meta.url))
const folders: string[] = []
const folder = async (): Promise<string> => {
  const value = await mkdtemp(join(tmpdir(), 'locust-opencode-2-guard-'))
  folders.push(value)
  return value
}

/** Load the actual generated ESM plugin and register the two public V2 hooks. */
const loaded = async (options: Partial<GuardOptions> = {}): Promise<{ readonly tool: Hook; readonly shell: Hook }> => {
  const target = await folder()
  const pluginPath = await writeOpenCode2CommandGuard(target, { node: process.execPath, guardPath: GUARD, parent: process.pid, ...options })
  const module = await import(pathToFileURL(join(fileURLToPath(pluginPath!), 'index.js')).href) as { default: { readonly id: string; setup(context: unknown): Promise<void> } }
  expect(module.default.id).toBe('locust.command-guard')
  const hooks = new Map<string, Hook>()
  await module.default.setup({
    tool: { hook: async (name: string, hook: Hook) => hooks.set(`tool.${name}`, hook) },
    shell: { hook: async (name: string, hook: Hook) => hooks.set(`shell.${name}`, hook) }
  })
  expect([...hooks.keys()]).toEqual(['tool.execute.before', 'shell.create.before'])
  return { tool: hooks.get('tool.execute.before')!, shell: hooks.get('shell.create.before')! }
}

const fakeGuard = async (source: string): Promise<string> => {
  const path = join(await folder(), 'fake-guard.mjs')
  await writeFile(path, source, 'utf8')
  return path
}

afterEach(async () => {
  vi.useRealTimers()
  vi.restoreAllMocks()
  await Promise.all(folders.splice(0).map((target) => rm(target, { recursive: true, force: true })))
})

describe('an OpenCode 2 teammate keeps the existing command guard', () => {
  it('writes a separate V2 entrypoint while leaving the V1 plugin unchanged', async () => {
    const target = await folder()
    const options = { node: process.execPath, guardPath: GUARD, parent: process.pid }
    const v1 = await writeOpenCodeCommandGuard(target, options)
    const v1Before = await readFile(fileURLToPath(v1!), 'utf8')
    const v2 = await writeOpenCode2CommandGuard(target, options)
    expect(v2).toBe(pathToFileURL(join(target, 'opencode-2-command-guard')).href)
    expect(v2).not.toBe(v1)
    expect(await readFile(fileURLToPath(v1!), 'utf8')).toBe(v1Before)
    expect(v1Before).toBe(openCodeCommandGuardPlugin(options))
    expect(await readFile(join(fileURLToPath(v2!), 'index.js'), 'utf8')).toBe(openCode2CommandGuardPlugin(options))
    expect(JSON.parse(await readFile(join(fileURLToPath(v2!), 'package.json'), 'utf8'))).toEqual({ name: 'locust-command-guard', private: true, type: 'module', main: './index.js', exports: './index.js' })
    const obstacle = join(target, 'a-file')
    await writeFile(obstacle, 'already a file', 'utf8')
    expect(await writeOpenCode2CommandGuard(obstacle, options)).toBeUndefined()
  })

  it('denies a browser termination before either a bash tool or a shell starts and permits harmless commands', async () => {
    const hooks = await loaded()
    const command = 'taskkill /F /IM msedge.exe'
    const reason = stoppedBecause(command, () => undefined)!
    await expect(hooks.tool({ tool: 'bash', input: { command } })).rejects.toThrow(reason)
    await expect(hooks.tool({ tool: 'shell', input: { command } })).rejects.toThrow(reason)
    await expect(hooks.shell({ command })).rejects.toThrow(reason)
    await expect(hooks.tool({ tool: 'bash', input: { command: 'echo hello' } })).resolves.toBeUndefined()
    await expect(hooks.tool({ tool: 'shell', input: { command: 'echo hello' } })).resolves.toBeUndefined()
    await expect(hooks.shell({ command: 'echo hello' })).resolves.toBeUndefined()
    await expect(hooks.tool({ tool: 'edit', input: { command } })).resolves.toBeUndefined()
    await expect(hooks.tool({ tool: 'bash', input: { command: 17 } })).resolves.toBeUndefined()
    await expect(hooks.tool({ tool: 'bash' })).resolves.toBeUndefined()
    await expect(hooks.tool({ tool: 'shell' })).resolves.toBeUndefined()
  }, 30_000)

  it('passes the hook envelope and Locust parent to the same guard without running the requested command', async () => {
    const trace = join(await folder(), 'guard-input.json')
    const guardPath = await fakeGuard([
      "import { writeFileSync } from 'node:fs'",
      "let input = ''; for await (const chunk of process.stdin) input += chunk",
      `writeFileSync(${JSON.stringify(trace)}, JSON.stringify({ input: JSON.parse(input), parent: process.env.LOCUST_GUARD_PARENT, electron: process.env.ELECTRON_RUN_AS_NODE, log: process.env.LOCUST_GUARD_LOG }))`,
      "process.stdout.write(JSON.stringify({ hookSpecificOutput: { permissionDecision: 'allow' } }))"
    ].join('\n'))
    const hooks = await loaded({ guardPath, parent: 321, log: 'a guard log path' })
    await expect(hooks.tool({ tool: 'bash', input: { command: 'echo fake only' } })).resolves.toBeUndefined()
    expect(JSON.parse(await readFile(trace, 'utf8'))).toEqual({
      input: { hook_event_name: 'PreToolUse', tool_name: 'Bash', tool_input: { command: 'echo fake only' } },
      parent: '321', electron: '1', log: 'a guard log path'
    })
    const invalid = await loaded({ guardPath, parent: Number.NaN })
    await expect(invalid.shell({ command: 'echo invalid parent' })).resolves.toBeUndefined()
    expect(JSON.parse(await readFile(trace, 'utf8')).parent).toBe('0')
  })

  it('fails open promptly when the guard cannot start or returns malformed output', async () => {
    const missing = await loaded({ node: join(await folder(), 'missing-node.exe') })
    const timer = vi.spyOn(globalThis, 'clearTimeout')
    await expect(missing.tool({ tool: 'bash', input: { command: 'echo missing' } })).resolves.toBeUndefined()
    expect(timer).toHaveBeenCalled()
    const malformed = await loaded({ guardPath: await fakeGuard("process.stdout.write('not JSON')") })
    await expect(malformed.shell({ command: 'echo malformed' })).resolves.toBeUndefined()
    const noReason = await loaded({ guardPath: await fakeGuard("process.stdout.write(JSON.stringify({ hookSpecificOutput: { permissionDecision: 'deny' } }))") })
    await expect(noReason.shell({ command: 'echo incomplete' })).resolves.toBeUndefined()
  })

  it('bounds guard output and kills an overflowing child instead of accumulating it', async () => {
    const guardPath = await fakeGuard("process.stdout.write(' '.repeat(65 * 1024) + JSON.stringify({ hookSpecificOutput: { permissionDecision: 'deny', permissionDecisionReason: 'oversized refusal' } }))")
    const hooks = await loaded({ guardPath })
    const kill = vi.spyOn(ChildProcess.prototype, 'kill')
    await expect(hooks.shell({ command: 'echo oversized' })).resolves.toBeUndefined()
    expect(kill).toHaveBeenCalledOnce()
  })

  it('bounds a stalled guard to fifteen seconds and releases the child once', async () => {
    const guardPath = await fakeGuard('setInterval(() => {}, 1000)')
    const hooks = await loaded({ guardPath })
    vi.useFakeTimers()
    const kill = vi.spyOn(ChildProcess.prototype, 'kill')
    let settled = false
    const pending = hooks.shell({ command: 'echo stalled' }).then(() => { settled = true })
    await vi.advanceTimersByTimeAsync(14_999)
    expect(settled).toBe(false)
    await vi.advanceTimersByTimeAsync(1)
    await expect(pending).resolves.toBeUndefined()
    expect(kill).toHaveBeenCalledOnce()
    expect(vi.getTimerCount()).toBe(0)
  })
})
