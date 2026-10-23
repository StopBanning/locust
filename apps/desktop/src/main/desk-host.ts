import { deskAccess, DESK_EVENTS, WINDOW_ONLY_SAYS } from '../shared/desk-calls.js'
import { deskDecode, deskEncode, type DeskAsk, type DeskHello, type DeskTell } from '../shared/desk.js'
import { MISSION_APPROVAL_CHANNEL } from '../shared/ipc.js'

/*
 * LOCUST IN A BROWSER, ON THIS COMPUTER (0.830, shared/desk.ts). A browser that has the window makes the window's own
 * calls; each is answered by the very handler the window's call reaches, as if from the window itself -- for the
 * calls desk-calls.ts lets a browser make, and no other. What this computer tells its window, a browser that has the
 * window is told too. Only one window acts at a time: while a browser has it, this computer's window shows a cover
 * with nothing of the app running behind it, until the person takes it back here or another browser takes it.
 */

export type IpcHandler = (event: unknown, ...args: unknown[]) => unknown

/** ipcMain's handlers, kept as they are given it, so a browser's call reaches the same one the window's does. */
export function keepHandlers(ipc: unknown): ReadonlyMap<string, IpcHandler> {
  const host = ipc as { handle: (channel: string, listener: IpcHandler) => void; removeHandler: (channel: string) => void }
  const handlers = new Map<string, IpcHandler>()
  const handle = host.handle.bind(host)
  const removeHandler = host.removeHandler.bind(host)
  host.handle = (channel, listener) => {
    handle(channel, listener)
    handlers.set(channel, listener)
  }
  host.removeHandler = (channel) => {
    removeHandler(channel)
    handlers.delete(channel)
  }
  return handlers
}

/** The part of this computer's window a browser's call is made as. */
export interface DeskWindow {
  isDestroyed(): boolean
  readonly webContents: {
    readonly mainFrame: unknown
    isDestroyed(): boolean
    getProcessId(): number
  }
}

export interface DeskHostDeps {
  readonly handlers: ReadonlyMap<string, IpcHandler>
  /** This computer's window; a call is made as it, so the handler's own checks read it as the window's. */
  readonly window: () => DeskWindow | undefined
  /** Seals what the browser is told and sends it; false when it could not be sent. */
  readonly tell: (device: string, items: readonly DeskTell[]) => Promise<boolean>
  /** The cover on this computer's window, or the app again. */
  readonly cover: (on: boolean) => Promise<void>
  readonly hello: () => Promise<Omit<DeskHello, 'active'>>
  /** The approval cards waiting now, as the window was sent them. */
  readonly waiting: () => readonly unknown[]
  /** Who has the window changed. */
  readonly changed?: (active: string | undefined) => void
  readonly flushMs?: number
  readonly later?: (task: () => void, ms: number) => unknown
}

/** A browser's call, made as this computer's window: the event every handler reads its sender from. */
function asTheWindow(window: DeskWindow): unknown {
  const sender = window.webContents
  return {
    sender,
    senderFrame: sender.mainFrame,
    processId: sender.getProcessId(),
    frameId: 0,
    ports: [],
    defaultPrevented: false,
    preventDefault: () => undefined
  }
}

const errorWords = (error: unknown): string => {
  const said = error instanceof Error ? error.message : typeof error === 'string' ? error : ''
  return said.trim() === '' ? 'Locust on your computer could not do that.' : said.slice(0, 2_000)
}

