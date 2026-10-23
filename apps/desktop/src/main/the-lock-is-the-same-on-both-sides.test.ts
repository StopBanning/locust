import type { webcrypto } from 'node:crypto'
import vm from 'node:vm'

import { describe, expect, it } from 'vitest'

import { RELAY_LOCK } from './phone-page.js'
import { aadOf, deviceIdOf, fromBase64url, importPrivateKey, linkKeys, newPcKeys, open, pairingProof, pairingToken, randomText, seal, toBase64url } from './relay-crypto.js'

type CryptoKey = webcrypto.CryptoKey
type CryptoKeyPair = webcrypto.CryptoKeyPair

/**
 * THE LOCK IS THE SAME ON BOTH SIDES (0.826). A phone and a PC seal and open each other's messages with code written
 * twice: relay-crypto.ts here, and RELAY_LOCK in the page the phone runs. Two copies drift; so the page's own code is
 * run here, against the PC's, on the platform crypto both use.
 */
interface PageLock {
  b64(bytes: Uint8Array): string
  unb64(text: string): Uint8Array
  pairingToken(secret: string): Promise<string>
  pairingProof(secret: string): Promise<string>
  deviceIdOf(publicKey: string): Promise<string>
  aad(room: string, device: string, direction: string): string
  linkKeys(privateKey: CryptoKey, peerPublicKey: string, room: string, side: 'pc' | 'device'): Promise<{ seal: CryptoKey; open: CryptoKey }>
  seal(key: CryptoKey, value: unknown, aad: string): Promise<{ n: string; c: string }>
  open(key: CryptoKey, n: string, c: string, aad: string): Promise<unknown>
}

function pageLock(): PageLock {
  const context = vm.createContext({ crypto: globalThis.crypto, TextEncoder, TextDecoder, btoa, atob })
  vm.runInContext(RELAY_LOCK, context)
  return (context as { lock: PageLock }).lock
}

/** A phone as the page makes one: its own X25519 pair, the private half never leaving the browser. */
async function phoneKeys(): Promise<{ privateKey: CryptoKey; publicKey: string }> {
  const pair = await globalThis.crypto.subtle.generateKey({ name: 'X25519' }, false, ['deriveBits']) as unknown as CryptoKeyPair
  return { privateKey: pair.privateKey, publicKey: toBase64url(new Uint8Array(await globalThis.crypto.subtle.exportKey('raw', pair.publicKey))) }
}

describe('the lock is the same on both sides', () => {
  it('writes and reads bytes as the PC does', () => {
    const lock = pageLock()
    for (const size of [0, 1, 2, 3, 16, 32, 33, 255]) {
      const bytes = globalThis.crypto.getRandomValues(new Uint8Array(size))
      expect(lock.b64(bytes)).toBe(toBase64url(bytes))
      expect([...lock.unb64(toBase64url(bytes))]).toEqual([...bytes])
      expect([...fromBase64url(lock.b64(bytes))]).toEqual([...bytes])
    }
  })

  it('makes the same pairing token, proof and device id from the same code', async () => {
    const lock = pageLock()
    const secret = randomText(16)
    expect(await lock.pairingToken(secret)).toBe(await pairingToken(secret))
    expect(await lock.pairingProof(secret)).toBe(await pairingProof(secret))
    // The relay sees the token; the proof it never sees must not follow from it.
    expect(await pairingProof(secret)).not.toBe(await pairingToken(secret))
    const { publicKey } = await phoneKeys()
    expect(await lock.deviceIdOf(publicKey)).toBe(await deviceIdOf(publicKey))
    expect(lock.aad('r', 'd', 'device-to-pc')).toBe(aadOf('r', 'd', 'device-to-pc'))
  })

  it('opens on the PC what the page sealed, and in the page what the PC sealed', async () => {
    const lock = pageLock()
    const room = randomText(16)
    const pc = await newPcKeys()
    const phone = await phoneKeys()
    const device = await deviceIdOf(phone.publicKey)
    const onPhone = await lock.linkKeys(phone.privateKey, pc.publicKey, room, 'device')
    const onPc = await linkKeys(await importPrivateKey(pc.privateJwk), phone.publicKey, room, 'pc')

    const asked = await lock.seal(onPhone.seal, { kind: 'req', path: '/api/state', words: 'Grüße ✓' }, lock.aad(room, device, 'device-to-pc'))
    expect(await open(onPc.open, asked.n, asked.c, aadOf(room, device, 'device-to-pc'))).toEqual({ kind: 'req', path: '/api/state', words: 'Grüße ✓' })

    const answered = await seal(onPc.seal, { kind: 'res', status: 200 }, aadOf(room, device, 'pc-to-device'))
    expect(await lock.open(onPhone.open, answered.n, answered.c, lock.aad(room, device, 'pc-to-device'))).toEqual({ kind: 'res', status: 200 })
  })

  it('does not open a box sent back to the side that sealed it, or named for another device or room', async () => {
    const lock = pageLock()
    const room = randomText(16)
    const pc = await newPcKeys()
    const phone = await phoneKeys()
    const device = await deviceIdOf(phone.publicKey)
    const onPhone = await lock.linkKeys(phone.privateKey, pc.publicKey, room, 'device')
    const onPc = await linkKeys(await importPrivateKey(pc.privateJwk), phone.publicKey, room, 'pc')

    const answered = await seal(onPc.seal, { kind: 'res' }, aadOf(room, device, 'pc-to-device'))
    // Reflected to the PC: its opening key is the other direction's.
    await expect(open(onPc.open, answered.n, answered.c, aadOf(room, device, 'pc-to-device'))).rejects.toThrow()
    // Read as if for another device, or another room, or the other way round.
    await expect(lock.open(onPhone.open, answered.n, answered.c, lock.aad(room, randomText(16).slice(0, 22), 'pc-to-device'))).rejects.toThrow()
    await expect(lock.open(onPhone.open, answered.n, answered.c, lock.aad(randomText(16), device, 'pc-to-device'))).rejects.toThrow()
    await expect(lock.open(onPhone.open, answered.n, answered.c, lock.aad(room, device, 'device-to-pc'))).rejects.toThrow()
    // A phone that derived its keys for another room cannot open the PC's box either.
    const elsewhere = await lock.linkKeys(phone.privateKey, pc.publicKey, randomText(16), 'device')
    await expect(lock.open(elsewhere.open, answered.n, answered.c, lock.aad(room, device, 'pc-to-device'))).rejects.toThrow()
  })

  it('never seals two boxes with the same nonce', async () => {
    const pc = await newPcKeys()
    const phone = await phoneKeys()
    const keys = await linkKeys(await importPrivateKey(pc.privateJwk), phone.publicKey, randomText(16), 'pc')
    const nonces = new Set<string>()
    for (let i = 0; i < 200; i += 1) nonces.add((await seal(keys.seal, i, 'aad')).n)
    expect(nonces.size).toBe(200)
  })
})
