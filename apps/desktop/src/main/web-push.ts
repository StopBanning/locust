import type { webcrypto } from 'node:crypto'

/**
 * AN ALERT LOCKED FOR ONE PHONE'S BROWSER (0.827). A phone's alert travels from this PC through Locust's relay to the
 * browser's own push service (Apple's or Google's), and only that browser can open it: it is encrypted here to the
 * browser's push key, as the Web Push standard sets out (RFC 8291, with RFC 8188's aes128gcm). The relay adds only
 * its signature (VAPID); neither it nor the push service can read what the alert says.
 *
 * Tested against the RFC's own worked example (an-alert-is-locked-for-its-phone-alone.test.ts).
 */
type CryptoKey = webcrypto.CryptoKey
type CryptoKeyPair = webcrypto.CryptoKeyPair

const subtle = globalThis.crypto.subtle
const utf8 = new TextEncoder()
const fromBase64url = (text: string): Uint8Array => new Uint8Array(Buffer.from(text, 'base64url'))

/** What a browser hands back when it subscribes: where to send, and the two keys to encrypt with. */
export interface PushSubscriptionKeys {
  readonly endpoint: string
  /** The browser's P-256 public key, uncompressed, base64url. */
  readonly p256dh: string
  /** The 16-byte authentication secret, base64url. */
  readonly auth: string
}

/** One record is plenty: an alert is a few hundred bytes. */
const RECORD_SIZE = 4096

const concat = (...parts: readonly Uint8Array[]): Uint8Array => {
  const out = new Uint8Array(parts.reduce((size, part) => size + part.length, 0))
  let at = 0
  for (const part of parts) { out.set(part, at); at += part.length }
  return out
}

const hkdf = async (salt: Uint8Array, ikm: Uint8Array, info: Uint8Array, bytes: number): Promise<Uint8Array> => {
  const key = await subtle.importKey('raw', ikm, 'HKDF', false, ['deriveBits'])
  return new Uint8Array(await subtle.deriveBits({ name: 'HKDF', hash: 'SHA-256', salt, info }, key, bytes * 8))
}

/**
 * The body of one push message: salt, record size, this message's own public key, then the sealed text. A fresh key
 * pair and salt for every message, unless a test hands its own in.
 */
export async function encryptPush(subscription: Pick<PushSubscriptionKeys, 'p256dh' | 'auth'>, text: string, fixed: { readonly salt?: Uint8Array; readonly keys?: CryptoKeyPair } = {}): Promise<Uint8Array> {
  const receiverRaw = fromBase64url(subscription.p256dh)
  const auth = fromBase64url(subscription.auth)
  if (receiverRaw.length !== 65 || receiverRaw[0] !== 4) throw new Error('That push key is not a P-256 public key.')
  if (auth.length !== 16) throw new Error('That push secret is not 16 bytes.')
  const receiver = await subtle.importKey('raw', receiverRaw, { name: 'ECDH', namedCurve: 'P-256' }, false, [])
  const sender = fixed.keys ?? await subtle.generateKey({ name: 'ECDH', namedCurve: 'P-256' }, true, ['deriveBits']) as unknown as CryptoKeyPair
  const senderRaw = new Uint8Array(await subtle.exportKey('raw', sender.publicKey))
  const shared = new Uint8Array(await subtle.deriveBits({ name: 'ECDH', public: receiver }, sender.privateKey, 256))
  const ikm = await hkdf(auth, shared, concat(utf8.encode('WebPush: info\0'), receiverRaw, senderRaw), 32)
  const salt = fixed.salt ?? globalThis.crypto.getRandomValues(new Uint8Array(16))
  const cek = await hkdf(salt, ikm, utf8.encode('Content-Encoding: aes128gcm\0'), 16)
  const nonce = await hkdf(salt, ikm, utf8.encode('Content-Encoding: nonce\0'), 12)
  const key = await subtle.importKey('raw', cek, 'AES-GCM', false, ['encrypt'])
  // The last (and only) record ends with the delimiter 2.
  const sealed = new Uint8Array(await subtle.encrypt({ name: 'AES-GCM', iv: nonce }, key, concat(utf8.encode(text), new Uint8Array([2]))))
  if (sealed.length + 86 > RECORD_SIZE) throw new Error('That alert is too long for one push.')
  const header = new Uint8Array(21)
  header.set(salt, 0)
  new DataView(header.buffer).setUint32(16, RECORD_SIZE)
  header[20] = senderRaw.length
  return concat(header, senderRaw, sealed)
}

/** The private key of a test's fixed sender, from its raw parts. */
export async function senderKeysForTest(privateKey: string, publicKey: string): Promise<CryptoKeyPair> {
  const raw = fromBase64url(publicKey)
  const b64 = (bytes: Uint8Array): string => Buffer.from(bytes).toString('base64url')
  const jwk = { kty: 'EC', crv: 'P-256', d: privateKey, x: b64(raw.slice(1, 33)), y: b64(raw.slice(33, 65)), ext: true }
  return {
    privateKey: await subtle.importKey('jwk', jwk, { name: 'ECDH', namedCurve: 'P-256' }, true, ['deriveBits']) as CryptoKey,
    publicKey: await subtle.importKey('raw', raw, { name: 'ECDH', namedCurve: 'P-256' }, true, []) as CryptoKey
  }
}
