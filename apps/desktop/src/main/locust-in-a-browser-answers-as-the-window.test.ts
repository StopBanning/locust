import { describe, expect, it, vi } from 'vitest'

import { deskDecode, deskEncode, type DeskTell } from '../shared/desk.js'
import { WINDOW_ONLY_SAYS } from '../shared/desk-calls.js'
import { CODEX_MISSION_START_CHANNEL, CODEX_MISSION_UPDATE_CHANNEL, MISSION_APPROVAL_CHANNEL, MISSION_HISTORY_CHANNEL } from '../shared/ipc.js'
import { FOLDER_PULL_REQUEST_CHANNEL } from '../shared/pull-request.js'
import { GITHUB_SIGN_IN_CODE_CHANNEL } from '../shared/github-account.js'
import { TERMINAL_CREATE } from '../shared/terminal.js'
import { createDeskHost, keepHandlers, type DeskWindow, type IpcHandler } from './desk-host.js'

/**
 * LOCUST IN A BROWSER ANSWERS AS THE WINDOW (0.830, desk-host.ts). A browser that has the window is answered by the
 * very handler the window's own call reaches, made as if from the window; what stays on this computer is refused in
 * words; only one window acts at a time, and the others are told; what the window is told, the browser that has it
 * is told too -- but never what it may not hear.
 */
function aDesk(handlers: Record<string, IpcHandler> = {}) {
  const told: { device: string; items: DeskTell[] }[] = []
  const covers: boolean[] = []
  const sender = { mainFrame: { parent: null }, isDestroyed: () => false, getProcessId: () => 7 }
  const window: DeskWindow = { isDestroyed: () => false, webContents: sender }
  const waiting: unknown[] = []
  const flushes: (() => void)[] = []
  const desk = createDeskHost({
    handlers: new Map(Object.entries(handlers)),
    window: () => window,
    tell: async (device, items) => { told.push({ device, items: [...items] }); return true },
    cover: async (on) => { covers.push(on) },
    hello: async () => ({ version: '0.830.0', computer: 'DESK-PC', theme: 'dark' }),
    waiting: () => waiting,
    later: (task) => { flushes.push(task); return flushes.length }
  })
  /** Every answer and update sent so far, flushed. */
  const flush = async (): Promise<DeskTell[]> => {
    await new Promise((done) => setTimeout(done, 0))
    while (flushes.length > 0) flushes.shift()!()
    await new Promise((done) => setTimeout(done, 0))
    return told.flatMap((one) => one.items)
  }
  const to = (device: string): DeskTell[] => told.filter((one) => one.device === device).flatMap((one) => one.items)
  return { desk, told, covers, sender, waiting, flush, to }
}
const call = (id: string, channel: string, args: unknown[] = []) => ({ kind: 'call' as const, id, channel, args: deskEncode(args) })
const answer = (items: readonly DeskTell[], id: string): DeskTell | undefined => items.find((item) => item.kind === 'res' && item.id === id)

