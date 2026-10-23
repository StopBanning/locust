import { createHash, randomBytes } from 'node:crypto'

import { describe, expect, it } from 'vitest'

import type { RelayFrame } from '../src/protocol.js'
import { AWAY_POSTS_PER_HOUR, AWAY_PUSHES_PER_DAY, LETTER_KEPT_MS, MAX_LETTERS, roomCore, type Attachment, type Peer, type RoomHost } from '../src/room-core.js'

/**
 * SCOUT'S AWAY LOOK LEAVES LETTERS (0.829). With the PC off, a workflow in the watched repository looks for Scout and
 * leaves a letter at the relay, sealed with a key the relay never has. Only a token the PC opened gets in; the PC and
 * its phones read the letters; a letter may ring the phones that are not open, with alerts their PC sealed ahead of
 * time, a few times a day at most; and nothing is kept past a week.
 */
const token = (): string => randomBytes(32).toString('base64url')
const hash = (text: string): string => createHash('sha256').update(text, 'utf8').digest('base64url')
const deviceId = (): string => randomBytes(16).toString('base64url')
const sealed = (): { n: string; c: string } => ({ n: randomBytes(12).toString('base64url'), c: randomBytes(300).toString('base64url') })
const pushBody = (): string => randomBytes(200).toString('base64url')
const FCM = 'https://fcm.googleapis.com/fcm/send/'

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

function aRoom(answer: (endpoint: string) => number = () => 201) {
  const stored = new Map<string, unknown>()
  const peers: FakePeer[] = []
  const rung: { endpoint: string; body: string }[] = []
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
    sha256: async (text) => hash(text),
    push: async (endpoint, body) => { rung.push({ endpoint, body }); return answer(endpoint) }
  }
  const core = roomCore(host)
  const drop = (peer: FakePeer): void => { const at = peers.indexOf(peer); if (at >= 0) peers.splice(at, 1) }
  const join = async (): Promise<FakePeer> => { const peer = new FakePeer(drop); peers.push(peer); await core.opened(); return peer }
  const say = (peer: FakePeer, frame: unknown): Promise<void> => core.message(peer, JSON.stringify(frame))
  const pc = async (secret = token()): Promise<FakePeer> => { const peer = await join(); await say(peer, { t: 'auth', as: 'pc', token: secret }); return peer }
  const phone = async (by: FakePeer, device = deviceId()): Promise<{ peer: FakePeer; device: string }> => {
    const secret = token()
    await say(by, { t: 'allow', device, hash: hash(secret), id: 'a' })
    const peer = await join()
    await say(peer, { t: 'auth', as: 'device', device, token: secret })
    return { peer, device }
  }
  const openAway = async (by: FakePeer): Promise<string> => {
    const secret = token()
    await say(by, { t: 'away-open', hash: hash(secret), id: 'w' })
    expect(by.last()).toEqual({ t: 'done', of: 'away-open', id: 'w' })
    return secret
  }
  const post = (secret: string | undefined, body: unknown = sealed()) => core.away({ method: 'POST', ...(secret === undefined ? {} : { token: secret }), body: JSON.stringify(body) })
  const get = (secret: string) => core.away({ method: 'GET', token: secret })
  const leave = async (peer: FakePeer): Promise<void> => { await core.closed(peer); drop(peer) }
  return { core, join, say, pc, phone, openAway, post, get, leave, rung, stored, tick: (ms: number) => { now += ms } }
}

