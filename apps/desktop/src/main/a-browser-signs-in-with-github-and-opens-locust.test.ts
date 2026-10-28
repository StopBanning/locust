import { createHash, randomBytes, type webcrypto } from 'node:crypto'
import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, describe, expect, it, vi } from 'vitest'

import { roomCore, type Attachment, type Peer, type RoomHost } from '../../../relay/src/room-core.js'
import type { RelayFrame } from '../../../relay/src/protocol.js'
import { createPhoneAnywhere, type RelaySocket } from './phone-anywhere.js'
import type { PhoneCalls } from './phone-host.js'
import { aadOf, deviceIdOf, linkKeys, open, pairingProof, pairingToken, seal, toBase64url } from './relay-crypto.js'

/**
 * A BROWSER SIGNS IN WITH GITHUB AND OPENS LOCUST (0.838). Colin, 2026-10-28: a browser version you "just get to"
 * without the pairing step. The relay's rules (room-core.ts) run in memory with Locust's own link (phone-anywhere.ts):
 * the PC ties its room to a GitHub account once; a browser the relay hands a ticket comes in with GitHub's token, and
 * the PC asks GitHub whose it is before it lets the browser in, with the full window. Open in your browser's code does
 * the same for this computer's own browser.
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
  constructor(readonly send: (text: string) => void, readonly close: () => void) {}
}

function memoryRelay(github?: RoomHost['github']) {
  const stored = new Map<string, unknown>()
  const peers = new Map<MemoryLine, MemoryPeer>()
  let work: Promise<unknown> = Promise.resolve()
  const serial = <T,>(step: () => Promise<T>): Promise<T> => { const next = work.then(step); work = next.catch(() => undefined); return next }
  const core = roomCore({
    store: {
      get: async <T,>(key: string) => structuredClone(stored.get(key)) as T | undefined,
      put: async (key, value) => { stored.set(key, structuredClone(value)) },
      delete: async (keys) => { for (const key of keys) stored.delete(key) },
      deleteAll: async () => { stored.clear() }
    },
    peers: () => [...peers.values()],
    now: () => Date.now(),
    setAlarm: async () => undefined,
    random: (bytes) => randomBytes(bytes).toString('base64url'),
    sha256: async (text) => createHash('sha256').update(text, 'utf8').digest('base64url'),
    ...(github === undefined ? {} : { github })
  })
  const hangUp = (line: MemoryLine): void => { if (peers.delete(line)) setImmediate(() => line.onclose?.()) }
  const connect = (): MemoryLine => {
    const line: MemoryLine = new MemoryLine((text) => {
      const peer = peers.get(line)
      if (peer !== undefined) void serial(() => core.message(peer, text))
    }, () => {
      const peer = peers.get(line)
      if (peer !== undefined) void serial(async () => { await core.closed(peer); peers.delete(line) })
    })
    peers.set(line, new MemoryPeer(line, hangUp))
    void serial(() => core.opened())
    setImmediate(() => line.onopen?.())
    return line
  }
  return {
    connect,
    // What the Worker does once GitHub has said who signed in (apps/relay/src/github.ts).
    linked: (request: Parameters<typeof core.githubLinked>[0]) => serial(() => core.githubLinked(request)),
    ticket: (user: number) => serial(() => core.githubTicket({ user }))
  }
}

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

/**
 * What the page does (phone-page.ts): its own key, then a code's token and proof, or a ticket and GitHub's token. The
 * PC's sealed answer, or the relay's refusal.
 */
