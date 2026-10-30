import { ROOM_ID } from './protocol.js'

/*
 * YOUR LOCUST ON EVERY COMPUTER YOU SIGN IN TO (docs/PLAN-2026-10-30-ACCOUNTS.md). The relay keeps each signed-in
 * computer's copy of its profile: files sealed on the computer with a key the relay never has, under ids the computer
 * cut from their names with that key. What the relay sees is how many files, how big, and when.
 *
 * The rules are here, against SyncObjects rather than Cloudflare's R2, so they are tested where they are written
 * (account.ts holds the R2 bucket and the account's own storage, and hands them to this). Every object lives under its
 * account's number: `acct/<github id>/<room>/<file id>`. A computer writes only under its own room, and reads any room
 * of its own account, which is how a new computer starts from an old one's copy.
 */

/** A file's id: 32 bytes, base64url (an HMAC of its path under the profile key, cut on the computer). */
export const SYNC_FILE_ID = /^[A-Za-z0-9_-]{43}$/
/** The most one sealed file may be. A conversation larger than this goes up in chunks, each its own file. */
export const MAX_SYNC_OBJECT = 8 * 1024 * 1024
/** What one account may keep, every computer together. */
export const SYNC_QUOTA = 1024 * 1024 * 1024
/** Writes (puts and deletes) one account may make in an hour, every computer together. */
export const SYNC_WRITES_PER_HOUR = 3_600

const HOUR_MS = 60 * 60_000

export interface SyncObject { readonly key: string; readonly size: number; readonly uploaded: number }

/** The few things the rules need of a bucket. */
export interface SyncObjects {
  head(key: string): Promise<SyncObject | undefined>
  get(key: string): Promise<{ readonly body: ReadableStream | ArrayBuffer; readonly size: number } | undefined>
  put(key: string, body: ReadableStream | ArrayBuffer, size: number): Promise<void>
  list(prefix: string): Promise<readonly SyncObject[]>
  delete(keys: readonly string[]): Promise<void>
}

/** The account's own storage: what it keeps in all, and its writes this hour. */
export interface SyncLedger {
  get<T>(key: string): Promise<T | undefined>
  put(key: string, value: unknown): Promise<void>
}

export interface SyncAnswer { readonly status: number; readonly body?: Readonly<Record<string, unknown>>; readonly bytes?: ReadableStream | ArrayBuffer; readonly size?: number }

const USED = 'sync-used'
const WRITES = 'sync-writes'
interface Writes { readonly start: number; readonly count: number }

export const syncPrefix = (user: number, room?: string): string => `acct/${String(user)}/${room === undefined ? '' : `${room}/`}`
export const syncKey = (user: number, room: string, id: string): string => `${syncPrefix(user, room)}${id}`

export function syncCore(host: { readonly objects: SyncObjects; readonly ledger: SyncLedger; readonly now: () => number }) {
  const used = async (): Promise<number> => (await host.ledger.get<number>(USED)) ?? 0
  /** One more write this hour, or false when the hour's writes are spent. */
  const write = async (): Promise<boolean> => {
    const now = host.now()
    const held = await host.ledger.get<Writes>(WRITES)
    const current = held === undefined || now - held.start >= HOUR_MS ? { start: now, count: 0 } : held
    if (current.count >= SYNC_WRITES_PER_HOUR) return false
    await host.ledger.put(WRITES, { start: current.start, count: current.count + 1 })
    return true
  }
  const fileOk = (room: string, id: string): boolean => ROOM_ID.test(room) && SYNC_FILE_ID.test(id)

  return {
    /** One sealed file, under the writing computer's own room. Its old size, if any, is given back to the quota. */
    async put(request: { readonly user: number; readonly room: string; readonly id: string; readonly size: number; readonly body: ReadableStream | ArrayBuffer }): Promise<SyncAnswer> {
      if (!fileOk(request.room, request.id)) return { status: 400, body: { error: 'bad-file' } }
      if (!Number.isSafeInteger(request.size) || request.size < 1) return { status: 411, body: { error: 'length' } }
      if (request.size > MAX_SYNC_OBJECT) return { status: 413, body: { error: 'too-big', most: MAX_SYNC_OBJECT } }
      const key = syncKey(request.user, request.room, request.id)
      const before = (await host.objects.head(key))?.size ?? 0
      const total = await used()
      if (total - before + request.size > SYNC_QUOTA) return { status: 507, body: { error: 'quota', used: total, quota: SYNC_QUOTA } }
      if (!(await write())) return { status: 429, body: { error: 'too-fast' } }
      await host.objects.put(key, request.body, request.size)
      const now = total - before + request.size
      await host.ledger.put(USED, now)
      return { status: 204, body: { used: now } }
    },

    /** One sealed file from any computer of the same account. */
    async get(request: { readonly user: number; readonly from: string; readonly id: string }): Promise<SyncAnswer> {
      if (!fileOk(request.from, request.id)) return { status: 400, body: { error: 'bad-file' } }
      const held = await host.objects.get(syncKey(request.user, request.from, request.id))
      if (held === undefined) return { status: 404, body: { error: 'no-file' } }
      return { status: 200, bytes: held.body, size: held.size }
    },

    /** Every computer of the account that keeps a copy, with its files' ids, sizes and times; and the quota. */
    async list(request: { readonly user: number }): Promise<SyncAnswer> {
      const prefix = syncPrefix(request.user)
      const rooms = new Map<string, { id: string; size: number; at: number }[]>()
      for (const object of await host.objects.list(prefix)) {
        const [room, id] = object.key.slice(prefix.length).split('/')
        if (room === undefined || id === undefined || !fileOk(room, id)) continue
        const files = rooms.get(room) ?? []
        files.push({ id, size: object.size, at: object.uploaded })
        rooms.set(room, files)
      }
      return {
        status: 200,
        body: {
          used: await used(),
          quota: SYNC_QUOTA,
          computers: [...rooms].map(([room, files]) => ({ room, files, bytes: files.reduce((sum, one) => sum + one.size, 0) }))
        }
      }
    },

    /** One file of the computer's own copy. */
    async remove(request: { readonly user: number; readonly room: string; readonly id: string }): Promise<SyncAnswer> {
      if (!fileOk(request.room, request.id)) return { status: 400, body: { error: 'bad-file' } }
      const key = syncKey(request.user, request.room, request.id)
      const held = await host.objects.head(key)
      if (held === undefined) return { status: 204, body: { used: await used() } }
      if (!(await write())) return { status: 429, body: { error: 'too-fast' } }
      await host.objects.delete([key])
      const now = Math.max(0, (await used()) - held.size)
      await host.ledger.put(USED, now)
      return { status: 204, body: { used: now } }
    },

    /** "Delete my copy on Locust's server": every file of every computer of the account. Never rate limited. */
    async removeAll(request: { readonly user: number }): Promise<SyncAnswer> {
      const keys = (await host.objects.list(syncPrefix(request.user))).map((object) => object.key)
      for (let at = 0; at < keys.length; at += 1000) await host.objects.delete(keys.slice(at, at + 1000))
      await host.ledger.put(USED, 0)
      return { status: 204, body: { used: 0, removed: keys.length } }
    }
  }
}
