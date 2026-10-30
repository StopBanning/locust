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
  delete(keys: string[]): Promise<number>
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

/** The parts of an R2 bucket the relay's sync uses (account.ts). https://developers.cloudflare.com/r2/api/workers/workers-api-reference/ */
interface R2Object {
  readonly key: string
  readonly size: number
  readonly uploaded: Date
}

interface R2ObjectBody extends R2Object {
  readonly body: ReadableStream
}

interface R2Objects {
  readonly objects: R2Object[]
  readonly truncated: boolean
  readonly cursor?: string
}

interface R2Bucket {
  head(key: string): Promise<R2Object | null>
  get(key: string): Promise<R2ObjectBody | null>
  put(key: string, value: ReadableStream | ArrayBuffer): Promise<R2Object | null>
  list(options?: { readonly prefix?: string; readonly cursor?: string; readonly limit?: number }): Promise<R2Objects>
  delete(keys: string | string[]): Promise<void>
}

declare class FixedLengthStream {
  constructor(length: number)
  readonly readable: ReadableStream
  readonly writable: WritableStream
}