async function comeIn(relay: ReturnType<typeof memoryRelay>, room: string, pcKey: string, way: { readonly secret: string } | { readonly ticket: string; readonly github: string }) {
  const pair = await globalThis.crypto.subtle.generateKey({ name: 'X25519' }, false, ['deriveBits']) as unknown as CryptoKeyPair
  const publicKey = toBase64url(new Uint8Array(await globalThis.crypto.subtle.exportKey('raw', pair.publicKey)))
  const device = await deviceIdOf(publicKey)
  const keys = await linkKeys(pair.privateKey, pcKey, room, 'device')
  const line = relay.connect()
  const heard = listen(line)
  await opened(line)
  line.send(JSON.stringify({ t: 'auth', as: 'pairing', token: 'ticket' in way ? way.ticket : await pairingToken(way.secret) }))
  const ok = await heard.next()
  if (ok.t !== 'ok') { line.close(); return { refused: String(ok.code) } }
  const hello = 'ticket' in way
    ? { v: 1, kind: 'signin', github: way.github, name: 'Chrome on Windows' }
    : { v: 1, kind: 'pair', proof: await pairingProof(way.secret), name: 'Chrome on Windows' }
  line.send(JSON.stringify({ t: 'send', to: 'pc', ...(await seal(keys.seal, hello, aadOf(room, device, 'device-to-pc'))), k: publicKey }))
  try {
    const answer = await heard.next((frame) => frame.t === 'msg')
    return { device, said: await open(keys.open, answer.n as string, answer.c as string, aadOf(room, device, 'pc-to-device')) as Record<string, unknown> }
  } finally {
    line.close()
  }
}

const calls = (): PhoneCalls => ({ state: vi.fn(async () => ({ waiting: [], conversations: [] })), turns: vi.fn(async () => []), decide: vi.fn(async () => true), send: vi.fn(async () => ({ ok: true as const })) })
const GOOD = `gho_${'a'.repeat(36)}`
const OTHER = `gho_${'b'.repeat(36)}`
const UNKNOWN = `gho_${'c'.repeat(36)}`

const directories: string[] = []
afterEach(async () => { for (const directory of directories.splice(0)) await rm(directory, { recursive: true, force: true }) })

async function aPc(relay: ReturnType<typeof memoryRelay>) {
  const folder = await mkdtemp(join(tmpdir(), 'locust-github-'))
  directories.push(folder)
  const asked: string[] = []
  const pc = createPhoneAnywhere({
    directory: folder, calls: calls(), relay: () => 'https://relay.test', connect: () => relay.connect(), protectFile: async () => undefined, retryMs: () => 20,
    // GitHub, as the PC asks it: whose each token is.
    githubUser: async (token) => { asked.push(token); return token === GOOD ? 42 : token === OTHER ? 7 : undefined },
    computerName: () => 'DESKTOP-1<b>'
  })
  await pc.setEnabled(true)
  await until(async () => pc.connected())
  return { pc, asked }
}

/** The PC sends the person to GitHub, and the Worker comes back with GitHub's word that they are octo, number 42. */
async function tied(relay: ReturnType<typeof memoryRelay>, pc: Awaited<ReturnType<typeof aPc>>['pc']) {
  const start = new URL((await pc.githubLink())!)
  expect(start.origin + start.pathname).toBe('https://relay.test/v1/github/start')
  const room = start.searchParams.get('room')!
  const answer = await relay.linked({ room, link: start.searchParams.get('link')!, user: 42, login: 'octo' })
  expect(answer.status).toBe(200)
  // What the account's list keeps: the PC's public key, and its name in plain words.
  expect(answer.body.name).toBe('DESKTOP-1b')
  await until(async () => (await pc.settings()).github?.login === 'octo')
  return { room, key: String(answer.body.key) }
}

