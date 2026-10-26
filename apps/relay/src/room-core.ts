import { isPushEndpoint, parseAwayPost, parseClientFrame, HOOK_BODY_CHARS, LETTERS_AT_ONCE, MAX_FRAME, TOKEN, type Address, type AwayAlert, type ClientFrame, type ErrorCode, type HookLetter, type Letter, type LetterFrom, type RelayFrame } from './protocol.js'

/**
 * ONE PC'S ROOM (0.826). The room holds the connections of one PC, the devices paired with it and, while the PC
 * shows a code, phones being paired. It lets in a PC that has its token, a device the PC paired, and a phone with an
 * open code's token; it passes sealed messages between them and says who is connected. It cannot read a message.
 *
 * Written against RoomHost, not Cloudflare's objects, so the rules are tested where they are written (room.ts is
 * the Durable Object that hands it real connections and storage). Between messages the room's object may be put to
 * sleep and woken again, so nothing lives in memory here: what a connection is lives on the connection, and the
 * rest in storage. Called links (2026-10-25) leave readable letters for the PC, separate from the sealed phone traffic.
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
  delete(keys: readonly string[]): Promise<void>
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
  /**
   * Posts one sealed alert to a phone's push service, signed as this relay (vapid.ts): the push service's answer, or
   * undefined when this relay has no key to sign with. Never called for an address that is not a push service's.
   */
  push?(endpoint: string, body: string, urgency: 'normal' | 'high'): Promise<number | undefined>
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
/**
 * Frames one connection may send in RATE_WINDOW_MS; past that it is told to slow down and the frame is dropped. A
 * device's, and since Locust in a browser (0.830) the PC's own is higher: it tells a browser that has the window
 * what its window is told, as a turn streams, a few times a second.
 */
export const RATE_LIMIT = 120
export const PC_RATE_LIMIT = 600
export const RATE_WINDOW_MS = 10_000
/** A room whose PC has been away this long is deleted, devices and all. */
export const IDLE_ROOM_MS = 180 * 24 * 60 * 60_000

/** How long a letter is kept for a PC or phone that has not come for it, and how many at most. */
export const LETTER_KEPT_MS = 7 * 24 * 60 * 60_000
export const MAX_LETTERS = 200
/** An away look comes every hour or two: more than this an hour is something else, and is refused. */
export const AWAY_POSTS_PER_HOUR = 12
/** Sixty calls an hour lets integrations burst, without one leaked link spending a plan in a tight loop. */
export const HOOK_POSTS_PER_HOUR = 60
/** As many hooks as routines; a room cannot grow an unbounded hash table. */
export const MAX_HOOKS = 64
/** The most times a day the away looks ring the phones, whatever they ask. */
export const AWAY_PUSHES_PER_DAY = 5
const HOUR_MS = 60 * 60_000
const DAY_MS = 24 * HOUR_MS

/** The close codes a client acts on. */
export const CLOSE = { replaced: 4001, removed: 4003, reset: 4004, refused: 4401, timeout: 4408 } as const

interface DeviceEntry { readonly hash: string; readonly added: number }
interface LetterEntry { readonly id: string; readonly at: number; readonly from: LetterFrom | 'hook'; readonly hook?: string }
interface Hook { readonly hash: string; readonly posts: readonly number[]; readonly lost: number }
interface Away {
  /** SHA-256 of the away look's token. */
  readonly hash: string
  readonly since: number
  /** When it last posted, and last rang the phones: counted against the hour's and the day's limits. */
  readonly posts: readonly number[]
  readonly pushes: readonly number[]
}
interface Meta {
  /** SHA-256 of the PC's token. */
  readonly owner: string
  readonly created: number
  /** When the PC last connected or left. */
  readonly seen: number
  readonly devices: Readonly<Record<string, DeviceEntry>>
  readonly pairing?: { readonly hash: string; readonly until: number; readonly tries: number }
  readonly away?: Away
  readonly hooks?: Readonly<Record<string, Hook>>
  /** The letters kept, oldest first; each one's box is stored under its own key. */
  readonly letters?: readonly LetterEntry[]
}
const META = 'meta'
const AWAY_ALERTS = 'away-alerts'
const letterKey = (id: string): string => `letter:${id}`
/** Cloudflare values are capped at 128 KiB; 16K UTF-16 characters fit even when JSON must escape every character. */
const HOOK_PART_CHARS = 16 * 1024
type StoredHookLetter = Omit<HookLetter, 'body'> & { readonly parts: number }

