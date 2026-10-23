/**
 * THE LOCK ON YOUR PHONE'S MESSAGES (0.826, apps/relay). A PC and each device it pairs share two keys, one for each
 * direction, made from an X25519 exchange through HKDF; neither leaves the two of them, and Locust's relay between
 * them passes only nonces and sealed boxes. Every box binds the room, the device and the direction into it, so the
 * relay cannot hand one device's message to another, or a message back to the side that sealed it.
 *
 * The browser's own crypto does the same steps under the same names in the phone's page (phone-page.ts,
 * RELAY_LOCK), and the-lock-is-the-same-on-both-sides.test.ts runs that page's code against this one.
 */
import type { webcrypto } from 'node:crypto'

/** The platform's own crypto types, by the names the page uses. */
export type CryptoKey = webcrypto.CryptoKey
export type JsonWebKey = webcrypto.JsonWebKey
type CryptoKeyPair = webcrypto.CryptoKeyPair

const VERSION = 'locust-relay-v1'
const subtle = globalThis.crypto.subtle
const utf8 = new TextEncoder()

export type Direction = 'pc-to-device' | 'device-to-pc'

export const toBase64url = (bytes: Uint8Array): string => Buffer.from(bytes).toString('base64url')
export const fromBase64url = (text: string): Uint8Array => new Uint8Array(Buffer.from(text, 'base64url'))

/** `bytes` random bytes, base64url: a room's id (16), a token (32), a code's secret (16). */
export const randomText = (bytes: number): string => toBase64url(globalThis.crypto.getRandomValues(new Uint8Array(bytes)))

export const sha256 = async (text: string): Promise<string> => toBase64url(new Uint8Array(await subtle.digest('SHA-256', utf8.encode(text))))

/** What a phone shows the relay to be let in while a code is open. The relay keeps only its hash. */
export const pairingToken = (secret: string): Promise<string> => sha256(`${VERSION}|pair-relay|${secret}`)
/** What a phone shows the PC inside its sealed hello: the relay, which saw only the token, cannot make it. */
export const pairingProof = (secret: string): Promise<string> => sha256(`${VERSION}|pair-proof|${secret}`)
/** A device's id, cut from its public key: the PC and the device both work it out, and nobody can choose it. */
export const deviceIdOf = async (publicKey: string): Promise<string> => (await sha256(`${VERSION}|device|${publicKey}`)).slice(0, 22)

export const aadOf = (room: string, device: string, direction: Direction): string => `${VERSION}|${room}|${device}|${direction}`

export interface LinkKeys {
  readonly seal: CryptoKey
  readonly open: CryptoKey
}

/** The PC's own key pair: the private half as JWK, for Locust's file of what it pairs; the public half, raw. */
export async function newPcKeys(): Promise<{ readonly privateJwk: JsonWebKey; readonly publicKey: string }> {
  const pair = await subtle.generateKey({ name: 'X25519' }, true, ['deriveBits']) as unknown as CryptoKeyPair
  return {
    privateJwk: await subtle.exportKey('jwk', pair.privateKey),
    publicKey: toBase64url(new Uint8Array(await subtle.exportKey('raw', pair.publicKey)))
  }
}

export const importPrivateKey = (jwk: JsonWebKey): Promise<CryptoKey> => subtle.importKey('jwk', jwk, { name: 'X25519' }, false, ['deriveBits'])

/** The two keys this side seals and opens with, from its private key and the other side's public one. */
export async function linkKeys(privateKey: CryptoKey, peerPublicKey: string, room: string, side: 'pc' | 'device'): Promise<LinkKeys> {
  const peer = await subtle.importKey('raw', fromBase64url(peerPublicKey), { name: 'X25519' }, false, [])
  const shared = new Uint8Array(await subtle.deriveBits({ name: 'X25519', public: peer }, privateKey, 256))
  // A key of the wrong shape can make an exchange that agrees on zero: refused, whatever the platform does with it.
  if (shared.every((byte) => byte === 0)) throw new Error('That key locks nothing.')
  const base = await subtle.importKey('raw', shared, 'HKDF', false, ['deriveKey'])
  const key = (direction: Direction, usage: 'encrypt' | 'decrypt'): Promise<CryptoKey> => subtle.deriveKey(
    { name: 'HKDF', hash: 'SHA-256', salt: utf8.encode(`${VERSION}|${room}`), info: utf8.encode(direction) },
    base, { name: 'AES-GCM', length: 256 }, false, [usage]
  )
  const mine: Direction = side === 'pc' ? 'pc-to-device' : 'device-to-pc'
  const theirs: Direction = side === 'pc' ? 'device-to-pc' : 'pc-to-device'
  return { seal: await key(mine, 'encrypt'), open: await key(theirs, 'decrypt') }
}

export async function seal(key: CryptoKey, value: unknown, aad: string): Promise<{ readonly n: string; readonly c: string }> {
  const iv = globalThis.crypto.getRandomValues(new Uint8Array(12))
  const sealed = await subtle.encrypt({ name: 'AES-GCM', iv, additionalData: utf8.encode(aad) }, key, utf8.encode(JSON.stringify(value)))
  return { n: toBase64url(iv), c: toBase64url(new Uint8Array(sealed)) }
}

/** What the box held, or a throw: a box sealed with another key, for another device or direction, does not open. */
export async function open(key: CryptoKey, n: string, c: string, aad: string): Promise<unknown> {
  const plain = await subtle.decrypt({ name: 'AES-GCM', iv: fromBase64url(n), additionalData: utf8.encode(aad) }, key, fromBase64url(c))
  return JSON.parse(new TextDecoder().decode(plain)) as unknown
}
