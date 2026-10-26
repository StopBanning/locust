import { createHash, randomBytes } from 'node:crypto'

import { describe, expect, it } from 'vitest'

import { MAX_NOTES, parseClientFrame, type RelayFrame } from '../src/protocol.js'
import { LETTER_KEPT_MS, roomCore, type Attachment, type Peer, type RoomHost } from '../src/room-core.js'

/**
 * A PHONE LEAVES A NOTE FOR ITS PC (0.837). With the PC off, a paired phone leaves a message at the relay, sealed with
 * the key it shares with its PC. The relay keeps the sealed box, who left it and when; the PC reads every note when it
 * is back, a phone only which of its own still wait; only the phone that left one takes it back; and the PC either
 * collects it or says in one word why it held it.
 */
const token = (): string => randomBytes(32).toString('base64url')
const hash = (text: string): string => createHash('sha256').update(text, 'utf8').digest('base64url')
const deviceId = (): string => randomBytes(16).toString('base64url')
const sealed = (): { n: string; c: string } => ({ n: randomBytes(12).toString('base64url'), c: randomBytes(300).toString('base64url') })

class FakePeer implements Peer {
  attachment: Attachment = { as: 'new', at: 0 }
  readonly heard: RelayFrame[] = []
  closed = false
  constructor(private readonly gone: (peer: FakePeer) => void) {}
  attach(next: Attachment): void { this.attachment = next }
  send(frame: RelayFrame): void { if (!this.closed) this.heard.push(frame) }
  close(): void { this.closed = true; this.gone(this) }
  last(): RelayFrame | undefined { return this.heard.at(-1) }
}

function aRoom() {
  const stored = new Map<string, unknown>()
  const peers: FakePeer[] = []
  let now = 1_800_000_000_000
  const host: RoomHost = {
    store: {
      get: async <T>(key: string) => structuredClone(stored.get(key)) as T | undefined,
      put: async (key, value) => { stored.set(key, structuredClone(value)) },
      delete: async (keys) => { for (const key of keys) stored.delete(key) },
      deleteAll: async () => { stored.clear() }
    },
    peers: () => [...peers],
    now: () => now,
    setAlarm: async () => undefined,
    random: (bytes) => randomBytes(bytes).toString('base64url'),
    sha256: async (text) => hash(text)
  }
  const core = roomCore(host)
  const drop = (peer: FakePeer): void => { const at = peers.indexOf(peer); if (at >= 0) peers.splice(at, 1) }
  const join = async (): Promise<FakePeer> => { const peer = new FakePeer(drop); peers.push(peer); await core.opened(); return peer }
  const say = (peer: FakePeer, frame: unknown): Promise<void> => core.message(peer, JSON.stringify(frame))
  const pcToken = token()
  const pc = async (): Promise<FakePeer> => { const peer = await join(); await say(peer, { t: 'auth', as: 'pc', token: pcToken }); return peer }
  const leave = async (peer: FakePeer): Promise<void> => { await core.closed(peer); drop(peer) }
  const phone = async (by: FakePeer): Promise<{ peer: FakePeer; device: string }> => {
    const device = deviceId()
    const secret = token()
    await say(by, { t: 'allow', device, hash: hash(secret), id: 'a' })
    const peer = await join()
    await say(peer, { t: 'auth', as: 'device', device, token: secret })
    return { peer, device }
  }
  const note = async (peer: FakePeer, box = sealed()): Promise<string> => {
    await say(peer, { t: 'note', ...box, id: 'n' })
    const said = peer.last()
    if (said?.t !== 'noted') throw new Error(`not kept: ${JSON.stringify(said)}`)
    return said.note
  }
  return { core, say, pc, phone, leave, note, stored, tick: (ms: number) => { now += ms } }
}