/** What the away look is answered over HTTP: a status, and a few words of JSON. */
export interface AwayAnswer { readonly status: number; readonly body: Readonly<Record<string, unknown>> }

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

type Command = Extract<ClientFrame, { readonly t: 'allow' | 'remove' | 'pair-open' | 'pair-close' | 'reset' | 'push' | 'away-open' | 'away-close' | 'away-alerts' | 'post' | 'hook-open' | 'hook-close' | 'hook-collected' | 'hook-letters' }>

export function roomCore(host: RoomHost) {
  const meta = (): Promise<Meta | undefined> => host.store.get<Meta>(META)
  const save = (next: Meta): Promise<void> => host.store.put(META, next)
  const pcBesides = (peer?: Peer): Peer | undefined => host.peers().find((other) => other !== peer && other.attachment.as === 'pc')
  const err = (peer: Peer, code: ErrorCode, id?: string): void => peer.send({ t: 'err', code, ...(id === undefined ? {} : { id }) })
  const refuse = (peer: Peer, code: ErrorCode): void => { err(peer, code); peer.close(CLOSE.refused, code) }
  const forgetLetters = async (letters: readonly LetterEntry[]): Promise<void> => {
    for (const letter of letters) {
      const key = letterKey(letter.id)
      const box = letter.hook === undefined ? undefined : await host.store.get<HookLetter | StoredHookLetter>(key)
      const parts = box !== undefined && 'parts' in box ? Array.from({ length: box.parts }, (_, at) => `${key}:${String(at)}`) : []
      await host.store.delete([key, ...parts])
    }
  }
  const keepHookLetter = async (letter: HookLetter): Promise<void> => {
    const key = letterKey(letter.id)
    if (letter.body.length <= HOOK_PART_CHARS) { await host.store.put(key, letter); return }
    const { body, ...header } = letter
    const parts = Math.ceil(body.length / HOOK_PART_CHARS)
    for (let at = 0; at < parts; at += 1) await host.store.put(`${key}:${String(at)}`, body.slice(at * HOOK_PART_CHARS, (at + 1) * HOOK_PART_CHARS))
    await host.store.put(key, { ...header, parts } satisfies StoredHookLetter)
  }
  const readHookLetter = async (id: string): Promise<HookLetter | undefined> => {
    const key = letterKey(id)
    const box = await host.store.get<HookLetter | StoredHookLetter>(key)
    if (box === undefined) return undefined
    // Only the brief's bounded excerpt leaves storage; even escaped binary data fits comfortably in a socket frame.
    const body = 'body' in box ? box.body : await host.store.get<string>(`${key}:0`)
    if (body === undefined) return undefined
    return { id: box.id, at: box.at, hook: box.hook, contentType: box.contentType, body: body.slice(0, HOOK_BODY_CHARS + 1) }
  }

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
    const oldest = held?.letters?.[0]
    if (oldest !== undefined) times.push(oldest.at + LETTER_KEPT_MS)
    if (times.length > 0) await host.setAlarm(Math.max(now + 1_000, Math.min(...times)))
  }

  /** The letters still kept: none older than a week, and only the newest MAX_LETTERS. */
  const kept = (letters: readonly LetterEntry[], now: number): { readonly stay: readonly LetterEntry[]; readonly gone: readonly LetterEntry[] } => {
    const fresh = letters.filter((letter) => now - letter.at < LETTER_KEPT_MS)
    const stay = fresh.slice(-MAX_LETTERS)
    return { stay, gone: letters.filter((letter) => !stay.includes(letter)) }
  }

  /** Expired or displaced calls leave a count, never a body, for the routine's history on return. */
  const prune = async (held: Meta): Promise<Meta> => {
    const { stay, gone } = kept(held.letters ?? [], host.now())
    if (gone.length === 0) return held
    const hooks = { ...held.hooks }
    for (const letter of gone) {
      const hook = letter.hook === undefined ? undefined : hooks[letter.hook]
      if (hook !== undefined) hooks[letter.hook!] = { ...hook, lost: hook.lost + 1 }
    }
    await forgetLetters(gone)
    const next = { ...held, hooks, letters: stay }
    await save(next)
    return next
  }

  /** A letter left for the PC and its phones: stored, then handed at once to whoever of them is connected. */
  const leave = async (held: Meta, from: LetterFrom, n: string, c: string, poster?: Peer): Promise<Letter> => {
    const now = host.now()
    const letter: Letter = { id: `${String(now).padStart(13, '0')}-${host.random(6)}`, at: now, from, n, c }
    await host.store.put(letterKey(letter.id), letter)
    const next = { ...held, letters: [...(held.letters ?? []), { id: letter.id, at: now, from }] }
    await save(next)
    await prune(next)
    for (const other of host.peers()) if (other !== poster && (other.attachment.as === 'pc' || other.attachment.as === 'device')) other.send({ t: 'letter', item: letter })
    await schedule()
    return letter
  }

  /** The letters a PC catching up asks for (after the last it read), or the newest few a phone shows. */
  const letters = async (peer: Peer, frame: Extract<ClientFrame, { readonly t: 'letters' }>): Promise<void> => {
    const at = peer.attachment
    if (at.as !== 'pc' && at.as !== 'device') { err(peer, 'not-owner', frame.id); return }
    const held = await meta()
    const index = (held === undefined ? [] : (await prune(held)).letters ?? []).filter((letter) => letter.from !== 'hook')
    const after = frame.after
    const rest = frame.newest !== undefined ? index.slice(-frame.newest) : after === undefined ? index : index.filter((letter) => letter.id > after)
    const chosen = rest.slice(0, LETTERS_AT_ONCE)
    const items = (await Promise.all(chosen.map((letter) => host.store.get<Letter>(letterKey(letter.id))))).filter((item): item is Letter => item !== undefined)
    peer.send({ t: 'letters', items, more: rest.length > chosen.length, ...(frame.id === undefined ? {} : { id: frame.id }) })
  }

  /** Each connection's frames are counted; one that sends too many is told so and the rest are dropped. */
  const within = (peer: Peer): boolean => {
    const at = peer.attachment
    if (at.as === 'new') return true
    const now = host.now()
    const rate: Rate = now - at.rate.start >= RATE_WINDOW_MS ? { start: now, count: 1 } : { start: at.rate.start, count: at.rate.count + 1 }
    peer.attach({ ...at, rate })
    return rate.count <= (at.as === 'pc' ? PC_RATE_LIMIT : RATE_LIMIT)
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
    const done = (): void => { if (frame.t !== 'hook-letters') peer.send({ t: 'done', of: frame.t, ...(frame.id === undefined ? {} : { id: frame.id }) }) }
    const now = host.now()
    switch (frame.t) {
      case 'hook-open': {
        if (held.hooks?.[frame.hook] === undefined && Object.keys(held.hooks ?? {}).length >= MAX_HOOKS) { err(peer, 'full', frame.id); return }
        const old = held.hooks?.[frame.hook]
        await save({ ...held, hooks: { ...held.hooks, [frame.hook]: { posts: old?.posts ?? [], lost: old?.lost ?? 0, hash: frame.hash } } })
        done()
        return
      }
      case 'hook-close': {
        const { [frame.hook]: _gone, ...hooks } = held.hooks ?? {}
        const gone = (held.letters ?? []).filter((letter) => letter.hook === frame.hook)
        await forgetLetters(gone)
        await save({ ...held, hooks, letters: (held.letters ?? []).filter((letter) => letter.hook !== frame.hook) })
        done()
        return
      }
      case 'hook-collected': {
        const gone = (held.letters ?? []).filter((letter) => letter.hook === frame.hook && frame.letters.includes(letter.id))
        await forgetLetters(gone)
        await save({ ...held, letters: (held.letters ?? []).filter((letter) => !gone.includes(letter)) })
        done()
        return
      }
      case 'hook-letters': {
        const fresh = await prune(held)
        const chosen = (fresh.letters ?? []).filter((letter) => letter.hook === frame.hook).slice(0, 10)
        const items = (await Promise.all(chosen.map((letter) => readHookLetter(letter.id)))).filter((item): item is HookLetter => item !== undefined)
        peer.send({ t: 'hook-letters', items, lost: fresh.hooks?.[frame.hook]?.lost ?? 0, ...(frame.id === undefined ? {} : { id: frame.id }) })
        return
      }
      case 'allow': {
        if (held.devices[frame.device] === undefined && Object.keys(held.devices).length >= MAX_DEVICES) { err(peer, 'full', frame.id); return }
        await save({ ...held, devices: { ...held.devices, [frame.device]: { hash: frame.hash, added: now } } })
        done()
        return
      }
      case 'remove': {
        const { [frame.device]: _gone, ...devices } = held.devices
        await save({ ...held, devices })
        // A phone removed is rung no more by the away looks either.
        const alerts = await host.store.get<readonly AwayAlert[]>(AWAY_ALERTS)
        if (alerts?.some((alert) => alert.device === frame.device)) await host.store.put(AWAY_ALERTS, alerts.filter((alert) => alert.device !== frame.device))
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
      case 'push': {
        // An alert for one of this PC's phones, sealed for that phone's browser: the relay signs and posts, and reads
        // nothing. Only to a push service's address, so the relay is never a way to reach anywhere else.
        if (!isPushEndpoint(frame.endpoint)) { err(peer, 'bad-frame', frame.id); return }
        if (host.push === undefined) { err(peer, 'push-off', frame.id); return }
        const status = await host.push(frame.endpoint, frame.body, frame.urgency).catch(() => 0)
        if (status === undefined) { err(peer, 'push-off', frame.id); return }
        if (status === 404 || status === 410) { err(peer, 'push-gone', frame.id); return }
        if (status < 200 || status >= 300) { err(peer, 'push-failed', frame.id); return }
        done()
        return
      }
      case 'away-open': {
        // A new token for the away look: the one before it stops working. Its counts carry on, so a new token is no
        // way around the day's limit.
        await save({ ...held, away: { posts: [], pushes: [], ...held.away, hash: frame.hash, since: now } })
        done()
        return
      }
      case 'away-close': {
        // Off: the away look is let go, and every letter and kept alert with it.
        const { away: _off, ...rest } = held
        const gone = (held.letters ?? []).filter((letter) => letter.from !== 'hook')
        if (gone.length > 0) await host.store.delete(gone.map((letter) => letterKey(letter.id)))
        await host.store.delete([AWAY_ALERTS])
        await save({ ...rest, letters: (held.letters ?? []).filter((letter) => letter.from === 'hook') })
        done()
        return
      }
      case 'away-alerts': {
        // Only for phones this PC paired.
        await host.store.put(AWAY_ALERTS, frame.alerts.filter((alert) => held.devices[alert.device] !== undefined))
        done()
        return
      }
      case 'post': {
        // What the PC told the person itself, so the away look that follows does not tell it again.
        await leave(held, 'pc', frame.n, frame.c, peer)
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
    /** A called link (2026-10-25): accepted into storage, without waiting for a PC or an agent. */
    async hook(request: { readonly method: string; readonly secret: string; readonly body: string; readonly contentType: string }): Promise<AwayAnswer> {
      if (request.method !== 'POST') return { status: 405, body: { error: 'method' } }
      if (new TextEncoder().encode(request.body).length > MAX_FRAME) return { status: 413, body: { error: 'too-big' } }
      const held = await meta()
      if (held === undefined || !TOKEN.test(request.secret)) return { status: 404, body: { error: 'not-found' } }
      const hash = await host.sha256(request.secret)
      const match = Object.entries(held.hooks ?? {}).find(([, hook]) => same(hook.hash, hash))
      if (match === undefined) return { status: 404, body: { error: 'not-found' } }
      const [id, hook] = match
      const now = host.now()
      const posts = hook.posts.filter((at) => now - at < HOUR_MS)
      if (posts.length >= HOOK_POSTS_PER_HOUR) return { status: 429, body: { error: 'too-fast' } }
      const letter: HookLetter = { id: `${String(now).padStart(13, '0')}-${host.random(6)}`, at: now, hook: id, body: request.body, contentType: request.contentType.slice(0, 200) }
      await keepHookLetter(letter)
      const next: Meta = { ...held, hooks: { ...held.hooks, [id]: { ...hook, posts: [...posts, now] } }, letters: [...(held.letters ?? []), { id: letter.id, at: now, from: 'hook', hook: id }] }
      await save(next)
      await prune(next)
      pcBesides()?.send({ t: 'hook-letter', hook: id })
      await schedule()
      return { status: 202, body: { accepted: true } }
    },
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
      if (frame.t === 'letters') { await letters(peer, frame); return }
      await command(peer, frame)
    },

    /**
     * The away look, over HTTPS with its own token (a workflow has no WebSocket to keep): GET asks whether the PC is
     * on and for the newest letter, so the look knows what was told already; POST leaves a letter, and may ring the
     * phones that are not open, with the alerts their PC sealed ahead of time, at most AWAY_PUSHES_PER_DAY a day.
     */
    async away(request: { readonly method: string; readonly token?: string; readonly body?: string }): Promise<AwayAnswer> {
      const held = await meta()
      if (held === undefined) return { status: 404, body: { error: 'room' } }
      if (held.away === undefined) return { status: 403, body: { error: 'away-off' } }
      if (request.token === undefined || !TOKEN.test(request.token) || !same(held.away.hash, await host.sha256(request.token))) return { status: 401, body: { error: 'auth' } }
      const pc = pcBesides() !== undefined
      if (request.method === 'GET') {
        const newest = (await prune(held)).letters?.filter((letter) => letter.from !== 'hook').at(-1)
        const latest = newest === undefined ? undefined : await host.store.get<Letter>(letterKey(newest.id))
        return { status: 200, body: { pc, ...(latest === undefined ? {} : { latest }) } }
      }
      if (request.method !== 'POST') return { status: 405, body: { error: 'bad-frame' } }
      const now = host.now()
      const posts = held.away.posts.filter((at) => now - at < HOUR_MS)
      if (posts.length >= AWAY_POSTS_PER_HOUR) return { status: 429, body: { error: 'too-fast' } }
      const post = request.body === undefined ? undefined : parseAwayPost(request.body)
      if (post === undefined) return { status: 400, body: { error: 'bad-frame' } }
      let pushes = held.away.pushes.filter((at) => now - at < DAY_MS)
      let rung = 0
      // With the PC on, it rings the phones itself.
      if (post.alert !== undefined && !pc && pushes.length < AWAY_PUSHES_PER_DAY && host.push !== undefined) {
        const alerts = (await host.store.get<readonly AwayAlert[]>(AWAY_ALERTS)) ?? []
        const open = new Set(host.peers().flatMap((peer) => peer.attachment.as === 'device' ? [peer.attachment.device] : []))
        const gone: string[] = []
        for (const alert of alerts) {
          if (open.has(alert.device) || held.devices[alert.device] === undefined || !isPushEndpoint(alert.endpoint)) continue
          const status = await host.push(alert.endpoint, alert[post.alert], 'normal').catch(() => 0)
          if (status === 404 || status === 410) gone.push(alert.device)
          else if (status !== undefined && status >= 200 && status < 300) rung += 1
        }
        if (gone.length > 0) await host.store.put(AWAY_ALERTS, alerts.filter((alert) => !gone.includes(alert.device)))
        if (rung > 0) pushes = [...pushes, now]
      }
      const letter = await leave({ ...held, away: { ...held.away, posts: [...posts, now], pushes } }, 'away', post.n, post.c)
      return { status: 200, body: { id: letter.id, rung } }
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
      if (held !== undefined) {
        let next: Meta = held
        if (next.pairing !== undefined && next.pairing.until <= now) {
          const { pairing: _over, ...rest } = next
          next = rest
        }
        // Letters a week old are deleted whether or not anyone came for them.
        next = await prune(next)
        if (next !== held) await save(next)
      }
      await schedule()
    }
  }
}
