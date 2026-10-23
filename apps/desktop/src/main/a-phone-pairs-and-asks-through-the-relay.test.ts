import { createHash, randomBytes, type webcrypto } from 'node:crypto'
import { existsSync } from 'node:fs'
import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it, vi } from 'vitest'

import { roomCore, type Attachment, type Peer } from '../../../relay/src/room-core.js'
import type { RelayFrame } from '../../../relay/src/protocol.js'
import { createPhoneAnywhere, PHONE_ANYWHERE_FILE, type RelaySocket } from './phone-anywhere.js'
import type { PhoneCalls } from './phone-host.js'
import { aadOf, deviceIdOf, linkKeys, open, pairingProof, pairingToken, seal, toBase64url, type LinkKeys } from './relay-crypto.js'

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
function memoryRelay() {
  const stored = new Map<string, unknown>()
  const peers = new Map<MemoryLine, MemoryPeer>()
  let work: Promise<unknown> = Promise.resolve()
  const serial = (step: () => Promise<void>): void => { work = work.then(step).catch(() => undefined) }
  const core = roomCore({
    store: {
      get: async <T>(key: string) => structuredClone(stored.get(key)) as T | undefined,
      put: async (key, value) => { stored.set(key, structuredClone(value)) },
      deleteAll: async () => { stored.clear() }
    },
    peers: () => [...peers.values()],
    now: () => Date.now(),
    setAlarm: async () => undefined,
    random: (bytes) => randomBytes(bytes).toString('base64url'),
    sha256: async (text) => createHash('sha256').update(text, 'utf8').digest('base64url')
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
  return { connect, drop, stored, settled: () => work, pcAuths: () => pcAuths }
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

async function aPc(relay: ReturnType<typeof memoryRelay>, calls = fakeCalls(), directory?: string) {
  const folder = directory ?? await mkdtemp(join(tmpdir(), 'locust-anywhere-'))
  if (directory === undefined) directories.push(folder)
  const pc = createPhoneAnywhere({ directory: folder, calls, relay: () => 'https://relay.test', connect: (url) => relay.connect(url), protectFile: async () => undefined, retryMs: () => 20 })
  return { pc, calls, folder }
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
