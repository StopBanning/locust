import { createHash, randomBytes, type webcrypto } from 'node:crypto'
import { existsSync } from 'node:fs'
import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it, vi } from 'vitest'

import { roomCore, type Attachment, type Peer } from '../../../relay/src/room-core.js'
import type { Letter, RelayFrame } from '../../../relay/src/protocol.js'
import { AWAY_FOUND_ALERT, createPhoneAnywhere, PHONE_ANYWHERE_FILE, type RelaySocket } from './phone-anywhere.js'
import type { PhoneCalls } from './phone-host.js'
import { aadOf, deviceIdOf, linkKeys, open, pairingProof, pairingToken, seal, toBase64url, type LinkKeys } from './relay-crypto.js'
import { deskJoiner, deskParts } from '../shared/desk.js'

/**
 * A PHONE PAIRS AND ASKS THROUGH THE RELAY (0.826). Locust's own link (phone-anywhere.ts), the relay's own rules
 * (apps/relay/src/room-core.ts) run in memory, and a phone doing what the page does with the same lock: a code pairs
 * one phone once, the phone asks what it may, and nothing it asks is done twice.
 */
type CryptoKeyPair = webcrypto.CryptoKeyPair

class MemoryPeer implements Peer {
  attachment: Attachment = { as: 'new', at: Date.now() }
  constructor(private readonly line: MemoryLine, private readonly hangUp: (line: MemoryLine) => void) {}
  attach(next: Attachment): void { this.attachment = next }
  send(frame: RelayFrame): void { setImmediate(() => this.line.onmessage?.(JSON.stringify(frame))) }
  close(): void { this.hangUp(this.line) }
}

class MemoryLine implements RelaySocket {
  onopen: (() => void) | null = null
  onmessage: ((text: string) => void) | null = null
  onclose: (() => void) | null = null
  constructor(readonly url: string, readonly send: (text: string) => void, readonly close: () => void) {}
}

/** One room of the relay, in memory: frames are taken one at a time, as Cloudflare gives them to a room. */
function memoryRelay(pushed?: (endpoint: string, body: string) => number) {
  const stored = new Map<string, unknown>()
  const peers = new Map<MemoryLine, MemoryPeer>()
  let work: Promise<unknown> = Promise.resolve()
  const serial = (step: () => Promise<void>): void => { work = work.then(step).catch(() => undefined) }
  const core = roomCore({
    store: {
      get: async <T>(key: string) => structuredClone(stored.get(key)) as T | undefined,
      put: async (key, value) => { stored.set(key, structuredClone(value)) },
      delete: async (keys) => { for (const key of keys) stored.delete(key) },
      deleteAll: async () => { stored.clear() }
    },
    peers: () => [...peers.values()],
    now: () => Date.now(),
    setAlarm: async () => undefined,
    random: (bytes) => randomBytes(bytes).toString('base64url'),
    sha256: async (text) => createHash('sha256').update(text, 'utf8').digest('base64url'),
    ...(pushed === undefined ? {} : { push: async (endpoint: string, body: string) => pushed(endpoint, body) })
  })
  /** The relay lets a connection go: the other side hears it close. */
  const hangUp = (line: MemoryLine): void => {
    if (!peers.delete(line)) return
    setImmediate(() => line.onclose?.())
  }
  let pcAuths = 0
  const connect = (url: string): MemoryLine => {
    const line: MemoryLine = new MemoryLine(url, (text) => {
      const peer = peers.get(line)
      if (peer === undefined) return
      if (text.includes('"as":"pc"')) pcAuths += 1
      serial(() => core.message(peer, text))
    }, () => {
      const peer = peers.get(line)
      if (peer === undefined) return
      serial(async () => { await core.closed(peer); peers.delete(line) })
    })
    peers.set(line, new MemoryPeer(line, hangUp))
    serial(() => core.opened())
    setImmediate(() => line.onopen?.())
    return line
  }
  /** The network drops: every connection closes without a word. */
  const drop = (): void => {
    for (const [line, peer] of [...peers]) serial(async () => { await core.closed(peer); hangUp(line) })
  }
  return { connect, drop, stored, settled: () => work, pcAuths: () => pcAuths, away: (request: Parameters<typeof core.away>[0]) => core.away(request) }
}