describe('a browser signs in with GitHub and opens Locust', () => {
  it('lets in a browser with a ticket and the account’s own token, with the full window', async () => {
    const relay = memoryRelay({ forget: async () => undefined })
    const { pc, asked } = await aPc(relay)
    try {
      const { room, key } = await tied(relay, pc)
      const ticket = await relay.ticket(42)
      expect(ticket.status).toBe(200)
      expect(ticket.body.key).toBe(key)
      // With the PC's own code open, a ticket is checked first and does not cost the code a try.
      const came = await comeIn(relay, room, key, { ticket: String(ticket.body.ticket), github: GOOD })
      expect(came.said).toMatchObject({ v: 1, kind: 'paired', device: came.device })
      // GitHub was asked, by the PC, about this token.
      expect(asked).toEqual([GOOD])
      await until(async () => (await pc.settings()).devices.length === 1)
      expect((await pc.settings()).devices[0]).toMatchObject({ name: 'Chrome on Windows', desk: true, github: true })
      // The ticket was good once: after that it is only a wrong code.
      expect(await comeIn(relay, room, key, { ticket: String(ticket.body.ticket), github: GOOD })).toEqual({ refused: 'auth' })
    } finally {
      await pc.dispose()
    }
  })

  it('refuses another account or a token GitHub does not know, and pairs nothing', async () => {
    const relay = memoryRelay({ forget: async () => undefined })
    const { pc } = await aPc(relay)
    try {
      const { room, key } = await tied(relay, pc)
      const other = await comeIn(relay, room, key, { ticket: String((await relay.ticket(42)).body.ticket), github: OTHER })
      expect(other.said).toMatchObject({ kind: 'refused' })
      expect(String(other.said?.message)).toContain('(octo)')
      const unknown = await comeIn(relay, room, key, { ticket: String((await relay.ticket(42)).body.ticket), github: UNKNOWN })
      expect(String(unknown.said?.message)).toContain('could not check your account with GitHub')
      // A ticket is only ever given for the account the room is tied to.
      expect((await relay.ticket(7)).status).toBe(404)
      // Nor does a made-up ticket get in.
      expect(await comeIn(relay, room, key, { ticket: randomBytes(32).toString('base64url'), github: GOOD })).toEqual({ refused: 'auth' })
      expect((await pc.settings()).devices).toEqual([])
    } finally {
      await pc.dispose()
    }
  })

  it('signing out lets the account go: the relay forgets it and gives no more tickets', async () => {
    const forget = vi.fn(async () => undefined)
    const relay = memoryRelay({ forget })
    const { pc } = await aPc(relay)
    try {
      const { room, key } = await tied(relay, pc)
      const waiting = String((await relay.ticket(42)).body.ticket)
      expect((await pc.githubSignOut()).github).toBeUndefined()
      expect(forget).toHaveBeenCalledWith(42, room)
      expect((await relay.ticket(42)).status).toBe(404)
      // A ticket handed out before stops working too.
      expect(await comeIn(relay, room, key, { ticket: waiting, github: GOOD })).toEqual({ refused: 'auth' })
    } finally {
      await pc.dispose()
    }
  })

  it('Open in your browser makes a code for the full window, which a phone is not shown', async () => {
    const relay = memoryRelay()
    const { pc } = await aPc(relay)
    try {
      const url = (await pc.deskCode())!
      expect(url).toMatch(/^https:\/\/relay\.test\/#p=[A-Za-z0-9_-]{22}\.[A-Za-z0-9_-]{43}\.[A-Za-z0-9_-]{22}\.d$/)
      expect((await pc.settings()).code).toMatchObject({ url, desk: true })
      const [room, pcKey, secret] = url.split('#p=')[1]!.split('.') as [string, string, string]
      const came = await comeIn(relay, room, pcKey, { secret })
      expect(came.said).toMatchObject({ kind: 'paired' })
      await until(async () => (await pc.settings()).devices.length === 1)
      expect((await pc.settings()).devices[0]).toMatchObject({ desk: true })
      expect((await pc.settings()).devices[0]!.github).toBeUndefined()
      // An ordinary code pairs without the full window, as before.
      await pc.newCode()
      const plain = (await pc.settings()).code!
      expect(plain.desk).toBeUndefined()
      const [, , next] = plain.url.split('#p=')[1]!.split('.') as [string, string, string]
      await comeIn(relay, room, pcKey, { secret: next })
      await until(async () => (await pc.settings()).devices.length === 2)
      expect((await pc.settings()).devices[1]!.desk).toBeUndefined()
    } finally {
      await pc.dispose()
    }
  })

  it('a relay that does not offer GitHub says so, and nothing is opened', async () => {
    const relay = memoryRelay()
    const { pc } = await aPc(relay)
    try {
      expect(await pc.githubLink()).toBeUndefined()
      expect((await pc.settings()).message).toBe('Locust’s relay does not offer signing in with GitHub yet.')
    } finally {
      await pc.dispose()
    }
  })
})
