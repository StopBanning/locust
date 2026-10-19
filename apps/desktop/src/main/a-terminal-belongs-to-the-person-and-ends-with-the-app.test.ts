import { afterEach, describe, expect, it, vi } from 'vitest'
import { fileURLToPath } from 'node:url'
import type { IPty } from '@lydell/node-pty'
import { createTerminalHost, endTerminalTree } from './terminal-host.js'
import { terminalFolder } from './terminal-folder.js'
import { parseOrganizationPolicy } from './organization-policy.js'
import { policyTerminalRefusal } from '../shared/organization-policy.js'

const shell = { id: 'test', name: 'Test shell', file: process.execPath, args: [] }
const folder = fileURLToPath(new URL('.', import.meta.url))
function fixture(extra: Partial<Parameters<typeof createTerminalHost>[0]> = {}) {
  let data: (data: string) => void = () => undefined
  let exit: (event: { exitCode: number; signal?: number }) => void = () => undefined
  const pty = { pid: 123, write: vi.fn(), resize: vi.fn(), kill: vi.fn(), pause: vi.fn(), resume: vi.fn(),
    onData: vi.fn(fn => { data = fn; return { dispose() {} } }),
    onExit: vi.fn(fn => { exit = fn; return { dispose() {} } }) } as unknown as IPty
  const spawn = vi.fn(async () => pty)
  const end = vi.fn(async () => undefined)
  const emit = vi.fn()
  const host = createTerminalHost({ folder: async () => folder, refusal: () => undefined, shells: () => [shell], spawn, end, emit, ...extra })
  return { host, pty, spawn, end, emit, data: (text: string) => data(text), exit: () => exit({ exitCode: 0 }) }
}
afterEach(() => vi.useRealTimers())