describe('Scout\'s away look leaves letters', () => {
  it('is let in only with the token the PC opened, and only while it is open', async () => {
    const room = aRoom()
    expect((await room.post(token())).status).toBe(404)
    const pc = await room.pc()
    expect(await room.post(token())).toEqual({ status: 403, body: { error: 'away-off' } })
    const secret = await room.openAway(pc)
    expect((await room.post(token())).status).toBe(401)
    expect((await room.post(undefined)).status).toBe(401)
    expect((await room.post(secret)).status).toBe(200)
    // A new token: the old one stops working.
    const next = await room.openAway(pc)
    expect((await room.post(secret)).status).toBe(401)
    expect((await room.post(next)).status).toBe(200)
    // Off: the letters go with it.
    await room.say(pc, { t: 'away-close', id: 'x' })
    expect(pc.last()).toEqual({ t: 'done', of: 'away-close', id: 'x' })
    expect((await room.post(next)).status).toBe(403)
    expect([...room.stored.keys()].filter((key) => key.startsWith('letter:'))).toEqual([])
  })

  it('refuses a letter it cannot read as one, before keeping anything', async () => {
    const room = aRoom()
    const pc = await room.pc()
    const secret = await room.openAway(pc)
    for (const body of [{}, { n: 'short', c: sealed().c }, { ...sealed(), alert: 'loud' }, { ...sealed(), extra: 1 }, { n: sealed().n, c: randomBytes(70 * 1024).toString('base64url') }]) {
      expect((await room.post(secret, body)).status).toBe(400)
    }
    expect([...room.stored.keys()].filter((key) => key.startsWith('letter:'))).toEqual([])
  })

  it('hands a letter at once to the PC and phones connected, and says whether the PC is on', async () => {
    const room = aRoom()
    const pc = await room.pc()
    const secret = await room.openAway(pc)
    const { peer: phone } = await room.phone(pc)
    const box = sealed()
    const posted = await room.post(secret, box)
    expect(posted.status).toBe(200)
    const live = { t: 'letter', item: { id: posted.body.id, at: expect.any(Number), from: 'away', ...box } }
    expect(pc.last()).toEqual(live)
    expect(phone.last()).toEqual(live)
    expect(await room.get(secret)).toEqual({ status: 200, body: { pc: true, latest: live.item } })
    await room.leave(pc)
    expect((await room.get(secret)).body.pc).toBe(false)
  })

  it('keeps what the PC itself told, as a letter from the PC, for the next away look to read', async () => {
    const room = aRoom()
    const pc = await room.pc()
    const secret = await room.openAway(pc)
    const box = sealed()
    await room.say(pc, { t: 'post', ...box, id: 'b' })
    expect(pc.last()).toEqual({ t: 'done', of: 'post', id: 'b' })
    expect((await room.get(secret)).body.latest).toMatchObject({ from: 'pc', ...box })
  })

  it('gives a PC catching up the letters after the last it read, a page at a time, and a phone the newest', async () => {
    const room = aRoom()
    const pc = await room.pc()
    const secret = await room.openAway(pc)
    const ids: string[] = []
    for (let i = 0; i < 60; i += 1) {
      room.tick(61_000 * 6)
      ids.push(String((await room.post(secret)).body.id))
    }
    await room.say(pc, { t: 'letters', id: 'l1' })
    const first = pc.last() as Extract<RelayFrame, { t: 'letters' }>
    expect(first.items.map((item) => item.id)).toEqual(ids.slice(0, 50))
    expect(first.more).toBe(true)
    await room.say(pc, { t: 'letters', after: ids[49], id: 'l2' })
    const second = pc.last() as Extract<RelayFrame, { t: 'letters' }>
    expect(second.items.map((item) => item.id)).toEqual(ids.slice(50))
    expect(second.more).toBe(false)

    const { peer: phone } = await room.phone(pc)
    await room.say(phone, { t: 'letters', newest: 3, id: 'p' })
    expect((phone.last() as Extract<RelayFrame, { t: 'letters' }>).items.map((item) => item.id)).toEqual(ids.slice(-3))
  })

  it('rings only the phones that are not open, with the alert of the kind asked, a few times a day at most', async () => {
    const room = aRoom()
    const pc = await room.pc()
    const secret = await room.openAway(pc)
    const away = await room.phone(pc)
    const open = await room.phone(pc)
    await room.leave(away.peer)
    const bodies = { [away.device]: { found: pushBody(), brief: pushBody() }, [open.device]: { found: pushBody(), brief: pushBody() } }
    const stranger = deviceId()
    await room.say(pc, { t: 'away-alerts', alerts: [
      { device: away.device, endpoint: `${FCM}away`, ...bodies[away.device] },
      { device: open.device, endpoint: `${FCM}open`, ...bodies[open.device] },
      { device: stranger, endpoint: `${FCM}stranger`, found: pushBody(), brief: pushBody() }
    ], id: 'k' })
    expect(pc.last()).toEqual({ t: 'done', of: 'away-alerts', id: 'k' })

    // With the PC on, it rings the phones itself.
    expect((await room.post(secret, { ...sealed(), alert: 'found' })).body.rung).toBe(0)
    await room.leave(pc)
    expect((await room.post(secret, { ...sealed(), alert: 'brief' })).body.rung).toBe(1)
    expect(room.rung).toEqual([{ endpoint: `${FCM}away`, body: bodies[away.device]!.brief }])
    // No alert asked, none rung.
    expect((await room.post(secret)).body.rung).toBe(0)
    for (let i = 1; i < AWAY_PUSHES_PER_DAY; i += 1) { room.tick(13 * 60_000); expect((await room.post(secret, { ...sealed(), alert: 'found' })).body.rung).toBe(1) }
    room.tick(13 * 60_000)
    expect((await room.post(secret, { ...sealed(), alert: 'found' })).body.rung).toBe(0)
    // A day after the first, one more may ring.
    room.tick(24 * 60 * 60_000)
    expect((await room.post(secret, { ...sealed(), alert: 'found' })).body.rung).toBe(1)
    expect(room.rung.every((push) => push.endpoint === `${FCM}away`)).toBe(true)
  })

  it('stops ringing a phone whose push service says it is gone, or that its PC removed', async () => {
    const room = aRoom((endpoint) => endpoint.endsWith('gone') ? 410 : 201)
    const pc = await room.pc()
    const secret = await room.openAway(pc)
    const gone = await room.phone(pc)
    const removed = await room.phone(pc)
    const kept = await room.phone(pc)
    for (const one of [gone, removed, kept]) await room.leave(one.peer)
    const alert = (device: string, name: string) => ({ device, endpoint: `${FCM}${name}`, found: pushBody(), brief: pushBody() })
    await room.say(pc, { t: 'away-alerts', alerts: [alert(gone.device, 'gone'), alert(removed.device, 'removed'), alert(kept.device, 'kept')], id: 'k' })
    await room.say(pc, { t: 'remove', device: removed.device, id: 'r' })
    await room.leave(pc)
    expect((await room.post(secret, { ...sealed(), alert: 'found' })).body.rung).toBe(1)
    room.tick(13 * 60_000)
    await room.post(secret, { ...sealed(), alert: 'found' })
    expect(room.rung.map((push) => push.endpoint.slice(FCM.length))).toEqual(['gone', 'kept', 'kept'])
  })

  it('takes a dozen letters an hour at most, and a new token does not start the count again', async () => {
    const room = aRoom()
    const pc = await room.pc()
    const secret = await room.openAway(pc)
    for (let i = 0; i < AWAY_POSTS_PER_HOUR; i += 1) expect((await room.post(secret)).status).toBe(200)
    expect(await room.post(secret)).toEqual({ status: 429, body: { error: 'too-fast' } })
    const next = await room.openAway(pc)
    expect((await room.post(next)).status).toBe(429)
    room.tick(60 * 60_000)
    expect((await room.post(next)).status).toBe(200)
  })

  it('keeps no letter past a week, and no more than the newest few hundred', async () => {
    const room = aRoom()
    const pc = await room.pc()
    const secret = await room.openAway(pc)
    const first = String((await room.post(secret)).body.id)
    room.tick(LETTER_KEPT_MS)
    await room.core.alarm()
    expect(room.stored.has(`letter:${first}`)).toBe(false)
    const ids: string[] = []
    for (let i = 0; i < MAX_LETTERS + 5; i += 1) { room.tick(6 * 60_000); ids.push(String((await room.post(secret)).body.id)) }
    const kept = [...room.stored.keys()].filter((key) => key.startsWith('letter:')).map((key) => key.slice(7)).sort()
    expect(kept).toEqual(ids.slice(-MAX_LETTERS))
  })
})
