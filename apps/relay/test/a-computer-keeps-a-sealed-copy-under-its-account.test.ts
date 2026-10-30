import { createHash, randomBytes } from 'node:crypto'

import { describe, expect, it } from 'vitest'

import { roomCore } from '../src/room-core.js'
import { MAX_SYNC_OBJECT, SYNC_QUOTA, SYNC_WRITES_PER_HOUR, syncCore, syncKey, type SyncObject } from '../src/sync.js'

/**
 * A COMPUTER KEEPS A SEALED COPY UNDER ITS ACCOUNT (docs/PLAN-2026-10-30-ACCOUNTS.md). The relay's rules for an
 * account's copies: only the room's own PC, by its token, once the room is tied to an account; a copy is written only
 * under its own room, and read from any room of the same account, never another's; an account keeps at most its quota,
 * a file at most its size, and writes at most so many an hour; and "Delete my copy" takes every file.
 */
const id = (): string => randomBytes(32).toString('base64url')
const room = (): string => randomBytes(16).toString('base64url')
const bytes = (size: number): ArrayBuffer => new Uint8Array(size).buffer

function aBucket() {
  const held = new Map<string, { size: number; uploaded: number }>()
  let now = 1_800_000_000_000
  const ledger = new Map<string, unknown>()
  const core = syncCore({
    objects: {
      head: async (key) => { const one = held.get(key); return one === undefined ? undefined : { key, ...one } },
      get: async (key) => { const one = held.get(key); return one === undefined ? undefined : { body: bytes(one.size), size: one.size } },
      put: async (key, _body, size) => { held.set(key, { size, uploaded: now }) },
      list: async (prefix) => [...held].filter(([key]) => key.startsWith(prefix)).map(([key, one]): SyncObject => ({ key, ...one })),
      delete: async (keys) => { for (const key of keys) held.delete(key) }
    },
    ledger: { get: async <T>(key: string) => ledger.get(key) as T | undefined, put: async (key, value) => { ledger.set(key, value) } },
    now: () => now
  })
  return { core, held, tick: (ms: number) => { now += ms } }
}