/** The frames a line hears, taken in order by what a test waits for. */
function listen(line: MemoryLine) {
  const frames: Record<string, unknown>[] = []
  const waiting: { test: (frame: Record<string, unknown>) => boolean; resolve: (frame: Record<string, unknown>) => void }[] = []
  line.onmessage = (text) => {
    const frame = JSON.parse(text) as Record<string, unknown>
    const at = waiting.findIndex((wait) => wait.test(frame))
    if (at >= 0) waiting.splice(at, 1)[0]!.resolve(frame)
    else frames.push(frame)
  }
  return {
    frames,
    next: (test: (frame: Record<string, unknown>) => boolean = () => true, ms = 3_000): Promise<Record<string, unknown>> => {
      const at = frames.findIndex(test)
      if (at >= 0) return Promise.resolve(frames.splice(at, 1)[0]!)
      return new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error('nothing came')), ms)
        waiting.push({ test, resolve: (frame) => { clearTimeout(timer); resolve(frame) } })
      })
    }
  }
}

const opened = (line: MemoryLine): Promise<void> => new Promise((resolve) => { line.onopen = () => resolve() })
const until = async (check: () => Promise<boolean>, ms = 3_000): Promise<void> => {
  const end = Date.now() + ms
  while (!(await check())) {
    if (Date.now() > end) throw new Error('it never happened')
    await new Promise((resolve) => setTimeout(resolve, 10))
  }
}

interface Paired { readonly room: string; readonly device: string; readonly token: string; readonly keys: LinkKeys }

/** What the page does with a scanned code: its own key, the token to the relay, the proof to the PC. */
async function pairPhone(relay: ReturnType<typeof memoryRelay>, url: string, options: { readonly proof?: string } = {}): Promise<Paired> {
  const [room, pcKey, secret] = url.split('#p=')[1]!.split('.') as [string, string, string]
  const pair = await globalThis.crypto.subtle.generateKey({ name: 'X25519' }, false, ['deriveBits']) as unknown as CryptoKeyPair
  const publicKey = toBase64url(new Uint8Array(await globalThis.crypto.subtle.exportKey('raw', pair.publicKey)))
  const device = await deviceIdOf(publicKey)
  const keys = await linkKeys(pair.privateKey, pcKey, room, 'device')
  const line = relay.connect(`wss://relay.test/v1/room/${room}`)
  const heard = listen(line)
  await opened(line)
  line.send(JSON.stringify({ t: 'auth', as: 'pairing', token: await pairingToken(secret) }))
  const ok = await heard.next()
  if (ok.t !== 'ok') throw new Error(`refused: ${String(ok.code)}`)
  const hello = await seal(keys.seal, { v: 1, kind: 'pair', proof: options.proof ?? await pairingProof(secret), name: 'iPhone' }, aadOf(room, device, 'device-to-pc'))
  line.send(JSON.stringify({ t: 'send', to: 'pc', ...hello, k: publicKey }))
  try {
    const answer = await heard.next((frame) => frame.t === 'msg', 1_000)
    const said = await open(keys.open, answer.n as string, answer.c as string, aadOf(room, device, 'pc-to-device')) as { token: string }
    return { room, device, token: said.token, keys }
  } finally {
    line.close()
  }
}

/** A paired phone, asking as the page asks. */
async function phoneLine(relay: ReturnType<typeof memoryRelay>, phone: Paired, firstSeq = 0) {
  const line = relay.connect(`wss://relay.test/v1/room/${phone.room}`)
  const heard = listen(line)
  await opened(line)
  line.send(JSON.stringify({ t: 'auth', as: 'device', device: phone.device, token: phone.token }))
  const ok = await heard.next()
  let seq = firstSeq
  return {
    ok,
    line,
    heard,
    ask: async (path: string, body?: unknown): Promise<{ said: Record<string, unknown>; frame: string }> => {
      seq += 1
      const id = `q${String(seq)}`
      const box = await seal(phone.keys.seal, { v: 1, kind: 'req', id, seq, t: Date.now(), path, ...(body === undefined ? {} : { body }) }, aadOf(phone.room, phone.device, 'device-to-pc'))
      const frame = JSON.stringify({ t: 'send', to: 'pc', ...box })
      line.send(frame)
      const answer = await heard.next((heardFrame) => heardFrame.t === 'msg')
      const said = await open(phone.keys.open, answer.n as string, answer.c as string, aadOf(phone.room, phone.device, 'pc-to-device')) as Record<string, unknown>
      expect(said.id).toBe(id)
      return { said, frame }
    }
  }
}

