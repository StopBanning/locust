import { roomCore, type Attachment, type Peer } from './room-core.js'

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

  constructor(private readonly state: DurableObjectState, _env: unknown) {
    state.setWebSocketAutoResponse(new WebSocketRequestResponsePair('ping', 'pong'))
    this.core = roomCore({
      store: {
        get: (key) => state.storage.get(key),
        put: (key, value) => state.storage.put(key, value),
        deleteAll: () => state.storage.deleteAll()
      },
      peers: () => state.getWebSockets().map((ws) => this.peerOf(ws)),
      now: () => Date.now(),
      setAlarm: (at) => state.storage.setAlarm(at),
      random: (bytes) => base64url(crypto.getRandomValues(new Uint8Array(bytes))),
      sha256: async (text) => base64url(new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(text))))
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

  async fetch(request: Request): Promise<Response> {
    if (request.headers.get('upgrade')?.toLowerCase() !== 'websocket') return new Response('Expected a WebSocket.', { status: 426 })
    const pair = new WebSocketPair()
    const client = pair[0]
    const server = pair[1]
    this.state.acceptWebSocket(server)
    server.serializeAttachment({ as: 'new', at: Date.now() } satisfies Attachment)
    await this.core.opened()
    return new Response(null, { status: 101, webSocket: client } as unknown as ResponseInit)
  }

  async webSocketMessage(ws: WebSocket, message: string | ArrayBuffer): Promise<void> {
    // The relay speaks text only: a binary frame is read as nothing, and refused as a frame it does not know.
    await this.core.message(this.peerOf(ws), typeof message === 'string' ? message : '')
  }

  async webSocketClose(ws: WebSocket, code: number, reason: string): Promise<void> {
    await this.core.closed(this.peerOf(ws))
    // 1005 and 1006 say no code came; they cannot be sent back.
    try { ws.close(code === 1005 || code === 1006 ? 1000 : code, reason) } catch { /* Closed already. */ }
  }

  async webSocketError(ws: WebSocket): Promise<void> {
    await this.core.closed(this.peerOf(ws))
  }

  async alarm(): Promise<void> {
    await this.core.alarm()
  }
}
