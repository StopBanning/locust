/**
 * THE LOCK ON YOUR LOCUST'S COPY (docs/PLAN-2026-10-30-ACCOUNTS.md). A signed-in computer keeps a copy of its profile
 * at Locust's relay, and every byte of it is sealed here first, with the profile key: 32 random bytes made on the first
 * computer, kept by the operating system, handed to a new computer only sealed to it, and shown to the person once as
 * a recovery code. The relay holds boxes and ids; it never holds the key, a file's name or a word of what is in it.
 *
 * From the profile key, by HKDF, two keys: one seals files (AES-256-GCM, a fresh nonce for every box, the account and
 * the file's id bound in), the other cuts each file's id from its name (HMAC-SHA-256), so ids repeat for the same name
 * on every computer of the account without saying what the name is.
 *
 * A file is copied in CHUNK_BYTES pieces at fixed offsets. A conversation is appended to, never rewritten, so a new
 * turn changes only its last piece, and only that piece goes up again.
 */
import { createHash, webcrypto } from 'node:crypto'

const VERSION = 'locust-sync-v1'
const subtle = webcrypto.subtle
const utf8 = new TextEncoder()

/** The size of each piece a file is copied in. */
export const CHUNK_BYTES = 4 * 1024 * 1024
/** A sealed piece: its nonce, the piece and GCM's tag. */
export const SEAL_OVERHEAD = 12 + 16
/** The profile key's length. */
export const PROFILE_KEY_BYTES = 32

export interface SyncKeys {
  readonly seal: webcrypto.CryptoKey
  readonly name: webcrypto.CryptoKey
  /** The account the copy is kept under: bound into every box, so a box cannot be passed off as another account's. */
  readonly account: number
}

const base64url = (bytes: Uint8Array): string => Buffer.from(bytes).toString('base64url')

/** A new profile key, made once, on the account's first computer. */
export const newProfileKey = (): Uint8Array => webcrypto.getRandomValues(new Uint8Array(PROFILE_KEY_BYTES))

export async function syncKeys(profileKey: Uint8Array, account: number): Promise<SyncKeys> {
  if (profileKey.length !== PROFILE_KEY_BYTES) throw new Error('A profile key is 32 bytes.')
  const base = await subtle.importKey('raw', profileKey, 'HKDF', false, ['deriveKey'])
  const derive = (info: string, algorithm: webcrypto.AesKeyGenParams | webcrypto.HmacKeyGenParams, usages: webcrypto.KeyUsage[]): Promise<webcrypto.CryptoKey> =>
    subtle.deriveKey({ name: 'HKDF', hash: 'SHA-256', salt: utf8.encode(VERSION), info: utf8.encode(info) }, base, algorithm, false, usages)
  return {
    seal: await derive('seal', { name: 'AES-GCM', length: 256 }, ['encrypt', 'decrypt']),
    name: await derive('name', { name: 'HMAC', hash: 'SHA-256', length: 256 }, ['sign']),
    account
  }
}

/** A name's id at the relay: the same on every computer of the account, and no way back to the name. */
export async function fileIdOf(keys: SyncKeys, name: string): Promise<string> {
  return base64url(new Uint8Array(await subtle.sign('HMAC', keys.name, utf8.encode(`${VERSION}|${name}`))))
}

/** The id of one piece of a profile file. */
export const chunkIdOf = (keys: SyncKeys, path: string, index: number): Promise<string> => fileIdOf(keys, `file|${path}|${String(index)}`)
/**
 * The id the copy's list of files is kept under: the same for every account, not cut with the key. A new computer
 * with the wrong key then finds the list and is told the key is wrong, not that there is no copy; and the relay learns
 * nothing it did not already, since the list is the file written last on every sync.
 */
export const MANIFEST_ID = createHash('sha256').update(`${VERSION}|manifest`).digest('base64url')

const aadOf = (keys: SyncKeys, id: string): Uint8Array => utf8.encode(`${VERSION}|${String(keys.account)}|${id}`)

