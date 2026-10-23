/*
 * The few parts of Cloudflare's Workers runtime the relay uses, declared here so the package needs no types package
 * of its own. https://developers.cloudflare.com/durable-objects/best-practices/websockets/ (the Hibernation API).
 */

interface WebSocket {
  /** Kept with the connection while the room's object sleeps; at most 2,048 bytes. */
  serializeAttachment(value: unknown): void
  deserializeAttachment(): unknown
}

declare class WebSocketPair {
  0: WebSocket
  1: WebSocket
}

declare class WebSocketRequestResponsePair {
  constructor(request: string, response: string)
}

interface DurableObjectStorage {
  get<T>(key: string): Promise<T | undefined>
  put(key: string, value: unknown): Promise<void>
  deleteAll(): Promise<void>
  setAlarm(scheduledTime: number): Promise<void>
}

interface DurableObjectState {
  readonly storage: DurableObjectStorage
  acceptWebSocket(ws: WebSocket, tags?: string[]): void
  getWebSockets(tag?: string): WebSocket[]
  setWebSocketAutoResponse(pair: WebSocketRequestResponsePair): void
}

interface DurableObjectId {}

interface DurableObjectStub {
  fetch(request: Request): Promise<Response>
}

interface DurableObjectNamespace {
  idFromName(name: string): DurableObjectId
  get(id: DurableObjectId): DurableObjectStub
}

/** An image bundled into the Worker as bytes (wrangler.jsonc's Data rule). */
declare module '*.png' {
  const bytes: ArrayBuffer
  export default bytes
}