const STATE = { waiting: [{ approvalId: 'ap1', missionId: 'm1', summary: 'Run npm test', requestedAt: '2026-10-23T00:00:00.000Z' }], conversations: [] }
function fakeCalls() {
  return {
    state: vi.fn(async () => STATE),
    turns: vi.fn(async (missionId: string) => missionId === 'm1' ? [] : undefined),
    decide: vi.fn(async () => true),
    send: vi.fn(async () => ({ ok: true as const }))
  } satisfies PhoneCalls
}

const directories: string[] = []
afterEach(async () => { for (const directory of directories.splice(0)) await rm(directory, { recursive: true, force: true }) })

async function aPc(relay: ReturnType<typeof memoryRelay>, calls = fakeCalls(), directory?: string, away?: Parameters<typeof createPhoneAnywhere>[0]['away']) {
  const folder = directory ?? await mkdtemp(join(tmpdir(), 'locust-anywhere-'))
  if (directory === undefined) directories.push(folder)
  const pc = createPhoneAnywhere({ directory: folder, calls, relay: () => 'https://relay.test', connect: (url) => relay.connect(url), protectFile: async () => undefined, retryMs: () => 20, ...(away === undefined ? {} : { away }) })
  return { pc, calls, folder }
}

/** What the phone's browser does with a push: opens it with its own key (RFC 8291), as Chrome or Safari would. */
async function openPush(body: Buffer, receiver: CryptoKeyPair, auth: Uint8Array): Promise<unknown> {
  const subtle = globalThis.crypto.subtle
  const salt = body.subarray(0, 16)
  const keyLength = body[20]!
  const senderRaw = body.subarray(21, 21 + keyLength)
  const sealed = body.subarray(21 + keyLength)
  const receiverRaw = new Uint8Array(await subtle.exportKey('raw', receiver.publicKey))
  const sender = await subtle.importKey('raw', senderRaw, { name: 'ECDH', namedCurve: 'P-256' }, false, [])
  const shared = new Uint8Array(await subtle.deriveBits({ name: 'ECDH', public: sender }, receiver.privateKey, 256))
  const hkdf = async (saltBytes: Uint8Array, ikm: Uint8Array, info: Uint8Array, bytes: number): Promise<Uint8Array> =>
    new Uint8Array(await subtle.deriveBits({ name: 'HKDF', hash: 'SHA-256', salt: saltBytes, info }, await subtle.importKey('raw', ikm, 'HKDF', false, ['deriveBits']), bytes * 8))
  const text = new TextEncoder()
  const ikm = await hkdf(auth, shared, Buffer.concat([text.encode('WebPush: info\0'), receiverRaw, senderRaw]), 32)
  const cek = await hkdf(salt, ikm, text.encode('Content-Encoding: aes128gcm\0'), 16)
  const nonce = await hkdf(salt, ikm, text.encode('Content-Encoding: nonce\0'), 12)
  const plain = new Uint8Array(await subtle.decrypt({ name: 'AES-GCM', iv: nonce }, await subtle.importKey('raw', cek, 'AES-GCM', false, ['decrypt']), sealed))
  // The last byte is the record's delimiter, 2.
  expect(plain.at(-1)).toBe(2)
  return JSON.parse(new TextDecoder().decode(plain.subarray(0, -1)))
}

