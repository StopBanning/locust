import { createHash, randomBytes } from 'node:crypto'

import { describe, expect, it, vi } from 'vitest'

import type { RelayFrame } from '../src/protocol.js'
import { GITHUB_LINK_MS, IDLE_ROOM_MS, TICKET_MS, TICKETS_PER_HOUR, roomCore, type Attachment, type Peer, type RoomHost } from '../src/room-core.js'

/**
 * A ROOM TIED TO GITHUB GIVES TICKETS ONCE (0.838). The relay's rules for signing in with GitHub: only a relay that
 * offers it ties a room; the PC's link is good for ten minutes, once; a ticket only for the account the room is tied
 * to, good once, within ten minutes, a limited number an hour; and a room let go or deleted forgets its account.
 */
const token = (): string => randomBytes(32).toString('base64url')
const hash = (text: string): string => createHash('sha256').update(text, 'utf8').digest('base64url')
const ROOM = randomBytes(16).toString('base64url')
const KEY = randomBytes(32).toString('base64url')

class FakePeer implements Peer {
  attachment: Attachment
  readonly heard: RelayFrame[] = []
  closed: number | undefined
  constructor(at: number) { this.attachment = { as: 'new', at } }
  attach(next: Attachment): void { this.attachment = next }
  send(frame: RelayFrame): void { if (this.closed === undefined) this.heard.push(frame) }
  close(code: number): void { this.closed ??= code }
  last(): RelayFrame | undefined { return this.heard.at(-1) }
}

function aRoom(github?: RoomHost['github']) {
  const stored = new Map<string, unknown>()
  const peers: FakePeer[] = []
  let now = 1_800_000_000_000
  const core = roomCore({
    store: {
      get: async <T>(key: string) => structuredClone(stored.get(key)) as T | undefined,
      put: async (key, value) => { stored.set(key, structuredClone(value)) },
      delete: async (keys) => { for (const key of keys) stored.delete(key) },
      deleteAll: async () => { stored.clear() }
    },
    peers: () => peers.filter((peer) => peer.closed === undefined),
    now: () => now,
    setAlarm: async () => undefined,
    random: (bytes) => randomBytes(bytes).toString('base64url'),
    sha256: async (text) => hash(text),
    ...(github === undefined ? {} : { github })
  })
  const join = async (): Promise<FakePeer> => { const peer = new FakePeer(now); peers.push(peer); await core.opened(); return peer }
  const say = (peer: FakePeer, frame: unknown): Promise<void> => core.message(peer, JSON.stringify(frame))
  const secret = token()
  const pc = async (): Promise<FakePeer> => { const peer = await join(); await say(peer, { t: 'auth', as: 'pc', token: secret }); return peer }
  /** The PC sends the person to GitHub: its link, opened at the relay. */
  const open = async (peer: FakePeer): Promise<string> => {
    const link = token()
    await say(peer, { t: 'github-open', hash: hash(link), key: KEY, name: 'DESKTOP-1', id: 'g1' })
    return link
  }
  const comeIn = async (ticket: string): Promise<RelayFrame | undefined> => { const peer = await join(); await say(peer, { t: 'auth', as: 'pairing', token: ticket }); return peer.last() }
  return { core, pc, open, say, comeIn, tick: (ms: number) => { now += ms } }
}

