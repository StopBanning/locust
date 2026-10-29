import { roomCore, type Attachment, type Peer } from './room-core.js'
import { vapidAuthorization, type VapidKeys } from './vapid.js'
import { forgetAccountRoom, githubOffered } from './github.js'
import { MAX_FRAME } from './protocol.js'
import { MAX_MCP_BODY, MCP_ASK_MS } from './mcp.js'
import { mcpPage, mcpPageHeaders } from './mcp-page.js'

/** An app's request as the PC answered it, or the relay's own reason it got none. */
interface McpReply { readonly status: number; readonly body: string }

const json = (status: number, body: unknown, more: Record<string, string> = {}): Response =>
  new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json', 'cache-control': 'no-store', ...more } })

/**
 * Where an app is told to sign in, when it calls with no key or a key the PC refused (RFC 9728): the room's own
 * address for its protected resource's metadata.
 */
export function mcpChallenge(url: URL, room: string): string {
  return `Bearer resource_metadata="${url.origin}/.well-known/oauth-protected-resource/v1/room/${room}/mcp"`
}

/** A JSON-RPC request's id, when the PC cannot answer it: the app is told why in its own terms. */
function rpcId(body: string): string | number | null | undefined {
  try {
    const value = JSON.parse(body) as Record<string, unknown>
    return typeof value.id === 'string' || typeof value.id === 'number' ? value.id : value.id === null ? null : undefined
  } catch { return undefined }
}

/** A streaming sender may omit Content-Length: stop reading at the byte limit, before decoding. */
export async function hookBody(request: Request): Promise<string | undefined> {
  if (Number(request.headers.get('content-length') ?? '0') > MAX_FRAME) return undefined
  const reader = request.body?.getReader()
  if (reader === undefined) return ''
  const chunks: Uint8Array[] = []
  let size = 0
  try {
    for (;;) {
      const part = await reader.read()
      if (part.done) break
      size += part.value.byteLength
      if (size > MAX_FRAME) { await reader.cancel(); return undefined }
      chunks.push(part.value)
    }
  } finally { reader.releaseLock() }
  const bytes = new Uint8Array(size)
  let offset = 0
  for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength }
  return new TextDecoder().decode(bytes)
}

/** What the Worker is given: the push signing key is a Cloudflare secret, set with `wrangler secret put`. */
export interface RoomEnv {
  readonly VAPID_PRIVATE_JWK?: string
  readonly VAPID_PUBLIC_KEY?: string
  /** Signing in with GitHub (0.838, github.ts): the rooms each account opens, and the OAuth app. */
  readonly ACCOUNTS?: DurableObjectNamespace
  readonly GITHUB_CLIENT_ID?: string
  readonly GITHUB_CLIENT_SECRET?: string
}

/** The relay's push key, or undefined when this copy of the relay has none (then alerts are off). */
export function vapidKeysOf(env: RoomEnv): VapidKeys | undefined {
  if (env.VAPID_PRIVATE_JWK === undefined || env.VAPID_PUBLIC_KEY === undefined) return undefined
  try {
    return { privateJwk: JSON.parse(env.VAPID_PRIVATE_JWK) as JsonWebKey, publicKey: env.VAPID_PUBLIC_KEY, subject: 'https://locust.lol' }
  } catch {
    return undefined
  }
}

const base64url = (bytes: Uint8Array): string => {
  let text = ''
  for (const byte of bytes) text += String.fromCharCode(byte)
  return btoa(text).replaceAll('+', '-').replaceAll('/', '_').replaceAll('=', '')
}

/**
 * The Durable Object that is one PC's room (room-core.ts holds the rules). Connections are accepted with Cloudflare's
 * Hibernation API, so a room nobody is talking in sleeps and costs nothing, and a PC's keep-alive ('ping') is answered
 * by Cloudflare without waking it.
 */
