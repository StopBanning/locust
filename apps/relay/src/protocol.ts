/**
 * LOCUST'S RELAY (0.826): what a PC, a paired phone and a phone being paired say to the relay, and what it says back.
 *
 * The relay sees who talks to whom and when, never what. Every message's words are sealed on one of the person's
 * devices and opened on another (apps/desktop/src/main/relay-crypto.ts, and the same steps in the phone's page), so
 * `n` and `c` below are a nonce and a sealed box it only passes on. What it does keep is small: a hash of the PC's
 * token, a hash of each paired device's token, and while a code is shown, a hash of that code's token.
 */

/** 16 random bytes, base64url: a room is one PC's. */
export const ROOM_ID = /^[A-Za-z0-9_-]{22}$/
/** A paired device's id, cut from a hash of its public key. */
export const DEVICE_ID = /^[A-Za-z0-9_-]{22}$/
/** 32 random bytes, base64url: the PC's token, a device's token, a pairing token. */
export const TOKEN = /^[A-Za-z0-9_-]{43}$/
/** SHA-256, base64url. */
export const HASH = /^[A-Za-z0-9_-]{43}$/
/** An X25519 public key, base64url: sent only by a phone being paired, so the PC can open what it sealed. */
export const PUBLIC_KEY = /^[A-Za-z0-9_-]{43}$/
/** AES-GCM's 12-byte nonce, base64url. */
export const NONCE = /^[A-Za-z0-9_-]{16}$/
/** A sealed box: at least AES-GCM's 16-byte tag, at most 512 KiB. A conversation's last turns fit in one. */
export const MAX_SEALED_BYTES = 512 * 1024
const SEALED = new RegExp(`^[A-Za-z0-9_-]{22,${String(Math.ceil((MAX_SEALED_BYTES * 4) / 3))}}$`)
/** A phone being paired, until it is: the relay names its connection, and the PC answers it by that name. */
export const PAIRING_TAG = /^[A-Za-z0-9_-]{12}$/
/** The longest frame read at all. Cloudflare's own limit is 1 MiB. */
export const MAX_FRAME = 720 * 1024
/** A request id the PC may put on a command, so it knows which one the relay has done. */
const COMMAND_ID = /^[A-Za-z0-9_-]{1,32}$/

/** Who a message goes to: the PC, a paired device, or a phone being paired. */
export type Address = 'pc' | `pairing:${string}` | string

export type ClientFrame =
  | { readonly t: 'auth'; readonly as: 'pc'; readonly token: string }
  | { readonly t: 'auth'; readonly as: 'device'; readonly device: string; readonly token: string }
  | { readonly t: 'auth'; readonly as: 'pairing'; readonly token: string }
  | { readonly t: 'send'; readonly to: Address; readonly n: string; readonly c: string; readonly k?: string }
  | { readonly t: 'allow'; readonly device: string; readonly hash: string; readonly id?: string }
  | { readonly t: 'remove'; readonly device: string; readonly id?: string }
  | { readonly t: 'pair-open'; readonly hash: string; readonly until: number; readonly id?: string }
  | { readonly t: 'pair-close'; readonly id?: string }
  | { readonly t: 'reset'; readonly id?: string }
  | { readonly t: 'push'; readonly endpoint: string; readonly body: string; readonly urgency: 'normal' | 'high'; readonly id?: string }

/** Where a browser's subscription may point. The relay posts to nothing else. */
const PUSH_HOSTS = [/^fcm\.googleapis\.com$/, /^android\.googleapis\.com$/, /^updates\.push\.services\.mozilla\.com$/, /^push\.services\.mozilla\.com$/, /^web\.push\.apple\.com$/, /^[a-z0-9-]+\.notify\.windows\.com$/]

export function isPushEndpoint(endpoint: string): boolean {
  let url: URL
  try { url = new URL(endpoint) } catch { return false }
  return url.protocol === 'https:' && url.port === '' && url.username === '' && PUSH_HOSTS.some((host) => host.test(url.hostname))
}

/** A push's sealed body, at most one 4 KiB record (RFC 8291), base64url. */
const PUSH_BODY = /^[A-Za-z0-9_-]{120,5462}$/

export type ErrorCode =
  /** The token was wrong, or the device is not paired (any more). */
  | 'auth'
  /** No PC has made this room. */
  | 'room'
  /** No code is open, or this one was used or ran out. */
  | 'pairing-closed'
  /** The PC is not connected now. */
  | 'pc-offline'
  /** The one being written to is not connected now. */
  | 'peer-offline'
  | 'bad-frame'
  | 'too-big'
  | 'too-fast'
  /** Only the PC may do that. */
  | 'not-owner'
  /** The room has as many devices as it may. */
  | 'full'
  /** The push service refused the alert, or could not be reached. */
  | 'push-failed'
  /** The push service says the phone's subscription is gone: stop sending to it. */
  | 'push-gone'
  /** This relay has no key to sign pushes with. */
  | 'push-off'

