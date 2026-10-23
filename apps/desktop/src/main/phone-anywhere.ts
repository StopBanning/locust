import { timingSafeEqual } from 'node:crypto'
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import type { PhoneAnywhereSettings } from '../shared/phone.js'
import { privateMcpFile } from './locust-mcp-host.js'
import { answerPhone, MAX_BODY, type PhoneCalls } from './phone-host.js'
import { aadOf, deviceIdOf, importPrivateKey, linkKeys, newPcKeys, open, pairingProof, pairingToken, randomText, seal, sha256, type CryptoKey, type JsonWebKey, type LinkKeys } from './relay-crypto.js'
import { replaceFile } from './replace-file.js'

/*
 * YOUR PHONE, FROM ANYWHERE (0.826). The phone page (0.775) reaches Locust only on the home network. This reaches it
 * from anywhere through Locust's relay (apps/relay, at app.locust.lol): this PC connects out to it and keeps one quiet
 * connection open, so nothing here listens to the internet, and a phone paired by its code asks through it. Every
 * request and answer is sealed on one side and opened on the other (relay-crypto.ts); the relay passes along what
 * it cannot read, and keeps only hashes of the tokens it lets in.
 *
 * Pairing: the code holds this PC's room, its public key and a secret. The phone shows the relay a token made from
 * the secret, and this PC a proof made from it inside its sealed hello; the relay never sees the proof, so it cannot
 * pair a phone of its own. The phone is then given a token of its own, and the code is spent.
 */
export const PHONE_ANYWHERE_FILE = 'phone-relay.json'
export const DEFAULT_RELAY = 'https://app.locust.lol'
/** How long a code may be scanned. */
export const CODE_MS = 10 * 60_000
/** A phone's request whose time is further than this from this PC's clock is refused. */
const MAX_SKEW_MS = 10 * 60_000
/** Keep-alive: the relay answers 'ping' itself; silence for longer than STALE_MS means the line is dead. */
const PING_MS = 30_000
const STALE_MS = 75_000
const COMMAND_MS = 10_000
const MAX_NAME = 40

interface SavedDevice {
  readonly id: string
  readonly name: string
  /** The device's public key, base64url. */
  readonly key: string
  readonly added: number
  readonly seen?: number
  /** The last request number taken from it: one not above it is a copy, and dropped. */
  readonly seq: number
}

interface Saved {
  readonly v: 1
  readonly relay: string
  readonly room: string
  /** This PC's token for its room on the relay. */
  readonly token: string
  readonly key: JsonWebKey
  readonly publicKey: string
  readonly devices: readonly SavedDevice[]
}

/** The part of a WebSocket this uses, so a test can stand an in-memory relay in its place. */
export interface RelaySocket {
  send(text: string): void
  close(): void
  onopen: (() => void) | null
  onmessage: ((text: string) => void) | null
  onclose: (() => void) | null
}

/** A real connection: Node's own WebSocket, which Locust's Electron has. */
export function webRelaySocket(url: string): RelaySocket {
  const ws = new WebSocket(url)
  const socket: RelaySocket = { send: (text) => ws.send(text), close: () => ws.close(), onopen: null, onmessage: null, onclose: null }
  ws.addEventListener('open', () => socket.onopen?.())
  ws.addEventListener('message', (event) => { if (typeof event.data === 'string') socket.onmessage?.(event.data) })
  ws.addEventListener('close', () => socket.onclose?.())
  // An error is always followed by a close, which is where it is handled.
  ws.addEventListener('error', () => undefined)
  return socket
}

const sameText = (a: string, b: string): boolean => a.length === b.length && timingSafeEqual(Buffer.from(a), Buffer.from(b))