/** One box: its nonce, then the sealed bytes. */
export async function sealFile(keys: SyncKeys, id: string, bytes: Uint8Array): Promise<Uint8Array> {
  const iv = webcrypto.getRandomValues(new Uint8Array(12))
  const sealed = new Uint8Array(await subtle.encrypt({ name: 'AES-GCM', iv, additionalData: aadOf(keys, id) }, keys.seal, bytes))
  const box = new Uint8Array(iv.length + sealed.length)
  box.set(iv)
  box.set(sealed, iv.length)
  return box
}

/** What a box held, or a throw: a box sealed with another key, for another account or under another id, does not open. */
export async function openFile(keys: SyncKeys, id: string, box: Uint8Array): Promise<Uint8Array> {
  if (box.length < SEAL_OVERHEAD) throw new Error('That box is too short to hold anything.')
  return new Uint8Array(await subtle.decrypt({ name: 'AES-GCM', iv: box.subarray(0, 12), additionalData: aadOf(keys, id) }, keys.seal, box.subarray(12)))
}

/** The pieces a file of `size` bytes is copied in: [start, end) at fixed offsets. An empty file is one empty piece. */
export function chunksOf(size: number): readonly (readonly [number, number])[] {
  if (size === 0) return [[0, 0]]
  const pieces: [number, number][] = []
  for (let start = 0; start < size; start += CHUNK_BYTES) pieces.push([start, Math.min(size, start + CHUNK_BYTES)])
  return pieces
}

/*
 * THE RECOVERY CODE. The profile key and two bytes of its hash, in Crockford's base32 (no I, L, O or U, so nothing is
 * misread), in groups of five: 55 letters and digits. The two bytes catch a code copied wrong before it is tried.
 */
const ALPHABET = '0123456789ABCDEFGHJKMNPQRSTVWXYZ'

export async function recoveryCode(profileKey: Uint8Array): Promise<string> {
  if (profileKey.length !== PROFILE_KEY_BYTES) throw new Error('A profile key is 32 bytes.')
  const check = new Uint8Array(await subtle.digest('SHA-256', profileKey)).subarray(0, 2)
  const bytes = new Uint8Array([...profileKey, ...check])
  let bits = 0
  let value = 0
  let text = ''
  for (const byte of bytes) {
    value = (value << 8) | byte
    bits += 8
    while (bits >= 5) { text += ALPHABET[(value >>> (bits - 5)) & 31]; bits -= 5 }
    value &= (1 << bits) - 1
  }
  if (bits > 0) text += ALPHABET[(value << (5 - bits)) & 31]
  return text.match(/.{1,5}/g)!.join('-')
}

/** The profile key a recovery code holds, or undefined for a code copied wrong. Spaces, dashes and case do not matter. */
export async function profileKeyFromCode(code: string): Promise<Uint8Array | undefined> {
  const text = code.toUpperCase().replace(/[\s-]/g, '').replace(/[IL]/g, '1').replace(/O/g, '0')
  if (text.length !== 55 || [...text].some((character) => !ALPHABET.includes(character))) return undefined
  const bytes: number[] = []
  let bits = 0
  let value = 0
  for (const character of text) {
    value = (value << 5) | ALPHABET.indexOf(character)
    bits += 5
    if (bits >= 8) { bytes.push((value >>> (bits - 8)) & 255); bits -= 8; value &= (1 << bits) - 1 }
  }
  if (bytes.length !== PROFILE_KEY_BYTES + 2) return undefined
  const key = new Uint8Array(bytes.slice(0, PROFILE_KEY_BYTES))
  const check = new Uint8Array(await subtle.digest('SHA-256', key)).subarray(0, 2)
  return check[0] === bytes[32] && check[1] === bytes[33] ? key : undefined
}

/*
 * APPROVING A NEW COMPUTER FROM ONE THAT HAS YOUR LOCUST (accounts slice 4b). The new computer makes a fresh X25519
 * key and leaves an ask in its own place at the relay: its name and the key's public half. A computer with the profile
 * key sees the ask and the person approves it there; that computer seals the profile key to the ask's key (X25519, then
 * HKDF with both rooms bound in, then AES-256-GCM) and leaves the grant in its own place, for that room alone.
 *
 * The relay carries both, so it could swap the ask's key for its own. Both screens therefore show the MATCH CODE, cut
 * from the key the ask carries: the person approves only when the two read the same. Eight letters and digits, 40 bits,
 * so making a key with the same code would take far longer than the ten minutes an ask waits.
 */