export class Room {
  private readonly core: ReturnType<typeof roomCore>
  /** One Peer per connection for as long as this object is awake, so two lookups of a connection are the same Peer. */
  private readonly peers = new WeakMap<WebSocket, Peer>()
  /** HTTP calls and sockets may interleave across storage awaits: one room changes its index at a time. */
  private queue: Promise<unknown> = Promise.resolve()
  /** Apps' requests held open for the PC's answer (0.841, mcp.ts): in memory, never stored. */
  private readonly waiting = new Map<string, (reply: McpReply) => void>()
  private serial<T>(work: () => Promise<T>): Promise<T> {
    const next = this.queue.then(work)
    this.queue = next.catch(() => undefined)
    return next
  }

  constructor(private readonly state: DurableObjectState, env: RoomEnv) {
    const vapid = vapidKeysOf(env)
    state.setWebSocketAutoResponse(new WebSocketRequestResponsePair('ping', 'pong'))
    this.core = roomCore({
      store: {
        get: (key) => state.storage.get(key),
        put: (key, value) => state.storage.put(key, value),
        delete: async (keys) => { await state.storage.delete([...keys]) },
        deleteAll: () => state.storage.deleteAll()
      },
      peers: () => state.getWebSockets().map((ws) => this.peerOf(ws)),
      now: () => Date.now(),
      setAlarm: (at) => state.storage.setAlarm(at),
      random: (bytes) => base64url(crypto.getRandomValues(new Uint8Array(bytes))),
      sha256: async (text) => base64url(new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(text)))),
      push: async (endpoint, body, urgency) => {
        if (vapid === undefined) return undefined
        const bytes = Uint8Array.from(atob(body.replaceAll('-', '+').replaceAll('_', '/')), (character) => character.charCodeAt(0))
        const answer = await fetch(endpoint, {
          method: 'POST',
          headers: { authorization: await vapidAuthorization(endpoint, vapid, Date.now()), 'content-encoding': 'aes128gcm', 'content-type': 'application/octet-stream', ttl: '86400', urgency },
          body: bytes
        })
        return answer.status
      },
      ...(githubOffered(env) ? { github: { forget: (user: number, room: string) => forgetAccountRoom(env, user, room) } } : {}),
      answered: (ask, status, body) => { this.waiting.get(ask)?.({ status, body }) }
    })
  }

  private peerOf(ws: WebSocket): Peer {
    const known = this.peers.get(ws)
    if (known !== undefined) return known
    const peer: Peer = {
      get attachment(): Attachment { return (ws.deserializeAttachment() as Attachment | null) ?? { as: 'new', at: 0 } },
      attach: (next) => ws.serializeAttachment(next),
      send: (frame) => { try { ws.send(JSON.stringify(frame)) } catch { /* Gone already. */ } },
      close: (code, reason) => { try { ws.close(code, reason) } catch { /* Gone already. */ } }
    }
    this.peers.set(ws, peer)
    return peer
  }

  /** A request for the PC to answer, held until it does or MCP_ASK_MS passes. Registered before it is sent. */
  private async held(kind: 'token' | 'call', body: string, auth?: string): Promise<McpReply> {
    const ask = base64url(crypto.getRandomValues(new Uint8Array(12)))
    let settle: (reply: McpReply) => void = () => undefined
    const answered = new Promise<McpReply>((resolve) => { settle = resolve })
    const timer = setTimeout(() => settle({ status: 504, body: JSON.stringify({ error: 'pc-timeout' }) }), MCP_ASK_MS)
    this.waiting.set(ask, (reply) => settle(reply))
    try {
      const refused = await this.serial(() => this.core.mcp.forward(ask, kind, body, auth))
      if (refused !== undefined) return { status: refused.status, body: JSON.stringify(refused.body) }
      return await answered
    } finally {
      clearTimeout(timer)
      this.waiting.delete(ask)
    }
  }

  /** Other apps from anywhere (0.841): signing in, and the MCP requests themselves. */
  private async mcpFetch(request: Request, url: URL, room: string, what: string): Promise<Response> {
    if (what === 'oauth/register') {
      if (request.method !== 'POST') return json(405, { error: 'method' }, { allow: 'POST' })
      const body = await hookBody(request)
      if (body === undefined) return json(413, { error: 'too-big' })
      const answer = await this.serial(() => this.core.mcp.register(body))
      return json(answer.status, answer.body)
    }
    if (what === 'oauth/authorize') {
      if (request.method !== 'GET') return json(405, { error: 'method' }, { allow: 'GET' })
      const shown = await this.serial(() => this.core.mcp.authorize(url.searchParams))
      if (shown.page === 'redirect') return new Response(null, { status: 302, headers: { location: shown.to, 'cache-control': 'no-store', 'referrer-policy': 'no-referrer' } })
      return new Response(mcpPage(shown), { status: shown.page === 'waiting' ? 200 : shown.page === 'bad' ? 400 : shown.page === 'busy' ? 429 : shown.page === 'offline' ? 503 : 404, headers: mcpPageHeaders(url.protocol === 'https:') })
    }
    if (what === 'oauth/wait') {
      const answer = await this.serial(() => this.core.mcp.wait(url.searchParams.get('ask') ?? ''))
      return json(answer.status, answer.body)
    }
    if (what === 'oauth/token') {
      if (request.method !== 'POST') return json(405, { error: 'method' }, { allow: 'POST' })
      const body = await hookBody(request)
      if (body === undefined || body.length > MAX_MCP_BODY) return json(413, { error: 'too-big' })
      const reply = await this.held('token', body)
      return new Response(reply.body, { status: reply.status, headers: { 'content-type': 'application/json', 'cache-control': 'no-store', pragma: 'no-cache' } })
    }
    // what === 'mcp'. Answered as JSON, one request at a time; there is no stream to open (GET) or session to end.
    if (request.method !== 'POST') return json(405, { error: 'method' }, { allow: 'POST' })
    if (!(await this.serial(() => this.core.mcp.on()))) return json(404, { error: 'not-found' })
    const bearer = /^Bearer ([A-Za-z0-9_-]{43})$/.exec(request.headers.get('authorization') ?? '')?.[1]
    if (bearer === undefined) return json(401, { error: 'invalid_token' }, { 'www-authenticate': mcpChallenge(url, room) })
    const body = await hookBody(request)
    if (body === undefined || body.length > MAX_MCP_BODY) return json(413, { error: 'too-big' })
    const reply = await this.held('call', body, bearer)
    if (reply.status === 401) return new Response(reply.body, { status: 401, headers: { 'content-type': 'application/json', 'cache-control': 'no-store', 'www-authenticate': mcpChallenge(url, room) } })
    // The computer off, or slow: the app is told in its own terms, so its person reads why.
    if (reply.status === 503 || reply.status === 504) {
      const id = rpcId(body)
      if (id !== undefined) {
        const message = reply.status === 503 ? 'Locust is not open on the computer it runs on. Open Locust there and try again.' : 'Locust did not answer in time. Nothing was retried; check Locust before asking again.'
        return json(200, { jsonrpc: '2.0', id, error: { code: -32000, message } })
      }
    }
    if (reply.status === 202 && reply.body === '') return new Response(null, { status: 202, headers: { 'cache-control': 'no-store' } })
    return new Response(reply.body, { status: reply.status, headers: { 'content-type': 'application/json', 'cache-control': 'no-store' } })
  }

  async fetch(request: Request): Promise<Response> {
    const address = new URL(request.url)
    const mcpPath = /^\/v1\/room\/([^/]+)\/(oauth\/(?:register|authorize|wait|token)|mcp)$/.exec(address.pathname)
    if (mcpPath !== null) return this.mcpFetch(request, address, mcpPath[1]!, mcpPath[2]!)
    // From the relay's own Worker only (github.ts): no address from outside is routed to these paths.
    const path = address.pathname
    if (path === '/github/linked' || path === '/github/ticket') {
      const said = await request.json().catch(() => undefined) as Record<string, unknown> | undefined
      const user = typeof said?.user === 'number' && Number.isSafeInteger(said.user) && said.user > 0 ? said.user : undefined
      if (said === undefined || user === undefined) return new Response('{"error":"bad-frame"}', { status: 400 })
      const answer = await this.serial(() => path === '/github/linked'
        ? this.core.githubLinked({ room: String(said.room), link: String(said.link), user, login: String(said.login) })
        : this.core.githubTicket({ user }))
      return new Response(JSON.stringify(answer.body), { status: answer.status, headers: { 'content-type': 'application/json', 'cache-control': 'no-store' } })
    }
    const hook = /\/hook\/([^/]+)$/.exec(new URL(request.url).pathname)?.[1]
    if (hook !== undefined) {
      if (request.method !== 'POST') return new Response('{"error":"method"}', { status: 405, headers: { allow: 'POST' } })
      // THE PASSWORD IS CHECKED BEFORE THE BODY IS READ (2026-10-26). It is in the path, so a wrong one is answered without
      // pulling a byte of what came with it, and without waiting in the room's queue behind a body that is still arriving.
      const refused = await this.serial(() => this.core.hookGate(hook))
      if (refused !== undefined) return new Response(JSON.stringify(refused.body), { status: refused.status, headers: { 'content-type': 'application/json', 'cache-control': 'no-store' } })
      const body = await hookBody(request)
      if (body === undefined) return new Response('{"error":"too-big"}', { status: 413 })
      const answer = await this.serial(() => this.core.hook({ method: request.method, secret: hook, body, contentType: request.headers.get('content-type') ?? 'application/octet-stream' }))
      return new Response(JSON.stringify(answer.body), { status: answer.status, headers: { 'content-type': 'application/json', 'cache-control': 'no-store' } })
    }
    // Scout's away look (0.829): the one kind of request that is not a WebSocket, with its token as a bearer.
    if (new URL(request.url).pathname.endsWith('/away')) {
      const bearer = /^Bearer ([A-Za-z0-9_-]{43})$/.exec(request.headers.get('authorization') ?? '')?.[1]
      const body = request.method === 'POST' ? await request.text() : undefined
      const answer = await this.serial(() => this.core.away({ method: request.method, ...(bearer === undefined ? {} : { token: bearer }), ...(body === undefined ? {} : { body }) }))
      return new Response(JSON.stringify(answer.body), { status: answer.status, headers: { 'content-type': 'application/json', 'cache-control': 'no-store' } })
    }
    if (request.headers.get('upgrade')?.toLowerCase() !== 'websocket') return new Response('Expected a WebSocket.', { status: 426 })
    const pair = new WebSocketPair()
    const client = pair[0]
    const server = pair[1]
    this.state.acceptWebSocket(server)
    server.serializeAttachment({ as: 'new', at: Date.now() } satisfies Attachment)
    await this.serial(() => this.core.opened())
    return new Response(null, { status: 101, webSocket: client } as unknown as ResponseInit)
  }

  async webSocketMessage(ws: WebSocket, message: string | ArrayBuffer): Promise<void> {
    // The relay speaks text only: a binary frame is read as nothing, and refused as a frame it does not know.
    await this.serial(() => this.core.message(this.peerOf(ws), typeof message === 'string' ? message : ''))
  }

  async webSocketClose(ws: WebSocket, code: number, reason: string): Promise<void> {
    await this.serial(() => this.core.closed(this.peerOf(ws)))
    // 1005 and 1006 say no code came; they cannot be sent back.
    try { ws.close(code === 1005 || code === 1006 ? 1000 : code, reason) } catch { /* Closed already. */ }
  }

  async webSocketError(ws: WebSocket): Promise<void> {
    await this.serial(() => this.core.closed(this.peerOf(ws)))
  }

  async alarm(): Promise<void> {
    await this.serial(() => this.core.alarm())
  }
}
