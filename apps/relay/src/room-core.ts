import { parseClientFrame, MAX_FRAME, type Address, type ClientFrame, type ErrorCode, type RelayFrame } from './protocol.js'

/**
 * ONE PC'S ROOM (0.826). The room holds the connections of one PC, the devices paired with it and, while the PC
 * shows a code, phones being paired. It lets in a PC that has its token, a device the PC paired, and a phone with an
 * open code's token; it passes sealed messages between them and says who is connected. It cannot read a message.
 *
 * Written against RoomHost, not Cloudflare's objects, so the rules are tested where they are written (room.ts is
 * the Durable Object that hands it real connections and storage). Between messages the room's object may be put to
 * sleep and woken again, so nothing lives in memory here: what a connection is lives on the connection, and the
 * rest in storage.
 */

/** What a connection is, kept on the connection itself. */
export type Attachment =
  | { readonly as: 'new'; readonly at: number }
  | { readonly as: 'pc'; readonly at: number; readonly rate: Rate }
  | { readonly as: 'device'; readonly device: string; readonly at: number; readonly rate: Rate }
  | { readonly as: 'pairing'; readonly tag: string; readonly at: number; readonly sent: boolean; readonly rate: Rate }

export interface Rate { readonly start: number; readonly count: number }

export interface Peer {
  readonly attachment: Attachment
  attach(next: Attachment): void
  send(frame: RelayFrame): void
  close(code: number, reason: string): void
}

export interface RoomStore {
  get<T>(key: string): Promise<T | undefined>
  put(key: string, value: unknown): Promise<void>
  deleteAll(): Promise<void>
}

export interface RoomHost {
  readonly store: RoomStore
  /** Every connection the room holds now; one closing may still be among them. */
  peers(): readonly Peer[]
  now(): number
  setAlarm(at: number): Promise<void>
  /** `bytes` random bytes, base64url. */
  random(bytes: number): string
  /** SHA-256 of the text's UTF-8 bytes, base64url. */
  sha256(text: string): Promise<string>
}

/** How long a new connection has to say who it is. */
export const AUTH_WITHIN_MS = 10_000
/** How long a phone being paired may stay connected. */
export const PAIRING_SOCKET_MS = 120_000
/** The longest a code stays open, whatever the PC asks. */
export const MAX_PAIRING_MS = 15 * 60_000
/** Right or wrong, the most tries one code gets. */
export const MAX_PAIRING_TRIES = 10
/** The most devices one PC pairs. */
export const MAX_DEVICES = 20
/** Frames one connection may send in RATE_WINDOW_MS; past that it is told to slow down and the frame is dropped. */
export const RATE_LIMIT = 60
export const RATE_WINDOW_MS = 10_000
/** A room whose PC has been away this long is deleted, devices and all. */
export const IDLE_ROOM_MS = 180 * 24 * 60 * 60_000

/** The close codes a client acts on. */
export const CLOSE = { replaced: 4001, removed: 4003, reset: 4004, refused: 4401, timeout: 4408 } as const

interface DeviceEntry { readonly hash: string; readonly added: number }
interface Meta {
  /** SHA-256 of the PC's token. */
  readonly owner: string
  readonly created: number
  /** When the PC last connected or left. */
  readonly seen: number
  readonly devices: Readonly<Record<string, DeviceEntry>>
  readonly pairing?: { readonly hash: string; readonly until: number; readonly tries: number }
}
const META = 'meta'

/** Two hashes compared without stopping at the first difference. */
function same(a: string, b: string): boolean {
  if (a.length !== b.length) return false
  let diff = 0
  for (let i = 0; i < a.length; i += 1) diff |= a.charCodeAt(i) ^ b.charCodeAt(i)
  return diff === 0
}

/** The name a connection is written to by: the PC, a device's id, or a phone being paired. */
export const addressOf = (attachment: Attachment): Address | undefined =>
  attachment.as === 'pc' ? 'pc' : attachment.as === 'device' ? attachment.device : attachment.as === 'pairing' ? `pairing:${attachment.tag}` : undefined

type Command = Extract<ClientFrame, { readonly t: 'allow' | 'remove' | 'pair-open' | 'pair-close' | 'reset' }>