export const ASK_ID = createHash('sha256').update(`${VERSION}|ask`).digest('base64url')
/** Where a computer leaves the grant for one asking room: in its own place, under an id only that room looks for. */
export const grantIdOf = (room: string): string => createHash('sha256').update(`${VERSION}|grant|${room}`).digest('base64url')

export interface AskKeys {
  readonly privateKey: webcrypto.CryptoKey
  /** Raw X25519 public key, base64url. */
  readonly publicKey: string
}

export async function newAskKeys(): Promise<AskKeys> {
  const pair = await subtle.generateKey({ name: 'X25519' }, true, ['deriveBits']) as unknown as webcrypto.CryptoKeyPair
  return { privateKey: pair.privateKey, publicKey: base64url(new Uint8Array(await subtle.exportKey('raw', pair.publicKey))) }
}

/** The code both screens show for one ask's key: XXXX-XXXX in the recovery code's letters. */
export async function matchOf(publicKey: string): Promise<string> {
  const digest = new Uint8Array(await subtle.digest('SHA-256', utf8.encode(`${VERSION}|match|${publicKey}`)))
  let bits = 0n
  for (const byte of digest.subarray(0, 5)) bits = (bits << 8n) | BigInt(byte)
  let text = ''
  for (let shift = 35n; shift >= 0n; shift -= 5n) text += ALPHABET[Number((bits >> shift) & 31n)]
  return `${text.slice(0, 4)}-${text.slice(4)}`
}

const grantKey = async (privateKey: webcrypto.CryptoKey, peer: string, info: string): Promise<webcrypto.CryptoKey> => {
  const peerKey = await subtle.importKey('raw', Buffer.from(peer, 'base64url'), { name: 'X25519' }, false, [])
  const shared = await subtle.deriveBits({ name: 'X25519', public: peerKey }, privateKey, 256)
  const base = await subtle.importKey('raw', shared, 'HKDF', false, ['deriveKey'])
  return subtle.deriveKey({ name: 'HKDF', hash: 'SHA-256', salt: utf8.encode(VERSION), info: utf8.encode(info) }, base, { name: 'AES-GCM', length: 256 }, false, ['encrypt', 'decrypt'])
}
const grantInfo = (from: string, to: string): string => `${VERSION}|grant|${from}|${to}`

/** The profile key sealed to an ask's key, from room `from` to room `to`: what the grant file holds. */
export async function sealGrant(profileKey: Uint8Array, askPublicKey: string, from: string, to: string): Promise<Uint8Array> {
  if (profileKey.length !== PROFILE_KEY_BYTES) throw new Error('A profile key is 32 bytes.')
  const mine = await newAskKeys()
  const key = await grantKey(mine.privateKey, askPublicKey, grantInfo(from, to))
  const nonce = webcrypto.getRandomValues(new Uint8Array(12))
  const sealed = new Uint8Array(await subtle.encrypt({ name: 'AES-GCM', iv: nonce, additionalData: utf8.encode(grantInfo(from, to)) }, key, profileKey))
  return utf8.encode(JSON.stringify({ v: 1, from, key: mine.publicKey, n: base64url(nonce), c: base64url(sealed) }))
}

/** The profile key a grant holds for this room, or undefined for one not sealed to this ask's key. */
export async function openGrant(grant: Uint8Array, ask: AskKeys, to: string): Promise<{ readonly profileKey: Uint8Array; readonly from: string } | undefined> {
  try {
    const said = JSON.parse(new TextDecoder().decode(grant)) as { v?: unknown; from?: unknown; key?: unknown; n?: unknown; c?: unknown }
    if (said.v !== 1 || typeof said.from !== 'string' || typeof said.key !== 'string' || typeof said.n !== 'string' || typeof said.c !== 'string') return undefined
    const key = await grantKey(ask.privateKey, said.key, grantInfo(said.from, to))
    const opened = new Uint8Array(await subtle.decrypt({ name: 'AES-GCM', iv: Buffer.from(said.n, 'base64url'), additionalData: utf8.encode(grantInfo(said.from, to)) }, key, Buffer.from(said.c, 'base64url')))
    return opened.length === PROFILE_KEY_BYTES ? { profileKey: opened, from: said.from } : undefined
  } catch {
    return undefined
  }
}