describe('a room tied to GitHub gives tickets once', () => {
  it('is not tied on a relay that does not offer GitHub', async () => {
    const room = aRoom()
    const pc = await room.pc()
    // A relay without GitHub says nothing of it to the PC, either.
    expect(pc.last()).toEqual({ t: 'ok', as: 'pc', devices: [] })
    await room.open(pc)
    expect(pc.last()).toEqual({ t: 'err', code: 'github-off', id: 'g1' })
  })

  it('takes the PC’s link once, within ten minutes, and tells the PC the account', async () => {
    const room = aRoom({ forget: async () => undefined })
    const pc = await room.pc()
    expect(pc.last()).toEqual({ t: 'ok', as: 'pc', devices: [], github: false })
    const late = await room.open(pc)
    expect(pc.last()).toEqual({ t: 'done', of: 'github-open', id: 'g1' })
    room.tick(GITHUB_LINK_MS)
    expect((await room.core.githubLinked({ room: ROOM, link: late, user: 42, login: 'octo' })).status).toBe(410)
    const link = await room.open(pc)
    expect((await room.core.githubLinked({ room: ROOM, link: token(), user: 42, login: 'octo' })).status).toBe(410)
    expect(await room.core.githubLinked({ room: ROOM, link, user: 42, login: 'octo' })).toEqual({ status: 200, body: { key: KEY, name: 'DESKTOP-1' } })
    expect(pc.last()).toEqual({ t: 'github', account: { user: 42, login: 'octo' } })
    expect((await room.core.githubLinked({ room: ROOM, link, user: 42, login: 'octo' })).status).toBe(410)
    // Back after a network change, the PC is told again.
    const again = await room.pc()
    expect(again.last()).toEqual({ t: 'ok', as: 'pc', devices: [], github: { user: 42, login: 'octo' } })
  })

  it('gives a ticket only for its account, good once and for ten minutes, and only so many an hour', async () => {
    const room = aRoom({ forget: async () => undefined })
    const pc = await room.pc()
    await room.core.githubLinked({ room: ROOM, link: await room.open(pc), user: 42, login: 'octo' })
    expect((await room.core.githubTicket({ user: 7 })).status).toBe(404)
    const first = await room.core.githubTicket({ user: 42 })
    expect(first.body).toMatchObject({ key: KEY, name: 'DESKTOP-1' })
    expect(await room.comeIn(String(first.body.ticket))).toEqual({ t: 'ok', as: 'pairing', pc: true })
    expect(await room.comeIn(String(first.body.ticket))).toEqual({ t: 'err', code: 'pairing-closed' })
    const stale = await room.core.githubTicket({ user: 42 })
    room.tick(TICKET_MS)
    expect(await room.comeIn(String(stale.body.ticket))).toEqual({ t: 'err', code: 'pairing-closed' })
    for (let i = 2; i < TICKETS_PER_HOUR; i += 1) expect((await room.core.githubTicket({ user: 42 })).status).toBe(200)
    expect((await room.core.githubTicket({ user: 42 })).status).toBe(429)
    room.tick(60 * 60_000)
    expect((await room.core.githubTicket({ user: 42 })).status).toBe(200)
  })

  it('forgets its account when the PC lets it go, when it is reset, and when it is deleted for being idle', async () => {
    const forget = vi.fn(async () => undefined)
    const room = aRoom({ forget })
    let pc = await room.pc()
    const tie = async (): Promise<void> => { await room.core.githubLinked({ room: ROOM, link: await room.open(pc), user: 42, login: 'octo' }) }
    await tie()
    const waiting = String((await room.core.githubTicket({ user: 42 })).body.ticket)
    await room.say(pc, { t: 'github-close', id: 'c1' })
    expect(pc.last()).toEqual({ t: 'done', of: 'github-close', id: 'c1' })
    expect(forget).toHaveBeenLastCalledWith(42, ROOM)
    expect(await room.comeIn(waiting)).toEqual({ t: 'err', code: 'pairing-closed' })
    expect((await room.core.githubTicket({ user: 42 })).status).toBe(404)

    await tie()
    await room.say(pc, { t: 'reset', id: 'r1' })
    expect(forget).toHaveBeenCalledTimes(2)

    pc = await room.pc()
    await tie()
    pc.close(1000)
    await room.core.closed(pc)
    room.tick(IDLE_ROOM_MS)
    await room.core.alarm()
    expect(forget).toHaveBeenCalledTimes(3)
  })
})
