import { COMPUTER_NAME, PUBLIC_KEY, ROOM_ID } from './protocol.js'
import { MAX_SYNC_OBJECT, syncCore, type SyncAnswer, type SyncObject, type SyncObjects } from './sync.js'

/**
 * ONE GITHUB ACCOUNT'S COMPUTERS (0.838): the rooms a browser signed in as that account may be given a ticket into,
 * with each PC's public key and name. One Durable Object per account, named by its number. Since the account's sync
 * (docs/PLAN-2026-10-30-ACCOUNTS.md) it also keeps what the account's sealed copies take up, and is the one place they
 * are written, so the quota is counted in one line. Each room says what it keeps itself (room-core.ts), so a room that
 * let the account go is dropped here when it is met.
 */
export interface AccountRoom { readonly room: string; readonly key: string; readonly name: string; readonly at: number }

/** The most computers one account opens. */
export const MAX_ACCOUNT_ROOMS = 10

export const accountName = (user: number): string => `github:${String(user)}`

/** The rooms after one is added (newest last, once each, at most MAX_ACCOUNT_ROOMS) or removed. */
export function changeRooms(rooms: readonly AccountRoom[], change: { readonly add: AccountRoom } | { readonly remove: string }): readonly AccountRoom[] {
  if ('remove' in change) return rooms.filter((one) => one.room !== change.remove)
  return [...rooms.filter((one) => one.room !== change.add.room), change.add].slice(-MAX_ACCOUNT_ROOMS)
}

const roomOf = (value: unknown, at: number): AccountRoom | undefined => {
  if (typeof value !== 'object' || value === null) return undefined
  const said = value as Record<string, unknown>
  if (typeof said.room !== 'string' || !ROOM_ID.test(said.room) || typeof said.key !== 'string' || !PUBLIC_KEY.test(said.key) || typeof said.name !== 'string' || !COMPUTER_NAME.test(said.name)) return undefined
  return { room: said.room, key: said.key, name: said.name, at }
}

const ROOMS = 'rooms'

export interface AccountEnv {
  /** The sealed copies (wrangler.jsonc's `locust-sync` bucket). A relay without it offers no sync. */
  readonly SYNC?: R2Bucket
}

/** The bucket, as sync.ts's rules ask for it. */
export function r2Objects(bucket: R2Bucket): SyncObjects {
  const objectOf = (object: R2Object): SyncObject => ({ key: object.key, size: object.size, uploaded: object.uploaded.getTime() })
  return {
    head: async (key) => { const held = await bucket.head(key); return held === null ? undefined : objectOf(held) },
    get: async (key) => { const held = await bucket.get(key); return held === null ? undefined : { body: held.body, size: held.size } },
    put: async (key, body) => { await bucket.put(key, body) },
    list: async (prefix) => {
      const found: SyncObject[] = []
      let cursor: string | undefined
      do {
        const page = await bucket.list({ prefix, limit: 1000, ...(cursor === undefined ? {} : { cursor }) })
        found.push(...page.objects.map(objectOf))
        cursor = page.truncated ? page.cursor : undefined
      } while (cursor !== undefined)
      return found
    },
    delete: async (keys) => { if (keys.length > 0) await bucket.delete([...keys]) }
  }
}

/** A sync answer as an HTTP response: a file's bytes, or a few words of JSON. */
export function syncResponse(answer: SyncAnswer): Response {
  const headers = { 'cache-control': 'no-store' }
  if (answer.bytes !== undefined) return new Response(answer.bytes, { status: answer.status, headers: { ...headers, 'content-type': 'application/octet-stream', ...(answer.size === undefined ? {} : { 'content-length': String(answer.size) }) } })
  if (answer.status === 204) return new Response(null, { status: 204, headers })
  return new Response(JSON.stringify(answer.body ?? {}), { status: answer.status, headers: { ...headers, 'content-type': 'application/json' } })
}

/** Reached only by the relay's own Worker and rooms, never from outside: the Worker routes no address to it. */
export class Account {
  constructor(private readonly state: DurableObjectState, private readonly env: AccountEnv = {}) {}

  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url)
    const path = url.pathname
    if (path.startsWith('/sync/')) return this.sync(request, url)
    const rooms = (await this.state.storage.get<readonly AccountRoom[]>(ROOMS)) ?? []
    if (request.method === 'GET' && path === '/rooms') return Response.json({ rooms })
    if (request.method !== 'POST') return new Response(null, { status: 405 })
    const said = await request.json().catch(() => undefined) as Record<string, unknown> | undefined
    if (path === '/add') {
      const room = roomOf(said, Date.now())
      if (room === undefined) return new Response(null, { status: 400 })
      await this.state.storage.put(ROOMS, changeRooms(rooms, { add: room }))
      return new Response(null, { status: 204 })
    }
    if (path === '/remove') {
      if (typeof said?.room !== 'string' || !ROOM_ID.test(said.room)) return new Response(null, { status: 400 })
      const next = changeRooms(rooms, { remove: said.room })
      // The sync ledger stays with the account's copies: only the list of rooms goes when its last room does.
      if (next.length === 0) await this.state.storage.delete([ROOMS])
      else await this.state.storage.put(ROOMS, next)
      return new Response(null, { status: 204 })
    }
    return new Response(null, { status: 404 })
  }

  /**
   * The sealed copies, asked for by a room that has checked its PC's token (room.ts): the account's number and the
   * room come from that room, never from the PC's own words. `/sync/<op>?user=&room=&id=&from=`.
   */
  private async sync(request: Request, url: URL): Promise<Response> {
    if (this.env.SYNC === undefined) return syncResponse({ status: 501, body: { error: 'sync-off' } })
    const user = Number(url.searchParams.get('user'))
    const room = url.searchParams.get('room') ?? ''
    const id = url.searchParams.get('id') ?? ''
    if (!Number.isSafeInteger(user) || user <= 0) return syncResponse({ status: 400, body: { error: 'bad-frame' } })
    const core = syncCore({ objects: r2Objects(this.env.SYNC), ledger: this.state.storage, now: () => Date.now() })
    const op = url.pathname.slice('/sync/'.length)
    if (op === 'put' && request.method === 'PUT') {
      // The length is checked before a byte is read; R2 is then given the whole file, whose length it can trust.
      const said = Number(request.headers.get('content-length') ?? Number.NaN)
      if (!Number.isSafeInteger(said) || said < 1 || said > MAX_SYNC_OBJECT) return syncResponse(await core.put({ user, room, id, size: said, body: new ArrayBuffer(0) }))
      const bytes = await request.arrayBuffer()
      return syncResponse(await core.put({ user, room, id, size: bytes.byteLength, body: bytes }))
    }
    if (op === 'get' && request.method === 'GET') return syncResponse(await core.get({ user, from: url.searchParams.get('from') ?? '', id }))
    if (op === 'list' && request.method === 'GET') return syncResponse(await core.list({ user }))
    if (op === 'remove' && request.method === 'DELETE') return syncResponse(await core.remove({ user, room, id }))
    if (op === 'remove-all' && request.method === 'DELETE') return syncResponse(await core.removeAll({ user }))
    return syncResponse({ status: 405, body: { error: 'method' } })
  }
}