describe('Locust in a browser answers as the window', () => {
  it('says what the computer is, and who has the window', async () => {
    const d = aDesk()
    await d.desk.handle('phone1', [{ kind: 'hello', id: 'h' }])
    const said = answer(await d.flush(), 'h')
    expect(said?.kind === 'res' && said.ok ? deskDecode(said.value) : undefined).toEqual({ version: '0.830.0', computer: 'DESK-PC', theme: 'dark', active: 'computer' })
  })

  it('answers a call with the window’s own handler, made as from the window, once it has the window', async () => {
    const start = vi.fn(async (event: unknown, request: unknown) => ({ ok: true, missionId: 'm1', sawSender: (event as { sender: unknown }).sender, request }))
    const d = aDesk({ [CODEX_MISSION_START_CHANNEL]: start, [MISSION_HISTORY_CHANNEL]: async () => ({ ok: true, missions: [] }), [FOLDER_PULL_REQUEST_CHANNEL]: async () => undefined })
    // Reading needs no window of its own; acting does.
    await d.desk.handle('laptop', [call('r1', MISSION_HISTORY_CHANNEL), call('a1', CODEX_MISSION_START_CHANNEL, [{ prompt: 'hi' }])])
    let items = await d.flush()
    expect(answer(items, 'r1')).toMatchObject({ ok: true })
    expect(answer(items, 'a1')).toEqual({ kind: 'res', id: 'a1', ok: false, refused: true, message: 'Locust is open somewhere else now. Open it here again to do that.' })
    expect(start).not.toHaveBeenCalled()

    await d.desk.handle('laptop', [{ kind: 'take', id: 't' }, call('a2', CODEX_MISSION_START_CHANNEL, [{ prompt: 'hi' }]), call('u', FOLDER_PULL_REQUEST_CHANNEL)])
    items = await d.flush()
    expect(d.covers).toEqual([true])
    expect(start).toHaveBeenCalledTimes(1)
    expect((start.mock.calls[0]![0] as { sender: unknown; senderFrame: unknown }).sender).toBe(d.sender)
    expect((start.mock.calls[0]![0] as { senderFrame: unknown }).senderFrame).toBe(d.sender.mainFrame)
    const started = answer(items, 'a2')
    expect(started?.kind === 'res' && started.ok ? deskDecode(started.value) : undefined).toMatchObject({ ok: true, missionId: 'm1', request: { prompt: 'hi' } })
    // An answer of nothing is nothing, not null.
    const none = answer(items, 'u')
    expect(none?.kind === 'res' && none.ok ? deskDecode(none.value) : 'missing').toBeUndefined()
  })

  it('refuses in words what stays on this computer, a call it does not know, and says a handler’s own failure', async () => {
    const terminal = vi.fn()
    const d = aDesk({ [TERMINAL_CREATE]: terminal, [MISSION_HISTORY_CHANNEL]: async () => { throw new Error('The ledger could not be read.') } })
    await d.desk.handle('laptop', [{ kind: 'take', id: 't' }, call('w', TERMINAL_CREATE, [{}]), call('x', 'no:such-call'), call('e', MISSION_HISTORY_CHANNEL)])
    const items = await d.flush()
    expect(answer(items, 'w')).toEqual({ kind: 'res', id: 'w', ok: false, refused: true, message: WINDOW_ONLY_SAYS })
    expect(terminal).not.toHaveBeenCalled()
    expect(answer(items, 'x')).toMatchObject({ ok: false, refused: true })
    expect(answer(items, 'e')).toEqual({ kind: 'res', id: 'e', ok: false, message: 'The ledger could not be read.' })
  })

  it('tells the browser that has the window what the window is told, and nothing it may not hear', async () => {
    const d = aDesk()
    d.desk.mirror(CODEX_MISSION_UPDATE_CHANNEL, [{ kind: 'event' }])
    await d.flush()
    expect(d.told).toEqual([])
    await d.desk.handle('laptop', [{ kind: 'take', id: 't' }])
    d.desk.mirror(CODEX_MISSION_UPDATE_CHANNEL, [{ kind: 'event', n: 1 }])
    d.desk.mirror(GITHUB_SIGN_IN_CODE_CHANNEL, [{ code: 'ABCD-1234' }])
    const items = await d.flush()
    const events = items.filter((item) => item.kind === 'ev')
    expect(events).toEqual([{ kind: 'ev', channel: CODEX_MISSION_UPDATE_CHANNEL, args: deskEncode([{ kind: 'event', n: 1 }]) }])
    expect(JSON.stringify(items)).not.toContain('ABCD-1234')
  })

  it('lets one window act at a time: another browser takes it, this computer takes it back, each is told', async () => {
    const d = aDesk()
    await d.desk.handle('laptop', [{ kind: 'take', id: 't1' }])
    await d.desk.handle('tablet', [{ kind: 'take', id: 't2' }])
    await d.flush()
    expect(d.to('laptop')).toContainEqual({ kind: 'taken', by: 'browser' })
    expect(d.desk.active()).toBe('tablet')
    // Still covered: the computer's window never came back in between.
    expect(d.covers).toEqual([true])
    await d.desk.takeBack()
    await d.flush()
    expect(d.to('tablet')).toContainEqual({ kind: 'taken', by: 'computer' })
    expect(d.covers).toEqual([true, false])
    expect(d.desk.active()).toBeUndefined()
  })

  it('lets the window go when the browser that has it may no longer have it', async () => {
    const d = aDesk()
    await d.desk.handle('laptop', [{ kind: 'take', id: 't' }])
    await d.desk.forget('laptop')
    expect(d.desk.active()).toBeUndefined()
    expect(d.covers).toEqual([true, false])
  })

  it('gives a window just opened the approval cards already waiting', async () => {
    const d = aDesk()
    d.waiting.push({ approvalId: 'ap1', summary: 'npm test' })
    await d.desk.handle('laptop', [{ kind: 'replay', id: 'r0', what: 'approvals' }])
    expect((await d.flush()).filter((item) => item.kind === 'ev')).toEqual([])
    await d.desk.handle('laptop', [{ kind: 'take', id: 't' }, { kind: 'replay', id: 'r1', what: 'approvals' }])
    const items = await d.flush()
    expect(items).toContainEqual({ kind: 'ev', channel: MISSION_APPROVAL_CHANNEL, args: deskEncode([{ approvalId: 'ap1', summary: 'npm test' }]) })
  })

  it('keeps every handler ipcMain is given, as it is given them', () => {
    const registered = new Map<string, unknown>()
    const ipc = { handle: (channel: string, listener: unknown) => { registered.set(channel, listener) }, removeHandler: (channel: string) => { registered.delete(channel) } }
    const kept = keepHandlers(ipc)
    const handler = async () => 1
    ipc.handle('a:b', handler)
    expect(registered.get('a:b')).toBe(handler)
    expect(kept.get('a:b')).toBe(handler)
    ipc.removeHandler('a:b')
    expect(kept.has('a:b')).toBe(false)
  })
})
