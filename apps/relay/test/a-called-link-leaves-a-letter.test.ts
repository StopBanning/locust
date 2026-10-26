import { createHash, randomBytes } from 'node:crypto'
import { describe, expect, it } from 'vitest'
import { HOOK_POSTS_PER_HOUR, LETTER_KEPT_MS, MAX_LETTERS, roomCore, type Attachment, type Peer, type RoomHost } from '../src/room-core.js'
import { HOOK_BODY_CHARS, MAX_FRAME, parseClientFrame, type RelayFrame } from '../src/protocol.js'
import { hookBody } from '../src/room.js'

/** A CALLED LINK LEAVES A LETTER (2026-10-25). Accepted means stored, not that a PC or agent has answered. */
const secret = (): string => randomBytes(32).toString('base64url')
const hash = (text: string): string => createHash('sha256').update(text).digest('base64url')
async function fixture() {
  let now = 1_800_000_000_000
  const stored = new Map<string, unknown>()
  const frames: RelayFrame[] = []
  let attachment: Attachment = { as: 'new', at: now }
  const peer: Peer = { get attachment() { return attachment }, attach: (next) => { attachment = next }, send: (frame) => { frames.push(frame) }, close: () => undefined }
  const peers: Peer[] = [peer]
  const host: RoomHost = {
    store: { get: async <T>(key: string) => structuredClone(stored.get(key)) as T | undefined, put: async (key, value) => { stored.set(key, structuredClone(value)) }, delete: async (keys) => { for (const key of keys) stored.delete(key) }, deleteAll: async () => { stored.clear() } },
    peers: () => peers, now: () => now, setAlarm: async () => undefined, random: (size) => randomBytes(size).toString('base64url'), sha256: async (text) => hash(text)
  }
  const core = roomCore(host)
  const say = (frame: unknown) => core.message(peer, JSON.stringify(frame))
  await say({ t: 'auth', as: 'pc', token: secret() })
  const password = secret()
  await say({ t: 'hook-open', hook: 'rt_a', hash: hash(password) })
  const call = (token = password, body = 'hello', method = 'POST') => core.hook({ secret: token, body, method, contentType: 'text/plain' })
  return { core, say, frames, stored, peers, password, call, tick: (ms: number) => { now += ms } }
}

