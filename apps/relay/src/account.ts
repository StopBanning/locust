import { COMPUTER_NAME, PUBLIC_KEY, ROOM_ID } from './protocol.js'

/**
 * ONE GITHUB ACCOUNT'S COMPUTERS (0.838): the rooms a browser signed in as that account may be given a ticket into,
 * with each PC's public key and name. One Durable Object per account, named by its number; it holds nothing else.
 * Each room says what it keeps itself (room-core.ts), so a room that let the account go is dropped here when it is met.
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

/** Reached only by the relay's own Worker and rooms, never from outside: the Worker routes no address to it. */
export class Account {
  constructor(private readonly state: DurableObjectState) {}

  async fetch(request: Request): Promise<Response> {
    const path = new URL(request.url).pathname
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
      if (next.length === 0) await this.state.storage.deleteAll()
      else await this.state.storage.put(ROOMS, next)
      return new Response(null, { status: 204 })
    }
    return new Response(null, { status: 404 })
  }
}
