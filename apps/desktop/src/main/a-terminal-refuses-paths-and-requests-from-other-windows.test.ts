import { beforeEach, expect, it, vi } from 'vitest'
const { handlers, openExternal, writeText } = vi.hoisted(() => ({ handlers: new Map<string, (...args: any[]) => any>(), openExternal: vi.fn(), writeText: vi.fn() }))
vi.mock('electron', () => ({ ipcMain: { handle: (channel: string, fn: (...args: any[]) => any) => handlers.set(channel, fn) }, shell: { openExternal }, clipboard: { writeText } }))
import { registerTerminalIpc } from './terminal-ipc.js'
import { TERMINAL_CREATE, TERMINAL_INPUT, TERMINAL_RESIZE, TERMINAL_CLOSE, TERMINAL_LINK, TERMINAL_COPY } from '../shared/terminal.js'

const host = { create: vi.fn(async () => ({ id: 'tab' })), shells: vi.fn(() => []), input: vi.fn(), resize: vi.fn(), close: vi.fn(), dispose: vi.fn() }
beforeEach(() => { vi.clearAllMocks(); registerTerminalIpc(host as any, event => (event as any).own === true) })
it('refuses renderer-invented paths and executable names, and every request from another window', async () => {
  const create = handlers.get(TERMINAL_CREATE)!
  expect((await create({ own: true }, { cwd: '/elsewhere' })).ok).toBe(false)
  expect((await create({ own: true }, { missionId: 'known', file: 'evil.exe' })).ok).toBe(false)
  expect((await create({ own: false }, {})).ok).toBe(false)
  expect(host.create).not.toHaveBeenCalled()
  expect((await create({ own: true }, { missionId: 'known' }, 'powershell')).ok).toBe(true)
  expect(host.create).toHaveBeenCalledWith({ missionId: 'known' }, 'powershell')
})
it('bounds input and geometry and ignores unowned input, closes and resizes', async () => {
  const id = 'a'.repeat(36)
  handlers.get(TERMINAL_INPUT)!({ own: false }, id, 'hi')
  handlers.get(TERMINAL_CLOSE)!({ own: false }, id)
  handlers.get(TERMINAL_RESIZE)!({ own: false }, id, 80, 24)
  handlers.get(TERMINAL_INPUT)!({ own: true }, id, 'x'.repeat(65537))
  handlers.get(TERMINAL_RESIZE)!({ own: true }, id, Infinity, 24)
  expect(host.input).not.toHaveBeenCalled(); expect(host.close).not.toHaveBeenCalled(); expect(host.resize).not.toHaveBeenCalled()
  handlers.get(TERMINAL_INPUT)!({ own: true }, id, 'hello')
  handlers.get(TERMINAL_RESIZE)!({ own: true }, id, 100, 30)
  expect(host.input).toHaveBeenCalledWith(id, 'hello'); expect(host.resize).toHaveBeenCalledWith(id, 100, 30)
})
it('opens only HTTP and HTTPS terminal links in the person’s browser', async () => {
  for (const url of ['file:///etc/passwd', 'javascript:alert(1)', 'locust://hello', 'not a url']) expect((await handlers.get(TERMINAL_LINK)!({ own: true }, url)).ok).toBe(false)
  await handlers.get(TERMINAL_LINK)!({ own: false }, 'https://example.com')
  expect(openExternal).not.toHaveBeenCalled()
  await handlers.get(TERMINAL_LINK)!({ own: true }, 'https://example.com')
  expect(openExternal).toHaveBeenCalledExactlyOnceWith('https://example.com')
})
it('copies selected terminal text through the host without granting clipboard reads', async () => {
  expect((await handlers.get(TERMINAL_COPY)!({ own: false }, 'selected')).ok).toBe(false)
  expect(writeText).not.toHaveBeenCalled()
  expect((await handlers.get(TERMINAL_COPY)!({ own: true }, 'selected')).ok).toBe(true)
  expect(writeText).toHaveBeenCalledExactlyOnceWith('selected')
})
