import { createHash, randomBytes } from 'node:crypto'

import { describe, expect, it } from 'vitest'

import type { RelayFrame } from '../src/protocol.js'
import { AUTH_WITHIN_MS, CLOSE, IDLE_ROOM_MS, MAX_DEVICES, MAX_PAIRING_TRIES, PAIRING_SOCKET_MS, RATE_LIMIT, roomCore, type Attachment, type Peer, type RoomHost } from '../src/room-core.js'

/**
 * A ROOM LETS IN ONLY ITS PC AND PAIRED DEVICES (0.826). The relay's rules, run against a stand-in for Cloudflare: a
 * connection is whatever it proved it is, and nothing else; the room keeps hashes, never a token or a message.
 */
const token = (): string => randomBytes(32).toString('base64url')
const hash = (text: string): string => createHash('sha256').update(text, 'utf8').digest('base64url')
const deviceId = (): string => randomBytes(16).toString('base64url')
const box = { n: randomBytes(12).toString('base64url'), c: randomBytes(40).toString('base64url') }

class FakePeer implements Peer {
  attachment: Attachment
  readonly heard: RelayFrame[] = []
  closed: { readonly code: number; readonly reason: string } | undefined
  constructor(at: number, private readonly gone: (peer: FakePeer) => void) { this.attachment = { as: 'new', at } }
  attach(next: Attachment): void { this.attachment = next }
  send(frame: RelayFrame): void { if (this.closed === undefined) this.heard.push(frame) }
  close(code: number, reason: string): void {
    if (this.closed !== undefined) return
    this.closed = { code, reason }
    this.gone(this)
  }
  last(): RelayFrame | undefined { return this.heard.at(-1) }
}

function aRoom() {
  const stored = new Map<string, unknown>()
  const peers: FakePeer[] = []
  let now = 1_800_000_000_000
  let alarm: number | undefined
  const host: RoomHost = {
    store: {
      get: async <T>(key: string) => structuredClone(stored.get(key)) as T | undefined,
      put: async (key, value) => { stored.set(key, structuredClone(value)) },
      deleteAll: async () => { stored.clear() }
    },
    peers: () => [...peers],
    now: () => now,
    setAlarm: async (at) => { alarm = at },
    random: (bytes) => randomBytes(bytes).toString('base64url'),
    sha256: async (text) => hash(text)
  }
  const core = roomCore(host)
  const drop = (peer: FakePeer): void => { const at = peers.indexOf(peer); if (at >= 0) peers.splice(at, 1) }
  const join = async (): Promise<FakePeer> => {
    const peer = new FakePeer(now, drop)
    peers.push(peer)
    await core.opened()
    return peer
  }
  const say = (peer: FakePeer, frame: unknown): Promise<void> => core.message(peer, typeof frame === 'string' ? frame : JSON.stringify(frame))
  /** The other side hung up: Cloudflare tells the room, then lets the connection go. */
  const leave = async (peer: FakePeer): Promise<void> => { await core.closed(peer); drop(peer) }
  const pcWith = async (secret: string): Promise<FakePeer> => { const pc = await join(); await say(pc, { t: 'auth', as: 'pc', token: secret }); return pc }
  return {
    core, join, say, leave, pcWith, stored,
    tick: (ms: number): void => { now += ms },
    now: (): number => now,
    alarm: (): number | undefined => alarm
  }
}