describe('an account’s copies', () => {
  it('are kept under the account and the writing room, and counted', async () => {
    const { core, held } = aBucket()
    const mine = room()
    const file = id()
    expect((await core.put({ user: 7, room: mine, id: file, size: 100, body: bytes(100) })).status).toBe(204)
    expect(held.has(syncKey(7, mine, file))).toBe(true)
    // The same file again counts once: its old size is given back.
    const again = await core.put({ user: 7, room: mine, id: file, size: 40, body: bytes(40) })
    expect(again.body).toEqual({ used: 40 })
  })

  it('are read from any room of the same account, and never from another account', async () => {
    const { core } = aBucket()
    const old = room()
    const file = id()
    await core.put({ user: 7, room: old, id: file, size: 10, body: bytes(10) })
    expect((await core.get({ user: 7, from: old, id: file })).status).toBe(200)
    expect((await core.get({ user: 8, from: old, id: file })).status).toBe(404)
    const listed = await core.list({ user: 8 })
    expect(listed.body?.computers).toEqual([])
    const mine = await core.list({ user: 7 })
    expect(mine.body?.computers).toEqual([{ room: old, bytes: 10, files: [{ id: file, size: 10, at: 1_800_000_000_000 }] }])
  })

  it('refuse a name that is not a file id or a room', async () => {
    const { core } = aBucket()
    expect((await core.put({ user: 7, room: room(), id: '../../acct/8/x', size: 1, body: bytes(1) })).status).toBe(400)
    expect((await core.put({ user: 7, room: 'not a room', id: id(), size: 1, body: bytes(1) })).status).toBe(400)
    expect((await core.get({ user: 7, from: room(), id: 'short' })).status).toBe(400)
  })

  it('keep a file no larger than MAX_SYNC_OBJECT, and an account no larger than its quota', async () => {
    const { core } = aBucket()
    const mine = room()
    expect((await core.put({ user: 7, room: mine, id: id(), size: MAX_SYNC_OBJECT + 1, body: bytes(1) })).status).toBe(413)
    expect((await core.put({ user: 7, room: mine, id: id(), size: 0, body: bytes(0) })).status).toBe(411)
    // Fill the quota with files at the most a file may be, then one byte more is refused, and nothing is written.
    const files = Math.floor(SYNC_QUOTA / MAX_SYNC_OBJECT)
    for (let at = 0; at < files; at += 1) expect((await core.put({ user: 7, room: mine, id: id(), size: MAX_SYNC_OBJECT, body: bytes(1) })).status).toBe(204)
    const over = await core.put({ user: 7, room: mine, id: id(), size: 1, body: bytes(1) })
    expect(over.status).toBe(507)
    expect(over.body).toMatchObject({ error: 'quota', used: files * MAX_SYNC_OBJECT })
  })

  it('take at most SYNC_WRITES_PER_HOUR writes an hour, then more after it', async () => {
    const { core, tick } = aBucket()
    const mine = room()
    const file = id()
    for (let at = 0; at < SYNC_WRITES_PER_HOUR; at += 1) await core.put({ user: 7, room: mine, id: file, size: 1, body: bytes(1) })
    expect((await core.put({ user: 7, room: mine, id: file, size: 1, body: bytes(1) })).status).toBe(429)
    tick(60 * 60_000)
    expect((await core.put({ user: 7, room: mine, id: file, size: 1, body: bytes(1) })).status).toBe(204)
  })

  it('go when deleted, one or all, and give their size back', async () => {
    const { core, held } = aBucket()
    const mine = room()
    const other = room()
    const file = id()
    await core.put({ user: 7, room: mine, id: file, size: 30, body: bytes(30) })
    await core.put({ user: 7, room: other, id: id(), size: 20, body: bytes(20) })
    // Another account's file in the same bucket (each account counts in its own object, so it is put straight in).
    held.set(syncKey(8, room(), id()), { size: 5, uploaded: 0 })
    expect((await core.remove({ user: 7, room: mine, id: file })).body).toEqual({ used: 20 })
    expect((await core.removeAll({ user: 7 })).body).toEqual({ used: 0, removed: 1 })
    // Another account's copy is not touched.
    expect([...held.keys()].every((key) => key.startsWith('acct/8/'))).toBe(true)
    expect(held.size).toBe(1)
  })
})

describe('who may keep a copy at a room', () => {
  const hash = (text: string): string => createHash('sha256').update(text, 'utf8').digest('base64url')
  const roomWith = (stored: Record<string, unknown>, github = true) => {
    const store = new Map<string, unknown>(Object.entries(stored))
    return roomCore({
      store: { get: async <T>(key: string) => store.get(key) as T | undefined, put: async (key, value) => { store.set(key, value) }, delete: async () => undefined, deleteAll: async () => undefined },
      peers: () => [],
      now: () => 1_800_000_000_000,
      setAlarm: async () => undefined,
      random: (size) => randomBytes(size).toString('base64url'),
      sha256: async (text) => hash(text),
      ...(github ? { github: { forget: async () => undefined } } : {})
    })
  }
  const token = randomBytes(32).toString('base64url')
  const ROOM = room()
  const tied = { owner: hash(token), created: 0, seen: 0, devices: {}, github: { user: 7, login: 'colin', key: id(), name: 'Mac', room: ROOM, since: 0 } }

  it('is the room’s own PC, by its token, once the room is tied to an account', async () => {
    expect(await roomWith({ meta: tied }).syncWho(token)).toEqual({ status: 200, user: 7, room: ROOM })
  })

  it('is nobody with another token, no token, or at a room not tied to an account', async () => {
    expect((await roomWith({ meta: tied }).syncWho(randomBytes(32).toString('base64url'))).status).toBe(401)
    expect((await roomWith({ meta: tied }).syncWho(undefined)).status).toBe(401)
    const { github: _none, ...untied } = tied
    expect((await roomWith({ meta: untied }).syncWho(token)).status).toBe(403)
    expect((await roomWith({ meta: tied }, false).syncWho(token)).status).toBe(403)
    expect((await roomWith({}).syncWho(token)).status).toBe(404)
  })
})
