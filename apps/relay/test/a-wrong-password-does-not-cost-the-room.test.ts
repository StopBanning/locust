import { createHash, randomBytes } from 'node:crypto'

import { describe, expect, it } from 'vitest'

import { HOOK_POSTS_PER_HOUR, roomCore, type Attachment, type Peer, type RoomHost } from '../src/room-core.js'
import { Room } from '../src/room.js'

/**
 * A WRONG PASSWORD DOES NOT COST THE ROOM (2026-10-25). A miss is not a call the link accepted. The room should not
 * read the body for one, and a run of them should be told to slow down the way a hit already is.
 */

const password = (): string => randomBytes(32).toString('base64url')
const digest = (text: string): string => createHash('sha256').update(text).digest('base64url')

async function room() {
  let now = 1_800_000_000_000
  const stored = new Map<string, unknown>()
  let attachment: Attachment = { as: 'new', at: now }
  const peer: Peer = { get attachment() { return attachment }, attach: (next) => { attachment = next }, send: () => undefined, close: () => undefined }
  const host: RoomHost = {
    store: {
      get: async <T>(key: string) => structuredClone(stored.get(key)) as T | undefined,
      put: async (key, value) => { stored.set(key, structuredClone(value)) },
      delete: async (keys) => { for (const key of keys) stored.delete(key) },
      deleteAll: async () => { stored.clear() }
    },
    peers: () => [peer],
    now: () => now,
    setAlarm: async () => undefined,
    random: (size) => randomBytes(size).toString('base64url'),
    sha256: async (text) => digest(text)
  }
  const core = roomCore(host)
  const say = (frame: unknown) => core.message(peer, JSON.stringify(frame))
  await say({ t: 'auth', as: 'pc', token: password() })
  const secret = password()
  await say({ t: 'hook-open', hook: 'rt_a', hash: digest(secret) })
  const call = (token: string, body = 'hello') => core.hook({ method: 'POST', secret: token, body, contentType: 'text/plain' })
  return { secret, call, tick: (ms: number) => { now += ms } }
}

describe('a wrong password does not cost the room', () => {
  it('tells a run of misses to slow down without using up the link\'s own hour', async () => {
    const made = await room()
    for (let index = 0; index < HOOK_POSTS_PER_HOUR; index += 1) expect((await made.call(password())).status).toBe(404)
    expect((await made.call(made.secret)).status).toBe(202)
    made.tick(1)
    expect((await made.call(password())).status).toBe(429)
  })

  it('does not read the body before the password matches', async () => {
    const globalScope = globalThis as unknown as { WebSocketRequestResponsePair?: new (request: string, response: string) => object }
    globalScope.WebSocketRequestResponsePair ??= class { constructor(readonly request: string, readonly response: string) {} }
    const stored = new Map<string, unknown>()
    const durable = new Room({
      storage: {
        get: async <T>(key: string) => stored.get(key) as T | undefined,
        put: async (key, value) => { stored.set(key, value) },
        delete: async (keys) => { for (const key of keys) stored.delete(key); return keys.length },
        deleteAll: async () => { stored.clear() },
        setAlarm: async () => undefined
      },
      acceptWebSocket: () => undefined,
      getWebSockets: () => [],
      setWebSocketAutoResponse: () => undefined
    }, {})
    let reads = 0
    const stream = new ReadableStream<Uint8Array>({
      pull(controller) {
        reads += 1
        controller.enqueue(new TextEncoder().encode('hello'))
        controller.close()
      }
    })
    const response = await durable.fetch(new Request(`https://example.test/v1/room/${'a'.repeat(22)}/hook/${password()}`, { method: 'POST', body: stream, duplex: 'half' } as RequestInit))
    expect(response.status).toBe(404)
    expect(reads).toBe(0)
  })
})