export function createDeskHost(deps: DeskHostDeps) {
  const flushMs = deps.flushMs ?? 40
  const later = deps.later ?? ((task: () => void, ms: number) => setTimeout(task, ms))
  let active: string | undefined
  const queues = new Map<string, DeskTell[]>()
  let flushing: unknown
  let sending: Promise<unknown> = Promise.resolve()

  const flush = (): void => {
    flushing = undefined
    const batches = [...queues.entries()]
    queues.clear()
    // In order, one batch after another: a call's answer never overtakes what the window was told before it.
    for (const [device, items] of batches) sending = sending.then(() => deps.tell(device, items)).catch(() => false)
  }
  const queue = (device: string, item: DeskTell): void => {
    const held = queues.get(device) ?? []
    held.push(item)
    queues.set(device, held)
    if (flushing === undefined) flushing = later(flush, flushMs)
  }
  const answer = (device: string, id: string, value: unknown): void => queue(device, { kind: 'res', id, ok: true, value: deskEncode(value) })
  const refuse = (device: string, id: string, message: string, refused = false): void => queue(device, { kind: 'res', id, ok: false, message, ...(refused ? { refused: true as const } : {}) })
  const setActive = (next: string | undefined): void => {
    active = next
    deps.changed?.(next)
  }

  const call = async (device: string, ask: Extract<DeskAsk, { readonly kind: 'call' }>): Promise<void> => {
    let args: unknown
    try { args = deskDecode(ask.args) } catch { refuse(device, ask.id, 'That call could not be read.'); return }
    if (!Array.isArray(args)) { refuse(device, ask.id, 'That call could not be read.'); return }
    const access = deskAccess(ask.channel, args)
    if (access === undefined) { refuse(device, ask.id, WINDOW_ONLY_SAYS, true); return }
    // One window acts at a time; another browser, or this computer's window, may still be read from.
    if (access === 'act' && active !== device) { refuse(device, ask.id, 'Locust is open somewhere else now. Open it here again to do that.', true); return }
    const handler = deps.handlers.get(ask.channel)
    if (handler === undefined) { refuse(device, ask.id, 'Locust on your computer does not know that call.'); return }
    const window = deps.window()
    if (window === undefined || window.isDestroyed() || window.webContents.isDestroyed()) { refuse(device, ask.id, 'Locust’s window on your computer is closed. Open Locust there again.'); return }
    try {
      answer(device, ask.id, await handler(asTheWindow(window), ...args))
    } catch (error) {
      refuse(device, ask.id, errorWords(error))
    }
  }

  const takeBack = async (): Promise<void> => {
    const was = active
    if (was === undefined) return
    setActive(undefined)
    queue(was, { kind: 'taken', by: 'computer' })
    await deps.cover(false)
  }

  return {
    /** A browser's asks, from one sealed frame, each answered in turn. */
    handle: async (device: string, asks: readonly DeskAsk[]): Promise<void> => {
      for (const ask of asks) {
        if (ask.kind === 'hello') {
          const hello = await deps.hello().catch(() => undefined)
          if (hello === undefined) refuse(device, ask.id, 'Locust on your computer could not say what it is.')
          else answer(device, ask.id, { ...hello, active: active === device ? 'here' : active === undefined ? 'computer' : 'other' } satisfies DeskHello)
        } else if (ask.kind === 'take') {
          const was = active
          setActive(device)
          if (was !== undefined && was !== device) queue(was, { kind: 'taken', by: 'browser' })
          if (was === undefined) await deps.cover(true).catch(() => undefined)
          answer(device, ask.id, true)
        } else if (ask.kind === 'replay') {
          // The cards already waiting: a window just opened has heard none of them.
          if (active === device) for (const request of deps.waiting()) queue(device, { kind: 'ev', channel: MISSION_APPROVAL_CHANNEL, args: deskEncode([request]) })
          answer(device, ask.id, true)
        } else {
          // Calls run side by side, as the window's do; each answer goes when it is ready.
          void call(device, ask)
        }
      }
    },

    /** What this computer tells its window: told to the browser that has it too, for what a browser may be told. */
    mirror: (channel: string, args: readonly unknown[]): void => {
      if (active === undefined || !DESK_EVENTS.has(channel)) return
      queue(active, { kind: 'ev', channel, args: deskEncode(args) })
    },

    /** "Use Locust here", on this computer's cover: the browser is told, and this window is the app again. */
    takeBack,

    /** A device that may no longer open the window -- removed, or its permission taken away -- lets it go. */
    forget: async (device: string): Promise<void> => {
      queues.delete(device)
      if (active === device) await takeBack()
    },

    active: (): string | undefined => active
  }
}

export type DeskHost = ReturnType<typeof createDeskHost>