/** A device's name as the phone gave it, kept to plain words: it is only ever shown in Settings. */
const cleanName = (value: unknown): string => {
  const text = typeof value === 'string' ? value.replace(/[^\p{L}\p{N} '.,()-]/gu, '').replace(/\s+/g, ' ').trim().slice(0, MAX_NAME) : ''
  return text === '' ? 'Phone' : text
}

interface Request { readonly id: string; readonly seq: number; readonly t: number; readonly path: string; readonly body?: unknown }
const isRequest = (value: unknown): value is Request => {
  if (typeof value !== 'object' || value === null) return false
  const said = value as Record<string, unknown>
  return said.v === 1 && said.kind === 'req' && typeof said.id === 'string' && said.id.length > 0 && said.id.length <= 64
    && typeof said.seq === 'number' && Number.isSafeInteger(said.seq) && said.seq > 0
    && typeof said.t === 'number' && Number.isFinite(said.t)
    && typeof said.path === 'string' && said.path.startsWith('/api/') && said.path.length <= 300
    && (said.body === undefined || (typeof said.body === 'object' && said.body !== null))
}

export function createPhoneAnywhere(options: {
  readonly directory: string
  readonly calls: PhoneCalls
  /** Where the relay is: Locust's own, unless the person runs theirs. */
  readonly relay?: () => string
  readonly refusal?: () => string | undefined
  /** Told whenever what Settings shows may have changed. */
  readonly changed?: (settings: PhoneAnywhereSettings) => void
  readonly connect?: (url: string) => RelaySocket
  readonly now?: () => number
  readonly protectFile?: (path: string) => Promise<void>
  /** The wait before the n-th try to reconnect. */
  readonly retryMs?: (attempt: number) => number
}) {
  const file = join(options.directory, PHONE_ANYWHERE_FILE)
  const now = options.now ?? Date.now
  const connectTo = options.connect ?? webRelaySocket
  const retryMs = options.retryMs ?? ((attempt: number) => Math.min(60_000, 1_000 * 2 ** Math.max(0, attempt - 1)))

  let saved: Saved | undefined
  let privateKey: CryptoKey | undefined
  const keys = new Map<string, LinkKeys>()
  let socket: RelaySocket | undefined
  let connected = false
  let refusedByRelay = false
  const online = new Set<string>()
  let pairing: { readonly secret: string; readonly hash: string; readonly until: number; used: boolean } | undefined
  let message: string | undefined
  let attempt = 0
  let retryTimer: ReturnType<typeof setTimeout> | undefined
  let pingTimer: ReturnType<typeof setInterval> | undefined
  let heard = 0
  let saveTimer: ReturnType<typeof setTimeout> | undefined
  let disposed = false
  let queue: Promise<unknown> = Promise.resolve()
  let commandId = 0
  const commands = new Map<string, (ok: boolean) => void>()
  const lines = new Map<string, Promise<void>>()
  /** Work a frame started that may still write: waited for before Locust lets go. */
  const inFlight = new Set<Promise<unknown>>()
  const track = (work: Promise<unknown>): void => {
    const held = work.catch(() => undefined)
    inFlight.add(held)
    void held.then(() => inFlight.delete(held))
  }

  const codeUrl = (held: Saved, secret: string): string => `${held.relay}/#p=${held.room}.${held.publicKey}.${secret}`
  const settings = (): PhoneAnywhereSettings => {
    const open = saved !== undefined && pairing !== undefined && !pairing.used && pairing.until > now() ? pairing : undefined
    const refusal = options.refusal?.()
    const note = refusal ?? message
    return {
      enabled: saved !== undefined,
      connected,
      ...(saved !== undefined && open !== undefined ? { code: { url: codeUrl(saved, open.secret), until: new Date(open.until).toISOString() } } : {}),
      devices: (saved?.devices ?? []).map((device) => ({
        id: device.id,
        name: device.name,
        added: new Date(device.added).toISOString(),
        ...(device.seen === undefined ? {} : { seen: new Date(device.seen).toISOString() }),
        online: online.has(device.id)
      })),
      ...(note === undefined ? {} : { message: note })
    }
  }
  const tell = (): void => { if (!disposed) options.changed?.(settings()) }

  /** Saves one at a time, each writing what is held when its turn comes: none after Off brings the file back. */
  let writing: Promise<void> = Promise.resolve()
  const persist = (): Promise<void> => {
    clearTimeout(saveTimer)
    saveTimer = undefined
    const next = writing.then(writeNow)
    writing = next.catch(() => undefined)
    return next
  }
  const writeNow = async (): Promise<void> => {
    const held = saved
    if (held === undefined) return
    await mkdir(options.directory, { recursive: true })
    const temp = file + '.tmp'
    await rm(temp, { force: true })
    await writeFile(temp, '', { mode: 0o600, flag: 'wx' })
    // The keys are written only once the file is the account's alone, as the phone page's key is.
    await (options.protectFile ?? privateMcpFile)(temp)
    await writeFile(temp, JSON.stringify(held), { mode: 0o600 })
    await replaceFile(temp, file)
  }
  /** Request numbers and when a device was seen change every few seconds while a phone is open: saved a moment later. */
  const persistSoon = (): void => {
    if (saveTimer !== undefined) return
    saveTimer = setTimeout(() => { void persist().catch(() => undefined) }, 2_000)
  }
  const updateDevice = (id: string, change: (device: SavedDevice) => SavedDevice): void => {
    if (saved === undefined) return
    saved = { ...saved, devices: saved.devices.map((device) => device.id === id ? change(device) : device) }
  }

  const send = (frame: Record<string, unknown>): boolean => {
    if (socket === undefined || !connected) return false
    try { socket.send(JSON.stringify(frame)); return true } catch { return false }
  }
  /** A command to the relay, answered by its 'done' (true) or its error (false). */
  const command = (frame: Record<string, unknown>): Promise<boolean> => new Promise((resolve) => {
    commandId += 1
    const id = `c${String(commandId)}`
    const timer = setTimeout(() => { commands.delete(id); resolve(false) }, COMMAND_MS)
    commands.set(id, (ok) => { clearTimeout(timer); commands.delete(id); resolve(ok) })
    if (!send({ ...frame, id })) commands.get(id)?.(false)
  })

  const openCode = (): void => {
    if (pairing !== undefined && !pairing.used && pairing.until > now()) send({ t: 'pair-open', hash: pairing.hash, until: pairing.until })
  }

  const stopSocket = (): void => {
    clearTimeout(retryTimer)
    retryTimer = undefined
    clearInterval(pingTimer)
    pingTimer = undefined
    const old = socket
    socket = undefined
    connected = false
    online.clear()
    for (const resolve of [...commands.values()]) resolve(false)
    if (old !== undefined) {
      old.onclose = null
      old.onmessage = null
      try { old.close() } catch { /* Gone already. */ }
    }
  }

  const reconnectLater = (): void => {
    if (disposed || saved === undefined || refusedByRelay || retryTimer !== undefined) return
    attempt += 1
    retryTimer = setTimeout(() => { retryTimer = undefined; connect() }, retryMs(attempt))
  }

  const pairingHello = async (from: string, key: string, n: string, c: string): Promise<void> => {
    const held = saved
    const code = pairing
    if (held === undefined || privateKey === undefined || code === undefined || code.used || code.until <= now()) return
    const device = await deviceIdOf(key)
    let link: LinkKeys
    let said: unknown
    try {
      link = await linkKeys(privateKey, key, held.room, 'pc')
      said = await open(link.open, n, c, aadOf(held.room, device, 'device-to-pc'))
    } catch { return }
    const hello = typeof said === 'object' && said !== null ? said as Record<string, unknown> : {}
    if (hello.v !== 1 || hello.kind !== 'pair' || typeof hello.proof !== 'string' || !sameText(hello.proof, await pairingProof(code.secret))) return
    // One code, one phone.
    code.used = true
    const token = randomText(32)
    // The relay must know the device before the phone comes back as one: allowed first, then answered.
    if (!(await command({ t: 'allow', device, hash: await sha256(token) }))) {
      code.used = false
      message = 'Locust’s relay did not take the new phone. Make a new code and scan it again.'
      tell()
      return
    }
    const reply = await seal(link.seal, { v: 1, kind: 'paired', device, token }, aadOf(held.room, device, 'pc-to-device'))
    send({ t: 'send', to: from, ...reply })
    void command({ t: 'pair-close' })
    if (saved === undefined) return
    const entry: SavedDevice = { id: device, name: cleanName(hello.name), key, added: now(), seen: now(), seq: 0 }
    saved = { ...saved, devices: [...saved.devices.filter((other) => other.id !== device), entry] }
    keys.set(device, link)
    pairing = undefined
    message = undefined
    await persist().catch(() => { message = 'The new phone could not be saved. It works until Locust closes.' })
    tell()
  }

  const request = async (device: string, n: string, c: string): Promise<void> => {
    const held = saved
    const link = keys.get(device)
    if (held === undefined || link === undefined) return
    let said: unknown
    try { said = await open(link.open, n, c, aadOf(held.room, device, 'device-to-pc')) } catch { return }
    if (!isRequest(said)) return
    const asked = said
    const entry = saved?.devices.find((other) => other.id === device)
    // A number already taken is a copy of a request answered before: dropped, never done twice.
    if (entry === undefined || asked.seq <= entry.seq) return
    updateDevice(device, (other) => ({ ...other, seq: asked.seq, seen: now() }))
    persistSoon()
    const answered = await (async () => {
      // An organization that turns phones off is obeyed at once, not at the next start.
      const refused = options.refusal?.()
      if (refused !== undefined) return { status: 403, data: { message: refused } }
      if (Math.abs(asked.t - now()) > MAX_SKEW_MS) return { status: 400, data: { message: 'This phone’s clock is too far from this computer’s. Check the time on both.' } }
      if (asked.body !== undefined && JSON.stringify(asked.body).length > MAX_BODY) return { status: 400, data: { message: 'That message could not be read.' } }
      try {
        return await answerPhone(options.calls, asked.body === undefined ? 'GET' : 'POST', asked.path, asked.body) ?? { status: 404, data: { message: 'Locust does not know that request.' } }
      } catch {
        return { status: 500, data: { message: 'Locust could not do that. Nothing was sent.' } }
      }
    })()
    const box = await seal(link.seal, { v: 1, kind: 'res', id: asked.id, status: answered.status, data: answered.data }, aadOf(held.room, device, 'pc-to-device'))
    send({ t: 'send', to: device, ...box })
  }

  const heardFrame = (text: string): void => {
    heard = now()
    if (text === 'pong') return
    let frame: Record<string, unknown>
    try { frame = JSON.parse(text) as Record<string, unknown> } catch { return }
    if (frame.t === 'ok' && frame.as === 'pc') {
      connected = true
      attempt = 0
      message = undefined
      online.clear()
      for (const device of Array.isArray(frame.devices) ? frame.devices : []) if (typeof device === 'string') online.add(device)
      openCode()
      tell()
      return
    }
    if (frame.t === 'done' && typeof frame.id === 'string') { commands.get(frame.id)?.(true); return }
    if (frame.t === 'err') {
      if (typeof frame.id === 'string' && commands.has(frame.id)) { commands.get(frame.id)?.(false); return }
      if (!connected && frame.code === 'auth') {
        // Someone else's token is on this room: it is not this PC's any more. Starting over makes a new one.
        refusedByRelay = true
        message = 'Locust’s relay no longer knows this computer. Turn this off and on again, then scan the new code.'
        stopSocket()
        tell()
      }
      return
    }
    if (frame.t === 'presence' && typeof frame.device === 'string' && typeof frame.on === 'boolean') {
      if (frame.on) online.add(frame.device)
      else online.delete(frame.device)
      updateDevice(frame.device, (device) => ({ ...device, seen: now() }))
      persistSoon()
      tell()
      return
    }
    if (frame.t !== 'msg' || typeof frame.from !== 'string' || typeof frame.n !== 'string' || typeof frame.c !== 'string') return
    const { from, n, c } = frame as { from: string; n: string; c: string }
    if (from.startsWith('pairing:')) {
      if (typeof frame.k === 'string') track(pairingHello(from, frame.k, n, c))
      return
    }
    // One line per device, so its requests are taken in the order it numbered them.
    const line = (lines.get(from) ?? Promise.resolve()).then(() => request(from, n, c)).catch(() => undefined)
    lines.set(from, line)
    track(line)
    void line.then(() => { if (lines.get(from) === line) lines.delete(from) })
  }

  function connect(): void {
    const held = saved
    if (disposed || held === undefined || socket !== undefined || refusedByRelay || options.refusal?.() !== undefined) return
    const url = `${held.relay.replace(/^http/, 'ws')}/v1/room/${held.room}`
    let made: RelaySocket
    try { made = connectTo(url) } catch { reconnectLater(); return }
    socket = made
    heard = now()
    made.onopen = () => { try { made.send(JSON.stringify({ t: 'auth', as: 'pc', token: held.token })) } catch { /* The close follows. */ } }
    made.onmessage = heardFrame
    made.onclose = () => {
      if (socket !== made) return
      stopSocket()
      tell()
      reconnectLater()
    }
    clearInterval(pingTimer)
    pingTimer = setInterval(() => {
      if (socket !== made) return
      // A line that went quiet (the PC slept, the network changed) is let go and made again.
      if (now() - heard > STALE_MS) { stopSocket(); tell(); reconnectLater(); return }
      try { made.send('ping') } catch { /* The close follows. */ }
    }, PING_MS)
  }

  const start = async (held: Saved): Promise<void> => {
    privateKey = await importPrivateKey(held.key)
    keys.clear()
    for (const device of held.devices) {
      try { keys.set(device.id, await linkKeys(privateKey, device.key, held.room, 'pc')) } catch { /* A key that will not load pairs nothing. */ }
    }
    saved = held
    refusedByRelay = false
    connect()
  }

  const isSaved = (value: unknown): value is Saved => {
    if (typeof value !== 'object' || value === null) return false
    const held = value as Record<string, unknown>
    return held.v === 1 && typeof held.relay === 'string' && /^https?:\/\//.test(held.relay) && typeof held.room === 'string' && /^[A-Za-z0-9_-]{22}$/.test(held.room)
      && typeof held.token === 'string' && /^[A-Za-z0-9_-]{43}$/.test(held.token) && typeof held.key === 'object' && held.key !== null
      && typeof held.publicKey === 'string' && /^[A-Za-z0-9_-]{43}$/.test(held.publicKey) && Array.isArray(held.devices)
  }

  const serial = (work: () => Promise<PhoneAnywhereSettings>): Promise<PhoneAnywhereSettings> => {
    const task = queue.then(work)
    queue = task.catch(() => undefined)
    return task
  }

  const newCode = async (): Promise<void> => {
    const secret = randomText(16)
    pairing = { secret, hash: await sha256(await pairingToken(secret)), until: now() + CODE_MS, used: false }
    openCode()
  }

  return {
    settings: async (): Promise<PhoneAnywhereSettings> => { await queue; return settings() },

    load: () => serial(async () => {
      if (disposed || saved !== undefined) return settings()
      try {
        const held = JSON.parse(await readFile(file, 'utf8')) as unknown
        if (isSaved(held)) await start(held)
      } catch { /* Missing or unreadable stays off. */ }
      return settings()
    }),

    setEnabled: (enabled: unknown) => serial(async () => {
      if (disposed || typeof enabled !== 'boolean') return settings()
      if (enabled) {
        if (options.refusal?.() !== undefined) return settings()
        if (saved === undefined) {
          const pc = await newPcKeys()
          const held: Saved = { v: 1, relay: (options.relay?.() ?? DEFAULT_RELAY).replace(/\/+$/, ''), room: randomText(16), token: randomText(32), key: pc.privateJwk, publicKey: pc.publicKey, devices: [] }
          try {
            saved = held
            await persist()
          } catch {
            saved = undefined
            message = 'Locust could not save what pairs your phone. It is still off.'
            return settings()
          }
          await start(held)
        } else connect()
        await newCode()
        message = undefined
        return settings()
      }
      // Off unpairs every phone: the relay forgets the room, and this PC its keys. The next time it is on, it is a new room.
      if (connected) await command({ t: 'reset' })
      stopSocket()
      saved = undefined
      privateKey = undefined
      keys.clear()
      pairing = undefined
      message = undefined
      refusedByRelay = false
      clearTimeout(saveTimer)
      saveTimer = undefined
      await writing
      // Retried as the stores' moves are (replace-file.ts): an antivirus scanning the file just written holds it a
      // moment, and a file left behind would turn this back on when Locust next starts.
      try {
        await rm(file, { force: true, recursive: true, maxRetries: 20, retryDelay: 100 })
      } catch {
        message = 'Locust could not delete what paired your phones. It will try again the next time you turn this off.'
      }
      return settings()
    }),

    /** A new code, for the next phone: the one before it stops working. */
    newCode: () => serial(async () => {
      if (disposed || saved === undefined) return settings()
      await newCode()
      return settings()
    }),

    remove: (device: unknown) => serial(async () => {
      if (disposed || saved === undefined || typeof device !== 'string' || !saved.devices.some((other) => other.id === device)) return settings()
      saved = { ...saved, devices: saved.devices.filter((other) => other.id !== device) }
      keys.delete(device)
      online.delete(device)
      await persist().catch(() => undefined)
      // Without the relay's word the phone still has no key this PC will answer: it is removed here either way.
      if (connected) await command({ t: 'remove', device })
      return settings()
    }),

    dispose: async (): Promise<void> => {
      disposed = true
      await queue.catch(() => undefined)
      // No new frames, then what the last ones started, then what is still to be saved.
      stopSocket()
      await Promise.all([...inFlight])
      if (saveTimer !== undefined) await persist().catch(() => undefined)
    }
  }
}