describe('a phone pairs and asks through the relay', () => {
  it('pairs a phone by its code, then answers what the page asks and does what it may', async () => {
    const relay = memoryRelay()
    const { pc, calls } = await aPc(relay)
    const on = await pc.setEnabled(true)
    expect(on.enabled).toBe(true)
    expect(on.code?.url).toMatch(/^https:\/\/relay\.test\/#p=[A-Za-z0-9_-]{22}\.[A-Za-z0-9_-]{43}\.[A-Za-z0-9_-]{22}$/)
    await until(async () => (await pc.settings()).connected)

    const phone = await pairPhone(relay, on.code!.url)
    await until(async () => (await pc.settings()).devices.length === 1)
    const shown = await pc.settings()
    expect(shown.devices[0]).toMatchObject({ id: phone.device, name: 'iPhone' })
    // The code is spent: Settings shows none until a new one is made.
    expect(shown.code).toBeUndefined()

    const device = await phoneLine(relay, phone)
    expect(device.ok).toEqual({ t: 'ok', as: 'device', pc: true })
    expect((await device.ask('/api/state')).said).toMatchObject({ status: 200, data: STATE })
    expect((await device.ask('/api/turns?id=gone')).said).toMatchObject({ status: 404 })
    expect((await device.ask('/api/decide', { approvalId: 'ap1', approve: true })).said).toMatchObject({ status: 200, data: { ok: true } })
    expect(calls.decide).toHaveBeenCalledWith('ap1', true)
    expect((await device.ask('/api/send', { missionId: 'm1', text: '   ' })).said).toMatchObject({ status: 400 })
    expect((await device.ask('/api/send', { missionId: 'm1', text: 'Carry on' })).said).toMatchObject({ status: 200, data: { ok: true } })
    expect(calls.send).toHaveBeenCalledWith('m1', 'Carry on')
    expect((await device.ask('/api/anything-else')).said).toMatchObject({ status: 404 })
    await pc.dispose()
  })

  it('sends an alert, sealed for that phone\'s browser alone, only to a phone that turned alerts on and is not open', async () => {
    const posted: { endpoint: string; body: string }[] = []
    let answer = 201
    const relay = memoryRelay((endpoint, body) => { posted.push({ endpoint, body }); return answer })
    const { pc } = await aPc(relay)
    const on = await pc.setEnabled(true)
    await until(async () => (await pc.settings()).connected)
    const phone = await pairPhone(relay, on.code!.url)
    const device = await phoneLine(relay, phone)

    // The browser's own push keys, as its subscription carries them.
    const receiver = await globalThis.crypto.subtle.generateKey({ name: 'ECDH', namedCurve: 'P-256' }, true, ['deriveBits']) as unknown as CryptoKeyPair
    const auth = new Uint8Array(randomBytes(16))
    const endpoint = 'https://fcm.googleapis.com/fcm/send/phone-1'
    const subscribed = await device.ask('/api/alerts', { endpoint, p256dh: toBase64url(new Uint8Array(await globalThis.crypto.subtle.exportKey('raw', receiver.publicKey))), auth: toBase64url(auth) })
    expect(subscribed.said).toMatchObject({ status: 200, data: { ok: true } })
    await until(async () => (await pc.settings()).devices[0]?.alerts === true)

    // Open on the phone: the page shows it itself, so nothing is pushed.
    expect(await pc.alert({ kind: 'approval', title: 'Mira needs your approval', body: 'The turn is paused until you answer.', tag: 'waiting' })).toBe(0)
    expect(posted).toEqual([])

    device.line.close()
    await until(async () => !(await pc.settings()).devices[0]?.online)
    expect(await pc.alert({ kind: 'approval', title: 'Mira needs your approval', body: 'The turn is paused until you answer.', tag: 'waiting' })).toBe(1)
    expect(posted.map((one) => one.endpoint)).toEqual([endpoint])
    const body = Buffer.from(posted[0]!.body, 'base64url')
    // Nothing of what it says travels in the clear.
    expect(body.toString('latin1')).not.toContain('Mira')
    expect(await openPush(body, receiver, auth)).toEqual({ title: 'Mira needs your approval', body: 'The turn is paused until you answer.', tag: 'waiting' })

    // The push service says the subscription is gone: the phone is sent nothing more until it signs up again.
    answer = 410
    expect(await pc.alert({ kind: 'scout', title: 'Scout', body: 'The folder’s check fails.', tag: 'scout:check' })).toBe(0)
    await until(async () => (await pc.settings()).devices[0]?.alerts !== true)
    expect(await pc.alert({ kind: 'scout', title: 'Scout', body: 'The folder’s check fails.', tag: 'scout:check' })).toBe(0)
    expect(posted).toHaveLength(2)
    await pc.dispose()
  })

  it('does a copied request once, and never again', async () => {
    const relay = memoryRelay()
    const { pc, calls } = await aPc(relay)
    const on = await pc.setEnabled(true)
    await until(async () => (await pc.settings()).connected)
    const device = await phoneLine(relay, await pairPhone(relay, on.code!.url))
    const { frame } = await device.ask('/api/send', { missionId: 'm1', text: 'Only once' })
    device.line.send(frame)
    device.line.send(frame)
    await relay.settled()
    await new Promise((resolve) => setTimeout(resolve, 50))
    expect(calls.send).toHaveBeenCalledTimes(1)
    expect(device.heard.frames.filter((heardFrame) => heardFrame.t === 'msg')).toHaveLength(0)
    await pc.dispose()
  })

  it('pairs one phone per code, and none that cannot prove it saw the code', async () => {
    const relay = memoryRelay()
    const { pc } = await aPc(relay)
    const on = await pc.setEnabled(true)
    await until(async () => (await pc.settings()).connected)

    // The relay sees the code's token, never its proof: with the token alone, nothing is paired.
    await expect(pairPhone(relay, on.code!.url, { proof: toBase64url(randomBytes(32)) })).rejects.toThrow('nothing came')
    expect((await pc.settings()).devices).toHaveLength(0)

    await pairPhone(relay, on.code!.url)
    await until(async () => (await pc.settings()).devices.length === 1)
    await expect(pairPhone(relay, on.code!.url)).rejects.toThrow('refused: pairing-closed')
    expect((await pc.settings()).devices).toHaveLength(1)

    // A new code pairs the next one.
    const next = await pc.newCode()
    await pairPhone(relay, next.code!.url)
    await until(async () => (await pc.settings()).devices.length === 2)
    await pc.dispose()
  })

  it('closes a removed phone, and forgets every phone and the room when it is turned off', async () => {
    const relay = memoryRelay()
    const { pc, folder } = await aPc(relay)
    const on = await pc.setEnabled(true)
    await until(async () => (await pc.settings()).connected)
    const phone = await pairPhone(relay, on.code!.url)
    const device = await phoneLine(relay, phone)

    await pc.remove(phone.device)
    expect(await device.heard.next((frame) => frame.t === 'removed')).toEqual({ t: 'removed' })
    expect((await pc.settings()).devices).toHaveLength(0)

    expect(existsSync(join(folder, PHONE_ANYWHERE_FILE))).toBe(true)
    const off = await pc.setEnabled(false)
    expect(off).toMatchObject({ enabled: false, connected: false, devices: [] })
    await relay.settled()
    expect(relay.stored.size).toBe(0)
    expect(existsSync(join(folder, PHONE_ANYWHERE_FILE))).toBe(false)
    await pc.dispose()
  })

  it('connects again after the line drops, and keeps its phones and their numbers across a restart', async () => {
    const relay = memoryRelay()
    const first = await aPc(relay)
    const on = await first.pc.setEnabled(true)
    await until(async () => (await first.pc.settings()).connected)
    const phone = await pairPhone(relay, on.code!.url)
    const device = await phoneLine(relay, phone)
    const { frame: asked } = await device.ask('/api/send', { missionId: 'm1', text: 'Before' })

    const before = relay.pcAuths()
    relay.drop()
    // Not the old connection's word: the PC has come back on a new one.
    await until(async () => relay.pcAuths() > before && (await first.pc.settings()).connected)
    await relay.settled()
    const again = await phoneLine(relay, phone, 1)
    expect((await again.ask('/api/state')).said).toMatchObject({ status: 200 })
    await first.pc.dispose()

    const calls = fakeCalls()
    const second = await aPc(relay, calls, first.folder)
    await second.pc.load()
    await until(async () => (await second.pc.settings()).connected)
    expect((await second.pc.settings()).devices.map((shown) => shown.id)).toEqual([phone.device])
    const later = await phoneLine(relay, phone, 2)
    expect((await later.ask('/api/send', { missionId: 'm1', text: 'After' })).said).toMatchObject({ status: 200 })
    // The request sent before the restart is still a copy after it.
    later.line.send(asked)
    await relay.settled()
    await new Promise((resolve) => setTimeout(resolve, 50))
    expect(calls.send).toHaveBeenCalledTimes(1)
    expect(calls.send).toHaveBeenCalledWith('m1', 'After')
    await second.pc.dispose()
  })
})

describe('Scout’s away look through the relay (0.829)', () => {
  it('gives a phone the away key, keeps its alerts at the relay to ring with the PC off, and hands the PC the letters', async () => {
    const posted: { endpoint: string; body: string }[] = []
    const relay = memoryRelay((endpoint, body) => { posted.push({ endpoint, body }); return 201 })
    let refreshed = 0
    const heard: Letter[] = []
    const key = toBase64url(new Uint8Array(randomBytes(32)))
    let awayOn = true
    const { pc } = await aPc(relay, fakeCalls(), undefined, {
      forPhone: async () => (awayOn ? { key, repo: 'acme/app' } : undefined),
      refresh: () => { refreshed += 1 },
      letter: (letter) => { heard.push(letter) }
    })
    const on = await pc.setEnabled(true)
    await until(async () => (await pc.settings()).connected)
    expect(refreshed).toBe(1)
    const phone = await pairPhone(relay, on.code!.url)
    const device = await phoneLine(relay, phone)

    // With the PC on, the phone is given the key to open the letters, over its sealed line.
    expect((await device.ask('/api/away')).said).toMatchObject({ status: 200, data: { key, repo: 'acme/app' } })
    awayOn = false
    expect((await device.ask('/api/away')).said).toMatchObject({ status: 200, data: { off: true } })

    const receiver = await globalThis.crypto.subtle.generateKey({ name: 'ECDH', namedCurve: 'P-256' }, true, ['deriveBits']) as unknown as CryptoKeyPair
    const auth = new Uint8Array(randomBytes(16))
    const endpoint = 'https://fcm.googleapis.com/fcm/send/phone-1'
    await device.ask('/api/alerts', { endpoint, p256dh: toBase64url(new Uint8Array(await globalThis.crypto.subtle.exportKey('raw', receiver.publicKey))), auth: toBase64url(auth) })
    // A phone's alerts changed: the away look's are seen to again.
    expect(refreshed).toBe(2)

    const token = toBase64url(new Uint8Array(randomBytes(32)))
    expect(await pc.awayOpen(createHash('sha256').update(token, 'utf8').digest('base64url'))).toBe(true)
    expect(await pc.keepAwayAlerts()).toBe(true)
    // Sealed ahead of time for the phone's browser alone: nothing of what it says is in the clear.
    expect(posted).toEqual([])

    // A letter while the PC is connected: it hears it at once.
    const box = { n: toBase64url(new Uint8Array(randomBytes(12))), c: toBase64url(new Uint8Array(randomBytes(80))) }
    const first = await relay.away({ method: 'POST', token, body: JSON.stringify(box) })
    expect(first.status).toBe(200)
    await until(async () => heard.length === 1)
    expect(heard[0]).toMatchObject({ from: 'away', ...box })

    // The PC gone and the phone closed: the away look rings the phone with what its PC kept for it.
    await pc.dispose()
    device.line.close()
    await relay.settled()
    await until(async () => (await relay.away({ method: 'GET', token })).body.pc === false)
    const rung = await relay.away({ method: 'POST', token, body: JSON.stringify({ ...box, alert: 'found' }) })
    expect(rung.body.rung).toBe(1)
    expect(posted.map((one) => one.endpoint)).toEqual([endpoint])
    const body = Buffer.from(posted[0]!.body, 'base64url')
    expect(await openPush(body, receiver, auth)).toEqual(AWAY_FOUND_ALERT)

    // Back: the letters left while it was off, a page at a time; and its own, saying what it told.
    const back = await aPc(relay, fakeCalls(), (await import('node:path')).dirname(join(directories[0]!, PHONE_ANYWHERE_FILE)))
    await back.pc.load()
    await until(async () => (await back.pc.settings()).connected)
    const page = await back.pc.letters()
    expect(page?.items.map((item) => item.from)).toEqual(['away', 'away'])
    expect(await back.pc.post(box)).toBe(true)
    const after = await back.pc.letters(page!.items.at(-1)!.id)
    expect(after?.items.map((item) => item.from)).toEqual(['pc'])
    await back.pc.dispose()
  })
})

describe('Locust in a browser through the relay (0.830)', () => {
  it('takes a browser’s frames only once Full window is on, in order, once each, and in parts when long', async () => {
    const relay = memoryRelay()
    const frames: { device: string; asks: readonly unknown[] }[] = []
    const folder = await mkdtemp(join(tmpdir(), 'locust-anywhere-desk-'))
    directories.push(folder)
    const pc = createPhoneAnywhere({
      directory: folder, calls: fakeCalls(), relay: () => 'https://relay.test', connect: (url) => relay.connect(url), protectFile: async () => undefined, retryMs: () => 20,
      desk: { frame: (device, asks) => { frames.push({ device, asks }) }, forget: () => undefined }
    })
    const on = await pc.setEnabled(true)
    await until(async () => (await pc.settings()).connected)
    const phone = await pairPhone(relay, on.code!.url)
    const device = await phoneLine(relay, phone)
    const session = toBase64url(new Uint8Array(randomBytes(9)))
    const sendFrame = async (q: number, items: unknown[]): Promise<void> => {
      const box = await seal(phone.keys.seal, { v: 1, kind: 'desk', s: session, q, t: Date.now(), items }, aadOf(phone.room, phone.device, 'device-to-pc'))
      device.line.send(JSON.stringify({ t: 'send', to: 'pc', ...box }))
    }
    const heardFrame = async (): Promise<Record<string, unknown>> => {
      const frame = await device.heard.next((one) => one.t === 'msg')
      return await open(phone.keys.open, frame.n as string, frame.c as string, aadOf(phone.room, phone.device, 'pc-to-device')) as Record<string, unknown>
    }

    // Full window off: refused, by id, in words.
    await sendFrame(1, [{ kind: 'hello', id: 'h1' }])
    expect(await heardFrame()).toMatchObject({ v: 1, kind: 'desk', items: [{ kind: 'res', id: 'h1', ok: false, refused: true }] })
    expect(frames).toEqual([])

    expect((await pc.setDesk(phone.device, true)).devices[0]?.desk).toBe(true)
    await sendFrame(2, [{ kind: 'hello', id: 'h2' }])
    await until(async () => frames.length === 1)
    expect(frames[0]).toEqual({ device: phone.device, asks: [{ kind: 'hello', id: 'h2' }] })
    // The same frame again, from the relay: dropped. An older one too.
    await sendFrame(2, [{ kind: 'hello', id: 'again' }])
    await sendFrame(1, [{ kind: 'hello', id: 'older' }])
    await sendFrame(3, [{ kind: 'hello', id: 'h3' }])
    await until(async () => frames.length === 2)
    await relay.settled()
    expect(frames.map((one) => (one.asks[0] as { id: string }).id)).toEqual(['h2', 'h3'])

    // A long frame from the browser, in parts.
    const text = JSON.stringify({ v: 1, kind: 'desk', s: session, q: 4, t: Date.now(), items: [{ kind: 'call', id: 'big', channel: 'mission:read', args: JSON.stringify(['x'.repeat(300_000)]) }] })
    for (const part of deskParts('big-frame', text)) {
      const box = await seal(phone.keys.seal, part, aadOf(phone.room, phone.device, 'device-to-pc'))
      device.line.send(JSON.stringify({ t: 'send', to: 'pc', ...box }))
    }
    await until(async () => frames.length === 3)
    expect((frames[2]!.asks[0] as { id: string }).id).toBe('big')

    // And a long answer to it, in parts the browser puts back together.
    expect(await pc.tellDesk(phone.device, [{ kind: 'res', id: 'big', ok: true, value: JSON.stringify('y'.repeat(500_000)) }])).toBe(true)
    const rejoin = deskJoiner()
    let whole: string | undefined
    while (whole === undefined) {
      const said = await heardFrame()
      expect(said.kind).toBe('desk-part')
      whole = rejoin(said as never)
    }
    expect(JSON.parse(whole)).toMatchObject({ v: 1, kind: 'desk', items: [{ kind: 'res', id: 'big', ok: true }] })

    // Full window off again: the browser that had the window lets it go.
    expect((await pc.setDesk(phone.device, false)).devices[0]?.desk).toBeUndefined()
    await pc.dispose()
  })
})