describe('a room lets in only its PC and paired devices', () => {
  it('is made by the first PC with a token, and lets in as the PC only that token again', async () => {
    const room = aRoom()
    const owner = token()
    const first = await room.pcWith(owner)
    expect(first.last()).toEqual({ t: 'ok', as: 'pc', devices: [] })

    const stranger = await room.pcWith(token())
    expect(stranger.heard).toEqual([{ t: 'err', code: 'auth' }])
    expect(stranger.closed?.code).toBe(CLOSE.refused)

    // The same PC come back (a new connection after a network change): the old connection is let go.
    const again = await room.pcWith(owner)
    expect(again.last()).toEqual({ t: 'ok', as: 'pc', devices: [] })
    expect(first.closed?.code).toBe(CLOSE.replaced)
  })

  it('refuses a device until the PC allows it, then lets it in with its own token only', async () => {
    const room = aRoom()
    const pc = await room.pcWith(token())
    const device = deviceId()
    const secret = token()

    const early = await room.join()
    await room.say(early, { t: 'auth', as: 'device', device, token: secret })
    expect(early.heard).toEqual([{ t: 'err', code: 'auth' }])

    await room.say(pc, { t: 'allow', device, hash: hash(secret), id: 'a1' })
    expect(pc.last()).toEqual({ t: 'done', of: 'allow', id: 'a1' })

    const wrong = await room.join()
    await room.say(wrong, { t: 'auth', as: 'device', device, token: token() })
    expect(wrong.closed?.code).toBe(CLOSE.refused)

    const phone = await room.join()
    await room.say(phone, { t: 'auth', as: 'device', device, token: secret })
    expect(phone.last()).toEqual({ t: 'ok', as: 'device', pc: true })
    expect(pc.last()).toEqual({ t: 'presence', device, on: true })
  })

  it('refuses a device or a pairing phone in a room no PC has made', async () => {
    const room = aRoom()
    const device = await room.join()
    await room.say(device, { t: 'auth', as: 'device', device: deviceId(), token: token() })
    expect(device.heard).toEqual([{ t: 'err', code: 'room' }])
    const pairing = await room.join()
    await room.say(pairing, { t: 'auth', as: 'pairing', token: token() })
    expect(pairing.heard).toEqual([{ t: 'err', code: 'room' }])
  })

  it('lets a phone with an open code send the PC one hello with its key, and the PC answer it by name', async () => {
    const room = aRoom()
    const pc = await room.pcWith(token())
    const pairingToken = token()

    const tooSoon = await room.join()
    await room.say(tooSoon, { t: 'auth', as: 'pairing', token: pairingToken })
    expect(tooSoon.heard).toEqual([{ t: 'err', code: 'pairing-closed' }])

    await room.say(pc, { t: 'pair-open', hash: hash(pairingToken), until: room.now() + 10 * 60_000, id: 'p1' })
    expect(pc.last()).toEqual({ t: 'done', of: 'pair-open', id: 'p1' })

    const phone = await room.join()
    await room.say(phone, { t: 'auth', as: 'pairing', token: pairingToken })
    expect(phone.last()).toEqual({ t: 'ok', as: 'pairing', pc: true })
    const key = randomBytes(32).toString('base64url')

    // Without its key, the hello is refused; with it, it reaches the PC, once.
    await room.say(phone, { t: 'send', to: 'pc', ...box })
    expect(phone.last()).toEqual({ t: 'err', code: 'bad-frame' })
    await room.say(phone, { t: 'send', to: 'pc', ...box, k: key })
    const hello = pc.last()
    expect(hello).toMatchObject({ t: 'msg', n: box.n, c: box.c, k: key })
    const from = (hello as { from: string }).from
    expect(from).toMatch(/^pairing:[A-Za-z0-9_-]{12}$/)
    await room.say(phone, { t: 'send', to: 'pc', ...box, k: key })
    expect(phone.last()).toEqual({ t: 'err', code: 'bad-frame' })

    await room.say(pc, { t: 'send', to: from, ...box })
    expect(phone.last()).toEqual({ t: 'msg', from: 'pc', ...box })
    // A phone being paired writes to nobody but the PC.
    await room.say(phone, { t: 'send', to: deviceId(), ...box, k: key })
    expect(phone.last()).toEqual({ t: 'err', code: 'bad-frame' })
  })

  it('counts every try at a code, right or wrong, and closes it after the last or when it runs out', async () => {
    const room = aRoom()
    const pc = await room.pcWith(token())
    const pairingToken = token()
    await room.say(pc, { t: 'pair-open', hash: hash(pairingToken), until: room.now() + 60_000 })
    for (let i = 0; i < MAX_PAIRING_TRIES; i += 1) {
      const guess = await room.join()
      await room.say(guess, { t: 'auth', as: 'pairing', token: token() })
      expect(guess.heard).toEqual([{ t: 'err', code: 'auth' }])
    }
    const late = await room.join()
    await room.say(late, { t: 'auth', as: 'pairing', token: pairingToken })
    expect(late.heard).toEqual([{ t: 'err', code: 'pairing-closed' }])

    const fresh = token()
    await room.say(pc, { t: 'pair-open', hash: hash(fresh), until: room.now() + 60_000 })
    room.tick(60_001)
    const expired = await room.join()
    await room.say(expired, { t: 'auth', as: 'pairing', token: fresh })
    expect(expired.heard).toEqual([{ t: 'err', code: 'pairing-closed' }])
  })

  it('keeps a code open no longer than 15 minutes, whatever the PC asks', async () => {
    const room = aRoom()
    const pc = await room.pcWith(token())
    const pairingToken = token()
    await room.say(pc, { t: 'pair-open', hash: hash(pairingToken), until: room.now() + 24 * 60 * 60_000 })
    room.tick(15 * 60_000 + 1)
    const phone = await room.join()
    await room.say(phone, { t: 'auth', as: 'pairing', token: pairingToken })
    expect(phone.heard).toEqual([{ t: 'err', code: 'pairing-closed' }])
  })

  it('takes commands from the PC only', async () => {
    const room = aRoom()
    const pc = await room.pcWith(token())
    const device = deviceId()
    const secret = token()
    await room.say(pc, { t: 'allow', device, hash: hash(secret) })
    const phone = await room.join()
    await room.say(phone, { t: 'auth', as: 'device', device, token: secret })
    for (const frame of [{ t: 'allow', device: deviceId(), hash: hash(token()), id: 'x' }, { t: 'remove', device, id: 'x' }, { t: 'pair-open', hash: hash(token()), until: room.now() + 1000, id: 'x' }, { t: 'reset', id: 'x' }]) {
      await room.say(phone, frame)
      expect(phone.last()).toEqual({ t: 'err', code: 'not-owner', id: 'x' })
    }
    expect(phone.closed).toBeUndefined()
  })

  it('carries a device\'s messages to its PC only, and says when the one written to is not there', async () => {
    const room = aRoom()
    const pc = await room.pcWith(token())
    const device = deviceId()
    const secret = token()
    await room.say(pc, { t: 'allow', device, hash: hash(secret) })
    const phone = await room.join()
    await room.say(phone, { t: 'auth', as: 'device', device, token: secret })

    await room.say(phone, { t: 'send', to: 'pc', ...box })
    expect(pc.last()).toEqual({ t: 'msg', from: device, ...box })
    await room.say(pc, { t: 'send', to: device, ...box })
    expect(phone.last()).toEqual({ t: 'msg', from: 'pc', ...box })

    await room.say(phone, { t: 'send', to: deviceId(), ...box })
    expect(phone.last()).toEqual({ t: 'err', code: 'bad-frame' })
    await room.say(phone, { t: 'send', to: 'pc', ...box, k: randomBytes(32).toString('base64url') })
    expect(phone.last()).toEqual({ t: 'err', code: 'bad-frame' })
    await room.say(pc, { t: 'send', to: deviceId(), ...box })
    expect(pc.last()).toEqual({ t: 'err', code: 'peer-offline' })

    await room.leave(pc)
    expect(phone.last()).toEqual({ t: 'presence', pc: false })
    await room.say(phone, { t: 'send', to: 'pc', ...box })
    expect(phone.last()).toEqual({ t: 'err', code: 'pc-offline' })
  })

  it('tells a device when its PC comes and goes, and the PC when a device does', async () => {
    const room = aRoom()
    const owner = token()
    const pc = await room.pcWith(owner)
    const device = deviceId()
    const secret = token()
    await room.say(pc, { t: 'allow', device, hash: hash(secret) })
    await room.leave(pc)

    const phone = await room.join()
    await room.say(phone, { t: 'auth', as: 'device', device, token: secret })
    expect(phone.last()).toEqual({ t: 'ok', as: 'device', pc: false })

    const back = await room.pcWith(owner)
    expect(back.last()).toEqual({ t: 'ok', as: 'pc', devices: [device] })
    expect(phone.last()).toEqual({ t: 'presence', pc: true })

    await room.leave(phone)
    expect(back.last()).toEqual({ t: 'presence', device, on: false })
  })

  it('closes and refuses a device the PC removes', async () => {
    const room = aRoom()
    const pc = await room.pcWith(token())
    const device = deviceId()
    const secret = token()
    await room.say(pc, { t: 'allow', device, hash: hash(secret) })
    const phone = await room.join()
    await room.say(phone, { t: 'auth', as: 'device', device, token: secret })

    await room.say(pc, { t: 'remove', device, id: 'r1' })
    expect(pc.last()).toEqual({ t: 'done', of: 'remove', id: 'r1' })
    expect(phone.last()).toEqual({ t: 'removed' })
    expect(phone.closed?.code).toBe(CLOSE.removed)

    const again = await room.join()
    await room.say(again, { t: 'auth', as: 'device', device, token: secret })
    expect(again.heard).toEqual([{ t: 'err', code: 'auth' }])
  })

  it('forgets the whole room on reset: its devices are closed, and nothing is kept', async () => {
    const room = aRoom()
    const pc = await room.pcWith(token())
    const device = deviceId()
    const secret = token()
    await room.say(pc, { t: 'allow', device, hash: hash(secret) })
    const phone = await room.join()
    await room.say(phone, { t: 'auth', as: 'device', device, token: secret })
    await room.say(pc, { t: 'reset', id: 'z' })
    expect(pc.last()).toEqual({ t: 'done', of: 'reset', id: 'z' })
    expect(phone.closed?.code).toBe(CLOSE.reset)
    expect(room.stored.size).toBe(0)
  })

  it('pairs no more devices than it may hold', async () => {
    const room = aRoom()
    const pc = await room.pcWith(token())
    for (let i = 0; i < MAX_DEVICES; i += 1) await room.say(pc, { t: 'allow', device: deviceId(), hash: hash(token()) })
    await room.say(pc, { t: 'allow', device: deviceId(), hash: hash(token()), id: 'f' })
    expect(pc.last()).toEqual({ t: 'err', code: 'full', id: 'f' })
  })

  it('closes a connection that never says who it is, and a phone that never finishes pairing', async () => {
    const room = aRoom()
    const silent = await room.join()
    expect(room.alarm()).toBe(room.now() + AUTH_WITHIN_MS)
    room.tick(AUTH_WITHIN_MS)
    await room.core.alarm()
    expect(silent.closed?.code).toBe(CLOSE.timeout)

    const pc = await room.pcWith(token())
    const pairingToken = token()
    await room.say(pc, { t: 'pair-open', hash: hash(pairingToken), until: room.now() + 10 * 60_000 })
    const phone = await room.join()
    await room.say(phone, { t: 'auth', as: 'pairing', token: pairingToken })
    room.tick(PAIRING_SOCKET_MS)
    await room.core.alarm()
    expect(phone.closed?.code).toBe(CLOSE.timeout)
    expect(pc.closed).toBeUndefined()
  })

  it('deletes a room whose PC has been away for 180 days', async () => {
    const room = aRoom()
    const pc = await room.pcWith(token())
    await room.say(pc, { t: 'allow', device: deviceId(), hash: hash(token()) })
    await room.leave(pc)
    expect(room.alarm()).toBe(room.now() + IDLE_ROOM_MS)
    room.tick(IDLE_ROOM_MS - 1)
    await room.core.alarm()
    expect(room.stored.size).toBe(1)
    room.tick(1)
    await room.core.alarm()
    expect(room.stored.size).toBe(0)
  })

  it('refuses a frame it does not know, and drops frames past the rate', async () => {
    const room = aRoom()
    const garbage = await room.join()
    await room.say(garbage, 'not json')
    expect(garbage.heard).toEqual([{ t: 'err', code: 'bad-frame' }])
    expect(garbage.closed?.code).toBe(CLOSE.refused)

    const extra = await room.join()
    await room.say(extra, { t: 'auth', as: 'pc', token: token(), also: 'this' })
    expect(extra.closed?.code).toBe(CLOSE.refused)

    const pc = await room.pcWith(token())
    const device = deviceId()
    const secret = token()
    await room.say(pc, { t: 'allow', device, hash: hash(secret) })
    const phone = await room.join()
    await room.say(phone, { t: 'auth', as: 'device', device, token: secret })
    for (let i = 0; i < RATE_LIMIT; i += 1) await room.say(phone, { t: 'send', to: 'pc', ...box })
    const before = pc.heard.length
    await room.say(phone, { t: 'send', to: 'pc', ...box })
    expect(phone.last()).toEqual({ t: 'err', code: 'too-fast' })
    expect(pc.heard.length).toBe(before)
    room.tick(10_000)
    await room.say(phone, { t: 'send', to: 'pc', ...box })
    expect(pc.heard.length).toBe(before + 1)
  })

  it('keeps hashes only: never a token, and never a message it passed on', async () => {
    const room = aRoom()
    const owner = token()
    const pc = await room.pcWith(owner)
    const device = deviceId()
    const secret = token()
    const pairingToken = token()
    await room.say(pc, { t: 'allow', device, hash: hash(secret) })
    await room.say(pc, { t: 'pair-open', hash: hash(pairingToken), until: room.now() + 60_000 })
    const phone = await room.join()
    await room.say(phone, { t: 'auth', as: 'device', device, token: secret })
    await room.say(phone, { t: 'send', to: 'pc', ...box })
    const kept = JSON.stringify([...room.stored.entries()])
    for (const secretWord of [owner, secret, pairingToken, box.c, box.n]) expect(kept).not.toContain(secretWord)
  })
})