describe('a terminal owned by the person', () => {
  it('closes Unix foreground and background descendants before ending their shell', async () => {
    const kill = vi.fn()
    const pty = { pid: 10, kill: vi.fn() }
    await endTerminalTree(pty, { platform: 'linux', processes: async () => '10 1\n20 10\n30 20\n40 10\n99 1\ninvalid\n1 0', kill })
    expect(new Set(kill.mock.calls.filter(([pid]) => pid > 0).map(([pid]) => pid))).toEqual(new Set([20, 30, 40]))
    expect(kill).toHaveBeenLastCalledWith(-10, 'SIGKILL')
    expect(kill).not.toHaveBeenCalledWith(99, 'SIGKILL')
    expect(pty.kill).not.toHaveBeenCalled()
  })
  it('still ends a Unix shell when its process group has already gone', async () => {
    const pty = { pid: 10, kill: vi.fn() }
    await endTerminalTree(pty, { platform: 'darwin', processes: async () => '', kill: () => { throw new Error('ESRCH') } })
    expect(pty.kill).toHaveBeenCalledExactlyOnceWith('SIGKILL')
  })
  it('starts only when requested, in the host-resolved conversation folder, with the person’s environment', async () => {
    const f = fixture()
    expect(f.spawn).not.toHaveBeenCalled()
    const tab = await f.host.create({ missionId: 'mission_1' })
    expect(tab.folder).toBe(folder)
    expect(f.spawn).toHaveBeenCalledWith(process.execPath, [], expect.objectContaining({ cwd: folder, env: { ...process.env, TERM: 'xterm-256color' } }))
    await f.host.dispose()
  })
  it('refuses an unknown folder or shell before spawning', async () => {
    const f = fixture({ folder: async () => undefined })
    await expect(f.host.create({})).rejects.toThrow('Choose a project folder')
    expect(f.spawn).not.toHaveBeenCalled()
    const other = fixture()
    await expect(other.host.create({}, 'invented.exe')).rejects.toThrow('unavailable')
    expect(other.spawn).not.toHaveBeenCalled()
  })
  it('closing a tab ends only that shell and quitting ends every remaining shell', async () => {
    const f = fixture()
    const first = await f.host.create({})
    await f.host.create({})
    await f.host.close(first.id)
    expect(f.end).toHaveBeenCalledTimes(1)
    await f.host.dispose()
    expect(f.end).toHaveBeenCalledTimes(2)
    await expect(f.host.create({})).rejects.toThrow()
  })
  it('a quit during an unfinished spawn ends the late shell too', async () => {
    let finish!: (pty: IPty) => void
    const f = fixture({ spawn: () => new Promise(resolve => { finish = resolve }) })
    const creating = f.host.create({})
    await vi.waitFor(() => expect(finish).toBeTypeOf('function'))
    const rejected = expect(creating).rejects.toThrow('closing')
    const quitting = f.host.dispose()
    finish(f.pty)
    await Promise.all([rejected, quitting])
    expect(f.end).toHaveBeenCalledWith(f.pty)
  })
  it('keeps output in frame-sized batches and flushes the last text before exit', async () => {
    const f = fixture()
    const tab = await f.host.create({})
    vi.useFakeTimers()
    f.data('hello'); f.data(' world')
    expect(f.emit).not.toHaveBeenCalled()
    await vi.advanceTimersByTimeAsync(16)
    expect(f.emit).toHaveBeenCalledExactlyOnceWith({ kind: 'data', id: tab.id, data: 'hello world' })
    f.data('last'); f.exit()
    expect(f.emit.mock.calls.slice(-2).map(call => call[0])).toEqual([{ kind: 'data', id: tab.id, data: 'last' }, { kind: 'exit', id: tab.id, exitCode: 0 }])
    await f.host.close(tab.id)
    expect(f.end).not.toHaveBeenCalled()
  })
  it('pauses a flooding shell and routes input and geometry only to a live tab', async () => {
    const f = fixture()
    const tab = await f.host.create({})
    vi.useFakeTimers()
    f.data('x'.repeat(256 * 1024))
    expect(f.pty.pause).toHaveBeenCalledTimes(1)
    await vi.advanceTimersByTimeAsync(16)
    expect(f.pty.resume).toHaveBeenCalledTimes(1)
    f.host.input(tab.id, '\x03'); f.host.resize(tab.id, 90, 30)
    expect(f.pty.write).toHaveBeenCalledWith('\x03')
    expect(f.pty.resize).toHaveBeenCalledWith(90, 30)
    await f.host.close(tab.id)
    f.host.input(tab.id, 'after'); f.host.resize(tab.id, 80, 24)
    expect(f.pty.write).toHaveBeenCalledTimes(1)
    expect(f.pty.resize).toHaveBeenCalledTimes(1)
  })
  it('limits concurrent opens even while their folders are still being resolved', async () => {
    let finish!: (folder: string) => void
    const pending = new Promise<string>(resolve => { finish = resolve })
    const f = fixture({ folder: () => pending })
    const opening = Array.from({ length: 8 }, () => f.host.create({}))
    await expect(f.host.create({})).rejects.toThrow('Close a terminal tab')
    finish(folder)
    await Promise.all(opening)
    expect(f.spawn).toHaveBeenCalledTimes(8)
    await f.host.dispose()
  })
  it('an organization terminal restriction refuses even a direct host request', async () => {
    const state = { path: '/etc/locust/policy.json', present: true, policy: parseOrganizationPolicy({ terminal: false }) }
    const f = fixture({ refusal: () => policyTerminalRefusal(state) })
    await expect(f.host.create({})).rejects.toThrow('Set by your organization')
    expect(() => f.host.shells()).toThrow('Set by your organization')
    expect(f.spawn).not.toHaveBeenCalled()
    expect(() => parseOrganizationPolicy({ terminal: 'false' })).toThrow('terminal')
    expect(policyTerminalRefusal({ ...state, policy: { terminal: true } })).toBeUndefined()
  })
  it('an unrestricted terminal cannot bypass other organization restrictions', () => {
    for (const policy of [{ highestMode: 'ask' as const }, { allowedAgents: [] }, { allowedModels: {} }, { monthlyLimitUsd: 0 }]) {
      expect(policyTerminalRefusal({ path: '/policy', present: true, policy })).toContain('Set by your organization')
    }
    expect(policyTerminalRefusal({ path: '/policy', present: true, policy: {}, error: 'Invalid organization policy' })).toBe('Invalid organization policy')
  })
})

describe('a terminal’s folder comes from Locust’s own state', () => {
  const options = {
    current: () => '/project', mission: async () => ({ workspaceId: 'ws_other' }), folderOf: async () => '/conversation',
    ownerOf: async () => 'alice', teammates: async () => [{ teammateId: 'alice', worktree: true }],
    trees: vi.fn(async () => [{ teammateId: 'alice', path: '/conversation/.locust/worktrees/alice' }])
  }
  it('uses the current project before a first message and a conversation’s own worktree afterwards', async () => {
    expect(await terminalFolder({}, options)).toBe('/project')
    expect(await terminalFolder({ missionId: 'mission_1' }, options)).toBe('/conversation/.locust/worktrees/alice')
    expect(options.trees).toHaveBeenLastCalledWith('/conversation')
    expect(await terminalFolder({ missionId: 'mission_1' }, { ...options, teammates: async () => [{ teammateId: 'alice' }] })).toBe('/conversation')
  })
  it('refuses unknown conversations, teammates and worktrees instead of guessing a folder', async () => {
    expect(await terminalFolder({ missionId: 'unknown' }, { ...options, mission: async () => undefined })).toBeUndefined()
    expect(await terminalFolder({ teammateId: 'unknown' }, options)).toBeUndefined()
    expect(await terminalFolder({ teammateId: 'alice' }, { ...options, trees: async () => [] })).toBeUndefined()
  })
})