export function roomCore(host: RoomHost) {
  const meta = (): Promise<Meta | undefined> => host.store.get<Meta>(META)
  const save = (next: Meta): Promise<void> => host.store.put(META, next)
  const pcBesides = (peer?: Peer): Peer | undefined => host.peers().find((other) => other !== peer && other.attachment.as === 'pc')
  const err = (peer: Peer, code: ErrorCode, id?: string): void => peer.send({ t: 'err', code, ...(id === undefined ? {} : { id }) })
  const refuse = (peer: Peer, code: ErrorCode): void => { err(peer, code); peer.close(CLOSE.refused, code) }

  /** One alarm, at the soonest thing the room must do on its own. `leaving` is a connection closing now. */
  const schedule = async (leaving?: Peer): Promise<void> => {
    const now = host.now()
    const times: number[] = []
    for (const peer of host.peers()) {
      if (peer === leaving) continue
      if (peer.attachment.as === 'new') times.push(peer.attachment.at + AUTH_WITHIN_MS)
      if (peer.attachment.as === 'pairing') times.push(peer.attachment.at + PAIRING_SOCKET_MS)
    }
    const held = await meta()
    if (held?.pairing !== undefined) times.push(held.pairing.until)
    if (held !== undefined && pcBesides(leaving) === undefined) times.push(held.seen + IDLE_ROOM_MS)
    if (times.length > 0) await host.setAlarm(Math.max(now + 1_000, Math.min(...times)))
  }

  /** Each connection's frames are counted; one that sends too many is told so and the rest are dropped. */
  const within = (peer: Peer): boolean => {
    const at = peer.attachment
    if (at.as === 'new') return true
    const now = host.now()
    const rate: Rate = now - at.rate.start >= RATE_WINDOW_MS ? { start: now, count: 1 } : { start: at.rate.start, count: at.rate.count + 1 }
    peer.attach({ ...at, rate })
    return rate.count <= RATE_LIMIT
  }

  const auth = async (peer: Peer, frame: Extract<ClientFrame, { readonly t: 'auth' }>): Promise<void> => {
    const now = host.now()
    const rate = { start: now, count: 0 }
    const held = await meta()
    if (frame.as === 'pc') {
      const hash = await host.sha256(frame.token)
      // The first PC to come with a token makes the room: its id is 16 random bytes nobody else has.
      if (held === undefined) await save({ owner: hash, created: now, seen: now, devices: {} })
      else if (!same(held.owner, hash)) { refuse(peer, 'auth'); return }
      else await save({ ...held, seen: now })
      // One PC at a time: a second connection with the token is the same PC come back, and the old one is let go.
      for (const other of host.peers()) if (other !== peer && other.attachment.as === 'pc') other.close(CLOSE.replaced, 'replaced')
      peer.attach({ as: 'pc', at: now, rate })
      const online = [...new Set(host.peers().flatMap((other) => other.attachment.as === 'device' ? [other.attachment.device] : []))]
      peer.send({ t: 'ok', as: 'pc', devices: online })
      for (const other of host.peers()) if (other.attachment.as === 'device' || other.attachment.as === 'pairing') other.send({ t: 'presence', pc: true })
      await schedule()
      return
    }
    if (held === undefined) { refuse(peer, 'room'); return }
    if (frame.as === 'device') {
      const entry = held.devices[frame.device]
      if (entry === undefined || !same(entry.hash, await host.sha256(frame.token))) { refuse(peer, 'auth'); return }
      peer.attach({ as: 'device', device: frame.device, at: now, rate })
      const pc = pcBesides(peer)
      peer.send({ t: 'ok', as: 'device', pc: pc !== undefined })
      pc?.send({ t: 'presence', device: frame.device, on: true })
      return
    }
    // A phone with the code's token. Every try counts, right or wrong, so a code cannot be guessed at.
    const window = held.pairing
    if (window === undefined || window.until <= now || window.tries >= MAX_PAIRING_TRIES) { refuse(peer, 'pairing-closed'); return }
    await save({ ...held, pairing: { ...window, tries: window.tries + 1 } })
    if (!same(window.hash, await host.sha256(frame.token))) { refuse(peer, 'auth'); return }
    peer.attach({ as: 'pairing', tag: host.random(9), at: now, sent: false, rate })
    peer.send({ t: 'ok', as: 'pairing', pc: pcBesides(peer) !== undefined })
    await schedule()
  }

  const send = (peer: Peer, frame: Extract<ClientFrame, { readonly t: 'send' }>): void => {
    const at = peer.attachment
    const from = addressOf(at)
    if (from === undefined) return
    // A phone being paired says one thing, to the PC, with its public key; nobody else sends a key.
    if (at.as === 'pairing' ? frame.to !== 'pc' || at.sent || frame.k === undefined : frame.k !== undefined) { err(peer, 'bad-frame'); return }
    // A device talks to its PC only; the PC to anyone but itself.
    if ((at.as === 'device' && frame.to !== 'pc') || (at.as === 'pc' && frame.to === 'pc')) { err(peer, 'bad-frame'); return }
    const targets = host.peers().filter((other) => other !== peer && addressOf(other.attachment) === frame.to)
    if (targets.length === 0) { err(peer, frame.to === 'pc' ? 'pc-offline' : 'peer-offline'); return }
    if (at.as === 'pairing') peer.attach({ ...at, sent: true })
    for (const target of targets) target.send({ t: 'msg', from, n: frame.n, c: frame.c, ...(frame.k === undefined ? {} : { k: frame.k }) })
  }

  const command = async (peer: Peer, frame: Command): Promise<void> => {
    if (peer.attachment.as !== 'pc') { err(peer, 'not-owner', frame.id); return }
    const held = await meta()
    if (held === undefined) { err(peer, 'room', frame.id); return }
    const done = (): void => peer.send({ t: 'done', of: frame.t, ...(frame.id === undefined ? {} : { id: frame.id }) })
    const now = host.now()
    switch (frame.t) {
      case 'allow': {
        if (held.devices[frame.device] === undefined && Object.keys(held.devices).length >= MAX_DEVICES) { err(peer, 'full', frame.id); return }
        await save({ ...held, devices: { ...held.devices, [frame.device]: { hash: frame.hash, added: now } } })
        done()
        return
      }
      case 'remove': {
        const { [frame.device]: _gone, ...devices } = held.devices
        await save({ ...held, devices })
        for (const other of host.peers()) {
          if (other.attachment.as === 'device' && other.attachment.device === frame.device) {
            other.send({ t: 'removed' })
            other.close(CLOSE.removed, 'removed')
          }
        }
        done()
        return
      }
      case 'pair-open': {
        const until = Math.min(frame.until, now + MAX_PAIRING_MS)
        if (until <= now) { err(peer, 'bad-frame', frame.id); return }
        await save({ ...held, pairing: { hash: frame.hash, until, tries: 0 } })
        done()
        await schedule()
        return
      }
      case 'pair-close': {
        const { pairing: _closed, ...rest } = held
        await save(rest)
        done()
        return
      }
      case 'reset': {
        await host.store.deleteAll()
        for (const other of host.peers()) if (other !== peer) other.close(CLOSE.reset, 'reset')
        done()
        return
      }
    }
  }

  return {
    /** A connection came in: it has AUTH_WITHIN_MS to say who it is. */
    async opened(): Promise<void> {
      await schedule()
    },

    async message(peer: Peer, text: string): Promise<void> {
      if (text.length > MAX_FRAME) { err(peer, 'too-big'); return }
      const frame = parseClientFrame(text)
      if (peer.attachment.as === 'new') {
        // A connection says who it is first, or it is closed.
        if (frame === undefined || frame.t !== 'auth') { refuse(peer, frame === undefined ? 'bad-frame' : 'auth'); return }
        await auth(peer, frame)
        return
      }
      if (!within(peer)) { err(peer, 'too-fast'); return }
      if (frame === undefined || frame.t === 'auth') { err(peer, 'bad-frame'); return }
      if (frame.t === 'send') { send(peer, frame); return }
      await command(peer, frame)
    },

    async closed(peer: Peer): Promise<void> {
      const at = peer.attachment
      if (at.as === 'pc' && pcBesides(peer) === undefined) {
        for (const other of host.peers()) if (other !== peer && (other.attachment.as === 'device' || other.attachment.as === 'pairing')) other.send({ t: 'presence', pc: false })
        const held = await meta()
        if (held !== undefined) await save({ ...held, seen: host.now() })
      }
      if (at.as === 'device' && !host.peers().some((other) => other !== peer && other.attachment.as === 'device' && other.attachment.device === at.device)) {
        pcBesides(peer)?.send({ t: 'presence', device: at.device, on: false })
      }
      await schedule(peer)
    },

    async alarm(): Promise<void> {
      const now = host.now()
      for (const peer of host.peers()) {
        const at = peer.attachment
        if (at.as === 'new' && now - at.at >= AUTH_WITHIN_MS) peer.close(CLOSE.timeout, 'auth timeout')
        if (at.as === 'pairing' && now - at.at >= PAIRING_SOCKET_MS) peer.close(CLOSE.timeout, 'pairing over')
      }
      const held = await meta()
      if (held !== undefined && pcBesides() === undefined && now - held.seen >= IDLE_ROOM_MS) {
        await host.store.deleteAll()
        for (const peer of host.peers()) peer.close(CLOSE.reset, 'room gone')
        return
      }
      if (held?.pairing !== undefined && held.pairing.until <= now) {
        const { pairing: _over, ...rest } = held
        await save(rest)
      }
      await schedule()
    }
  }
}