export type RelayFrame =
  | { readonly t: 'ok'; readonly as: 'pc'; readonly devices: readonly string[] }
  | { readonly t: 'ok'; readonly as: 'device' | 'pairing'; readonly pc: boolean }
  | { readonly t: 'msg'; readonly from: Address; readonly n: string; readonly c: string; readonly k?: string }
  | { readonly t: 'presence'; readonly pc: boolean }
  | { readonly t: 'presence'; readonly device: string; readonly on: boolean }
  | { readonly t: 'done'; readonly of: 'allow' | 'remove' | 'pair-open' | 'pair-close' | 'reset' | 'push'; readonly id?: string }
  | { readonly t: 'removed' }
  | { readonly t: 'err'; readonly code: ErrorCode; readonly id?: string }

const isAddress = (value: unknown): value is Address =>
  typeof value === 'string' && (value === 'pc' || DEVICE_ID.test(value) || (value.startsWith('pairing:') && PAIRING_TAG.test(value.slice(8))))

const withId = (fields: Record<string, unknown>): { readonly id?: string } | undefined => {
  if (fields.id === undefined) return {}
  return typeof fields.id === 'string' && COMMAND_ID.test(fields.id) ? { id: fields.id } : undefined
}

/** A frame as the relay reads it, or undefined for anything else: a field it does not know is refused, not kept. */
export function parseClientFrame(text: string): ClientFrame | undefined {
  if (text.length > MAX_FRAME) return undefined
  let value: unknown
  try { value = JSON.parse(text) } catch { return undefined }
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return undefined
  const fields = value as Record<string, unknown>
  const keys = Object.keys(fields)
  const only = (...allowed: readonly string[]): boolean => keys.every((key) => allowed.includes(key))
  const id = withId(fields)
  switch (fields.t) {
    case 'auth':
      if (fields.as === 'pc' && only('t', 'as', 'token') && typeof fields.token === 'string' && TOKEN.test(fields.token)) return { t: 'auth', as: 'pc', token: fields.token }
      if (fields.as === 'device' && only('t', 'as', 'device', 'token') && typeof fields.device === 'string' && DEVICE_ID.test(fields.device) && typeof fields.token === 'string' && TOKEN.test(fields.token)) {
        return { t: 'auth', as: 'device', device: fields.device, token: fields.token }
      }
      if (fields.as === 'pairing' && only('t', 'as', 'token') && typeof fields.token === 'string' && TOKEN.test(fields.token)) return { t: 'auth', as: 'pairing', token: fields.token }
      return undefined
    case 'send': {
      if (!only('t', 'to', 'n', 'c', 'k') || !isAddress(fields.to) || typeof fields.n !== 'string' || !NONCE.test(fields.n) || typeof fields.c !== 'string' || !SEALED.test(fields.c)) return undefined
      if (fields.k !== undefined && (typeof fields.k !== 'string' || !PUBLIC_KEY.test(fields.k))) return undefined
      return { t: 'send', to: fields.to, n: fields.n, c: fields.c, ...(typeof fields.k === 'string' ? { k: fields.k } : {}) }
    }
    case 'allow':
      if (id === undefined || !only('t', 'device', 'hash', 'id') || typeof fields.device !== 'string' || !DEVICE_ID.test(fields.device) || typeof fields.hash !== 'string' || !HASH.test(fields.hash)) return undefined
      return { t: 'allow', device: fields.device, hash: fields.hash, ...id }
    case 'remove':
      if (id === undefined || !only('t', 'device', 'id') || typeof fields.device !== 'string' || !DEVICE_ID.test(fields.device)) return undefined
      return { t: 'remove', device: fields.device, ...id }
    case 'pair-open':
      if (id === undefined || !only('t', 'hash', 'until', 'id') || typeof fields.hash !== 'string' || !HASH.test(fields.hash) || typeof fields.until !== 'number' || !Number.isSafeInteger(fields.until)) return undefined
      return { t: 'pair-open', hash: fields.hash, until: fields.until, ...id }
    case 'pair-close':
      if (id === undefined || !only('t', 'id')) return undefined
      return { t: 'pair-close', ...id }
    case 'reset':
      if (id === undefined || !only('t', 'id')) return undefined
      return { t: 'reset', ...id }
    case 'push':
      if (id === undefined || !only('t', 'endpoint', 'body', 'urgency', 'id') || typeof fields.endpoint !== 'string' || fields.endpoint.length > 1024
        || typeof fields.body !== 'string' || !PUSH_BODY.test(fields.body) || (fields.urgency !== 'normal' && fields.urgency !== 'high')) return undefined
      return { t: 'push', endpoint: fields.endpoint, body: fields.body, urgency: fields.urgency, ...id }
    default:
      return undefined
  }
}