describe('A phone leaves a note for its PC', () => {
  it('keeps the sealed box while the PC is off, and gives it to the PC when it is back', async () => {
    const room = aRoom()
    const first = await room.pc()
    const { peer: phone, device } = await room.phone(first)
    await room.leave(first)
    const box = sealed()
    const id = await room.note(phone, box)
    // What the relay keeps is the box it was given, who left it and when: nothing it could read.
    expect(room.stored.get(`note:${id}`)).toEqual({ id, at: expect.any(Number), device, ...box })
    const back = await room.pc()
    await room.say(back, { t: 'notes', id: 'q' })
    expect(back.last()).toEqual({ t: 'notes', items: [{ id, at: expect.any(Number), device, ...box }], id: 'q' })
  })

  it('tells a PC that is connected that a note came, and nobody else', async () => {
    const room = aRoom()
    const pc = await room.pc()
    const { peer: phone } = await room.phone(pc)
    const { peer: other } = await room.phone(pc)
    await room.note(phone)
    expect(pc.last()).toEqual({ t: 'notes-changed' })
    expect(other.heard.some((frame) => frame.t === 'notes-changed')).toBe(false)
  })

  it('lets only a phone leave one, and shows a phone only its own, without the box', async () => {
    const room = aRoom()
    const pc = await room.pc()
    await room.say(pc, { t: 'note', ...sealed(), id: 'p' })
    expect(pc.last()).toEqual({ t: 'err', code: 'not-owner', id: 'p' })
    const { peer: one } = await room.phone(pc)
    const { peer: two } = await room.phone(pc)
    const mine = await room.note(one)
    await room.note(two)
    await room.say(one, { t: 'notes', id: 'm' })
    expect(one.last()).toEqual({ t: 'my-notes', items: [{ id: mine, at: expect.any(Number) }], id: 'm' })
  })

  it('lets only the phone that left a note take it back', async () => {
    const room = aRoom()
    const pc = await room.pc()
    const { peer: one } = await room.phone(pc)
    const { peer: two } = await room.phone(pc)
    const id = await room.note(one)
    await room.say(two, { t: 'unnote', note: id, id: 'u' })
    expect(two.last()).toEqual({ t: 'err', code: 'note-gone', id: 'u' })
    await room.say(pc, { t: 'unnote', note: id, id: 'u' })
    expect(pc.last()).toEqual({ t: 'err', code: 'not-owner', id: 'u' })
    await room.say(one, { t: 'unnote', note: id, id: 'u' })
    expect(one.last()).toEqual({ t: 'unnoted', note: id, id: 'u' })
    expect(room.stored.has(`note:${id}`)).toBe(false)
    await room.say(one, { t: 'unnote', note: id, id: 'v' })
    expect(one.last()).toEqual({ t: 'err', code: 'note-gone', id: 'v' })
  })

  it('forgets a note the PC collected, keeps one it held with why, and tells the phones', async () => {
    const room = aRoom()
    const pc = await room.pc()
    const { peer: phone } = await room.phone(pc)
    const sent = await room.note(phone)
    const old = await room.note(phone)
    await room.say(pc, { t: 'notes-collected', notes: [sent], held: [{ note: old, why: 'old' }], id: 'c' })
    expect(pc.last()).toEqual({ t: 'done', of: 'notes-collected', id: 'c' })
    expect(phone.last()).toEqual({ t: 'notes-changed' })
    expect(room.stored.has(`note:${sent}`)).toBe(false)
    await room.say(phone, { t: 'notes', id: 'm' })
    expect(phone.last()).toEqual({ t: 'my-notes', items: [{ id: old, at: expect.any(Number), held: 'old' }], id: 'm' })
    // A phone cannot say what the PC did.
    await room.say(phone, { t: 'notes-collected', notes: [old], held: [], id: 'x' })
    expect(phone.last()).toEqual({ t: 'err', code: 'not-owner', id: 'x' })
  })

  it('keeps at most MAX_NOTES, none past a week, and none of a phone that was removed', async () => {
    const room = aRoom()
    const pc = await room.pc()
    const { peer: phone, device } = await room.phone(pc)
    for (let i = 0; i < MAX_NOTES; i += 1) await room.note(phone)
    await room.say(phone, { t: 'note', ...sealed(), id: 'f' })
    expect(phone.last()).toEqual({ t: 'err', code: 'full', id: 'f' })
    room.tick(LETTER_KEPT_MS)
    await room.core.alarm()
    expect([...room.stored.keys()].filter((key) => key.startsWith('note:'))).toEqual([])
    const kept = await room.note(phone)
    await room.say(pc, { t: 'remove', device, id: 'r' })
    expect(room.stored.has(`note:${kept}`)).toBe(false)
  })

  it('reads a note frame only in its own shape', () => {
    const box = sealed()
    expect(parseClientFrame(JSON.stringify({ t: 'note', ...box, id: 'n' }))).toEqual({ t: 'note', ...box, id: 'n' })
    expect(parseClientFrame(JSON.stringify({ t: 'note', ...box, text: 'hello', id: 'n' }))).toBeUndefined()
    expect(parseClientFrame(JSON.stringify({ t: 'note', n: 'short', c: box.c, id: 'n' }))).toBeUndefined()
    expect(parseClientFrame(JSON.stringify({ t: 'unnote', note: 'not-an-id', id: 'u' }))).toBeUndefined()
    const id = `${String(1_800_000_000_000)}-abcdefgh`
    expect(parseClientFrame(JSON.stringify({ t: 'notes-collected', notes: [], held: [{ note: id, why: 'busy' }], id: 'c' }))).toBeUndefined()
    expect(parseClientFrame(JSON.stringify({ t: 'notes-collected', notes: [id], held: [{ note: id, why: 'gone' }], id: 'c' }))).toEqual({ t: 'notes-collected', notes: [id], held: [{ note: id, why: 'gone' }], id: 'c' })
  })
})