describe('a called link leaves a letter', () => {
  it('answers 202 immediately, keeps body, type and time, and tells only the PC', async () => {
    const room = await fixture()
    expect(await room.call()).toEqual({ status: 202, body: { accepted: true } })
    expect(room.frames.at(-1)).toEqual({ t: 'hook-letter', hook: 'rt_a' })
    await room.say({ t: 'hook-letters', hook: 'rt_a' })
    const frame = room.frames.at(-1)
    if (frame?.t !== 'hook-letters') throw new Error('No letter')
    expect(frame.items[0]).toMatchObject({ hook: 'rt_a', body: 'hello', contentType: 'text/plain', at: 1_800_000_000_000 })
    expect(room.stored.get(`letter:${frame.items[0]!.id}`)).toEqual(frame.items[0])
    expect(JSON.stringify([...room.stored.values()])).not.toContain(room.password)
    room.peers.length = 0
    expect((await room.call()).status).toBe(202)
  })

  it('answers the same 404 for a wrong password, an unknown room and a closed link; rotation retires the old one', async () => {
    const room = await fixture()
    const wrong = await room.call(secret())
    expect(wrong).toEqual({ status: 404, body: { error: 'not-found' } })
    const next = secret()
    await room.say({ t: 'hook-open', hook: 'rt_a', hash: hash(next) })
    expect(await room.call()).toEqual(wrong)
    expect((await room.call(next)).status).toBe(202)
    await room.say({ t: 'hook-close', hook: 'rt_a' })
    expect(await room.call(next)).toEqual(wrong)
    expect([...room.stored.keys()].filter((key) => key.startsWith('letter:'))).toEqual([])
    room.stored.clear()
    expect(await room.call(next)).toEqual(wrong)
  })

  it('refuses non-POST and oversized UTF-8 bodies, and caps calls per hook per hour across rotation', async () => {
    const room = await fixture()
    expect((await room.call(room.password, '', 'GET')).status).toBe(405)
    expect((await room.call(room.password, 'é'.repeat(MAX_FRAME / 2 + 1))).status).toBe(413)
    for (let index = 0; index < HOOK_POSTS_PER_HOUR; index += 1) expect((await room.call()).status).toBe(202)
    expect((await room.call()).status).toBe(429)
    const next = secret()
    await room.say({ t: 'hook-open', hook: 'rt_a', hash: hash(next) })
    expect((await room.call(next)).status).toBe(429)
    room.tick(3_600_001)
    expect((await room.call(next)).status).toBe(202)
  })

  it('collects offline letters once acknowledged, drops a week-old body and reports its loss', async () => {
    const room = await fixture()
    await room.call()
    await room.say({ t: 'hook-letters', hook: 'rt_a', id: 'p' })
    let frame = room.frames.at(-1)
    expect(frame).toMatchObject({ t: 'hook-letters', lost: 0, items: [{ body: 'hello' }] })
    if (frame?.t !== 'hook-letters') throw new Error('No page')
    await room.say({ t: 'hook-collected', hook: 'rt_a', letters: frame.items.map((item) => item.id) })
    expect([...room.stored.keys()].filter((key) => key.startsWith('letter:'))).toEqual([])
    await room.call()
    room.tick(LETTER_KEPT_MS)
    // A read also prunes: expiry is not dependent on an alarm having run first.
    await room.say({ t: 'hook-letters', hook: 'rt_a' })
    frame = room.frames.at(-1)
    expect(frame).toMatchObject({ t: 'hook-letters', lost: 1, items: [] })
    expect([...room.stored.keys()].filter((key) => key.startsWith('letter:'))).toEqual([])
  })

  it('shares the existing cap, never exposes hook bodies to devices or Scout, and keeps them when away is turned off', async () => {
    const room = await fixture()
    for (let index = 0; index <= MAX_LETTERS; index += 1) {
      if (index % HOOK_POSTS_PER_HOUR === 0) room.tick(3_600_001)
      await room.call(room.password, String(index))
    }
    expect([...room.stored.keys()].filter((key) => key.startsWith('letter:'))).toHaveLength(MAX_LETTERS)
    await room.say({ t: 'letters' })
    expect(room.frames.at(-1)).toMatchObject({ t: 'letters', items: [] })
    await room.say({ t: 'away-close' })
    expect([...room.stored.keys()].filter((key) => key.startsWith('letter:'))).toHaveLength(MAX_LETTERS)
    await room.say({ t: 'hook-letters', hook: 'rt_a' })
    expect(room.frames.at(-1)).toMatchObject({ t: 'hook-letters', lost: 1 })
    const deviceFrames: RelayFrame[] = []
    room.peers.push({ attachment: { as: 'device', device: 'phone', at: 0, rate: { start: 0, count: 0 } }, attach: () => undefined, close: () => undefined, send: (frame) => { deviceFrames.push(frame) } })
    await room.call()
    expect(deviceFrames).toEqual([])
  })

  it('reads chunked bodies within the byte limit, cancels an oversized stream, and validates hook frames', async () => {
    let cancelled = false
    const stream = new ReadableStream<Uint8Array>({ start(controller) { controller.enqueue(new Uint8Array(MAX_FRAME)); controller.enqueue(new Uint8Array(1)) }, cancel() { cancelled = true } })
    expect(await hookBody(new Request('https://example.test', { method: 'POST', body: stream, duplex: 'half' } as RequestInit))).toBeUndefined()
    expect(cancelled).toBe(true)
    expect(await hookBody(new Request('https://example.test', { method: 'POST', body: 'hello' }))).toBe('hello')
    expect(parseClientFrame(JSON.stringify({ t: 'hook-open', hook: 'rt_a', hash: hash(secret()) }))?.t).toBe('hook-open')
    expect(parseClientFrame(JSON.stringify({ t: 'hook-open', hook: '../x', hash: hash(secret()) }))).toBeUndefined()
    expect(parseClientFrame(JSON.stringify({ t: 'hook-collected', hook: 'rt_a', letters: ['wrong'] }))).toBeUndefined()
  })

  it('keeps a full-size body in small storage values and deletes every part after collection', async () => {
    const room = await fixture()
    const body = '\u0001'.repeat(MAX_FRAME)
    expect((await room.call(room.password, body)).status).toBe(202)
    for (const value of room.stored.values()) expect(Buffer.byteLength(JSON.stringify(value))).toBeLessThan(128 * 1024)
    await room.say({ t: 'hook-letters', hook: 'rt_a' })
    const frame = room.frames.at(-1)
    if (frame?.t !== 'hook-letters') throw new Error('No letter')
    expect(frame.items[0]?.body).toBe(body.slice(0, HOOK_BODY_CHARS + 1))
    expect(Buffer.byteLength(JSON.stringify(frame))).toBeLessThan(MAX_FRAME)
    await room.say({ t: 'hook-collected', hook: 'rt_a', letters: frame.items.map((item) => item.id) })
    expect([...room.stored.keys()].filter((key) => key.startsWith('letter:'))).toEqual([])
  })
})
