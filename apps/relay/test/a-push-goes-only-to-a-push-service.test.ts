import { createHash, randomBytes, webcrypto } from 'node:crypto'

import { describe, expect, it } from 'vitest'

import type { RelayFrame } from '../src/protocol.js'
import { roomCore, type Attachment, type Peer, type RoomHost } from '../src/room-core.js'
import { isPushEndpoint, vapidAuthorization } from '../src/vapid.js'

/**
 * A PUSH GOES ONLY TO A PUSH SERVICE (0.827). The PC asks the relay to post an alert it already sealed for one
 * phone's browser. The relay signs it as itself, posts it only to a push service's address, and says when the
 * phone's subscription is gone; nobody but the PC may ask.
 */
const token = (): string => randomBytes(32).toString('base64url')
const hash = (text: string): string => createHash('sha256').update(text, 'utf8').digest('base64url')
const BODY = randomBytes(200).toString('base64url')
const FCM = 'https://fcm.googleapis.com/fcm/send/abc123'

class FakePeer implements Peer {
  attachment: Attachment = { as: 'new', at: 0 }
  readonly heard: RelayFrame[] = []
  attach(next: Attachment): void { this.attachment = next }
  send(frame: RelayFrame): void { this.heard.push(frame) }
  close(): void { /* not needed here */ }
}

function aRoom(push?: RoomHost['push']) {
  const stored = new Map<string, unknown>()
  const peers: FakePeer[] = []
  const posted: { endpoint: string; body: string; urgency: string }[] = []
  const core = roomCore({
    store: { get: async <T>(key: string) => stored.get(key) as T | undefined, put: async (key, value) => { stored.set(key, value) }, deleteAll: async () => { stored.clear() } },
    peers: () => peers,
    now: () => Date.now(),
    setAlarm: async () => undefined,
    random: (bytes) => randomBytes(bytes).toString('base64url'),
    sha256: async (text) => hash(text),
    ...(push === undefined ? {} : { push: async (endpoint: string, body: string, urgency: 'normal' | 'high') => { posted.push({ endpoint, body, urgency }); return push(endpoint, body, urgency) } })
  })
  const join = async (): Promise<FakePeer> => { const peer = new FakePeer(); peers.push(peer); await core.opened(); return peer }
  const say = (peer: FakePeer, frame: unknown): Promise<void> => core.message(peer, JSON.stringify(frame))
  return { join, say, posted }
}

describe('a push goes only to a push service', () => {
  it('takes the addresses of the browsers\' push services, and nothing else', () => {
    for (const ok of [FCM, 'https://web.push.apple.com/QGuQyavXutnMtc', 'https://updates.push.services.mozilla.com/wpush/v2/gAAA', 'https://wns2-par02p.notify.windows.com/w/?token=x']) expect(isPushEndpoint(ok)).toBe(true)
    for (const no of ['http://fcm.googleapis.com/x', 'https://fcm.googleapis.com.evil.example/x', 'https://evil.example/fcm.googleapis.com', 'https://fcm.googleapis.com:8443/x', 'https://user@fcm.googleapis.com/x', 'https://169.254.169.254/latest', 'not a url']) expect(isPushEndpoint(no)).toBe(false) // public-guard: allow (a made-up address with a user part, which must be refused)
  })

  it('posts the PC\'s sealed alert, and says done', async () => {
    const room = aRoom(async () => 201)
    const pc = await room.join()
    await room.say(pc, { t: 'auth', as: 'pc', token: token() })
    await room.say(pc, { t: 'push', endpoint: FCM, body: BODY, urgency: 'high', id: 'p1' })
    expect(pc.heard.at(-1)).toEqual({ t: 'done', of: 'push', id: 'p1' })
    expect(room.posted).toEqual([{ endpoint: FCM, body: BODY, urgency: 'high' }])
  })

  it('refuses an address that is not a push service, before anything is posted', async () => {
    const room = aRoom(async () => 201)
    const pc = await room.join()
    await room.say(pc, { t: 'auth', as: 'pc', token: token() })
    await room.say(pc, { t: 'push', endpoint: 'https://evil.example/collect', body: BODY, urgency: 'normal', id: 'p1' })
    expect(pc.heard.at(-1)).toEqual({ t: 'err', code: 'bad-frame', id: 'p1' })
    expect(room.posted).toEqual([])
  })

  it('says when the subscription is gone, when the service refused, and when this relay cannot sign', async () => {
    for (const [status, code] of [[410, 'push-gone'], [404, 'push-gone'], [400, 'push-failed'], [500, 'push-failed']] as const) {
      const room = aRoom(async () => status)
      const pc = await room.join()
      await room.say(pc, { t: 'auth', as: 'pc', token: token() })
      await room.say(pc, { t: 'push', endpoint: FCM, body: BODY, urgency: 'normal', id: 'p' })
      expect(pc.heard.at(-1)).toEqual({ t: 'err', code, id: 'p' })
    }
    const keyless = aRoom()
    const pc = await keyless.join()
    await keyless.say(pc, { t: 'auth', as: 'pc', token: token() })
    await keyless.say(pc, { t: 'push', endpoint: FCM, body: BODY, urgency: 'normal', id: 'p' })
    expect(pc.heard.at(-1)).toEqual({ t: 'err', code: 'push-off', id: 'p' })
  })

  it('takes a push from the PC only', async () => {
    const room = aRoom(async () => 201)
    const pc = await room.join()
    await room.say(pc, { t: 'auth', as: 'pc', token: token() })
    const device = randomBytes(16).toString('base64url')
    const secret = token()
    await room.say(pc, { t: 'allow', device, hash: hash(secret) })
    const phone = await room.join()
    await room.say(phone, { t: 'auth', as: 'device', device, token: secret })
    await room.say(phone, { t: 'push', endpoint: FCM, body: BODY, urgency: 'normal', id: 'x' })
    expect(phone.heard.at(-1)).toEqual({ t: 'err', code: 'not-owner', id: 'x' })
    expect(room.posted).toEqual([])
  })

  it('signs as the relay, for that push service only, for twelve hours', async () => {
    const pair = await webcrypto.subtle.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, true, ['sign', 'verify'])
    const publicKey = Buffer.from(await webcrypto.subtle.exportKey('raw', pair.publicKey)).toString('base64url')
    const now = Date.parse('2026-10-23T12:00:00Z')
    const header = await vapidAuthorization(FCM, { privateJwk: await webcrypto.subtle.exportKey('jwk', pair.privateKey), publicKey, subject: 'https://locust.lol' }, now)
    const [, jwt, k] = /^vapid t=([^,]+), k=(.+)$/.exec(header)!
    expect(k).toBe(publicKey)
    const [head, claims, signature] = jwt!.split('.') as [string, string, string]
    expect(JSON.parse(Buffer.from(claims, 'base64url').toString())).toEqual({ aud: 'https://fcm.googleapis.com', exp: now / 1000 + 12 * 3600, sub: 'https://locust.lol' })
    const valid = await webcrypto.subtle.verify({ name: 'ECDSA', hash: 'SHA-256' }, pair.publicKey, Buffer.from(signature, 'base64url'), new TextEncoder().encode(`${head}.${claims}`))
    expect(valid).toBe(true)
  })
})
